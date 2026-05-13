import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  addPath,
  getConfig,
  setConfig,
  type LibraryConfig,
} from "../lib/library";
import {
  installerInstall,
  installerStatus,
  onInstallProgress,
  type EmulatorInstallState,
  type InstallProgress,
} from "../lib/installer";
import { tailscaleStatus, type TailscaleStatus } from "../lib/network";

interface FirstRunWizardProps {
  onDone: () => void;
}

/**
 * Three-screen onboarding. Shown when `library.config.wizard_completed_at`
 * is null. Each step has a Skip — the wizard never blocks the user from
 * getting into the app.
 */
export function FirstRunWizard({ onDone }: FirstRunWizardProps) {
  const [step, setStep] = useState<0 | 1 | 2>(0);
  const [config, setLocalConfig] = useState<LibraryConfig | null>(null);
  const [ts, setTs]         = useState<TailscaleStatus | null>(null);
  const [installer, setInstaller] = useState<EmulatorInstallState[]>([]);
  const [progress, setProgress]   = useState<InstallProgress | null>(null);

  useEffect(() => {
    getConfig().then(setLocalConfig).catch(() => {});
    tailscaleStatus().then(setTs).catch(() => {});
    installerStatus().then(setInstaller).catch(() => {});
    const t = setInterval(() => {
      tailscaleStatus().then(setTs).catch(() => {});
      installerStatus().then(setInstaller).catch(() => {});
    }, 3000);
    return () => clearInterval(t);
  }, []);

  // Live install progress so the RetroArch button shows %.
  useEffect(() => {
    let unlisten: undefined | (() => void);
    onInstallProgress((p) => setProgress(p)).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, []);

  const finish = useCallback(async () => {
    if (config) {
      const next: LibraryConfig = { ...config, wizard_completed_at: new Date().toISOString() };
      await setConfig(next).catch(() => {});
    }
    onDone();
  }, [config, onDone]);

  const retroarch = useMemo(
    () => installer.find((r) => r.manifest.id === "retroarch"),
    [installer],
  );
  const installRetroarch = useCallback(async () => {
    setProgress({ phase: "start", id: "retroarch" });
    try {
      await installerInstall("retroarch");
      setInstaller(await installerStatus());
    } catch (e) {
      setProgress({ phase: "error", id: "retroarch", message: String(e) });
    }
  }, []);

  const pickFolder = useCallback(async () => {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose a folder Abyss should scan for games",
      });
      if (typeof picked === "string") {
        const updated = await addPath(picked);
        setLocalConfig(updated);
      }
    } catch { /* user cancelled */ }
  }, []);

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-abyss-bg/85 backdrop-blur-md">
      <div className="relative w-full max-w-xl rounded-2xl border border-abyss-accent/30 bg-abyss-panel p-8 shadow-[0_0_60px_-12px_rgba(61,220,255,0.35)]">
        {/* Header */}
        <div className="flex items-center gap-3">
          <div className="relative h-9 w-9">
            <div className="absolute inset-0 rounded-full bg-abyss-accent/20 blur-md" />
            <div className="relative h-full w-full rounded-full border border-abyss-accent/70 abyss-glow" />
          </div>
          <div>
            <h2 className="text-base font-semibold text-abyss-fg abyss-text-glow">
              Welcome to Abyss Singularity
            </h2>
            <p className="text-[11px] text-abyss-fg-muted">
              Step {step + 1} of 3 — quick setup, then you're playing
            </p>
          </div>
          <button
            type="button"
            onClick={finish}
            className="ml-auto text-[11px] text-abyss-fg-dim hover:text-abyss-fg-muted"
          >
            Skip setup
          </button>
        </div>

        {/* Progress dots */}
        <div className="my-5 flex items-center gap-2">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className={`h-1 flex-1 rounded-full transition-colors ${
                i <= step ? "bg-abyss-accent" : "bg-abyss-border"
              }`}
            />
          ))}
        </div>

        {/* Body */}
        <div className="min-h-[160px]">
          {step === 0 && <StepMesh ts={ts} />}
          {step === 1 && (
            <StepEmulator
              installed={retroarch?.installed ?? false}
              progress={progress}
              onInstall={installRetroarch}
            />
          )}
          {step === 2 && (
            <StepLibrary
              paths={config?.scan_paths ?? []}
              onPick={pickFolder}
            />
          )}
        </div>

        {/* Footer */}
        <div className="mt-6 flex items-center justify-between">
          <button
            type="button"
            onClick={() => setStep((s) => (s > 0 ? ((s - 1) as 0 | 1 | 2) : s))}
            disabled={step === 0}
            className="h-9 rounded-md px-3 text-sm text-abyss-fg-muted hover:text-abyss-fg disabled:opacity-30 disabled:cursor-not-allowed"
          >
            ← Back
          </button>
          {step < 2 ? (
            <button
              type="button"
              onClick={() => setStep((s) => ((s + 1) as 0 | 1 | 2))}
              className="h-9 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-4 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20"
            >
              Next →
            </button>
          ) : (
            <button
              type="button"
              onClick={finish}
              className="h-9 rounded-md border border-abyss-accent/60 bg-abyss-accent/15 px-4 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/25"
            >
              Finish
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function StepMesh({ ts }: { ts: TailscaleStatus | null }) {
  const onMesh   = ts?.backend_state === "Running" && !ts.needs_auth;
  const needLogin = ts?.needs_auth && ts.auth_url;

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-semibold text-abyss-fg">Connect to the mesh</h3>
      <p className="text-xs leading-relaxed text-abyss-fg-muted">
        Abyss ships with an <span className="text-abyss-accent">embedded Tailscale node</span> so
        your devices can find each other peer-to-peer. No router setup, no third-party game-server.
        First time only: sign in with any Tailscale account (free tier covers up to 100 devices).
      </p>
      <div className="rounded-md border border-abyss-border bg-abyss-panel-2/60 p-3 text-xs">
        {ts === null ? (
          <span className="text-abyss-fg-dim">Waiting for mesh sidecar…</span>
        ) : !ts.installed ? (
          <span className="text-abyss-danger">
            Sidecar not running. Check Settings → Diagnostics after this wizard.
          </span>
        ) : onMesh ? (
          <span className="text-abyss-success">
            ✓ Signed in as <code className="font-mono">{ts.self_dns ?? ts.self_ip ?? "—"}</code>
          </span>
        ) : needLogin ? (
          <div className="flex items-center justify-between gap-3">
            <span className="text-abyss-accent">● Sign in to a tailnet to join the mesh</span>
            <button
              type="button"
              onClick={() => openUrl(ts.auth_url!).catch(() => {})}
              className="h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
            >
              Open sign-in ↗
            </button>
          </div>
        ) : (
          <span className="text-abyss-fg-muted">
            State: <code className="font-mono">{ts.backend_state ?? "starting"}</code>
          </span>
        )}
      </div>
    </div>
  );
}

function StepEmulator({
  installed,
  progress,
  onInstall,
}: {
  installed: boolean;
  progress: InstallProgress | null;
  onInstall: () => void;
}) {
  const pct =
    progress?.phase === "download" && progress.bytes_total
      ? Math.round((progress.bytes_done / progress.bytes_total) * 100)
      : null;
  const inProgress = progress && progress.phase !== "finalize" && progress.phase !== "error" && !installed;

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-semibold text-abyss-fg">Install RetroArch</h3>
      <p className="text-xs leading-relaxed text-abyss-fg-muted">
        RetroArch covers <span className="text-abyss-accent">NES, SNES, N64, GBA, DS, Genesis,
        PS1, PSP, Atari, NeoGeo, Arcade</span>, and many more via swappable cores. One install
        gets you most retro consoles. You can add system-specific emulators later under Settings.
      </p>
      <div className="rounded-md border border-abyss-border bg-abyss-panel-2/60 p-3">
        <div className="flex items-center justify-between gap-3 text-xs">
          <div>
            <p className="font-medium text-abyss-fg">RetroArch (multi-system)</p>
            <p className="text-[11px] text-abyss-fg-dim">GPLv3 · ~90 MB · embeds inside Abyss</p>
          </div>
          {installed ? (
            <span className="rounded-full border border-abyss-success/40 bg-abyss-success/10 px-3 py-1 text-[11px] font-mono uppercase tracking-widest text-abyss-success">
              ✓ installed
            </span>
          ) : inProgress ? (
            <span className="font-mono text-[11px] text-abyss-accent">
              {progress!.phase === "download"
                ? `${pct ?? 0}%`
                : progress!.phase === "extract"
                  ? "extracting…"
                  : "starting…"}
            </span>
          ) : (
            <button
              type="button"
              onClick={onInstall}
              className="h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20"
            >
              Install
            </button>
          )}
        </div>
        {progress?.phase === "error" && (
          <p className="mt-2 text-[11px] text-abyss-danger">{progress.message}</p>
        )}
      </div>
    </div>
  );
}

function StepLibrary({
  paths,
  onPick,
}: {
  paths: string[];
  onPick: () => void;
}) {
  return (
    <div className="space-y-3">
      <h3 className="text-sm font-semibold text-abyss-fg">Add a games folder</h3>
      <p className="text-xs leading-relaxed text-abyss-fg-muted">
        Point Abyss at a folder containing your ROMs / game files. The scanner detects 50+ file
        types and figures out which emulator each game needs. You can add more folders any time
        from Settings.
      </p>
      <div className="rounded-md border border-abyss-border bg-abyss-panel-2/60 p-3">
        {paths.length === 0 ? (
          <div className="flex items-center justify-between gap-3">
            <span className="text-xs text-abyss-fg-dim">No folders added yet.</span>
            <button
              type="button"
              onClick={onPick}
              className="h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20"
            >
              + Add folder
            </button>
          </div>
        ) : (
          <>
            <ul className="space-y-1">
              {paths.map((p) => (
                <li key={p} className="truncate font-mono text-[11px] text-abyss-fg-muted">
                  ✓ {p}
                </li>
              ))}
            </ul>
            <button
              type="button"
              onClick={onPick}
              className="mt-3 h-8 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-sm text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent"
            >
              + Add another folder
            </button>
          </>
        )}
      </div>
    </div>
  );
}
