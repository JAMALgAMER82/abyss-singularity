/**
 * Thin wrapper around `@tauri-apps/plugin-updater` so the UI talks to
 * one stable surface — and so it's a one-line edit if we ever swap out
 * the underlying plugin.
 *
 * Behaviour:
 *   - check() resolves to `null` if no update OR if the endpoint isn't
 *     configured yet (the plugin throws when pubkey/endpoint are stubs;
 *     we swallow that so the UI stays calm during development).
 *   - downloadAndInstall returns a promise that resolves after the
 *     OS installer has been kicked off; Tauri relaunches the app once
 *     the new binary is in place.
 */

import { check as tauriCheck, type Update } from "@tauri-apps/plugin-updater";

export interface UpdateSummary {
  version:      string;
  body:         string | null;
  current:      string;
  /** Internal handle the UI hands back to [`install`] when the user accepts. */
  _handle:      Update;
}

export async function check(): Promise<UpdateSummary | null> {
  try {
    const update = await tauriCheck();
    if (!update?.available) return null;
    return {
      version: update.version,
      body:    update.body ?? null,
      current: update.currentVersion,
      _handle: update,
    };
  } catch (e) {
    // Endpoint placeholder, network down, signature mismatch — all fine,
    // we just don't surface anything to the user.
    console.warn("update check failed:", e);
    return null;
  }
}

export async function install(summary: UpdateSummary): Promise<void> {
  await summary._handle.downloadAndInstall();
}
