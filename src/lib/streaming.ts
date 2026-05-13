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
