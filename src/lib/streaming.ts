/**
 * Typed bridge to the Rust `streaming` module (Phase 5).
 */

import { invoke } from "@tauri-apps/api/core";

export interface KnownHost {
  id: string;
  name: string;
  host: string;
}

export interface StreamingConfig {
  sunshine_exe: string | null;
  sunshine_admin_url: string | null;
  sunshine_admin_user: string | null;
  sunshine_admin_pass: string | null;
  moonlight_exe: string | null;
  known_hosts: KnownHost[];
}

export interface HostStatus {
  configured: boolean;
  running: boolean;
  pid: number | null;
  admin_url: string | null;
  run_id: string | null;
}

export interface ClientLaunchResult {
  run_id: string;
  pid: number;
  command_line: string;
}

export const streamGetConfig    = () => invoke<StreamingConfig>("stream_get_config");
export const streamSetConfig    = (config: StreamingConfig) =>
  invoke<void>("stream_set_config", { config });
export const streamAddHost      = (host: KnownHost) =>
  invoke<StreamingConfig>("stream_add_host", { host });
export const streamRemoveHost   = (hostId: string) =>
  invoke<StreamingConfig>("stream_remove_host", { hostId });
export const streamHostStatus   = () => invoke<HostStatus>("stream_host_status");
export const streamStartHost    = () => invoke<HostStatus>("stream_start_host");
export const streamStopHost     = () => invoke<boolean>("stream_stop_host");
export const streamLaunchClient = (host?: string) =>
  invoke<ClientLaunchResult>("stream_launch_client", { host: host ?? null });
export const streamPairClient   = (
  pin: string,
  name?: string,
  adminUser?: string,
  adminPass?: string,
) =>
  invoke<void>("stream_pair_client", {
    pin,
    name:       name ?? null,
    adminUser:  adminUser ?? null,
    adminPass:  adminPass ?? null,
  });

// ---------------------- Phase 13 — auto-pair-and-launch ----------------------

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Fire-and-forget: ask the host to accept a freshly-generated PIN, spawn
 * Moonlight's pairing handshake, then auto-launch the stream when the
 * host confirms. The friend's UI listens for `onStreamPairProgress` to
 * know when it's done (or why it failed).
 */
export const streamRequestPairAndLaunch = (hostAddr: string) =>
  invoke<void>("stream_request_pair_and_launch", { hostAddr });

export interface ResetCredsReport { user: string; pass: string; }

/**
 * Force-reset Sunshine's admin credentials via `sunshine.exe --creds`
 * and persist the new pair into StreamingConfig. Use this when Sunshine
 * was installed manually (so the install-time auto-setup never fired)
 * to enable the auto-pair flow without ever opening Sunshine's web UI.
 * Triggers a single UAC prompt because the Sunshine config dir is under
 * Program Files.
 */
export const streamResetCredentials = () =>
  invoke<ResetCredsReport>("stream_reset_credentials");

export type StreamPairProgress =
  | { phase: "accepted"; host_addr: string }
  | { phase: "rejected"; host_addr: string; error: string }
  | { phase: "timeout";  host_addr: string };

export async function onStreamPairProgress(
  cb: (p: StreamPairProgress) => void,
): Promise<UnlistenFn> {
  return listen<StreamPairProgress>("abyss://stream/pair-progress", (e) => cb(e.payload));
}
