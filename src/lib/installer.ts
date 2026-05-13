/**
 * Typed bridge to the Rust `installer` module (Phase 8).
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Platform } from "./library";

export interface EmulatorManifest {
  id: string;
  name: string;
  homepage: string;
  license: string;
  url: string;
  archive_format: "zip" | "seven_z";
  exe_relpath: string;
  platforms: Platform[];
  approx_size_mb: number;
  embeddable: boolean;
}

export interface EmulatorInstallState {
  manifest: EmulatorManifest;
  installed: boolean;
  exe: string | null;
}

export interface InstallReport {
  id: string;
  exe: string;
  elapsed_ms: number;
}

export type InstallProgress =
  | { phase: "start";    id: string }
  | { phase: "download"; id: string; bytes_done: number; bytes_total: number | null }
  | { phase: "extract";  id: string }
  | { phase: "finalize"; id: string; exe: string }
  | { phase: "error";    id: string; message: string };

export const INSTALL_PROGRESS_EVENT = "abyss://installer/progress";

export const installerAvailable    = () => invoke<EmulatorManifest[]>("installer_available");
export const installerStatus       = () => invoke<EmulatorInstallState[]>("installer_status");
export const installerInstall      = (id: string) => invoke<InstallReport>("installer_install", { id });
export const installerUninstall    = (id: string) => invoke<void>("installer_uninstall", { id });
export const installerAutoAssign   = () => invoke<[Platform, string][]>("installer_auto_assign");

export function onInstallProgress(handler: (p: InstallProgress) => void): Promise<UnlistenFn> {
  return listen<InstallProgress>(INSTALL_PROGRESS_EVENT, (event) => handler(event.payload));
}
