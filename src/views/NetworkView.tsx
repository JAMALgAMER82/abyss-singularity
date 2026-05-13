import { useCallback, useEffect, useMemo, useState } from "react";
import {
  probeRegions,
  tailscaleStatus,
  type ProbeReport,
  type ProbeResult,
  type TailscaleStatus,
} from "../lib/network";

type ProbeState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "done"; report: ProbeReport }
  | { kind: "error"; message: string };

export function NetworkView() {
  const [ts, setTs] = useState<TailscaleStatus | null>(null);
  const [probe, setProbe] = useState<ProbeState>({ kind: "idle" });

  useEffect(() => {
    tailscaleStatus().then(setTs).catch(() => setTs(null));
  }, []);

  const refreshTs = useCallback(() => {
    tailscaleStatus().then(setTs).catch(() => setTs(null));
  }, []);

  const runProbe = useCallback(async () => {
    setProbe({ kind: "running" });
    try {
      const report = await probeRegions();
      setProbe({ kind: "done", report });
    } catch (e) {
      setProbe({ kind: "error", message: String(e) });
    }
  }, []);

  const grouped = useMemo<Map<string, ProbeResult[]>>(() => {
    const m = new Map<string, ProbeResult[]>();
    if (probe.kind !== "done") return m;
    for (const r of probe.report.results) {
      const arr = m.get(r.continent) ?? [];
      arr.push(r);
      m.set(r.continent, arr);
    }
    return m;
  }, [probe]);

  return (
    <div className="flex h-full flex-col overflow-auto">
      <header className="flex shrink-0 items-center gap-3 border-b border-abyss-border px-6 py-4">
        <h2 className="text-lg font-semibold text-abyss-fg abyss-text-glow">Network</h2>
        <span className="text-xs text-abyss-fg-dim">P2P mesh · regional latency</span>
        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={refreshTs}
            className="h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-sm font-medium text-abyss-fg transition-colors hover:border-abyss-accent/40 hover:text-abyss-accent"
          >
            Refresh status
          </button>
          <button
            type="button"
            onClick={runProbe}
            disabled={probe.kind === "running"}
            className="h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent transition-colors hover:bg-abyss-accent/20 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {probe.kind === "running" ? "Probing…" : "Probe regions"}
          </button>
        </div>
      </header>

      <div className="grid grid-cols-1 gap-6 p-6 lg:grid-cols-2">
        <TailscalePanel status={ts} />
        <ProbePanel state={probe} grouped={grouped} />
      </div>
    </div>
  );
}

function TailscalePanel({ status }: { status: TailscaleStatus | null }) {
  if (!status) {
    return (
      <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
        <h3 className="text-sm font-semibold text-abyss-fg">Tailscale</h3>
        <p className="mt-2 text-xs text-abyss-fg-muted">Querying daemon…</p>
      </section>
    );
  }

  if (!status.installed) {
    return (
      <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
        <h3 className="text-sm font-semibold text-abyss-fg">Tailscale</h3>
        <p className="mt-2 text-xs text-abyss-fg-muted">
          Tailscale CLI not detected on PATH. Install it from{" "}
          <code className="text-abyss-accent">tailscale.com/download</code>; we'll auto-detect
          on the next refresh.
        </p>
        {status.error && (
          <p className="mt-2 font-mono text-[11px] text-abyss-fg-dim">{status.error}</p>
        )}
      </section>
    );
  }

  const stateColor =
    status.backend_state === "Running"
      ? "text-abyss-success border-abyss-success/40 bg-abyss-success/10"
      : "text-abyss-fg-muted border-abyss-border bg-abyss-panel-2";

  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
      <div className="flex items-center gap-2">
        <h3 className="text-sm font-semibold text-abyss-fg">Tailscale</h3>
        <span
          className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest ${stateColor}`}
        >
          {status.backend_state ?? "unknown"}
        </span>
        {status.version && (
          <span className="font-mono text-[10px] text-abyss-fg-dim">v{status.version}</span>
        )}
      </div>

      <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
        <dt className="text-abyss-fg-dim">Self IP</dt>
        <dd className="font-mono text-abyss-fg-muted">{status.self_ip ?? "—"}</dd>
        <dt className="text-abyss-fg-dim">DNS</dt>
        <dd className="truncate font-mono text-abyss-fg-muted">{status.self_dns ?? "—"}</dd>
      </dl>

      <h4 className="mt-4 text-[11px] font-mono uppercase tracking-widest text-abyss-fg-dim">
        Peers ({status.peers.length})
      </h4>
      {status.peers.length === 0 ? (
        <p className="mt-2 text-xs text-abyss-fg-dim">No peers in your mesh.</p>
      ) : (
        <ul className="mt-2 divide-y divide-abyss-border rounded-sm border border-abyss-border">
          {status.peers.map((p) => (
            <li key={p.host_name} className="flex items-center gap-3 px-3 py-1.5">
              <span
                className={`h-1.5 w-1.5 rounded-full ${
                  p.online ? "bg-abyss-success" : "bg-abyss-fg-dim"
                }`}
              />
              <span className="flex-1 truncate text-xs text-abyss-fg-muted">{p.host_name}</span>
              <span className="font-mono text-[10px] text-abyss-fg-dim">
                {p.addrs[0] ?? "—"}
              </span>
              {p.os && (
                <span className="font-mono text-[10px] text-abyss-fg-dim">{p.os}</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function ProbePanel({
  state,
  grouped,
}: {
  state: ProbeState;
  grouped: Map<string, ProbeResult[]>;
}) {
  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-abyss-fg">Region latency</h3>
        {state.kind === "done" && state.report.recommended && (
          <span className="inline-flex items-center gap-1 rounded-full border border-abyss-accent/40 bg-abyss-accent/10 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest text-abyss-accent">
            ◎ best: {state.report.recommended.label} · {state.report.recommended.latency_ms}ms
          </span>
        )}
      </div>

      {state.kind === "idle" && (
        <p className="mt-3 text-xs text-abyss-fg-muted">
          Tap <em>Probe regions</em> to TCP-connect to ~20 global endpoints and rank them by RTT.
          The lowest-latency region is the best candidate for hosting a Sunshine/Moonlight session.
        </p>
      )}
      {state.kind === "running" && (
        <p className="mt-3 text-xs text-abyss-fg-muted">
          <span className="text-abyss-accent">●</span> Probing global endpoints (3s timeout each, concurrent)…
        </p>
      )}
      {state.kind === "error" && (
        <p className="mt-3 rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">
          {state.message}
        </p>
      )}

      {state.kind === "done" && (
        <>
          <p className="mt-2 text-[11px] text-abyss-fg-dim">
            {state.report.results.length} regions tested in {(state.report.elapsed_ms / 1000).toFixed(1)}s
          </p>
          <div className="mt-3 space-y-3">
            {Array.from(grouped.entries()).map(([continent, rows]) => (
              <div key={continent}>
                <h4 className="text-[11px] font-mono uppercase tracking-widest text-abyss-fg-dim">
                  {continent}
                </h4>
                <ul className="mt-1 divide-y divide-abyss-border rounded-sm border border-abyss-border">
                  {rows.map((r) => {
                    const isBest =
                      state.report.recommended?.id === r.id;
                    return (
                      <li
                        key={r.id}
                        className={`flex items-center gap-3 px-3 py-1.5 ${
                          isBest ? "bg-abyss-accent/5" : ""
                        }`}
                      >
                        <span className="flex-1 text-xs text-abyss-fg-muted">{r.label}</span>
                        <code className="font-mono text-[10px] text-abyss-fg-dim">{r.host}</code>
                        {r.latency_ms !== null ? (
                          <span
                            className={`w-14 text-right font-mono text-xs ${
                              r.latency_ms < 50
                                ? "text-abyss-success"
                                : r.latency_ms < 150
                                  ? "text-abyss-accent"
                                  : r.latency_ms < 300
                                    ? "text-abyss-fg-muted"
                                    : "text-abyss-danger"
                            }`}
                          >
                            {r.latency_ms} ms
                          </span>
                        ) : (
                          <span className="w-14 text-right font-mono text-[11px] text-abyss-fg-dim">
                            {r.error?.slice(0, 8) ?? "—"}
                          </span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))}
          </div>
        </>
      )}
    </section>
  );
}
