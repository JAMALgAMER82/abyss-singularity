/**
 * Typed bridge to the Rust `orchestration` module.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Platform } from "./library";

export interface EmulatorEntry {
  id: string;
  name: string;
  exe: string;                 // PathBuf as string from Rust
  args: string[];
  working_dir?: string | null;
  env: Record<string, string>;
  platforms: Platform[];
}

export interface OrchestrationConfig {
  emulators: EmulatorEntry[];
  /** Platform → emulator-id mapping. */
  assignments: Partial<Record<Platform, string>>;
}

export interface LaunchHandle {
  run_id: string;
  pid: number;
  started_at: string;
  emulator_id: string;
  entry_id: string;
  command_line: string;
}

export interface RunningProcess {
  run_id: string;
  pid: number;
  started_at: string;
  emulator_id: string;
  entry_id: string;
}

export type LaunchEvent =
  | { kind: "stdout"; run_id: string; line: string }
  | { kind: "stderr"; run_id: string; line: string }
  | { kind: "exited"; run_id: string; code: number | null };

export const LAUNCH_EVENT = "abyss://orchestration/event";

// ---- command wrappers ----------------------------------------------------

export const orchGetConfig       = () => invoke<OrchestrationConfig>("orch_get_config");
export const orchSetConfig       = (config: OrchestrationConfig) =>
  invoke<void>("orch_set_config", { config });
export const orchBuiltinRecipes  = () => invoke<EmulatorEntry[]>("orch_builtin_recipes");
export const orchLaunch          = (entryId: string) =>
  invoke<LaunchHandle>("orch_launch", { entryId });
export const orchTerminate       = (runId: string) =>
  invoke<boolean>("orch_terminate", { runId });
export const orchListRunning     = () =>
  invoke<RunningProcess[]>("orch_list_running");

export function onLaunchEvent(handler: (e: LaunchEvent) => void): Promise<UnlistenFn> {
  return listen<LaunchEvent>(LAUNCH_EVENT, (event) => handler(event.payload));
}
