import { useCallback, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  diagnosticsRunAll,
  type CheckResult,
  type CheckStatus,
  type DiagnosticsReport,
} from "../lib/diagnostics";

/**
 * One-button "Diagnose & Repair" panel. Walks every self-heal Abyss
 * knows about, reports the result per subsystem, surfaces actionable
 * paths/URLs for things the user has to do themselves (AV exception,
 * BIOS dump).
 */
export function DiagnosticsSection() {
  const [running, setRunning] = useState(false);
  const [report, setReport]   = useState<DiagnosticsReport | null>(null);
  const [error, setError]     = useState<string | null>(null);

  const runDiagnostics = useCallback(async () => {
    setRunning(true);
    setError(null);
    try {
      const r = await diagnosticsRunAll();
      setReport(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }, []);

  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-5">
      <header className="mb-4 flex items-start justify-between gap-3">
        <div>
          <h3 className="text-base font-semibold text-abyss-fg">Diagnose & Repair</h3>
          <p className="mt-1 text-xs leading-relaxed text-abyss-fg-muted">
            One click runs every self-heal Abyss knows about: mesh sidecar, emulator binaries,
            controller defaults, BIOS auto-find, RetroArch cores, Sunshine streaming host.
            Things Abyss can't legally fix (Sony BIOS dumps, antivirus exceptions) get
            surfaced as actionable next steps.
          </p>
        </div>
        <button
          type="button"
          onClick={runDiagnostics}
          disabled={running}
          className="h-9 shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-4 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {running ? "Running…" : report ? "Run again" : "Run Diagnose & Repair"}
        </button>
      </header>

      {error && (
        <div className="mb-3 rounded-md border border-abyss-danger/40 bg-abyss-danger/10 p-3 text-[11px] text-abyss-danger">
          {error}
        </div>
      )}

      {report && (
        <>
          <ResultsSummary report={report} />
          <ul className="mt-3 divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel-2/40">
            {report.checks.map((c) => (
              <CheckRow key={c.id} check={c} />
            ))}
          </ul>
          <ReportClipboard report={report} />
        </>
      )}
    </section>
  );
}

/**
 * "Copy report" — flattens the diagnostics result into a plain-text block
 * the user can paste into a chat / DM / email when asking the Abyss host
 * for help. Includes per-check status + the actionable hint each surfaces,
 * plus a header with timestamp and build version. No personal data — just
 * what's already shown in the panel.
 */
function ReportClipboard({ report }: { report: DiagnosticsReport }) {
  const [copied, setCopied] = useState(false);

  const text = useMemo(() => {
    const lines: string[] = [];
    lines.push("Abyss Singularity — Diagnostic report");
    lines.push(`Generated: ${new Date().toISOString()}`);
    lines.push(`Elapsed:   ${(report.elapsedMs / 1000).toFixed(1)}s`);
    lines.push(`Summary:   ${report.repairedCount} repaired · ${report.needsUserCount} needs-user · ${report.failedCount} failed (of ${report.checks.length} total)`);
    lines.push("");
    lines.push("───── checks ─────");
    for (const c of report.checks) {
      const tag = ({
        ok: "✓ OK",
        repaired: "↻ FIXED",
        needs_user: "! NEEDS USER",
        failed: "✗ FAILED",
        skipped: "– SKIPPED",
      } as Record<string, string>)[c.status] ?? c.status;
      lines.push(`[${tag}] ${c.title}`);
      lines.push(`         ${c.message}`);
      if (c.actionPath) lines.push(`         path: ${c.actionPath}`);
      if (c.actionUrl)  lines.push(`         url:  ${c.actionUrl}`);
    }
    return lines.join("\n");
  }, [report]);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* clipboard denied; user can manually copy from the textarea */ }
  }, [text]);

  return (
    <div className="mt-3 rounded-md border border-abyss-border bg-abyss-panel-2/40 p-3">
      <div className="flex items-center gap-2">
        <p className="text-xs font-medium text-abyss-fg">Share this report</p>
        <span className="text-[11px] text-abyss-fg-muted">
          Paste it to your Abyss host so they can see what's broken on your end.
        </span>
        <button
          type="button"
          onClick={copy}
          className="ml-auto h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
        >
          {copied ? "✓ Copied" : "Copy to clipboard"}
        </button>
      </div>
      <textarea
        readOnly
        value={text}
        onClick={(e) => (e.target as HTMLTextAreaElement).select()}
        className="mt-2 h-32 w-full resize-y rounded-sm border border-abyss-border bg-abyss-panel-2 px-2 py-1.5 font-mono text-[11px] text-abyss-fg-muted focus:border-abyss-accent/60 focus:outline-none"
      />
    </div>
  );
}

function ResultsSummary({ report }: { report: DiagnosticsReport }) {
  const okCount = report.checks.filter((c) => c.status === "ok").length;
  const total   = report.checks.length;
  return (
    <p className="text-[11px] text-abyss-fg-muted">
      <span className="text-abyss-success">{okCount}</span> healthy ·{" "}
      <span className="text-abyss-accent">{report.repairedCount}</span> repaired ·{" "}
      <span className="text-abyss-warning">{report.needsUserCount}</span> needs you ·{" "}
      <span className="text-abyss-danger">{report.failedCount}</span> failed
      <span className="text-abyss-fg-dim"> · {total} checks in {(report.elapsedMs / 1000).toFixed(1)}s</span>
    </p>
  );
}

function CheckRow({ check }: { check: CheckResult }) {
  return (
    <li className="px-4 py-3">
      <div className="flex items-start gap-3">
        <StatusIcon status={check.status} />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium text-abyss-fg">{check.title}</p>
          <p className="mt-0.5 text-[11px] leading-relaxed text-abyss-fg-muted">{check.message}</p>
          {check.actionPath && (
            <p className="mt-1.5 truncate font-mono text-[10px] text-abyss-fg-dim">
              {check.actionPath}
            </p>
          )}
          {check.actionUrl && (
            <button
              type="button"
              onClick={() => openUrl(check.actionUrl!).catch(() => {})}
              className="mt-1.5 text-[11px] text-abyss-accent hover:underline"
            >
              Learn more ↗
            </button>
          )}
        </div>
      </div>
    </li>
  );
}

function StatusIcon({ status }: { status: CheckStatus }) {
  const map: Record<CheckStatus, { c: string; bg: string; sym: string; label: string }> = {
    ok:         { c: "text-abyss-success", bg: "bg-abyss-success/15 border-abyss-success/40", sym: "✓", label: "OK" },
    repaired:   { c: "text-abyss-accent",  bg: "bg-abyss-accent/15  border-abyss-accent/40",  sym: "↻", label: "fixed" },
    needs_user: { c: "text-abyss-warning", bg: "bg-abyss-warning/15 border-abyss-warning/40", sym: "!", label: "you" },
    failed:     { c: "text-abyss-danger",  bg: "bg-abyss-danger/15  border-abyss-danger/40",  sym: "✗", label: "fail" },
    skipped:    { c: "text-abyss-fg-dim",  bg: "bg-abyss-border/40  border-abyss-border",     sym: "–", label: "skip" },
  };
  const cfg = map[status];
  return (
    <span
      className={`mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-[11px] font-bold ${cfg.bg} ${cfg.c}`}
      title={cfg.label}
    >
      {cfg.sym}
    </span>
  );
}
