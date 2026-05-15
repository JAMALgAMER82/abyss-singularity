/**
 * Typed bridge to the Rust `network` module (Phase 4).
 */

import { invoke } from "@tauri-apps/api/core";

export interface TailscalePeer {
  host_name: string;
  dns_name: string | null;
  addrs: string[];
  online: boolean;
  os: string | null;
}

export interface TailscaleStatus {
  installed: boolean;
  version: string | null;
  backend_state: string | null;
  self_ip: string | null;
  self_dns: string | null;
  peers: TailscalePeer[];
  needs_auth: boolean;
  auth_url: string | null;
  error: string | null;
}

export interface ProbeResult {
  id: string;
  label: string;
  continent: string;
  host: string;
  port: number;
  latency_ms: number | null;
  error: string | null;
}

export interface RecommendedRegion {
  id: string;
  label: string;
  latency_ms: number;
}

export interface ProbeReport {
  results: ProbeResult[];
  recommended: RecommendedRegion | null;
  elapsed_ms: number;
}

export interface LatencyProfile {
  measurements: Record<string, number>;
}

export const tailscaleStatus = () => invoke<TailscaleStatus>("net_tailscale_status");
export const probeRegions    = () => invoke<ProbeReport>("net_probe_regions");
export const myProfile       = () => invoke<LatencyProfile>("net_my_profile");
export const recommendPair   = (p1: LatencyProfile, p2: LatencyProfile) =>
  invoke<RecommendedRegion | null>("net_recommend_pair", { p1, p2 });

// ---------------------- Phase 12 — invite codes ----------------------

export interface NetworkConfig {
  host_invite_authkey: string | null;
  host_display_name:   string | null;
  redeemed_authkey:    string | null;
  redeemed_from:       string | null;
}

export const netGetConfig = () => invoke<NetworkConfig>("net_get_config");

/** Persist the host-side Tailscale auth key + display name. Empty strings clear them. */
export const netSetInviteConfig = (authkey: string, displayName: string) =>
  invoke<void>("net_set_invite_config", { authkey, displayName });

/** Returns a paste-able invite code wrapping the persisted auth key. */
export const netCreateInvite = () => invoke<string>("net_create_invite");

/** Returns the host's display name if successful. Respawns the mesh sidecar. */
export const netRedeemInvite = (code: string) =>
  invoke<string>("net_redeem_invite", { code });

export const netClearRedeemedInvite = () =>
  invoke<void>("net_clear_redeemed_invite");
