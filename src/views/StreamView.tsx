import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { NetplaySection } from "../components/NetplaySection";
import {
  streamAddHost,
  streamGetConfig,
  streamHostStatus,
  streamLaunchClient,
  streamPairClient,
  streamRemoveHost,
  streamResetCredentials,
  streamStartHost,
  streamStopHost,
  type HostStatus,
  type StreamingConfig,
} from "../lib/streaming";

export function StreamView() {
  const [config, setConfig] = useState<StreamingConfig | null>(null);
  const [host, setHost]     = useState<HostStatus | null>(null);
  const [error, setError]   = useState<string | null>(null);

  // New-host form state.
  const [newHostName, setNewHostName] = useState("");
  const [newHostAddr, setNewHostAddr] = useState("");

  const refresh = useCallback(async () => {
    try {
      const [c, h] = await Promise.all([streamGetConfig(), streamHostStatus()]);
      setConfig(c);
      setHost(h);
    } catch (e) { setError(String(e)); }
  }, []);

  // Stream view polling: refresh on mount, then every 10s while the tab
  // is visible. Each refresh calls `streamHostStatus` which on Windows
  // invokes `sc query` (process spawn, even though we now suppress the
  // console flash via CREATE_NO_WINDOW). Polling every 3s made the tab
  // feel laggy; 10s is plenty for "is Sunshine still running" without
  // burning CPU. Pausing while the tab is hidden saves more when the
  // user is in Library/Friends.
  useEffect(() => {
    refresh();
    let t: ReturnType<typeof setInterval> | null = null;
    const startTimer = () => {
      if (t === null) t = setInterval(refresh, 10000);
    };
    const stopTimer = () => {
      if (t !== null) { clearInterval(t); t = null; }
    };
    startTimer();
    const onVis = () => { document.hidden ? stopTimer() : (refresh(), startTimer()); };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      document.removeEventListener("visibilitychange", onVis);
      stopTimer();
    };
  }, [refresh]);

  const startHost = useCallback(async () => {
    setError(null);
    try { setHost(await streamStartHost()); } catch (e) { setError(String(e)); }
  }, []);
  const stopHost = useCallback(async () => {
    setError(null);
    try { await streamStopHost(); refresh(); } catch (e) { setError(String(e)); }
  }, [refresh]);
  const openAdmin = useCallback(async () => {
    if (host?.admin_url) {
      try { await openUrl(host.admin_url); } catch (e) { setError(String(e)); }
    }
  }, [host]);

  const addHost = useCallback(async () => {
    if (!newHostName.trim() || !newHostAddr.trim()) return;
    setError(null);
    try {
      const updated = await streamAddHost({
        id: crypto.randomUUID(),
        name: newHostName.trim(),
        host: newHostAddr.trim(),
      });
      setConfig(updated);
      setNewHostName("");
      setNewHostAddr("");
    } catch (e) { setError(String(e)); }
  }, [newHostName, newHostAddr]);

  const launchClient = useCallback(async (hostAddr?: string) => {
    setError(null);
    try { await streamLaunchClient(hostAddr); } catch (e) { setError(String(e)); }
  }, []);

  const dropHost = useCallback(async (id: string) => {
    try { setConfig(await streamRemoveHost(id)); } catch (e) { setError(String(e)); }
  }, []);

  return (
    <div className="flex h-full flex-col overflow-auto">
      <header className="flex shrink-0 items-center gap-3 border-b border-abyss-border px-6 py-4">
        <h2 className="text-lg font-semibold text-abyss-fg abyss-text-glow">Stream</h2>
        <span className="text-xs text-abyss-fg-dim">Sunshine host · Moonlight client</span>
      </header>

      <div className="grid grid-cols-1 gap-6 p-6 lg:grid-cols-2">
        {/* ============================== HOST ============================ */}
        <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-abyss-fg">Sunshine host</h3>
            <span
              className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest ${
                host?.running
                  ? "border-abyss-success/40 bg-abyss-success/10 text-abyss-success"
                  : host?.configured
                    ? "border-abyss-border bg-abyss-panel-2 text-abyss-fg-muted"
                    : "border-abyss-danger/30 bg-abyss-danger/10 text-abyss-danger"
              }`}
            >
              {host?.running ? "running" : host?.configured ? "stopped" : "not configured"}
            </span>
            {host?.pid && (
              <span className="font-mono text-[10px] text-abyss-fg-dim">pid {host.pid}</span>
            )}
          </div>

          {!host?.configured ? (
            <p className="mt-3 text-xs text-abyss-fg-muted">
              Set the Sunshine binary path under <em>Settings &gt; Streaming</em>, then come back here to
              start the host. Sunshine is the open-source GameStream-compatible server; install it
              from <code className="text-abyss-accent">github.com/LizardByte/Sunshine</code>.
            </p>
          ) : (
            <p className="mt-3 text-xs text-abyss-fg-muted">
              When the host is running it exposes the gaming PC to Moonlight clients on the mesh
              (configured under <em>Network</em>). The admin UI handles app definitions, codec
              settings, and the pairing PIN.
            </p>
          )}

          <div className="mt-4 flex flex-wrap items-center gap-2">
            {host?.running ? (
              <button type="button" onClick={stopHost} className={dangerBtn}>Stop host</button>
            ) : (
              <button
                type="button"
                onClick={startHost}
                disabled={!host?.configured}
                className={primaryBtn}
              >
                Start host
              </button>
            )}
            <button
              type="button"
              onClick={openAdmin}
              disabled={!host?.admin_url}
              className={secondaryBtn}
            >
              Open admin UI ↗
            </button>
            <ResetCredsButton
              configured={Boolean(host?.configured)}
              hasCreds={Boolean(config?.sunshine_admin_user && config?.sunshine_admin_pass)}
              onError={setError}
              onDone={() => refresh()}
            />
          </div>

          <PairClientPanel hostRunning={host?.running ?? false} config={config} />
        </section>

        {/* ============================== CLIENT ========================== */}
        <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-abyss-fg">Moonlight client</h3>
            <span
              className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest ${
                config?.moonlight_exe
                  ? "border-abyss-success/40 bg-abyss-success/10 text-abyss-success"
                  : "border-abyss-danger/30 bg-abyss-danger/10 text-abyss-danger"
              }`}
            >
              {config?.moonlight_exe ? "configured" : "not configured"}
            </span>
          </div>

          {!config?.moonlight_exe ? (
            <p className="mt-3 text-xs text-abyss-fg-muted">
              Set the Moonlight binary path under <em>Settings &gt; Streaming</em>. The client renders
              streams from any GameStream/Sunshine host on the mesh.
            </p>
          ) : (
            <>
              <p className="mt-3 text-xs text-abyss-fg-muted">
                Quick-connect to a known host on the mesh, or launch the client's own picker.
              </p>

              <ul className="mt-3 divide-y divide-abyss-border rounded-sm border border-abyss-border">
                {(config.known_hosts ?? []).length === 0 ? (
                  <li className="px-3 py-2 text-xs text-abyss-fg-dim">No known hosts yet.</li>
                ) : (
                  config.known_hosts.map((h) => (
                    <li key={h.id} className="flex items-center gap-3 px-3 py-2">
                      <div className="flex-1 min-w-0">
                        <p className="text-xs font-medium text-abyss-fg">{h.name}</p>
                        <code className="text-[10px] text-abyss-fg-dim">{h.host}</code>
                      </div>
                      <button type="button" onClick={() => launchClient(h.host)} className={primarySmallBtn}>
                        Connect
                      </button>
                      <button type="button" onClick={() => dropHost(h.id)} className={destructiveSmallBtn}>
                        ×
                      </button>
                    </li>
                  ))
                )}
              </ul>

              <div className="mt-3 grid grid-cols-1 gap-2 md:grid-cols-[1fr_1.4fr_auto]">
                <input
                  type="text"
                  value={newHostName}
                  onChange={(e) => setNewHostName(e.target.value)}
                  placeholder="Host nickname"
                  className={inputCls}
                />
                <input
                  type="text"
                  value={newHostAddr}
                  onChange={(e) => setNewHostAddr(e.target.value)}
                  placeholder="IP or hostname (e.g. 100.64.0.2)"
                  className={inputCls}
                />
                <button type="button" onClick={addHost} className={primaryBtn}>+ Add host</button>
              </div>

              <button type="button" onClick={() => launchClient()} className={`mt-3 ${secondaryBtn}`}>
                Open Moonlight (picker)
              </button>
            </>
          )}
        </section>

        {/* ============================== NETPLAY ========================== */}
        <div className="lg:col-span-2">
          <NetplaySection />
        </div>
      </div>

      {error && (
        <p className="mx-6 mb-6 rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">
          {error}
        </p>
      )}
    </div>
  );
}

const primaryBtn = `
  h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3
  text-sm font-medium text-abyss-accent transition-colors hover:bg-abyss-accent/20
  disabled:cursor-not-allowed disabled:opacity-50
`;
const primarySmallBtn = `
  h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-2
  text-[11px] font-medium text-abyss-accent transition-colors hover:bg-abyss-accent/20
`;
const secondaryBtn = `
  h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-3
  text-sm font-medium text-abyss-fg transition-colors
  hover:border-abyss-accent/40 hover:text-abyss-accent
  disabled:cursor-not-allowed disabled:opacity-50
`;
const dangerBtn = `
  h-8 rounded-md border border-abyss-danger/60 bg-abyss-danger/20 px-3
  text-sm font-medium text-abyss-danger transition-colors hover:bg-abyss-danger/30
`;
const destructiveSmallBtn = `
  h-7 w-7 rounded-sm border border-abyss-border bg-transparent text-[12px]
  text-abyss-fg-muted transition-colors hover:border-abyss-danger/40 hover:text-abyss-danger
`;
const inputCls = `
  h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-2 font-mono
  text-xs text-abyss-fg placeholder:text-abyss-fg-dim
  focus:border-abyss-accent/60 focus:outline-none
`;

/**
 * "Reset Sunshine credentials" button. Forces a fresh `sunshine --creds`
 * invocation so the in-app auto-pair flow works on machines where Sunshine
 * was installed manually (and the install-time auto-setup never fired).
 * One UAC prompt; new credentials persisted to StreamingConfig.
 */
function ResetCredsButton({
  configured,
  hasCreds,
  onError,
  onDone,
}: {
  configured: boolean;
  hasCreds:   boolean;
  onError:    (msg: string | null) => void;
  onDone:     () => void;
}) {
  const [busy,    setBusy]    = useState(false);
  const [report,  setReport]  = useState<{ user: string; pass: string } | null>(null);
  const [reveal,  setReveal]  = useState(false);

  const reset = useCallback(async () => {
    onError(null);
    setBusy(true);
    setReport(null);
    try {
      const r = await streamResetCredentials();
      setReport(r);
      onDone();
    } catch (e) { onError(String(e)); }
    finally { setBusy(false); }
  }, [onError, onDone]);

  return (
    <>
      <button
        type="button"
        onClick={reset}
        disabled={busy || !configured}
        title={hasCreds
          ? "Sunshine admin creds are already set in Abyss. Click to rotate them anyway."
          : "Generate Sunshine admin credentials so the in-app auto-pair flow can talk to Sunshine. One UAC prompt."}
        className={`${secondaryBtn} ${hasCreds ? "" : "border-abyss-warning/40 text-abyss-warning hover:border-abyss-warning/60"}`}
      >
        {busy ? "Resetting…" : hasCreds ? "Rotate Sunshine creds" : "Set Sunshine creds"}
      </button>
      {report && (
        <span className="ml-1 inline-flex items-center gap-1 rounded-md border border-abyss-success/40 bg-abyss-success/10 px-2 py-1 text-[11px] text-abyss-success">
          ✓ {report.user}/
          <code
            className="cursor-pointer font-mono"
            onClick={() => setReveal((r) => !r)}
            title="click to reveal/hide"
          >
            {reveal ? report.pass : "•".repeat(report.pass.length)}
          </code>
          <button
            type="button"
            onClick={() => { navigator.clipboard?.writeText(report.pass).catch(() => {}); }}
            className="text-abyss-fg-dim underline-offset-2 hover:underline"
          >
            copy
          </button>
        </span>
      )}
    </>
  );
}

/**
 * In-app Sunshine pairing — saves the friend from opening Sunshine's web UI.
 * Friend opens Moonlight → adds the host → Moonlight shows a 4-digit PIN →
 * host pastes it here → Abyss POSTs the PIN straight to Sunshine's REST
 * `/api/pin` endpoint. The new auto-pair flow under Friends → "stream"
 * does the same thing without the PIN-reading-aloud step; this panel is
 * still useful for manual pairing from an external Moonlight client.
 *
 * `config` is owned by the parent StreamView so credential changes (e.g.
 * the user clicking "Set Sunshine creds" above) flow through immediately
 * instead of being cached locally and going stale.
 */
function PairClientPanel({
  hostRunning,
  config,
}: {
  hostRunning: boolean;
  config:      StreamingConfig | null;
}) {
  const [pin, setPin]   = useState("");
  const [name, setName] = useState("");
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg]   = useState<string | null>(null);
  const [ok, setOk]     = useState(false);

  // Source of truth for "do we have admin creds?" — derived from the
  // config the parent fetched/refreshed. Was a useState + useEffect-once
  // cache before, which went stale after the "Set Sunshine creds" button
  // populated config behind our back.
  const credsKnown = config === null
    ? null
    : Boolean(config.sunshine_admin_user && config.sunshine_admin_pass);

  const pinValid       = pin.length === 4;
  const credsProvided  = credsKnown === true || (user.length > 0 && pass.length > 0);
  const canSubmit      = hostRunning && pinValid && credsProvided && !busy;

  const blockedReason: string | null =
    !hostRunning ? "Start the host first"
    : !pinValid  ? `${4 - pin.length} more digit${(4 - pin.length) === 1 ? "" : "s"}`
    : credsKnown === false && !credsProvided ? "Enter Sunshine creds"
    : null;

  const submit = useCallback(async () => {
    setBusy(true); setMsg(null); setOk(false);
    try {
      await streamPairClient(
        pin,
        name || undefined,
        credsKnown ? undefined : user,
        credsKnown ? undefined : pass,
      );
      setOk(true);
      setMsg("Paired. Friend can click your PC in Moonlight and start a stream.");
      setPin("");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  }, [pin, name, user, pass, credsKnown]);

  return (
    <div className="mt-4 rounded-md border border-abyss-border bg-abyss-panel-2/40 p-3">
      <div className="flex items-center gap-2">
        <p className="text-xs font-medium text-abyss-fg">Pair a Moonlight client</p>
        {credsKnown === true && (
          <span className="inline-flex items-center gap-1 rounded-full border border-abyss-success/40 bg-abyss-success/10 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest text-abyss-success">
            creds ✓
          </span>
        )}
      </div>
      <p className="mt-0.5 text-[11px] leading-relaxed text-abyss-fg-muted">
        For an external Moonlight pairing: friend adds this PC, Moonlight shows a 4-digit PIN, paste
        it here. (Friends inside Abyss can skip this — the Friends → <em>stream</em> button auto-pairs.)
      </p>
      {!hostRunning && (
        <p className="mt-1 text-[11px] text-abyss-warning">
          Start the Sunshine host first — pairing only works while it's running.
        </p>
      )}
      {credsKnown === false && (
        <div className="mt-2 grid grid-cols-2 gap-2">
          <input
            type="text"
            value={user}
            onChange={(e) => setUser(e.target.value)}
            placeholder="Sunshine admin user"
            className={inputCls}
          />
          <input
            type="password"
            value={pass}
            onChange={(e) => setPass(e.target.value)}
            placeholder="Sunshine admin password"
            className={inputCls}
          />
          <p className="col-span-2 text-[10px] text-abyss-fg-dim">
            One-time. Tip: click <em>Set Sunshine creds</em> in the buttons above to skip this entirely.
          </p>
        </div>
      )}
      <div className="mt-2 grid grid-cols-[6rem_1fr_auto] items-center gap-2">
        <input
          type="text"
          inputMode="numeric"
          maxLength={4}
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, "").slice(0, 4))}
          placeholder="PIN"
          className={`${inputCls} text-center font-mono text-base tracking-[0.4em]`}
        />
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Friend name (optional)"
          className={inputCls}
        />
        <button
          type="button"
          disabled={!canSubmit}
          onClick={submit}
          className="h-9 min-w-[6rem] shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-4 text-sm font-semibold text-abyss-accent hover:bg-abyss-accent/20 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {busy ? "Pairing…" : "Pair"}
        </button>
      </div>
      {!canSubmit && blockedReason && !msg && (
        <p className="mt-1 text-[10px] text-abyss-fg-dim">
          {blockedReason}
        </p>
      )}
      {msg && (
        <p className={`mt-2 text-[11px] ${ok ? "text-abyss-success" : "text-abyss-danger"}`}>
          {msg}
        </p>
      )}
    </div>
  );
}
