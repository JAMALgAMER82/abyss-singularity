import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  enrichLibraryMetadata,
  loadLibrary,
  onEnrichProgress,
  onScanProgress,
  scanLibrary,
  type EnrichProgressEvent,
  type EnrichReport,
  type LibraryEntry,
  type Platform,
  type ScanProgressEvent,
  type ScanReport,
  PLATFORM_DISPLAY,
} from "../lib/library";
import {
  onLaunchEvent,
  orchLaunch,
  orchListRunning,
  orchTerminate,
  type RunningProcess,
} from "../lib/orchestration";
import { chatGetPeers, type PeerSnapshot } from "../lib/chat";
import {
  onTransferEvent,
  transferSend,
  type TransferEvent,
} from "../lib/transfer";
import {
  installerInstallAll,
  installerRepair,
} from "../lib/installer";

type ScanState =
  | { kind: "idle" }
  | { kind: "running"; current: ScanProgressEvent | null }
  | { kind: "done"; report: ScanReport }
  | { kind: "error"; message: string };

type EnrichState =
  | { kind: "idle" }
  | { kind: "running"; current: EnrichProgressEvent | null }
  | { kind: "done"; report: EnrichReport }
  | { kind: "error"; message: string };

export function LibraryView() {
  const [entries, setEntries]     = useState<LibraryEntry[]>([]);
  const [scan, setScan]           = useState<ScanState>({ kind: "idle" });
  const [enrich, setEnrich]       = useState<EnrichState>({ kind: "idle" });
  const [filter, setFilter]       = useState("");
  const [platform, setPlatform]   = useState<Platform | "all">("all");
  const [running, setRunning]     = useState<RunningProcess[]>([]);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [peers, setPeers]             = useState<PeerSnapshot[]>([]);
  const [sendTarget, setSendTarget]   = useState<{ entry: LibraryEntry; peer: string } | null>(null);
  const [pickerForEntry, setPickerForEntry] = useState<LibraryEntry | null>(null);
  const [transferState, setTransferState]   = useState<Record<string, TransferEvent>>({});

  // Initial load from the on-disk cache. Cheap — just a JSON read.
  useEffect(() => {
    loadLibrary()
      .then(setEntries)
      .catch((e) => setScan({ kind: "error", message: String(e) }));
  }, []);

  // Subscribe to scan-progress events for the lifetime of the view.
  useEffect(() => {
    let unlisten: undefined | (() => void);
    onScanProgress((e) =>
      setScan((prev) => (prev.kind === "running" ? { kind: "running", current: e } : prev))
    ).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, []);

  // Subscribe to enrich-progress events for the lifetime of the view.
  useEffect(() => {
    let unlisten: undefined | (() => void);
    onEnrichProgress((e) =>
      setEnrich((prev) => (prev.kind === "running" ? { kind: "running", current: e } : prev))
    ).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, []);

  const startScan = useCallback(async () => {
    setScan({ kind: "running", current: null });
    try {
      const result = await scanLibrary();
      setEntries(result.entries);
      setScan({ kind: "done", report: result.report });
    } catch (err) {
      setScan({ kind: "error", message: String(err) });
    }
  }, []);

  const startEnrich = useCallback(async () => {
    setEnrich({ kind: "running", current: null });
    try {
      const report = await enrichLibraryMetadata(false);
      setEntries(report.entries);
      setEnrich({ kind: "done", report });
    } catch (err) {
      setEnrich({ kind: "error", message: String(err) });
    }
  }, []);

  const unenrichedCount = useMemo(
    () => entries.filter((e) => !e.igdb).length,
    [entries]
  );

  // Per-run telemetry: launch time + ring-buffered stderr, so we can
  // surface a useful banner when an emulator crashes within seconds of
  // spawn — the "click play, Abyss flickers minimised, nothing happens"
  // failure mode (missing BIOS, missing core, AV block, etc.).
  const runsRef = useRef<Map<string, {
    startedAt:  number;
    stderr:     string[];
    emulatorId: string;
    entryId:    string;
  }>>(new Map());
  const STDERR_CAP = 8;
  // Any launch that ends within 6s is suspicious enough to surface the
  // diagnostic banner — a real gameplay session never quits that fast.
  // We deliberately ignore the exit code: PCSX2 v2's `-batch` mode can
  // hit a fatal init error (missing BIOS, bad ROM path, GPU init fail)
  // and exit *clean* (code 0) without ever showing UI, which a
  // "non-zero only" detector would miss completely.
  const SHORT_LIFE_MS = 6000;

  const [crashReport, setCrashReport] = useState<{
    entryName:  string;
    emulatorId: string;
    lived:      number;
    exitCode:   number | null;
    stderrTail: string;
  } | null>(null);

  // Track currently-running emulator processes so we can show a badge +
  // a "Stop" button on the card of any game currently being played, and
  // detect short-lived crashes via the runsRef telemetry above.
  useEffect(() => {
    orchListRunning().then(setRunning).catch(() => { /* fine if none */ });
    let unlisten: undefined | (() => void);
    onLaunchEvent((e) => {
      if (e.kind === "stderr") {
        const r = runsRef.current.get(e.run_id);
        if (r) {
          r.stderr.push(e.line);
          if (r.stderr.length > STDERR_CAP) r.stderr.splice(0, r.stderr.length - STDERR_CAP);
        }
      } else if (e.kind === "exited") {
        const r = runsRef.current.get(e.run_id);
        runsRef.current.delete(e.run_id);
        setRunning((prev) => prev.filter((rp) => rp.run_id !== e.run_id));
        if (r) {
          const lived = Date.now() - r.startedAt;
          if (lived < SHORT_LIFE_MS) {
            const entry = entries.find((x) => x.id === r.entryId);
            const tail = r.stderr.length > 0 ? r.stderr.join("\n") : "(no stderr captured)";
            setCrashReport({
              entryName:  entry?.stem ?? "this game",
              emulatorId: r.emulatorId,
              lived,
              exitCode:   e.code,
              stderrTail: tail,
            });
          }
        }
      }
    }).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, [entries]);

  const runningByEntry = useMemo(() => {
    const m = new Map<string, RunningProcess>();
    for (const r of running) m.set(r.entry_id, r);
    return m;
  }, [running]);

  const play = useCallback(async (entry: LibraryEntry) => {
    setLaunchError(null);
    setCrashReport(null);
    try {
      const handle = await orchLaunch(entry.id);
      // Register telemetry for the crash detector. We hold the exact
      // command line we asked the OS to run so the crash banner can
      // surface it — useful when the user knows the emulator works if
      // launched manually and wants to compare what Abyss is doing.
      runsRef.current.set(handle.run_id, {
        startedAt:   Date.now(),
        stderr:      [],
        emulatorId:  handle.emulator_id,
        entryId:     handle.entry_id,
      });
      setLastCommand({
        runId:       handle.run_id,
        commandLine: handle.command_line,
        entryName:   entry.stem,
      });
      setRunning((prev) => [...prev, {
        run_id: handle.run_id,
        pid: handle.pid,
        started_at: handle.started_at,
        emulator_id: handle.emulator_id,
        entry_id: handle.entry_id,
      }]);
    } catch (err) {
      setLaunchError(String(err));
    }
  }, []);

  // Memory of the most recent launch attempt — only used by the crash
  // banner so it can show "Abyss ran: <exact command>".
  const [lastCommand, setLastCommand] = useState<{
    runId:       string;
    commandLine: string;
    entryName:   string;
  } | null>(null);

  const stop = useCallback(async (runId: string) => {
    try { await orchTerminate(runId); } catch { /* nothing to do */ }
  }, []);

  // Live peer list for the "Send to peer" picker.
  useEffect(() => {
    chatGetPeers().then(setPeers).catch(() => {});
    const t = setInterval(() => {
      chatGetPeers().then(setPeers).catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

  // Transfer events feed the per-card progress bars.
  useEffect(() => {
    let unlisten: undefined | (() => void);
    onTransferEvent((e) => {
      const id = (e as { transfer_id?: string }).transfer_id;
      if (id) setTransferState((prev) => ({ ...prev, [id]: e }));
    }).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, []);

  const sendToPeer = useCallback(async (entry: LibraryEntry, peerAddr: string) => {
    setPickerForEntry(null);
    try {
      await transferSend(entry.id, peerAddr);
      setSendTarget({ entry, peer: peerAddr });
    } catch (err) {
      setLaunchError(String(err));
    }
  }, []);

  const platforms = useMemo(() => {
    const set = new Set<Platform>();
    for (const e of entries) set.add(e.platform);
    return Array.from(set).sort();
  }, [entries]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return entries.filter((e) => {
      if (platform !== "all" && e.platform !== platform) return false;
      if (q && !e.stem.toLowerCase().includes(q)) return false;
      return true;
    });
  }, [entries, filter, platform]);

  return (
    <div className="flex h-full flex-col">
      <header className="flex shrink-0 items-center gap-3 border-b border-abyss-border px-6 py-4">
        <h2 className="text-lg font-semibold text-abyss-fg abyss-text-glow">Library</h2>
        <span className="text-xs text-abyss-fg-dim">{entries.length} entries cached</span>

        <div className="ml-auto flex items-center gap-2">
          <input
            type="search"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Filter by name…"
            className="
              h-8 w-56 rounded-md border border-abyss-border bg-abyss-panel-2
              px-3 text-sm text-abyss-fg placeholder:text-abyss-fg-dim
              focus:border-abyss-accent/60 focus:outline-none
            "
          />
          <select
            value={platform}
            onChange={(e) => setPlatform(e.target.value as Platform | "all")}
            className="
              h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-2
              text-sm text-abyss-fg focus:border-abyss-accent/60 focus:outline-none
            "
          >
            <option value="all">All platforms</option>
            {platforms.map((p) => (
              <option key={p} value={p}>{PLATFORM_DISPLAY[p]}</option>
            ))}
          </select>
          <button
            type="button"
            onClick={startEnrich}
            disabled={enrich.kind === "running" || entries.length === 0}
            title={
              entries.length === 0
                ? "Run a scan first"
                : unenrichedCount > 0
                  ? `Enrich ${unenrichedCount} entries via IGDB`
                  : "All entries already enriched"
            }
            className="
              h-8 rounded-md border border-abyss-border bg-abyss-panel-2
              px-3 text-sm font-medium text-abyss-fg transition-colors
              hover:border-abyss-accent/40 hover:text-abyss-accent
              disabled:cursor-not-allowed disabled:opacity-50
            "
          >
            {enrich.kind === "running" ? "Enriching…" : "Enrich"}
          </button>
          <button
            type="button"
            onClick={startScan}
            disabled={scan.kind === "running"}
            className="
              h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10
              px-3 text-sm font-medium text-abyss-accent transition-colors
              hover:bg-abyss-accent/20 disabled:cursor-not-allowed disabled:opacity-50
            "
          >
            {scan.kind === "running" ? "Scanning…" : "Scan"}
          </button>
        </div>
      </header>

      <ScanStatusBar state={scan} />
      <EnrichStatusBar state={enrich} />
      {launchError && (
        <LaunchErrorPrompt
          error={launchError}
          onDismiss={() => setLaunchError(null)}
        />
      )}
      {crashReport && (
        <CrashReportPrompt
          report={crashReport}
          lastCommand={lastCommand}
          onDismiss={() => setCrashReport(null)}
        />
      )}

      {pickerForEntry && (
        <PeerPicker
          entry={pickerForEntry}
          peers={peers.filter((p) => p.connected)}
          onPick={(addr) => sendToPeer(pickerForEntry, addr)}
          onCancel={() => setPickerForEntry(null)}
        />
      )}

      {Object.values(transferState).some((e) => e.kind === "progress") && (
        <TransferProgressBar events={transferState} />
      )}

      {sendTarget && Object.values(transferState).some(
        (e) => e.kind === "completed" || e.kind === "failed",
      ) && null}

      <div className="flex-1 overflow-auto p-6">
        {entries.length === 0 && scan.kind !== "running" ? (
          <EmptyState />
        ) : filtered.length === 0 ? (
          <p className="text-sm text-abyss-fg-muted">
            No entries match the current filter.
          </p>
        ) : (
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
            {filtered.map((e) => (
              <GameCard
                key={e.id}
                entry={e}
                running={runningByEntry.get(e.id)}
                onPlay={() => play(e)}
                onStop={(runId) => stop(runId)}
                onSend={() => setPickerForEntry(e)}
                hasConnectedPeers={peers.some((p) => p.connected)}
              />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function EnrichStatusBar({ state }: { state: EnrichState }) {
  if (state.kind === "idle") return null;
  if (state.kind === "running") {
    const c = state.current;
    const pct = c && c.total > 0 ? Math.round((c.processed / c.total) * 100) : 0;
    return (
      <div className="border-b border-abyss-border bg-abyss-panel-2/60 px-6 py-2 text-xs text-abyss-fg-muted">
        <span className="text-abyss-accent">●</span>{" "}
        {c
          ? `IGDB enrichment — ${c.processed}/${c.total} (${pct}%), matched ${c.matched}`
          : "Authenticating with IGDB…"}
        {c?.current && (
          <span className="ml-2 text-abyss-fg-dim">/ {truncate(c.current, 50)}</span>
        )}
      </div>
    );
  }
  if (state.kind === "done") {
    const r = state.report;
    return (
      <div className="border-b border-abyss-border bg-abyss-panel-2/60 px-6 py-2 text-xs text-abyss-fg-muted">
        <span className="text-abyss-success">✓</span>{" "}
        Enriched {r.processed} entries in {(r.elapsed_ms / 1000).toFixed(2)}s —{" "}
        {r.matched} matched, {r.errors} error{r.errors === 1 ? "" : "s"}.
      </div>
    );
  }
  return (
    <div className="border-b border-abyss-border bg-abyss-danger/10 px-6 py-2 text-xs text-abyss-danger">
      Enrichment failed: {state.message}
    </div>
  );
}

function ScanStatusBar({ state }: { state: ScanState }) {
  if (state.kind === "idle") return null;

  if (state.kind === "running") {
    const c = state.current;
    return (
      <div className="border-b border-abyss-border bg-abyss-panel-2/60 px-6 py-2 text-xs text-abyss-fg-muted">
        <span className="text-abyss-accent">●</span>{" "}
        {c
          ? `Scanning ${truncate(c.root)} — ${c.files_seen} files seen, ${c.games_found} games`
          : "Starting scan…"}
        {c?.current_file && (
          <span className="ml-2 text-abyss-fg-dim">/ {truncate(c.current_file, 50)}</span>
        )}
      </div>
    );
  }
  if (state.kind === "done") {
    const r = state.report;
    return (
      <div className="border-b border-abyss-border bg-abyss-panel-2/60 px-6 py-2 text-xs text-abyss-fg-muted">
        <span className="text-abyss-success">✓</span>{" "}
        Scanned {r.total_files_seen} files in {(r.elapsed_ms / 1000).toFixed(2)}s —{" "}
        {r.games_found} games ({r.games_new} new, {r.games_kept} kept).
      </div>
    );
  }
  return (
    <div className="border-b border-abyss-border bg-abyss-danger/10 px-6 py-2 text-xs text-abyss-danger">
      Scan failed: {state.message}
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex h-full flex-col items-center justify-center text-center">
      <div className="rounded-xl border border-dashed border-abyss-border-2 bg-abyss-panel/40 px-10 py-12 max-w-md">
        <p className="text-sm text-abyss-fg-muted">
          No games scanned yet. Add a scan path under{" "}
          <span className="text-abyss-accent">Settings</span> and hit{" "}
          <span className="text-abyss-accent">Scan</span>.
        </p>
        <p className="mt-2 text-xs text-abyss-fg-dim">
          Recognised file types include `.nes`, `.snes`, `.iso`, `.exe`, and many more.
        </p>
      </div>
    </div>
  );
}

function GameCard({
  entry,
  running,
  onPlay,
  onStop,
  onSend,
  hasConnectedPeers,
}: {
  entry: LibraryEntry;
  running: RunningProcess | undefined;
  onPlay: () => void;
  onStop: (runId: string) => void;
  onSend: () => void;
  hasConnectedPeers: boolean;
}) {
  const sizeMb = (entry.size_bytes / (1024 * 1024)).toFixed(1);
  const isRunning = Boolean(running);

  return (
    <li
      className={[
        "group relative flex flex-col overflow-hidden rounded-lg border",
        "bg-abyss-panel/60 transition-colors",
        isRunning
          ? "border-abyss-accent/60 abyss-glow"
          : "border-abyss-border hover:border-abyss-accent/40 hover:bg-abyss-panel-2/70",
      ].join(" ")}
      title={entry.path}
    >
      <div className="relative h-32 bg-gradient-to-br from-abyss-panel-2 to-abyss-bg">
        {entry.cover_local_path || entry.igdb?.cover_url ? (
          <img
            src={entry.igdb?.cover_url ?? `asset://localhost/${entry.cover_local_path}`}
            alt=""
            className="absolute inset-0 h-full w-full object-cover"
            loading="lazy"
          />
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            <span className="font-mono text-[10px] uppercase tracking-widest text-abyss-fg-dim">
              {PLATFORM_DISPLAY[entry.platform]}
            </span>
          </div>
        )}
        <span
          className="
            absolute right-1.5 top-1.5 rounded-sm border border-abyss-accent/30
            bg-abyss-bg/80 px-1.5 py-0.5 font-mono text-[9px] uppercase
            tracking-wider text-abyss-accent
          "
        >
          {PLATFORM_DISPLAY[entry.platform]}
        </span>

        {/* Play / Stop overlay — appears on hover or while running. */}
        <div
          className={[
            "absolute inset-0 flex items-end justify-end p-2",
            "bg-gradient-to-t from-abyss-bg/95 to-transparent",
            isRunning ? "opacity-100" : "opacity-0 group-hover:opacity-100 transition-opacity",
          ].join(" ")}
        >
          {isRunning ? (
            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); onStop(running!.run_id); }}
              className="h-7 rounded-md border border-abyss-danger/60 bg-abyss-danger/20 px-2 text-[11px] font-medium text-abyss-danger hover:bg-abyss-danger/30"
            >
              ■ Stop
            </button>
          ) : (
            <div className="flex gap-1.5">
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); onPlay(); }}
                className="h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/20 px-2 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/30"
              >
                ▶ Play
              </button>
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); onSend(); }}
                disabled={!hasConnectedPeers}
                title={hasConnectedPeers ? "Send to a mesh peer" : "Connect to a peer in Friends first"}
                className="h-7 rounded-md border border-abyss-border bg-abyss-panel-2 px-2 text-[11px] font-medium text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent disabled:opacity-40 disabled:cursor-not-allowed"
              >
                ↗ Send
              </button>
            </div>
          )}
        </div>
      </div>
      <div className="flex-1 p-2">
        <p className="line-clamp-2 text-sm font-medium text-abyss-fg">
          {entry.igdb?.name ?? entry.stem}
        </p>
        <p className="mt-0.5 font-mono text-[10px] text-abyss-fg-dim">
          .{entry.extension} · {sizeMb} MB
          {isRunning && (
            <span className="ml-2 text-abyss-accent">● running (pid {running!.pid})</span>
          )}
        </p>
      </div>
    </li>
  );
}

function truncate(s: string, n: number = 60): string {
  return s.length > n ? `…${s.slice(-(n - 1))}` : s;
}

function PeerPicker({
  entry, peers, onPick, onCancel,
}: {
  entry:    LibraryEntry;
  peers:    PeerSnapshot[];
  onPick:   (addr: string) => void;
  onCancel: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-abyss-bg/70 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-xl border border-abyss-border bg-abyss-panel p-5 shadow-2xl">
        <h3 className="text-sm font-semibold text-abyss-fg">Send to peer</h3>
        <p className="mt-1 text-xs text-abyss-fg-muted">
          <span className="text-abyss-accent">{entry.igdb?.name ?? entry.stem}</span>{" "}
          ({(entry.size_bytes / (1024 * 1024)).toFixed(1)} MB) over the mesh.
        </p>
        <ul className="mt-3 divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel-2/50">
          {peers.length === 0 ? (
            <li className="px-3 py-3 text-xs text-abyss-fg-dim">
              No connected peers. Open Friends and click <em>Connect</em> on someone first.
            </li>
          ) : (
            peers.map((p) => (
              <li key={p.addr}>
                <button
                  type="button"
                  onClick={() => onPick(p.addr)}
                  className="flex w-full items-center gap-3 px-3 py-2 text-left text-abyss-fg-muted hover:bg-abyss-panel-2 hover:text-abyss-fg"
                >
                  <span className="h-2 w-2 rounded-full bg-abyss-success" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">{p.display_name ?? p.addr}</p>
                    <code className="text-[10px] text-abyss-fg-dim">{p.addr}</code>
                  </div>
                </button>
              </li>
            ))
          )}
        </ul>
        <div className="mt-3 flex justify-end gap-2">
          <button type="button" onClick={onCancel} className="h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-sm text-abyss-fg-muted hover:text-abyss-fg">
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

function TransferProgressBar({ events }: { events: Record<string, TransferEvent> }) {
  const active = Object.values(events).filter((e) => e.kind === "progress") as Array<
    Extract<TransferEvent, { kind: "progress" }>
  >;
  if (active.length === 0) return null;
  return (
    <div className="absolute bottom-4 right-4 z-40 w-72 space-y-2">
      {active.map((e) => {
        const pct = e.bytes_total > 0 ? Math.round((e.bytes_done / e.bytes_total) * 100) : 0;
        return (
          <div key={e.transfer_id} className="rounded-md border border-abyss-accent/40 bg-abyss-panel/95 p-3 shadow-xl backdrop-blur">
            <div className="flex items-center justify-between text-xs text-abyss-fg">
              <span>↗ Transfer</span>
              <span className="font-mono text-abyss-accent">{pct}%</span>
            </div>
            <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-abyss-panel-2">
              <div className="h-full bg-abyss-accent transition-all" style={{ width: `${pct}%` }} />
            </div>
            <p className="mt-1 font-mono text-[10px] text-abyss-fg-dim">
              {fmt(e.bytes_done)} / {fmt(e.bytes_total)}
            </p>
          </div>
        );
      })}
    </div>
  );
}

function fmt(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Big, friendly, actionable launch-failure banner.
 *
 * The previous version was a one-line strip at the top — easy to miss
 * once you'd scrolled into the library grid. Most real launch failures
 * fall into a few patterns:
 *   * "no emulator assigned to platform X"           → emulator never installed
 *   * "emulator X is missing from config"            → config drift
 *   * "emulator X has no exe path set"               → install partially completed
 *   * "extracted archive but expected exe missing"   → install corrupted
 *
 * Each pattern gets a tailored explanation + a single-click fix action
 * (either `installerInstallAll` or `installerRepair`). Anything we
 * don't recognise falls back to a raw error message plus a "Run Repair"
 * button — `installer_repair` is idempotent so it's safe as a generic
 * "try fixing yourself" gesture.
 */
function LaunchErrorPrompt({ error, onDismiss }: { error: string; onDismiss: () => void }) {
  const [busy,     setBusy]     = useState(false);
  const [progress, setProgress] = useState<string | null>(null);
  const [done,     setDone]     = useState<string | null>(null);
  const [showRaw,  setShowRaw]  = useState(false);

  // Pattern-match the error string to figure out which fix to suggest.
  // Order matters — more-specific patterns first.
  const diagnosis: {
    title:       string;
    explanation: string;
    actionLabel: string;
    action:      "install_all" | "repair" | null;
  } =
    /no emulator assigned to platform/i.test(error)
      ? {
          title:       "No emulator installed for this game",
          explanation: "You haven't installed the emulator that runs this platform yet. " +
                       "Abyss can install every supported emulator in one go (~600 MB, 5–10 min on a normal connection).",
          actionLabel: "Install all emulators",
          action:      "install_all",
        }
      : /is missing from config/i.test(error)
      ? {
          title:       "Emulator missing",
          explanation: "Abyss has a record of an emulator that isn't actually installed. Run Repair to re-scan " +
                       "the install folder and patch the config.",
          actionLabel: "Run Repair",
          action:      "repair",
        }
      : /has no exe path set|expected exe missing|extracted archive but/i.test(error)
      ? {
          title:       "Emulator install didn't finish",
          explanation: "The emulator extracted but Abyss couldn't find its exe. Run Repair to re-scan " +
                       "the install folder — most often this fixes archives that nested into a versioned subdir.",
          actionLabel: "Run Repair",
          action:      "repair",
        }
      : /library entry not found/i.test(error)
      ? {
          title:       "Game not in your library",
          explanation: "Abyss couldn't find this game's library entry. Re-scan your games folder under the " +
                       "Library tab and try again.",
          actionLabel: "",
          action:      null,
        }
      : {
          title:       "Couldn't launch this game",
          explanation: "Something went wrong starting the emulator. Running Repair is safe and often clears " +
                       "the issue — it re-scans installed emulators and patches their config in place.",
          actionLabel: "Run Repair",
          action:      "repair",
        };

  const runFix = useCallback(async () => {
    if (diagnosis.action === null) return;
    setBusy(true);
    setProgress(diagnosis.action === "install_all"
      ? "Installing emulators (this can take 5–10 minutes)…"
      : "Running Repair…");
    setDone(null);
    try {
      if (diagnosis.action === "install_all") {
        const r = await installerInstallAll();
        setDone(
          r.installed.length > 0
            ? `Installed ${r.installed.length} emulator${r.installed.length === 1 ? "" : "s"}. ` +
              `Click Play again — it should work now.`
            : r.alreadyPresent.length > 0
              ? `All emulators were already installed. The issue is something else — see the technical details below.`
              : `Nothing installed. Check your internet connection.`
        );
      } else {
        const repaired = await installerRepair();
        setDone(repaired > 0
          ? `Repaired ${repaired} emulator config entr${repaired === 1 ? "y" : "ies"}. Click Play again.`
          : `Nothing to repair — the issue is elsewhere. See the technical details below.`);
      }
    } catch (e) {
      setDone(`Fix failed: ${String(e)}`);
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }, [diagnosis.action]);

  return (
    <div className="border-y border-abyss-danger/40 bg-abyss-danger/10 px-6 py-4">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 text-2xl text-abyss-danger">⚠</span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-abyss-danger">{diagnosis.title}</p>
          <p className="mt-1 text-xs text-abyss-fg-muted">{diagnosis.explanation}</p>

          {progress && (
            <p className="mt-2 text-xs text-abyss-accent">
              <span className="inline-block animate-pulse">●</span> {progress}
            </p>
          )}
          {done && (
            <p className="mt-2 rounded-sm border border-abyss-success/30 bg-abyss-success/10 px-3 py-1.5 text-xs text-abyss-success">
              {done}
            </p>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-2">
            {diagnosis.action && (
              <button
                type="button"
                disabled={busy}
                onClick={runFix}
                className="h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-4 text-xs font-semibold text-abyss-accent transition-colors hover:bg-abyss-accent/20 disabled:cursor-wait disabled:opacity-50"
              >
                {busy ? "Working…" : diagnosis.actionLabel}
              </button>
            )}
            <button
              type="button"
              onClick={() => setShowRaw((v) => !v)}
              className="text-[11px] text-abyss-fg-muted underline-offset-2 hover:underline"
            >
              {showRaw ? "Hide" : "Show"} technical details
            </button>
            <button
              type="button"
              onClick={onDismiss}
              className="ml-auto text-[11px] text-abyss-fg-dim hover:text-abyss-fg"
            >
              ✕ Dismiss
            </button>
          </div>

          {showRaw && (
            <pre className="mt-2 max-h-32 overflow-auto rounded-sm border border-abyss-border bg-abyss-panel-2/60 p-2 font-mono text-[11px] text-abyss-fg-muted">
              {error}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * Crash banner — shown when an emulator exits non-zero within seconds of
 * launch. This is the "manual launch works but Abyss launch silently
 * fails" failure mode: process spawn succeeded (so the regular
 * LaunchErrorPrompt path doesn't fire), but the emulator died too fast
 * to even show its window.
 *
 * Surfaces:
 *   1. The exact command line Abyss invoked — so the user can compare
 *      against their working manual launch and spot the difference
 *      (wrong core path, missing flag, paren-escaping, etc.).
 *   2. The last stderr lines we captured before exit — usually contains
 *      the actual reason (missing core .dll, BIOS not found, etc.).
 *   3. A "Copy details" button to clip the whole thing for sharing with
 *      the Abyss host.
 *   4. A "Run Repair" action because most "Abyss launches differently
 *      than manual" cases come from stale config the repair routine fixes.
 */
function CrashReportPrompt({
  report,
  lastCommand,
  onDismiss,
}: {
  report: {
    entryName:  string;
    emulatorId: string;
    lived:      number;
    exitCode:   number | null;
    stderrTail: string;
  };
  lastCommand: { runId: string; commandLine: string; entryName: string } | null;
  onDismiss:   () => void;
}) {
  const [repairing, setRepairing] = useState(false);
  const [repaired,  setRepaired]  = useState<string | null>(null);
  const [copied,    setCopied]    = useState(false);

  const cmd = (lastCommand && lastCommand.runId === lastCommand.runId)
    ? lastCommand.commandLine
    : "(command line not captured)";

  const fullText = useMemo(() => {
    return [
      `Abyss launch crash report`,
      `Game:      ${report.entryName}`,
      `Emulator:  ${report.emulatorId}`,
      `Lived for: ${report.lived} ms`,
      `Exit code: ${report.exitCode ?? "null"}`,
      "",
      `Command Abyss ran:`,
      cmd,
      "",
      `Last stderr lines:`,
      report.stderrTail,
    ].join("\n");
  }, [report, cmd]);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(fullText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* clipboard denied — user can drag-select the textarea */ }
  }, [fullText]);

  const repair = useCallback(async () => {
    setRepairing(true);
    setRepaired(null);
    try {
      const n = await installerRepair();
      setRepaired(n > 0
        ? `Repaired ${n} entr${n === 1 ? "y" : "ies"}. Click Play again.`
        : `Nothing to repair — config looks consistent. Compare the command line above against a working manual launch and check for path / arg differences.`);
    } catch (e) {
      setRepaired(`Repair failed: ${String(e)}`);
    } finally {
      setRepairing(false);
    }
  }, []);

  return (
    <div className="border-y border-abyss-warning/40 bg-abyss-warning/10 px-6 py-4">
      <div className="flex items-start gap-3">
        <span className="mt-0.5 text-2xl text-abyss-warning">⚡</span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-abyss-warning">
            "{report.entryName}" launched but died after {report.lived} ms
          </p>
          <p className="mt-1 text-xs text-abyss-fg-muted">
            The emulator process exited too fast to show its window
            (exit code {report.exitCode ?? "null"}). If running the emulator
            manually with the same game works, it's almost always an args /
            path difference between the manual launch and what Abyss invoked.
            Compare the command line below to spot it.
          </p>

          {/* The command line Abyss invoked */}
          <details className="mt-3 rounded-md border border-abyss-border bg-abyss-panel-2/60" open>
            <summary className="cursor-pointer px-3 py-2 text-[11px] font-medium text-abyss-fg-muted hover:text-abyss-fg">
              Command Abyss ran
            </summary>
            <pre className="overflow-auto border-t border-abyss-border bg-abyss-panel/60 px-3 py-2 font-mono text-[11px] text-abyss-fg whitespace-pre-wrap break-all">
              {cmd}
            </pre>
          </details>

          {/* Last stderr — usually has the actual reason */}
          <details className="mt-2 rounded-md border border-abyss-border bg-abyss-panel-2/60" open>
            <summary className="cursor-pointer px-3 py-2 text-[11px] font-medium text-abyss-fg-muted hover:text-abyss-fg">
              Last stderr lines (often has the real reason)
            </summary>
            <pre className="max-h-40 overflow-auto border-t border-abyss-border bg-abyss-panel/60 px-3 py-2 font-mono text-[11px] text-abyss-fg-muted whitespace-pre-wrap">
              {report.stderrTail}
            </pre>
          </details>

          {repaired && (
            <p className={`mt-2 rounded-sm border px-3 py-1.5 text-xs ${
              repaired.startsWith("Repaired")
                ? "border-abyss-success/30 bg-abyss-success/10 text-abyss-success"
                : "border-abyss-border bg-abyss-panel-2/40 text-abyss-fg-muted"
            }`}>
              {repaired}
            </p>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={repairing}
              onClick={repair}
              className="h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-4 text-xs font-semibold text-abyss-accent hover:bg-abyss-accent/20 disabled:cursor-wait disabled:opacity-50"
            >
              {repairing ? "Repairing…" : "Run Repair"}
            </button>
            <button
              type="button"
              onClick={copy}
              className="h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-xs font-medium text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent"
            >
              {copied ? "✓ Copied" : "Copy details"}
            </button>
            <button
              type="button"
              onClick={onDismiss}
              className="ml-auto text-[11px] text-abyss-fg-dim hover:text-abyss-fg"
            >
              ✕ Dismiss
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
