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

  // Install every emulator that isn't already on disk. Sequential, not
  // parallel — we want predictable progress and to avoid hammering GitHub
  // releases concurrently. Each finished install is reflected immediately
  // via `installerStatus()` so the UI tile flips to ✓ installed.
  const [installing, setInstalling] = useState(false);
  const allInstalled = useMemo(
    () => installer.length > 0 && installer.every((r) => r.installed),
    [installer],
  );
  const installAll = useCallback(async () => {
    if (installing) return;
    setInstalling(true);
    const fresh = await installerStatus().catch(() => installer);
    setInstaller(fresh);
    for (const r of fresh) {
      if (r.installed) continue;
      setProgress({ phase: "start", id: r.manifest.id });
      try {
        await installerInstall(r.manifest.id);
      } catch (e) {
        setProgress({ phase: "error", id: r.manifest.id, message: String(e) });
        // keep going — one broken upstream shouldn't block the rest
      }
      setInstaller(await installerStatus().catch(() => fresh));
    }
    setInstalling(false);
    setProgress(null);
  }, [installer, installing]);

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
              installer={installer}
              installing={installing}
              allInstalled={allInstalled}
              progress={progress}
              onInstallAll={installAll}
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

        <p className="mt-5 border-t border-abyss-border pt-3 text-center text-[10px] font-mono tracking-widest text-abyss-fg-dim">
          ABYSS&nbsp;SINGULARITY · by <span className="text-abyss-fg-muted">MasterMind&nbsp;George</span>
        </p>
      </div>
    </div>
  );
}

function StepMesh({ ts }: { ts: TailscaleStatus | null }) {
  const onMesh   = ts?.backend_state === "Running" && !ts.needs_auth;
  const needLogin = ts?.needs_auth && ts.auth_url;
  const [repairing, setRepairing] = useState(false);
  const [repairMsg, setRepairMsg] = useState<string | null>(null);

  const runRepair = useCallback(async () => {
    setRepairing(true);
    setRepairMsg(null);
    try {
      const { diagnosticsRunAll } = await import("../lib/diagnostics");
      const r = await diagnosticsRunAll();
      const mesh = r.checks.find((c) => c.id === "mesh");
      if (mesh) {
        setRepairMsg(`${mesh.title}: ${mesh.message}${mesh.actionPath ? `\nPath to whitelist: ${mesh.actionPath}` : ""}`);
      } else {
        setRepairMsg(`Diagnose finished: ${r.repairedCount} repaired, ${r.needsUserCount} need attention.`);
      }
    } catch (e) {
      setRepairMsg(String(e));
    } finally {
      setRepairing(false);
    }
  }, []);

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
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <span className="text-abyss-danger">
                Sidecar not running — likely your antivirus quarantined abyss-mesh.exe.
              </span>
              <button
                type="button"
                onClick={runRepair}
                disabled={repairing}
                className="h-7 shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20 disabled:opacity-50"
              >
                {repairing ? "Repairing…" : "Repair"}
              </button>
            </div>
            {repairMsg && (
              <pre className="whitespace-pre-wrap rounded border border-abyss-border bg-abyss-bg/40 p-2 font-mono text-[10px] text-abyss-fg-muted">
                {repairMsg}
              </pre>
            )}
          </div>
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
  installer,
  installing,
  allInstalled,
  progress,
  onInstallAll,
}: {
  installer: EmulatorInstallState[];
  installing: boolean;
  allInstalled: boolean;
  progress: InstallProgress | null;
  onInstallAll: () => void;
}) {
  const installedCount = installer.filter((r) => r.installed).length;
  const total = installer.length;
  const currentId = progress?.id;
  const currentManifest = installer.find((r) => r.manifest.id === currentId)?.manifest;
  const pct =
    progress?.phase === "download" && progress.bytes_total
      ? Math.round((progress.bytes_done / progress.bytes_total) * 100)
      : null;
  const totalSizeMb = installer.reduce((s, r) => s + (r.installed ? 0 : r.manifest.approx_size_mb), 0);

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-semibold text-abyss-fg">Install emulators</h3>
      <p className="text-xs leading-relaxed text-abyss-fg-muted">
        One click pulls every supported emulator from its official upstream release
        (RetroArch + 11 system-specific apps — Dolphin, PCSX2, RPCS3, PPSSPP, Cemu,
        DuckStation, Simple64, mGBA, DeSmuME, Snes9x, Stella).
        {!allInstalled && totalSizeMb > 0 && (
          <> Total download size: ~{Math.round(totalSizeMb)} MB.</>
        )}
      </p>
      <div className="rounded-md border border-abyss-border bg-abyss-panel-2/60 p-3">
        <div className="flex items-center justify-between gap-3 text-xs">
          <div className="min-w-0 flex-1">
            <p className="font-medium text-abyss-fg">
              {installedCount} / {total || "—"} emulators installed
            </p>
            <p className="truncate text-[11px] text-abyss-fg-dim">
              {installing && currentManifest
                ? `${currentManifest.name} · ${progress?.phase === "download" ? `${pct ?? 0}%` : progress?.phase === "extract" ? "extracting…" : "starting…"}`
                : allInstalled
                  ? "Everything's local — ready to play offline."
                  : "Sources: official GitHub releases · GPL / MPL only · no commercial ROMs"}
            </p>
          </div>
          {allInstalled ? (
            <span className="shrink-0 rounded-full border border-abyss-success/40 bg-abyss-success/10 px-3 py-1 text-[11px] font-mono uppercase tracking-widest text-abyss-success">
              ✓ all installed
            </span>
          ) : (
            <button
              type="button"
              onClick={onInstallAll}
              disabled={installing}
              className="h-8 shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {installing ? "Installing…" : "Install all"}
            </button>
          )}
        </div>
        {/* Progress bar: filled by total installed-count, not just current */}
        {total > 0 && (
          <div className="mt-3 h-1 w-full overflow-hidden rounded-full bg-abyss-border">
            <div
              className="h-full bg-abyss-accent transition-all"
              style={{ width: `${Math.round((installedCount / total) * 100)}%` }}
            />
          </div>
        )}
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
