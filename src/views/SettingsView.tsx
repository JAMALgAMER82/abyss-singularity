import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addPath,
  getConfig,
  removePath,
  setIgdbCredentials,
  type LibraryConfig,
  type Platform,
  PLATFORM_DISPLAY,
} from "../lib/library";
import {
  orchBuiltinRecipes,
  orchGetConfig,
  orchSetConfig,
  type EmulatorEntry,
  type OrchestrationConfig,
} from "../lib/orchestration";
import {
  streamGetConfig,
  streamSetConfig,
  type StreamingConfig,
} from "../lib/streaming";
import {
  installerAutoAssign,
  installerInstall,
  installerStatus,
  installerUninstall,
  onInstallProgress,
  type EmulatorInstallState,
  type InstallProgress,
} from "../lib/installer";
import { ControllerSection } from "../components/ControllerSection";
import { AboutSection } from "../components/AboutSection";
import { DiagnosticsSection } from "../components/DiagnosticsSection";
import { DirectorySection } from "../components/DirectorySection";

export function SettingsView() {
  const [config, setConfig]   = useState<LibraryConfig | null>(null);
  const [orch, setOrch]       = useState<OrchestrationConfig | null>(null);
  const [stream, setStream]   = useState<StreamingConfig | null>(null);
  const [installer, setInstaller]     = useState<EmulatorInstallState[]>([]);
  const [installProgress, setInstallProgress] = useState<Record<string, InstallProgress>>({});
  const [error, setError]     = useState<string | null>(null);

  const [igdbId, setIgdbId]         = useState("");
  const [igdbSecret, setIgdbSecret] = useState("");
  const [savedFlash, setSavedFlash] = useState(false);

  useEffect(() => {
    Promise.all([getConfig(), orchGetConfig(), streamGetConfig(), installerStatus()])
      .then(([cfg, oc, sc, inst]) => {
        setConfig(cfg);
        setOrch(oc);
        setStream(sc);
        setInstaller(inst);
        setIgdbId(cfg.igdb_client_id ?? "");
        setIgdbSecret(cfg.igdb_client_secret ?? "");
      })
      .catch((e) => setError(String(e)));
  }, []);

  // Live install progress
  useEffect(() => {
    let unlisten: undefined | (() => void);
    onInstallProgress((p) => {
      setInstallProgress((prev) => ({ ...prev, [p.id]: p }));
      if (p.phase === "finalize") {
        // refresh the install state so the badge flips to "installed"
        installerStatus().then(setInstaller).catch(() => {});
        orchGetConfig().then(setOrch).catch(() => {});
      }
    }).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, []);

  const installOne = useCallback(async (id: string) => {
    setError(null);
    setInstallProgress((prev) => ({ ...prev, [id]: { phase: "start", id } }));
    try {
      await installerInstall(id);
      setInstaller(await installerStatus());
      setOrch(await orchGetConfig());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const uninstallOne = useCallback(async (id: string) => {
    setError(null);
    try {
      await installerUninstall(id);
      setInstaller(await installerStatus());
      setOrch(await orchGetConfig());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const autoAssign = useCallback(async () => {
    setError(null);
    try {
      await installerAutoAssign();
      setOrch(await orchGetConfig());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const pickFolder = useCallback(async () => {
    setError(null);
    try {
      const picked = await open({ directory: true, multiple: false, title: "Choose a library folder" });
      if (typeof picked === "string") {
        const updated = await addPath(picked);
        setConfig(updated);
      }
    } catch (e) { setError(String(e)); }
  }, []);

  const drop = useCallback(async (path: string) => {
    try { setConfig(await removePath(path)); } catch (e) { setError(String(e)); }
  }, []);

  const saveIgdb = useCallback(async () => {
    setError(null);
    try {
      await setIgdbCredentials(igdbId, igdbSecret);
      setSavedFlash(true);
      setTimeout(() => setSavedFlash(false), 1800);
      setConfig(await getConfig());
    } catch (e) { setError(String(e)); }
  }, [igdbId, igdbSecret]);

  // ----- emulator handlers ------------------------------------------------
  const saveOrch = useCallback(async (next: OrchestrationConfig) => {
    setOrch(next);
    try { await orchSetConfig(next); } catch (e) { setError(String(e)); }
  }, []);

  const seedFromRecipes = useCallback(async () => {
    setError(null);
    try {
      const recipes = await orchBuiltinRecipes();
      const existing = new Set((orch?.emulators ?? []).map((e) => e.id));
      const added    = recipes.filter((r) => !existing.has(r.id));
      const next: OrchestrationConfig = {
        emulators:   [...(orch?.emulators ?? []), ...added],
        assignments: { ...(orch?.assignments ?? {}) },
      };
      await saveOrch(next);
    } catch (e) { setError(String(e)); }
  }, [orch, saveOrch]);

  const pickEmulatorExe = useCallback(async (emuId: string) => {
    if (!orch) return;
    const picked = await open({
      multiple: false,
      title: "Choose emulator executable",
      filters: [{ name: "Executable", extensions: ["exe", "app", "AppImage"] }],
    });
    if (typeof picked !== "string") return;
    const next: OrchestrationConfig = {
      ...orch,
      emulators: orch.emulators.map((e) => (e.id === emuId ? { ...e, exe: picked } : e)),
    };
    await saveOrch(next);
  }, [orch, saveOrch]);

  const deleteEmulator = useCallback(async (emuId: string) => {
    if (!orch) return;
    const next: OrchestrationConfig = {
      emulators:   orch.emulators.filter((e) => e.id !== emuId),
      assignments: Object.fromEntries(
        Object.entries(orch.assignments).filter(([, v]) => v !== emuId),
      ),
    };
    await saveOrch(next);
  }, [orch, saveOrch]);

  const assignPlatform = useCallback(async (platform: Platform, emuId: string) => {
    if (!orch) return;
    const next: OrchestrationConfig = {
      ...orch,
      assignments: emuId
        ? { ...orch.assignments, [platform]: emuId }
        : Object.fromEntries(Object.entries(orch.assignments).filter(([p]) => p !== platform)),
    };
    await saveOrch(next);
  }, [orch, saveOrch]);

  // Platforms that at least one configured emulator can launch.
  const assignablePlatforms = useMemo(() => {
    const set = new Set<Platform>();
    for (const e of orch?.emulators ?? []) for (const p of e.platforms) set.add(p);
    return Array.from(set).sort();
  }, [orch]);

  const candidatesFor = useCallback(
    (p: Platform): EmulatorEntry[] =>
      (orch?.emulators ?? []).filter((e) => e.platforms.includes(p) && e.exe),
    [orch],
  );

  const hasIgdbCreds = Boolean(config?.igdb_client_id) && Boolean(config?.igdb_client_secret);

  // ----- streaming binary pickers ---------------------------------------
  const pickStreamingExe = useCallback(async (kind: "sunshine" | "moonlight") => {
    if (!stream) return;
    setError(null);
    try {
      const picked = await open({
        multiple: false,
        title: `Choose ${kind === "sunshine" ? "Sunshine" : "Moonlight"} executable`,
        filters: [{ name: "Executable", extensions: ["exe", "app", "AppImage"] }],
      });
      if (typeof picked !== "string") return;
      const next: StreamingConfig =
        kind === "sunshine"
          ? { ...stream, sunshine_exe: picked }
          : { ...stream, moonlight_exe: picked };
      setStream(next);
      await streamSetConfig(next);
    } catch (e) { setError(String(e)); }
  }, [stream]);

  const clearStreamingExe = useCallback(async (kind: "sunshine" | "moonlight") => {
    if (!stream) return;
    const next: StreamingConfig =
      kind === "sunshine"
        ? { ...stream, sunshine_exe: null }
        : { ...stream, moonlight_exe: null };
    setStream(next);
    try { await streamSetConfig(next); } catch (e) { setError(String(e)); }
  }, [stream]);

  return (
    <div className="flex h-full flex-col overflow-auto">
      <header className="shrink-0 border-b border-abyss-border px-6 py-4">
        <h2 className="text-lg font-semibold text-abyss-fg abyss-text-glow">Settings</h2>
        <p className="mt-1 text-xs text-abyss-fg-muted">
          Library paths · IGDB credentials · emulator config. Stored locally only.
        </p>
      </header>

      <div className="space-y-8 p-6">
        {/* ============================== LIBRARY PATHS =================================== */}
        <section>
          <div className="mb-2 flex items-center justify-between">
            <div>
              <h3 className="text-sm font-semibold text-abyss-fg">Library scan paths</h3>
              <p className="mt-0.5 text-xs text-abyss-fg-muted">
                The scanner walks each of these directories (up to 6 levels deep).
              </p>
            </div>
            <button type="button" onClick={pickFolder} className={cardBtn}>
              + Add folder
            </button>
          </div>

          <ul className="divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel/40">
            {!config || config.scan_paths.length === 0 ? (
              <li className="px-4 py-3 text-xs text-abyss-fg-dim">No folders configured yet.</li>
            ) : (
              config.scan_paths.map((p) => (
                <li key={p} className="flex items-center gap-3 px-4 py-2">
                  <code className="flex-1 truncate font-mono text-xs text-abyss-fg-muted">{p}</code>
                  <button type="button" onClick={() => drop(p)} className={destructiveSmallBtn}>
                    Remove
                  </button>
                </li>
              ))
            )}
          </ul>
        </section>

        {/* ============================== CONTROLLERS ===================================== */}
        <ControllerSection />

        {/* ============================== EMULATOR MANAGER ================================ */}
        <section>
          <div className="mb-2 flex items-center justify-between">
            <div>
              <h3 className="text-sm font-semibold text-abyss-fg">Emulator manager</h3>
              <p className="mt-0.5 text-xs text-abyss-fg-muted">
                One-click installs from official sources. RetroArch covers most retro consoles
                (NES, SNES, N64, GBA, DS, Genesis, PS1, PSP, etc.) via libretro cores — install
                it first, the rest are for systems that need their own emulator.
              </p>
            </div>
            <button type="button" onClick={autoAssign} className={cardBtn}>
              Auto-assign by platform
            </button>
          </div>

          <ul className="divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel/40">
            {installer.map((row) => {
              const prog = installProgress[row.manifest.id];
              const isRunning = prog && prog.phase !== "finalize" && prog.phase !== "error";
              const pct =
                prog?.phase === "download" && prog.bytes_total
                  ? Math.round((prog.bytes_done / prog.bytes_total) * 100)
                  : null;
              return (
                <li key={row.manifest.id} className="px-4 py-3">
                  <div className="flex items-center gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <p className="text-sm font-medium text-abyss-fg">{row.manifest.name}</p>
                        {row.installed && (
                          <span className="inline-flex items-center gap-1 rounded-full border border-abyss-success/40 bg-abyss-success/10 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest text-abyss-success">
                            ● installed
                          </span>
                        )}
                        {row.manifest.embeddable && (
                          <span className="inline-flex items-center gap-1 rounded-full border border-abyss-accent/30 bg-abyss-accent/5 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest text-abyss-accent">
                            ◎ embeds in Abyss
                          </span>
                        )}
                      </div>
                      <p className="mt-0.5 text-[11px] text-abyss-fg-dim">
                        {row.manifest.license} · ~{row.manifest.approx_size_mb} MB · {row.manifest.platforms.length} platform{row.manifest.platforms.length === 1 ? "" : "s"}
                      </p>
                      {isRunning && (
                        <p className="mt-1 text-[11px] text-abyss-accent">
                          {prog!.phase === "download"
                            ? `Downloading ${formatBytes(prog!.bytes_done)}${prog!.bytes_total ? " / " + formatBytes(prog!.bytes_total) : ""}${pct !== null ? ` (${pct}%)` : ""}`
                            : prog!.phase === "extract"
                              ? "Extracting…"
                              : "Starting…"}
                        </p>
                      )}
                      {prog?.phase === "error" && (
                        <p className="mt-1 text-[11px] text-abyss-danger">{prog.message}</p>
                      )}
                    </div>
                    {row.installed ? (
                      <button type="button" onClick={() => uninstallOne(row.manifest.id)} className={destructiveSmallBtn}>
                        Uninstall
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => installOne(row.manifest.id)}
                        disabled={isRunning}
                        className={cardBtn}
                      >
                        {isRunning ? "Installing…" : "Install"}
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        </section>

        {/* ============================== EMULATORS (configured) ========================== */}
        <section>
          <div className="mb-2 flex items-center justify-between">
            <div>
              <h3 className="text-sm font-semibold text-abyss-fg">Emulators</h3>
              <p className="mt-0.5 text-xs text-abyss-fg-muted">
                Configure the binary on disk for each emulator, then assign it to a platform below.
              </p>
            </div>
            <button type="button" onClick={seedFromRecipes} className={cardBtn}>
              + Seed from recipes
            </button>
          </div>

          <ul className="divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel/40">
            {!orch || orch.emulators.length === 0 ? (
              <li className="px-4 py-3 text-xs text-abyss-fg-dim">
                No emulators configured. Click <em>Seed from recipes</em> to bootstrap the standard
                set (RetroArch, Dolphin, PCSX2, RPCS3, PPSSPP, Cemu, Simple64, DuckStation,
                mGBA, DeSmuME, Snes9x, Stella, Flycast).
              </li>
            ) : (
              orch.emulators.map((e) => {
                // `pc-direct` is a synthetic launcher: PC games' own
                // .exe / .lnk / .bat files ARE the program, so the
                // emulator entry's `exe` field is intentionally empty
                // and ignored by the launcher (see commands.rs:79).
                // Don't pester the user to set one.
                const isDirectLauncher = e.id === "pc-direct";
                return (
                  <li key={e.id} className="px-4 py-3">
                    <div className="flex items-center gap-3">
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium text-abyss-fg">{e.name}</p>
                        <p className="mt-0.5 truncate font-mono text-[11px] text-abyss-fg-dim">
                          {e.exe
                            ? e.exe
                            : isDirectLauncher
                              ? <span className="text-abyss-fg-muted/80">launches each game by its own path · no emulator needed</span>
                              : <span className="text-abyss-danger/80">no exe configured</span>}
                        </p>
                        <p className="mt-1 flex flex-wrap gap-1">
                          {e.platforms.map((p) => (
                            <span key={p} className={platformBadge}>{PLATFORM_DISPLAY[p]}</span>
                          ))}
                        </p>
                      </div>
                      {!isDirectLauncher && (
                        <button type="button" onClick={() => pickEmulatorExe(e.id)} className={cardBtn}>
                          {e.exe ? "Change exe" : "Choose exe"}
                        </button>
                      )}
                      {!isDirectLauncher && (
                        <button type="button" onClick={() => deleteEmulator(e.id)} className={destructiveSmallBtn}>
                          Remove
                        </button>
                      )}
                    </div>
                  </li>
                );
              })
            )}
          </ul>
        </section>

        {/* ============================== PLATFORM ASSIGNMENTS ============================ */}
        {assignablePlatforms.length > 0 && (
          <section>
            <h3 className="text-sm font-semibold text-abyss-fg">Default emulator per platform</h3>
            <p className="mt-0.5 mb-2 text-xs text-abyss-fg-muted">
              Only emulators that have an exe configured can be assigned.
            </p>
            <ul className="divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel/40">
              {assignablePlatforms.map((p) => {
                const candidates = candidatesFor(p);
                const current = orch?.assignments?.[p] ?? "";
                return (
                  <li key={p} className="flex items-center gap-3 px-4 py-2">
                    <span className="w-44 text-sm text-abyss-fg">{PLATFORM_DISPLAY[p]}</span>
                    <select
                      value={current}
                      onChange={(e) => assignPlatform(p, e.target.value)}
                      className="
                        h-8 flex-1 rounded-md border border-abyss-border bg-abyss-panel-2 px-2
                        text-sm text-abyss-fg focus:border-abyss-accent/60 focus:outline-none
                      "
                    >
                      <option value="">— unassigned —</option>
                      {candidates.map((c) => (
                        <option key={c.id} value={c.id}>{c.name}</option>
                      ))}
                    </select>
                  </li>
                );
              })}
            </ul>
          </section>
        )}

        {/* ============================== STREAMING BINARIES ============================== */}
        <section>
          <div className="flex items-start justify-between gap-3">
            <div>
              <h3 className="text-sm font-semibold text-abyss-fg">Streaming binaries</h3>
              <p className="mt-0.5 mb-2 text-xs text-abyss-fg-muted">
                Sunshine (game-streaming host), Moonlight (client), and the standalone Tailscale
                Windows client — Abyss downloads each directly from its official source and runs
                the installer. Sunshine + Tailscale each trigger a one-time UAC prompt for their
                system services; Moonlight is silent. Already-installed copies are detected and
                skipped. Tailscale standalone is optional — Abyss already has its own embedded
                tailnet stack in <code className="font-mono text-abyss-fg-muted">abyss-mesh.exe</code>,
                but the standalone gives you the system-tray UI for managing your tailnet.
              </p>
            </div>
            <StreamingAutoInstallButton onDone={async () => setStream(await streamGetConfig())} />
          </div>
          <ul className="divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel/40">
            {[
              { kind: "sunshine" as const,  label: "Sunshine host",      exe: stream?.sunshine_exe ?? null },
              { kind: "moonlight" as const, label: "Moonlight client",   exe: stream?.moonlight_exe ?? null },
            ].map((row) => (
              <li key={row.kind} className="flex items-center gap-3 px-4 py-2">
                <span className="w-44 text-sm text-abyss-fg">{row.label}</span>
                <code className="flex-1 truncate font-mono text-[11px] text-abyss-fg-muted">
                  {row.exe ?? <span className="text-abyss-fg-dim">not configured</span>}
                </code>
                <button type="button" onClick={() => pickStreamingExe(row.kind)} className={cardBtn}>
                  {row.exe ? "Change exe" : "Choose exe"}
                </button>
                {row.exe && (
                  <button type="button" onClick={() => clearStreamingExe(row.kind)} className={destructiveSmallBtn}>
                    Clear
                  </button>
                )}
              </li>
            ))}
          </ul>
        </section>

        {/* ============================== IGDB ============================================ */}
        <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-abyss-fg">IGDB credentials</h3>
            {hasIgdbCreds && (
              <span className="inline-flex items-center gap-1 rounded-full border border-abyss-success/40 bg-abyss-success/10 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest text-abyss-success">
                <span className="h-1 w-1 rounded-full bg-abyss-success" />
                configured
              </span>
            )}
          </div>
          <p className="mt-1 text-xs text-abyss-fg-muted">
            Free Twitch developer credentials power Library &gt; <em>Enrich metadata</em>.
            Generate a Client&nbsp;ID / Secret at{" "}
            <code className="text-abyss-accent">dev.twitch.tv/console/apps</code> and paste them here.
          </p>

          <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
            <label className="block">
              <span className="text-xs text-abyss-fg-muted">Client ID</span>
              <input
                type="text"
                spellCheck={false}
                value={igdbId}
                onChange={(e) => setIgdbId(e.target.value)}
                placeholder="abcdef0123…"
                className={inputClass}
              />
            </label>
            <label className="block">
              <span className="text-xs text-abyss-fg-muted">Client Secret</span>
              <input
                type="password"
                spellCheck={false}
                value={igdbSecret}
                onChange={(e) => setIgdbSecret(e.target.value)}
                placeholder="••••••••"
                className={inputClass}
              />
            </label>
          </div>

          <div className="mt-3 flex items-center gap-3">
            <button type="button" onClick={saveIgdb} className={cardBtn}>Save</button>
            {savedFlash && <span className="text-xs text-abyss-success">Saved.</span>}
          </div>
        </section>

        {error && (
          <p className="rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">
            {error}
          </p>
        )}

        {/* ============================== DIRECTORY (Discover) ============================ */}
        <DirectorySection />

        {/* ============================== DIAGNOSTICS ============================ */}
        <DiagnosticsSection />

        {/* ============================== ABOUT ============================ */}
        <AboutSection />
      </div>
    </div>
  );
}

const cardBtn = `
  h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10
  px-3 text-sm font-medium text-abyss-accent transition-colors
  hover:bg-abyss-accent/20
`;
const destructiveSmallBtn = `
  h-7 rounded-sm border border-abyss-border bg-transparent px-2 text-[11px]
  text-abyss-fg-muted transition-colors
  hover:border-abyss-danger/40 hover:text-abyss-danger
`;
const platformBadge = `
  inline-block rounded-sm border border-abyss-border bg-abyss-panel-2 px-1.5
  py-0.5 font-mono text-[9px] uppercase tracking-wider text-abyss-fg-muted
`;
const inputClass = `
  mt-1 h-9 w-full rounded-md border border-abyss-border bg-abyss-panel-2 px-3
  font-mono text-xs text-abyss-fg placeholder:text-abyss-fg-dim
  focus:border-abyss-accent/60 focus:outline-none
`;

function StreamingAutoInstallButton({ onDone }: { onDone: () => Promise<void> }) {
  const [running, setRunning] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const click = useCallback(async () => {
    setRunning(true);
    setMsg(null);
    try {
      const { installerInstallStreamingApps } = await import("../lib/installer");
      const r = await installerInstallStreamingApps();
      await onDone();
      setMsg(r.messages.join(" "));
    } catch (e) {
      setMsg(String(e));
    } finally {
      setRunning(false);
    }
  }, [onDone]);
  return (
    <div className="flex flex-col items-end gap-1">
      <button
        type="button"
        onClick={click}
        disabled={running}
        className="h-8 shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {running ? "Installing…" : "Install Sunshine + Moonlight + Tailscale"}
      </button>
      {msg && <p className="max-w-xs text-right text-[10px] text-abyss-fg-dim">{msg}</p>}
    </div>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
