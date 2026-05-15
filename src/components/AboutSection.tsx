import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getVersion, getTauriVersion } from "@tauri-apps/api/app";

const REPO_URL = "https://github.com/JAMALgAMER82/abyss-singularity";

/**
 * About panel — credits, version info, repo link. Rendered at the
 * bottom of SettingsView so users always know who built this and what
 * version they're running.
 */
export function AboutSection() {
  const [appVersion,   setAppVersion]   = useState<string>("");
  const [tauriVersion, setTauriVersion] = useState<string>("");

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion("?"));
    getTauriVersion().then(setTauriVersion).catch(() => setTauriVersion("?"));
  }, []);

  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-5">
      <div className="flex items-start gap-4">
        <div className="relative h-12 w-12 shrink-0">
          {/* Visual echo of the icon — the cosmic singularity */}
          <div className="absolute inset-0 rounded-full bg-abyss-accent/20 blur-md" />
          <div className="relative h-full w-full rounded-full border-2 border-abyss-accent/70 abyss-glow" />
          <div className="absolute left-1/2 top-1/2 h-1.5 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-abyss-accent abyss-glow" />
        </div>

        <div className="flex-1 min-w-0">
          <h3 className="text-base font-semibold text-abyss-fg abyss-text-glow">
            Abyss&nbsp;Singularity
          </h3>
          <p className="mt-0.5 text-xs text-abyss-fg-muted">
            By <span className="text-abyss-accent font-medium">MasterMind&nbsp;George</span> · cross-continental co-op gaming hub
          </p>

          <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[11px]">
            <dt className="text-abyss-fg-dim">Version</dt>
            <dd className="font-mono text-abyss-fg-muted">{appVersion || "—"}</dd>
            <dt className="text-abyss-fg-dim">Tauri</dt>
            <dd className="font-mono text-abyss-fg-muted">{tauriVersion || "—"}</dd>
            <dt className="text-abyss-fg-dim">Repo</dt>
            <dd>
              <button
                type="button"
                onClick={() => openUrl(REPO_URL).catch(() => {})}
                className="font-mono text-[11px] text-abyss-accent hover:underline"
              >
                {REPO_URL.replace(/^https?:\/\//, "")} ↗
              </button>
            </dd>
          </dl>

          <p className="mt-4 text-[10px] leading-relaxed text-abyss-fg-dim">
            100% free + open-source. Orchestrates Sunshine, Moonlight, Tailscale, and a
            curated set of emulators — never bundles or distributes commercial ROMs. All
            mesh traffic is peer-to-peer over an embedded Tailscale node; no third-party
            game servers, no telemetry, no accounts.
          </p>
        </div>
      </div>
    </section>
  );
}
