use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ============================== Tailscale ==================================

/// Snapshot of the embedded Tailscale mesh's status. Sourced from the
/// `abyss-mesh` sidecar via [`crate::mesh::control::status`] (Phase 7).
#[derive(Debug, Clone, Default, Serialize)]
pub struct TailscaleStatus {
    pub installed:   bool,
    pub version:     Option<String>,
    pub backend_state: Option<String>,
    pub self_ip:     Option<String>,
    pub self_dns:    Option<String>,
    pub peers:       Vec<TailscalePeer>,
    /// True when the sidecar is up but the user still has to authenticate
    /// the device to a tailnet. UI surfaces a "Sign in" button using
    /// [`Self::auth_url`].
    pub needs_auth:  bool,
    pub auth_url:    Option<String>,
    /// Whatever raw error the wrapper hit, if any. The UI surfaces this
    /// instead of silently showing "mesh not installed."
    pub error:       Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TailscalePeer {
    pub host_name: String,
    pub dns_name:  Option<String>,
    pub addrs:     Vec<String>,
    pub online:    bool,
    pub os:        Option<String>,
}

// ============================== Latency probe ==============================

/// One target we benchmark — a representative endpoint that's anchored in
/// a specific geographic region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeTarget {
    /// Stable id for the region (e.g. "aws-us-east-1", "cf-fra").
    pub id:       String,
    /// Human-readable label ("US East · Virginia").
    pub label:    String,
    pub continent: String,
    /// Host:port we open a TCP connection to. Connect time is the metric.
    pub host:     String,
    pub port:     u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub id:    String,
    pub label: String,
    pub continent: String,
    pub host:  String,
    pub port:  u16,
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub results: Vec<ProbeResult>,
    pub recommended: Option<RecommendedRegion>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendedRegion {
    pub id:    String,
    pub label: String,
    pub latency_ms: u32,
}

/// Used by the cross-player recommender. Two players each submit a
/// `BTreeMap<region_id, latency_ms>`; the combined picker returns the
/// region that minimises `max(p1, p2)`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LatencyProfile {
    pub measurements: BTreeMap<String, u32>,
}
