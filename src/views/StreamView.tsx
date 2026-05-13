import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  streamAddHost,
  streamGetConfig,
  streamHostStatus,
  streamLaunchClient,
  streamRemoveHost,
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

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 3000);
    return () => clearInterval(t);
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
          </div>
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
