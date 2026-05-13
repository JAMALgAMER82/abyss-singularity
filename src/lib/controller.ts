/**
 * Typed bridge to the Rust `controller` module (Phase 11).
 *
 * Live detection of plugged-in gamepads is done in the browser via the
 * Web Gamepad API — fast (60 Hz poll) and zero-friction. The backend's
 * job is persistence: writing emulator-specific config files so a
 * freshly-plugged controller "just works" without the user touching any
 * emulator's settings menu.
 */

import { invoke } from "@tauri-apps/api/core";

export type ControllerKind = "xbox" | "play_station" | "switch_pro" | "generic";

export interface AutoConfigReport {
  written_to: string;
  bytes: number;
}

export const controllerDetectKind = (id: string) =>
  invoke<ControllerKind>("controller_detect_kind", { id });

export const controllerApplyToRetroarch = (
  kind: ControllerKind,
  controllerName: string,
  force = false,
) =>
  invoke<AutoConfigReport>("controller_apply_to_retroarch", {
    kind, controllerName, force,
  });

// ---- Web Gamepad API helpers ------------------------------------------

export interface DetectedController {
  index:    number;
  id:       string;
  kind:     ControllerKind;
  mapping:  string;
  buttons:  number;
  axes:     number;
  connected: boolean;
}

/** Heuristic mirror of the Rust-side `ControllerKind::detect_from_id`. */
export function detectKind(id: string): ControllerKind {
  const lower = id.toLowerCase();
  if (lower.includes("xbox") || lower.includes("xinput") || lower.includes("microsoft")) return "xbox";
  if (lower.includes("dualshock") || lower.includes("dualsense")
   || lower.includes("playstation") || lower.includes("sony")
   || lower.includes("ds4") || lower.includes("ds5")
   || lower.includes("wireless controller")) return "play_station";
  if (lower.includes("nintendo") || lower.includes("pro controller")
   || lower.includes("switch") || lower.includes("joy-con")) return "switch_pro";
  return "generic";
}

export function snapshotGamepads(): DetectedController[] {
  const pads = navigator.getGamepads?.() ?? [];
  return Array.from(pads)
    .filter((p): p is Gamepad => p !== null)
    .map((p) => ({
      index:    p.index,
      id:       p.id,
      kind:     detectKind(p.id),
      mapping:  p.mapping || "unknown",
      buttons:  p.buttons.length,
      axes:     p.axes.length,
      connected: p.connected,
    }));
}

/** Live button + axis state for visualisation. */
export function readGamepadState(index: number): {
  buttons: number[];   // 0..1 each
  axes:    number[];   // -1..+1 each
} | null {
  const p = navigator.getGamepads?.()[index];
  if (!p) return null;
  return {
    buttons: p.buttons.map((b) => b.value),
    axes:    Array.from(p.axes),
  };
}
