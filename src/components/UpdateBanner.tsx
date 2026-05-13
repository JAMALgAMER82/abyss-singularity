import { useCallback, useEffect, useState } from "react";
import { check, type UpdateSummary } from "../lib/updater";

const SNOOZE_KEY = "abyss.update.snoozedVersion";

/**
 * Polls the configured updater endpoint once on launch; if an update is
 * available and the user hasn't snoozed that exact version, drops a
 * compact banner at the top of the window.
 *
 *   [ • Update available: v0.2.0  →  Install  |  Later ]
 *
 * Install kicks off Tauri's `downloadAndInstall` which fetches the
 * signed `.nsis.zip`, verifies it against the public key in
 * tauri.conf.json, runs the installer in passive mode, and relaunches
 * the app on completion. Later snoozes *this* version forever (the
 * banner reappears for the next release).
 */
export function UpdateBanner() {
  const [update, setUpdate]     = useState<UpdateSummary | null>(null);
  const [installing, setI]      = useState(false);
  const [error, setError]       = useState<string | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number | null } | null>(null);

  useEffect(() => {
    check().then((u) => {
      if (!u) return;
      const snoozed = localStorage.getItem(SNOOZE_KEY);
      if (snoozed === u.version) return;
      setUpdate(u);
    }).catch(() => { /* swallowed in lib/updater */ });
  }, []);

  const accept = useCallback(async () => {
    if (!update) return;
    setI(true); setError(null);
    try {
      // The plugin's API gives us per-chunk download progress through
      // an event listener — we attach via the handle's internal API.
      await update._handle.downloadAndInstall((event) => {
        if (event.event === "Started") {
          setProgress({ done: 0, total: event.data.contentLength ?? null });
        } else if (event.event === "Progress") {
          setProgress((p) => p ? { ...p, done: p.done + event.data.chunkLength } : null);
        }
      });
      // If downloadAndInstall returns without throwing, Tauri is about
      // to relaunch us. Leave the banner up so the user sees "Done".
    } catch (e) {
      setError(String(e));
      setI(false);
    }
  }, [update]);

  const later = useCallback(() => {
    if (update) localStorage.setItem(SNOOZE_KEY, update.version);
    setUpdate(null);
  }, [update]);

  if (!update) return null;

  const pct =
    progress && progress.total
      ? Math.round((progress.done / progress.total) * 100)
      : null;

  return (
    <div className="flex shrink-0 items-center gap-3 border-b border-abyss-accent/30 bg-abyss-accent/5 px-4 py-1.5 text-[11px]">
      <span className="text-abyss-accent">●</span>
      <span className="text-abyss-fg">
        Update available: <span className="font-mono text-abyss-accent">v{update.version}</span>
        <span className="ml-2 text-abyss-fg-dim">(you're on v{update.current})</span>
      </span>
      {installing && pct !== null && (
        <span className="font-mono text-abyss-accent">{pct}%</span>
      )}
      {installing && pct === null && (
        <span className="text-abyss-accent">downloading…</span>
      )}
      {error && (
        <span className="text-abyss-danger">{error}</span>
      )}
      <div className="ml-auto flex items-center gap-1">
        <button
          type="button"
          onClick={accept}
          disabled={installing}
          className="h-6 rounded-sm border border-abyss-accent/60 bg-abyss-accent/10 px-2 font-medium text-abyss-accent transition-colors hover:bg-abyss-accent/20 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {installing ? "Installing…" : "Install"}
        </button>
        <button
          type="button"
          onClick={later}
          disabled={installing}
          className="h-6 rounded-sm border border-abyss-border bg-transparent px-2 text-abyss-fg-muted hover:text-abyss-fg disabled:cursor-not-allowed disabled:opacity-50"
        >
          Later
        </button>
      </div>
    </div>
  );
}
