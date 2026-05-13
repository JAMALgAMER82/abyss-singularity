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
