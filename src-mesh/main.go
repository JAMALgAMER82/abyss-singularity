// abyss-mesh — Tailscale userspace mesh sidecar for Abyss Singularity.
//
// Runs an embedded Tailscale node via tsnet, exposes three local-only
// endpoints to the Rust parent:
//
//   tsnet ╮
//         ├──► HTTP control API   (127.0.0.1:CTL_PORT)
//         │     - GET  /status        : self IP + peer list
//         │     - GET  /auth          : interactive auth URL (until logged in)
//         │     - GET  /events  (SSE) : status change notifications
//         │     - POST /shutdown      : graceful stop
//         │
//         ├──► SOCKS5 proxy        (127.0.0.1:SOCKS_PORT)
//         │     dialer ⇒ tsnet.Server.Dial — outbound TCP to tailnet peers
//         │
//         └──► PROXY-v1 forwarder  (tsnet :CHAT_PORT → 127.0.0.1:CHAT_PORT)
//               prepends PROXY TCP4 line so Rust learns the real peer IP
//
// Lifecycle: spawned by Tauri's sidecar plugin at app start; stops on
// /shutdown or when the parent process exits (stdin EOF detection).

package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"time"

	socks5 "github.com/armon/go-socks5"
	"tailscale.com/tsnet"
)

type Flags struct {
	StateDir     string
	Hostname     string
	CtlPort      int
	SocksPort    int
	ChatPort     int
	TransferPort int
	AuthKey      string
	Ephemeral    bool
}

func main() {
	f := parseFlags()
	if err := os.MkdirAll(f.StateDir, 0o700); err != nil {
		log.Fatalf("abyss-mesh: state dir: %v", err)
	}

	logger := log.New(os.Stderr, "abyss-mesh: ", log.LstdFlags|log.Lmicroseconds)

	srv := &tsnet.Server{
		Dir:       f.StateDir,
		Hostname:  f.Hostname,
		AuthKey:   f.AuthKey,
		Ephemeral: f.Ephemeral,
		Logf:      func(format string, args ...any) { logger.Printf(format, args...) },
		UserLogf:  func(format string, args ...any) { logger.Printf(format, args...) },
	}
	defer srv.Close()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// 1. Wait for the mesh interface to come up — this triggers the
	//    interactive auth flow on first run.
	if _, err := srv.Up(ctx); err != nil {
		logger.Printf("tsnet.Up: %v (may still be waiting for auth)", err)
	}

	// 2. Start the SOCKS5 outbound proxy. Pure Go, no admin needed.
	go startSocks5(ctx, srv, fmt.Sprintf("127.0.0.1:%d", f.SocksPort), logger)

	// 3. Start the chat-port forwarder (tsnet:CHAT → 127.0.0.1:CHAT
	//    with PROXY v1 header injection).
	go startPortForwarder(ctx, srv, f.ChatPort, "chat", logger)

	// 3b. Start the file-transfer-port forwarder. Same PROXY v1 pattern;
	//     the Rust side just listens on a different localhost port.
	go startPortForwarder(ctx, srv, f.TransferPort, "transfer", logger)

	// 4. Start the HTTP control API.
	go startControlAPI(ctx, srv, fmt.Sprintf("127.0.0.1:%d", f.CtlPort), logger, cancel)

	// 5. Block on parent process: if stdin closes, we shut down. Tauri's
	//    sidecar plugin closes the child's stdin when the parent exits.
	go watchStdin(cancel, logger)

	<-ctx.Done()
	logger.Printf("shutting down")
}

func parseFlags() Flags {
	def := func(env, fallback string) string {
		if v := os.Getenv(env); v != "" {
			return v
		}
		return fallback
	}
	stateDir := flag.String("state",
		def("ABYSS_MESH_STATE", filepath.Join(os.Getenv("LOCALAPPDATA"), "AbyssSingularity", "tailscale")),
		"directory for tsnet state")
	hostname := flag.String("hostname", def("ABYSS_MESH_HOSTNAME", defaultHostname()), "tailnet hostname")
	ctl := flag.Int("ctl", 7080, "control HTTP port (localhost only)")
	socksPort := flag.Int("socks", 1080, "SOCKS5 proxy port (localhost only)")
	chatPort := flag.Int("chat", 47992, "chat port — same on tsnet + 127.0.0.1")
	xferPort := flag.Int("transfer", 47993, "file-transfer port — same on tsnet + 127.0.0.1")
	authKey := flag.String("authkey", def("TS_AUTHKEY", ""), "optional pre-auth key")
	ephemeral := flag.Bool("ephemeral", false, "register as ephemeral node")
	flag.Parse()
	return Flags{
		StateDir:     *stateDir,
		Hostname:     *hostname,
		CtlPort:      *ctl,
		SocksPort:    *socksPort,
		ChatPort:     *chatPort,
		TransferPort: *xferPort,
		AuthKey:      *authKey,
		Ephemeral:    *ephemeral,
	}
}

func defaultHostname() string {
	h, err := os.Hostname()
	if err != nil || h == "" {
		return "abyss-node"
	}
	return "abyss-" + h
}

// ---------- SOCKS5 outbound proxy ------------------------------------------

func startSocks5(ctx context.Context, srv *tsnet.Server, addr string, logger *log.Logger) {
	conf := &socks5.Config{
		Logger: logger,
		Dial: func(ctx context.Context, network, target string) (net.Conn, error) {
			return srv.Dial(ctx, network, target)
		},
	}
	s, err := socks5.New(conf)
	if err != nil {
		logger.Printf("socks5 init: %v", err)
		return
	}
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		logger.Printf("socks5 listen %s: %v", addr, err)
		return
	}
	logger.Printf("socks5 outbound proxy on %s (dial via tsnet)", addr)
	go func() {
		<-ctx.Done()
		_ = ln.Close()
	}()
	if err := s.Serve(ln); err != nil && ctx.Err() == nil {
		logger.Printf("socks5 serve: %v", err)
	}
}

// ---------- inbound TCP forwarders ----------------------------------------

// Generic forwarder: accepts on tsnet:port and pipes each conn to
// 127.0.0.1:port, prepending a PROXY v1 line so the Rust side learns
// the real peer IP. Used for both the chat and transfer ports — same
// pattern, just different port numbers.
func startPortForwarder(ctx context.Context, srv *tsnet.Server, port int, label string, logger *log.Logger) {
	tsAddr := fmt.Sprintf(":%d", port)
	local  := fmt.Sprintf("127.0.0.1:%d", port)

	ln, err := srv.Listen("tcp", tsAddr)
	if err != nil {
		logger.Printf("tsnet[%s] listen %s: %v", label, tsAddr, err)
		return
	}
	logger.Printf("tsnet[%s] inbound forwarder %s → %s (PROXY v1 enabled)", label, tsAddr, local)
	go func() {
		<-ctx.Done()
		_ = ln.Close()
	}()

	for {
		up, err := ln.Accept()
		if err != nil {
			if ctx.Err() != nil { return }
			logger.Printf("tsnet[%s] accept: %v", label, err)
			time.Sleep(200 * time.Millisecond)
			continue
		}
		go forwardWithProxyV1(up, local, port, logger)
	}
}

func forwardWithProxyV1(up net.Conn, downstreamAddr string, chatPort int, logger *log.Logger) {
	defer up.Close()
	down, err := net.DialTimeout("tcp", downstreamAddr, 5*time.Second)
	if err != nil {
		logger.Printf("downstream dial %s: %v", downstreamAddr, err)
		return
	}
	defer down.Close()

	// PROXY protocol v1 — tells Rust the real source IP.
	src, _ := up.RemoteAddr().(*net.TCPAddr)
	dst, _ := down.LocalAddr().(*net.TCPAddr)
	srcIP, srcPort := "0.0.0.0", 0
	if src != nil {
		if src.IP.To4() != nil {
			srcIP = src.IP.To4().String()
		} else {
			srcIP = src.IP.String()
		}
		srcPort = src.Port
	}
	dstIP := "127.0.0.1"
	if dst != nil {
		dstIP = dst.IP.String()
	}
	header := fmt.Sprintf("PROXY TCP4 %s %s %d %d\r\n", srcIP, dstIP, srcPort, chatPort)
	if _, err := down.Write([]byte(header)); err != nil {
		logger.Printf("proxy header write: %v", err)
		return
	}

	// Pipe in both directions until either side closes.
	var wg sync.WaitGroup
	wg.Add(2)
	go func() { defer wg.Done(); _, _ = io.Copy(down, up) }()
	go func() { defer wg.Done(); _, _ = io.Copy(up, down) }()
	wg.Wait()
}

// ---------- HTTP control API ---------------------------------------------

type Status struct {
	Installed    bool     `json:"installed"`
	BackendState string   `json:"backend_state"`
	Version      string   `json:"version"`
	SelfIP       string   `json:"self_ip"`
	SelfDNS      string   `json:"self_dns"`
	NeedsAuth    bool     `json:"needs_auth"`
	AuthURL      string   `json:"auth_url"`
	Peers        []Peer   `json:"peers"`
}

type Peer struct {
	HostName string   `json:"host_name"`
	DNSName  string   `json:"dns_name"`
	Addrs    []string `json:"addrs"`
	Online   bool     `json:"online"`
	OS       string   `json:"os"`
}

func startControlAPI(ctx context.Context, srv *tsnet.Server, addr string, logger *log.Logger, cancel context.CancelFunc) {
	mux := http.NewServeMux()
	mux.HandleFunc("/status",   handleStatus(srv))
	mux.HandleFunc("/shutdown", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNoContent)
		go func() { time.Sleep(100 * time.Millisecond); cancel() }()
	})
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("ok"))
	})

	server := &http.Server{
		Addr:         addr,
		Handler:      mux,
		ReadTimeout:  10 * time.Second,
		WriteTimeout: 30 * time.Second,
	}
	go func() { <-ctx.Done(); _ = server.Close() }()
	logger.Printf("control api on %s", addr)
	if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		logger.Printf("control api: %v", err)
	}
}

func handleStatus(srv *tsnet.Server) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
		defer cancel()
		out := Status{Installed: true}

		lc, err := srv.LocalClient()
		if err != nil {
			writeJSON(w, out)
			return
		}
		st, err := lc.Status(ctx)
		if err != nil {
			writeJSON(w, out)
			return
		}
		out.BackendState = st.BackendState
		out.Version      = st.Version
		if st.Self != nil {
			if len(st.Self.TailscaleIPs) > 0 {
				out.SelfIP = st.Self.TailscaleIPs[0].String()
			}
			out.SelfDNS = st.Self.DNSName
		}
		out.NeedsAuth = st.BackendState == "NeedsLogin" || st.BackendState == "NeedsMachineAuth"
		if out.NeedsAuth {
			out.AuthURL = st.AuthURL
		}
		for _, p := range st.Peer {
			if p == nil { continue }
			peer := Peer{
				HostName: p.HostName,
				DNSName:  p.DNSName,
				Online:   p.Online,
				OS:       p.OS,
			}
			for _, ip := range p.TailscaleIPs {
				peer.Addrs = append(peer.Addrs, ip.String())
			}
			out.Peers = append(out.Peers, peer)
		}
		writeJSON(w, out)
	}
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(v)
}

// ---------- parent-process watchdog ---------------------------------------

func watchStdin(cancel context.CancelFunc, logger *log.Logger) {
	// When Tauri's sidecar parent exits, our stdin gets EOF. Use that as
	// the signal to shut down — keeps no orphaned process behind.
	_, err := io.Copy(io.Discard, os.Stdin)
	logger.Printf("stdin closed (%v) — initiating shutdown", err)
	cancel()
}
