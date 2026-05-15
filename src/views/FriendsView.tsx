import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  chatConnectPeer,
  chatGetConfig,
  chatGetHistory,
  chatGetPeers,
  chatSend,
  chatSetConfig,
  chatSetPresence,
  chatStart,
  chatStatus,
  chatStop,
  onChatMessage,
  onChatPeers,
  type ChatConfig,
  type ChatHistoryEntry,
  type ChatStatus,
  type PeerSnapshot,
  type PresenceStatus,
} from "../lib/chat";
import { openUrl } from "@tauri-apps/plugin-opener";
import { tailscaleStatus, type TailscaleStatus } from "../lib/network";
import {
  onTransferEvent,
  transferAccept,
  transferListIncoming,
  transferReject,
  type PendingOffer,
} from "../lib/transfer";
import { InvitesSection } from "../components/InvitesSection";
import { LobbySection }   from "../components/LobbySection";
import { PartyHero }      from "../components/PartyHero";
import {
  onStreamPairProgress,
  streamRequestPairAndLaunch,
  type StreamPairProgress,
} from "../lib/streaming";

/**
 * Phase 6.x — Friends + chat + presence.
 *
 * Combines two data sources:
 *  * `tailscaleStatus()` — list of mesh peers (Phase 4 data).
 *  * `chatGetPeers()` — peers we actually have live chat sessions with.
 * The merged list keeps mesh peers visible even when the chat link isn't
 * connected yet ("offline in chat"), and shows live presence when it is.
 */
export function FriendsView() {
  const [ts,         setTs]         = useState<TailscaleStatus | null>(null);
  const [chatPeers,  setChatPeers]  = useState<PeerSnapshot[]>([]);
  const [config,     setConfig]     = useState<ChatConfig | null>(null);
  const [status,     setStatus]     = useState<ChatStatus | null>(null);
  const [selected,   setSelected]   = useState<string | null>(null);
  const [history,    setHistory]    = useState<ChatHistoryEntry[]>([]);
  const [error,      setError]      = useState<string | null>(null);
  const [incoming,   setIncoming]   = useState<PendingOffer[]>([]);
  const [transferProgress, setTransferProgress] = useState<Record<string, { bytes: number; total: number }>>({});
  const [streamPair, setStreamPair] = useState<StreamPairProgress | null>(null);
  const [streamPairBusy, setStreamPairBusy] = useState<string | null>(null);

  // ---- initial load ------------------------------------------------------
  useEffect(() => {
    tailscaleStatus().then(setTs).catch(() => setTs(null));
    chatGetConfig().then(setConfig).catch((e) => setError(String(e)));
    chatStatus().then(setStatus).catch(() => {});
    chatGetPeers().then(setChatPeers).catch(() => {});
    chatGetHistory().then(setHistory).catch(() => {});
    const t = setInterval(() => {
      tailscaleStatus().then(setTs).catch(() => {});
      chatStatus().then(setStatus).catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

  // ---- live events ------------------------------------------------------
  useEffect(() => {
    let unlistenMsg:  undefined | (() => void);
    let unlistenPeer: undefined | (() => void);
    let unlistenXfer: undefined | (() => void);
    let unlistenPair: undefined | (() => void);
    onChatMessage((entry) => {
      setHistory((prev) => [...prev, entry]);
    }).then((u) => { unlistenMsg = u; });
    onChatPeers((peers) => setChatPeers(peers)).then((u) => { unlistenPeer = u; });
    onStreamPairProgress((p) => {
      setStreamPair(p);
      setStreamPairBusy(null);
    }).then((u) => { unlistenPair = u; });
    transferListIncoming().then(setIncoming).catch(() => {});
    onTransferEvent((e) => {
      if (e.kind === "offered" && e.offer.direction === "incoming") {
        setIncoming((prev) =>
          prev.find((o) => o.transfer_id === e.offer.transfer_id) ? prev : [...prev, e.offer]);
      } else if (e.kind === "progress") {
        setTransferProgress((prev) => ({ ...prev, [e.transfer_id]: { bytes: e.bytes_done, total: e.bytes_total } }));
      } else if (e.kind === "completed" || e.kind === "failed" || e.kind === "rejected") {
        setIncoming((prev) => prev.filter((o) => o.transfer_id !== e.transfer_id));
        setTransferProgress((prev) => {
          const { [e.transfer_id]: _, ...rest } = prev;
          return rest;
        });
      }
    }).then((u) => { unlistenXfer = u; });
    return () => { unlistenMsg?.(); unlistenPeer?.(); unlistenXfer?.(); unlistenPair?.(); };
  }, []);

  const startStreamFromPeer = useCallback(async (addr: string) => {
    setError(null);
    setStreamPair(null);
    setStreamPairBusy(addr);
    try { await streamRequestPairAndLaunch(addr); }
    catch (e) {
      setError(String(e));
      setStreamPairBusy(null);
    }
  }, []);

  const acceptOffer = useCallback(async (tid: string) => {
    setError(null);
    try { await transferAccept(tid); } catch (e) { setError(String(e)); }
  }, []);
  const rejectOffer = useCallback(async (tid: string) => {
    setError(null);
    try { await transferReject(tid); } catch (e) { setError(String(e)); }
  }, []);

  // ---- merged peer list -------------------------------------------------
  const peers = useMemo<MergedPeer[]>(() => {
    const out = new Map<string, MergedPeer>();

    // Seed from mesh — anyone in Tailscale we can SEE.
    for (const p of ts?.peers ?? []) {
      const addr = p.addrs[0];
      if (!addr) continue;
      out.set(addr, {
        addr,
        meshName:     p.host_name,
        meshOs:       p.os,
        meshOnline:   p.online,
        chat:         null,
      });
    }
    // Layer in chat data — overrides + adds any peers we only have via chat.
    for (const c of chatPeers) {
      const prev = out.get(c.addr);
      out.set(c.addr, { ...(prev ?? { addr: c.addr, meshName: null, meshOs: null, meshOnline: false }), chat: c });
    }
    return Array.from(out.values()).sort((a, b) =>
      (a.chat?.display_name ?? a.meshName ?? a.addr).localeCompare(
        b.chat?.display_name ?? b.meshName ?? b.addr,
      ),
    );
  }, [ts, chatPeers]);

  const selectedPeer = useMemo(
    () => peers.find((p) => p.addr === selected) ?? null,
    [peers, selected],
  );
  const selectedHistory = useMemo(
    () => history.filter((h) => h.peer_addr === selected),
    [history, selected],
  );

  // ---- handlers ---------------------------------------------------------
  const toggleListener = useCallback(async () => {
    setError(null);
    try {
      if (status?.running) {
        await chatStop();
      } else {
        await chatStart();
      }
      setStatus(await chatStatus());
    } catch (e) { setError(String(e)); }
  }, [status?.running]);

  const persistConfig = useCallback(async (next: ChatConfig) => {
    setConfig(next);
    try {
      await chatSetConfig(next);
    } catch (e) { setError(String(e)); }
  }, []);

  const connectPeer = useCallback(async (addr: string) => {
    setError(null);
    try {
      await chatConnectPeer(addr, config?.listen_port ?? 47992);
    } catch (e) { setError(String(e)); }
  }, [config]);

  const sendMessage = useCallback(async (addr: string, body: string) => {
    setError(null);
    try { await chatSend(addr, body); } catch (e) { setError(String(e)); }
  }, []);

  const setPresence = useCallback(async (presence: PresenceStatus, activity?: string | null) => {
    setError(null);
    try {
      await chatSetPresence(presence, activity);
      setStatus(await chatStatus());
    } catch (e) { setError(String(e)); }
  }, []);

  return (
    <div className="flex h-full flex-col">
      <header className="flex shrink-0 items-center gap-3 border-b border-abyss-border px-6 py-4">
        <h2 className="text-lg font-semibold text-abyss-fg abyss-text-glow">Friends</h2>
        <span className="text-xs text-abyss-fg-dim">Mesh peers · chat · presence</span>

        <div className="ml-auto flex items-center gap-2">
          <PresencePicker
            current={status?.presence ?? "idle"}
            activity={status?.activity ?? null}
            onChange={setPresence}
            disabled={!status?.running}
          />
          <button
            type="button"
            onClick={toggleListener}
            className={status?.running ? offlineBtn : onlineBtn}
          >
            {status?.running ? "Go offline" : "Go online"}
          </button>
        </div>
      </header>

      {incoming.length > 0 && (
        <div className="border-b border-abyss-accent/40 bg-abyss-accent/5 px-6 py-3">
          <div className="text-[11px] font-mono uppercase tracking-widest text-abyss-accent mb-2">
            ↘ Incoming transfer{incoming.length > 1 ? "s" : ""}
          </div>
          <ul className="space-y-2">
            {incoming.map((o) => {
              const prog = transferProgress[o.transfer_id];
              const pct = prog && prog.total > 0 ? Math.round((prog.bytes / prog.total) * 100) : null;
              return (
                <li key={o.transfer_id} className="flex items-center gap-3 rounded-md border border-abyss-accent/30 bg-abyss-panel/60 px-3 py-2 text-xs">
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-abyss-fg">
                      <span className="font-medium">{o.peer_addr}</span> wants to send{" "}
                      <span className="font-mono text-abyss-accent">{o.file_name}</span>{" "}
                      ({(o.file_size / (1024 * 1024)).toFixed(1)} MB)
                    </p>
                    {pct !== null && (
                      <div className="mt-1 h-1 overflow-hidden rounded-full bg-abyss-panel-2">
                        <div className="h-full bg-abyss-accent transition-all" style={{ width: `${pct}%` }} />
                      </div>
                    )}
                  </div>
                  {pct === null && (
                    <>
                      <button type="button" onClick={() => acceptOffer(o.transfer_id)} className="h-7 rounded-md border border-abyss-success/40 bg-abyss-success/10 px-3 text-[11px] font-medium text-abyss-success hover:bg-abyss-success/20">
                        Accept
                      </button>
                      <button type="button" onClick={() => rejectOffer(o.transfer_id)} className="h-7 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-[11px] font-medium text-abyss-fg-muted hover:border-abyss-danger/40 hover:text-abyss-danger">
                        Decline
                      </button>
                    </>
                  )}
                  {pct !== null && (
                    <span className="font-mono text-[11px] text-abyss-accent">{pct}%</span>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      )}

      {ts?.needs_auth && ts.auth_url && (
        <div className="flex items-center justify-between gap-3 border-b border-abyss-accent/30 bg-abyss-accent/5 px-6 py-2 text-xs text-abyss-fg">
          <span>
            <span className="text-abyss-accent">●</span> Your device isn't on a tailnet yet —
            sign in to start meshing.
          </span>
          <button
            type="button"
            onClick={() => openUrl(ts.auth_url!).catch(() => {})}
            className="h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
          >
            Sign in to mesh ↗
          </button>
        </div>
      )}

      {!status?.running && (
        <div className="border-b border-abyss-border bg-abyss-panel-2/40 px-6 py-2 text-[11px] text-abyss-fg-muted">
          The chat listener is off — peers can't reach you and you can't open links to them. Click{" "}
          <em>Go online</em> to bind on port {config?.listen_port ?? 47992}.
        </div>
      )}

      {/* Phase 12 — invite codes + in-app netplay lobby. Stacked at the top
          so they're visible before the user scrolls into the per-peer chat. */}
      <div className="space-y-4 border-b border-abyss-border bg-abyss-panel-2/20 px-6 py-5">
        <PartyHero
          onInviteFriend={() => {
            // Scroll the InvitesSection into view + expand it.
            const el = document.getElementById("invites-section");
            if (el) {
              el.scrollIntoView({ behavior: "smooth", block: "start" });
              const details = el.querySelector("details");
              if (details && !details.open) (details as HTMLDetailsElement).open = true;
            }
          }}
          onHostGame={() => {
            const el = document.getElementById("lobby-section");
            el?.scrollIntoView({ behavior: "smooth", block: "start" });
          }}
        />
        <div id="invites-section"><InvitesSection /></div>
        <div id="lobby-section"><LobbySection /></div>
      </div>

      {streamPair && (
        <div className={`flex items-center justify-between gap-3 border-b px-6 py-2 text-xs ${
          streamPair.phase === "accepted"
            ? "border-abyss-success/30 bg-abyss-success/10 text-abyss-success"
            : streamPair.phase === "timeout"
              ? "border-abyss-warning/30 bg-abyss-warning/10 text-abyss-warning"
              : "border-abyss-danger/30 bg-abyss-danger/10 text-abyss-danger"
        }`}>
          <span>
            {streamPair.phase === "accepted" && <>✓ Paired with {streamPair.host_addr}, Moonlight is launching…</>}
            {streamPair.phase === "timeout"  && <>⏱ {streamPair.host_addr} didn't accept the pair within 30s. Are they running Abyss?</>}
            {streamPair.phase === "rejected" && <>✗ {streamPair.host_addr} rejected the pair: {streamPair.error}</>}
          </span>
          <button type="button" onClick={() => setStreamPair(null)} className="text-[11px] underline-offset-2 hover:underline">dismiss</button>
        </div>
      )}

      <div className="grid flex-1 grid-cols-1 gap-0 overflow-hidden md:grid-cols-[300px_1fr]">
        {/* ============================== PEER LIST =========================== */}
        <aside className="overflow-auto border-r border-abyss-border bg-abyss-panel/40">
          {ts === null ? (
            <p className="p-4 text-xs text-abyss-fg-muted">Loading mesh status…</p>
          ) : !ts.installed ? (
            <p className="p-4 text-xs text-abyss-fg-muted">
              Tailscale CLI not detected — install it to discover mesh peers.{" "}
              {ts.error && (
                <span className="font-mono text-[10px] text-abyss-fg-dim">({ts.error})</span>
              )}
            </p>
          ) : peers.length === 0 ? (
            <p className="p-4 text-xs text-abyss-fg-dim">
              No peers on your mesh yet.
            </p>
          ) : (
            <ul className="divide-y divide-abyss-border">
              {peers.map((p) => (
                <PeerRow
                  key={p.addr}
                  peer={p}
                  selected={p.addr === selected}
                  onSelect={() => setSelected(p.addr)}
                  onConnect={() => connectPeer(p.addr)}
                  onStream={() => startStreamFromPeer(p.addr)}
                  streamBusy={streamPairBusy === p.addr}
                />
              ))}
            </ul>
          )}
        </aside>

        {/* ============================== CHAT PANEL ========================== */}
        <main className="flex flex-1 flex-col overflow-hidden">
          {selectedPeer ? (
            <ChatPanel
              peer={selectedPeer}
              history={selectedHistory}
              onConnect={() => connectPeer(selectedPeer.addr)}
              onSend={(body) => sendMessage(selectedPeer.addr, body)}
            />
          ) : (
            <EmptyChat />
          )}
        </main>
      </div>

      {error && (
        <p className="mx-6 mb-3 rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">
          {error}
        </p>
      )}

      {/* Display-name config — small but worth surfacing prominently. */}
      {config && (
        <footer className="border-t border-abyss-border bg-abyss-panel/40 px-6 py-2">
          <label className="flex items-center gap-3 text-xs text-abyss-fg-muted">
            <span className="w-24">Your name</span>
            <input
              type="text"
              spellCheck={false}
              value={config.display_name ?? ""}
              onChange={(e) =>
                persistConfig({ ...config, display_name: e.target.value || null })
              }
              placeholder="Shown to peers on first connect"
              className="
                h-7 flex-1 max-w-xs rounded-md border border-abyss-border bg-abyss-panel-2 px-2
                text-xs text-abyss-fg placeholder:text-abyss-fg-dim
                focus:border-abyss-accent/60 focus:outline-none
              "
            />
            <span className="ml-auto font-mono text-[10px] text-abyss-fg-dim">
              listening port {config.listen_port}
            </span>
          </label>
        </footer>
      )}
    </div>
  );
}

interface MergedPeer {
  addr:       string;
  meshName:   string | null;
  meshOs:     string | null;
  meshOnline: boolean;
  chat:       PeerSnapshot | null;
}

function PeerRow({
  peer,
  selected,
  onSelect,
  onConnect,
  onStream,
  streamBusy,
}: {
  peer:       MergedPeer;
  selected:   boolean;
  onSelect:   () => void;
  onConnect:  () => void;
  onStream:   () => void;
  streamBusy: boolean;
}) {
  const display = peer.chat?.display_name ?? peer.meshName ?? peer.addr;
  const dot =
    peer.chat?.connected
      ? "bg-abyss-success"
      : peer.meshOnline
        ? "bg-abyss-accent/50"
        : "bg-abyss-fg-dim";
  const canStream = Boolean(peer.chat?.connected); // need live chat for the pair offer

  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        className={[
          "flex w-full items-center gap-3 px-4 py-2 text-left transition-colors",
          selected
            ? "bg-abyss-accent/10 text-abyss-fg"
            : "text-abyss-fg-muted hover:bg-abyss-panel-2 hover:text-abyss-fg",
        ].join(" ")}
      >
        <span className={`h-2 w-2 rounded-full ${dot}`} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{display}</p>
          <p className="truncate font-mono text-[10px] text-abyss-fg-dim">
            {peer.addr}
            {peer.chat?.activity && (
              <span className="text-abyss-accent"> · {peer.chat.activity}</span>
            )}
          </p>
        </div>
        {!peer.chat?.connected && peer.meshOnline && (
          <button
            type="button"
            onClick={(e) => { e.stopPropagation(); onConnect(); }}
            className="h-6 rounded-sm border border-abyss-accent/40 bg-abyss-accent/10 px-2 text-[10px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
          >
            link
          </button>
        )}
        {canStream && (
          <button
            type="button"
            disabled={streamBusy}
            onClick={(e) => { e.stopPropagation(); onStream(); }}
            title="Pair Moonlight + Sunshine automatically and start streaming from this peer's PC"
            className="h-6 rounded-sm border border-abyss-success/40 bg-abyss-success/10 px-2 text-[10px] font-medium text-abyss-success hover:bg-abyss-success/20 disabled:cursor-wait disabled:opacity-50"
          >
            {streamBusy ? "pairing…" : "stream"}
          </button>
        )}
      </button>
    </li>
  );
}

function ChatPanel({
  peer,
  history,
  onConnect,
  onSend,
}: {
  peer:      MergedPeer;
  history:   ChatHistoryEntry[];
  onConnect: () => void;
  onSend:    (body: string) => void;
}) {
  const [draft, setDraft] = useState("");
  const scrollerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to the bottom when new history arrives.
  useEffect(() => {
    if (scrollerRef.current) {
      scrollerRef.current.scrollTop = scrollerRef.current.scrollHeight;
    }
  }, [history]);

  const send = useCallback(() => {
    const body = draft.trim();
    if (!body) return;
    onSend(body);
    setDraft("");
  }, [draft, onSend]);

  const connected = Boolean(peer.chat?.connected);

  return (
    <>
      <header className="flex items-center gap-3 border-b border-abyss-border bg-abyss-panel/60 px-4 py-2">
        <span
          className={`h-2 w-2 rounded-full ${
            connected ? "bg-abyss-success" : peer.meshOnline ? "bg-abyss-accent/50" : "bg-abyss-fg-dim"
          }`}
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-semibold text-abyss-fg">
            {peer.chat?.display_name ?? peer.meshName ?? peer.addr}
          </p>
          <p className="truncate font-mono text-[10px] text-abyss-fg-dim">
            {peer.addr}
            {peer.chat?.presence && (
              <span className="ml-2 uppercase tracking-widest">{peer.chat.presence}</span>
            )}
            {peer.chat?.activity && (
              <span className="ml-2 text-abyss-accent">· {peer.chat.activity}</span>
            )}
          </p>
        </div>
        {!connected && (
          <button
            type="button"
            onClick={onConnect}
            className="h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
          >
            Connect
          </button>
        )}
      </header>

      <div ref={scrollerRef} className="flex-1 overflow-auto p-4">
        {history.length === 0 ? (
          <p className="mx-auto max-w-sm text-center text-xs text-abyss-fg-dim">
            No messages yet. {connected ? "Say hi." : "Connect first to start chatting."}
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {history.map((m) => (
              <li
                key={m.id}
                className={[
                  "flex flex-col gap-0.5 rounded-md px-3 py-1.5 text-sm max-w-[78%]",
                  m.direction === "outbound"
                    ? "self-end bg-abyss-accent/15 text-abyss-fg"
                    : "self-start bg-abyss-panel-2 text-abyss-fg",
                ].join(" ")}
              >
                <span className="whitespace-pre-wrap break-words">{m.body}</span>
                <span className="self-end font-mono text-[9px] text-abyss-fg-dim">
                  {new Date(m.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <footer className="border-t border-abyss-border bg-abyss-panel/60 px-3 py-2">
        <form
          onSubmit={(e) => { e.preventDefault(); send(); }}
          className="flex gap-2"
        >
          <input
            type="text"
            value={draft}
            disabled={!connected}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={connected ? "Message…" : "Connect to chat"}
            className="
              h-9 flex-1 rounded-md border border-abyss-border bg-abyss-panel-2 px-3
              text-sm text-abyss-fg placeholder:text-abyss-fg-dim
              focus:border-abyss-accent/60 focus:outline-none
              disabled:opacity-60
            "
          />
          <button
            type="submit"
            disabled={!connected || !draft.trim()}
            className="h-9 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            Send
          </button>
        </form>
      </footer>
    </>
  );
}

function PresencePicker({
  current, activity, onChange, disabled,
}: {
  current:  PresenceStatus;
  activity: string | null;
  onChange: (status: PresenceStatus, activity?: string | null) => void;
  disabled: boolean;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="font-mono text-[10px] uppercase tracking-widest text-abyss-fg-dim">
        you
      </span>
      <select
        value={current}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as PresenceStatus, activity)}
        className="
          h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-2
          text-xs text-abyss-fg focus:border-abyss-accent/60 focus:outline-none
          disabled:opacity-50
        "
      >
        <option value="idle">Idle</option>
        <option value="playing">Playing</option>
        <option value="streaming">Streaming</option>
        <option value="away">Away</option>
      </select>
    </div>
  );
}

function EmptyChat() {
  return (
    <div className="flex h-full items-center justify-center p-10">
      <div className="max-w-md rounded-xl border border-dashed border-abyss-border-2 bg-abyss-panel/40 px-8 py-10 text-center">
        <p className="text-sm text-abyss-fg-muted">
          Pick a peer on the left to open a chat window.
        </p>
        <p className="mt-2 text-xs text-abyss-fg-dim">
          Chat runs peer-to-peer over the Tailscale mesh — no relay servers, no accounts. The other
          side just needs to be running Abyss Singularity with the listener online.
        </p>
      </div>
    </div>
  );
}

const onlineBtn  = "h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent transition-colors hover:bg-abyss-accent/20";
const offlineBtn = "h-8 rounded-md border border-abyss-danger/40 bg-abyss-danger/10 px-3 text-sm font-medium text-abyss-danger transition-colors hover:bg-abyss-danger/20";
