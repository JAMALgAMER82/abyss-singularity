import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import {
  dirGetConfig,
  dirSetConfig,
  type DirectoryConfig,
} from "../lib/directory";

/**
 * Settings panel for the global directory (Discover tab).
 *
 * The directory is opt-in — Abyss works fully P2P without it; the user
 * only needs this if they want GameRanger-style discovery of strangers.
 * This panel collects:
 *   - Worker URL (pasted from `wrangler deploy` output)
 *   - Display name (default = hostname; mutable)
 *   - Country (2-letter ISO, optional, for region hints)
 *   - "Appear offline" toggle
 *
 * The user_id is auto-minted on first save and never shown — it's the
 * bearer of identity to the Worker. Their `handle` is what others see.
 */
export function DirectorySection() {
  const [cfg, setCfg]         = useState<DirectoryConfig | null>(null);
  const [workerUrl, setUrl]   = useState("");
  const [handle, setHandle]   = useState("");
  const [country, setCountry] = useState("");
  const [busy, setBusy]       = useState(false);
  const [error, setError]     = useState<string | null>(null);
  const [ok, setOk]           = useState<string | null>(null);

  useEffect(() => {
    dirGetConfig().then((c) => {
      setCfg(c);
      setUrl(c.worker_url ?? "");
      setHandle(c.handle ?? "");
      setCountry(c.country ?? "");
    }).catch((e) => setError(String(e)));
  }, []);

  const save = useCallback(async () => {
    setBusy(true); setError(null); setOk(null);
    try {
      const next = await dirSetConfig({
        workerUrl: workerUrl.trim(),
        handle:    handle.trim(),
        country:   country.trim(),
      });
      setCfg(next);
      setOk("Saved. Heartbeat will fire within 30 s — open Discover to see who's online.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }, [workerUrl, handle, country]);

  const toggleHidden = useCallback(async () => {
    if (!cfg) return;
    setError(null); setOk(null);
    try {
      const next = await dirSetConfig({ hidden: !cfg.hidden });
      setCfg(next);
    } catch (e) { setError(String(e)); }
  }, [cfg]);

  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-5">
      <header className="mb-4">
        <h3 className="text-base font-semibold text-abyss-fg">Directory (Discover tab)</h3>
        <p className="mt-1 text-xs leading-relaxed text-abyss-fg-muted">
          Connect to your Cloudflare Worker to enable global presence: see everyone using Abyss
          right now, add friends, chat with them. Optional — Abyss works fully peer-to-peer
          without this.
        </p>
        {cfg?.user_id && (
          <p className="mt-1 font-mono text-[10px] text-abyss-fg-dim">
            your id (never share): {cfg.user_id.slice(0, 12)}…
          </p>
        )}
      </header>

      <div className="space-y-3">
        <label className="block">
          <span className="block text-xs font-medium text-abyss-fg">Worker URL</span>
          <input
            type="text"
            value={workerUrl}
            onChange={(e) => setUrl(e.target.value)}
            spellCheck={false}
            placeholder="https://abyss-directory.you.workers.dev"
            className={inputCls}
          />
          <span className="mt-1 inline-flex items-center gap-2 text-[10px] text-abyss-fg-dim">
            Need one?
            <button
              type="button"
              onClick={() => openUrl("https://developers.cloudflare.com/workers/get-started/guide/").catch(() => {})}
              className="text-abyss-accent underline-offset-2 hover:underline"
            >
              Cloudflare Workers quickstart ↗
            </button>
            — the repo's <code className="text-abyss-fg-muted">abyss-directory/</code> folder has a one-step deploy.
          </span>
        </label>

        <label className="block">
          <span className="block text-xs font-medium text-abyss-fg">Display name</span>
          <input
            type="text"
            value={handle}
            onChange={(e) => setHandle(e.target.value)}
            placeholder="Bob"
            maxLength={24}
            className={inputCls}
          />
        </label>

        <label className="block">
          <span className="block text-xs font-medium text-abyss-fg">Country (optional, 2 letters)</span>
          <input
            type="text"
            value={country}
            onChange={(e) => setCountry(e.target.value.toUpperCase().slice(0, 2))}
            placeholder="US"
            maxLength={2}
            className={inputCls + " w-20"}
          />
        </label>

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={save}
            disabled={busy || !workerUrl.trim() || !handle.trim()}
            className="h-9 rounded-md border-2 border-abyss-accent/60 bg-abyss-accent/15 px-4 text-sm font-bold text-abyss-accent hover:bg-abyss-accent/25 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {busy ? "Saving…" : "Save"}
          </button>
          {cfg?.user_id && (
            <button
              type="button"
              onClick={toggleHidden}
              className={`h-9 rounded-md border px-3 text-xs font-medium transition-colors ${
                cfg?.hidden
                  ? "border-abyss-warning/40 bg-abyss-warning/10 text-abyss-warning hover:bg-abyss-warning/20"
                  : "border-abyss-border bg-abyss-panel-2 text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent"
              }`}
            >
              {cfg?.hidden ? "Appearing offline" : "Visible online"}
            </button>
          )}
        </div>

        {error && <p className="rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">{error}</p>}
        {ok    && !error && <p className="rounded-sm border border-abyss-success/30 bg-abyss-success/10 px-3 py-2 text-xs text-abyss-success">{ok}</p>}
      </div>
    </section>
  );
}

const inputCls = `
  mt-1 h-9 w-full rounded-md border border-abyss-border bg-abyss-panel-2 px-2
  font-mono text-xs text-abyss-fg placeholder:text-abyss-fg-dim
  focus:border-abyss-accent/60 focus:outline-none
`;
