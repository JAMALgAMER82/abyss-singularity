//! Region latency probing.
//!
//! TCP-connect timing is what we use as the "ping" — it works without
//! admin privileges (unlike ICMP), is cross-platform, and on modern
//! networks tracks ICMP RTT closely (one syn → syn-ack round trip).

use std::collections::BTreeMap;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::timeout;

use super::types::{LatencyProfile, ProbeResult, ProbeTarget, RecommendedRegion};

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Measure connect-time for a single target.
async fn probe_one(target: &ProbeTarget) -> ProbeResult {
    let host = target.host.clone();
    let port = target.port;
    // DNS resolution can itself block, so do it on the blocking pool. We
    // only need *some* resolved address — IPv4 or IPv6, whichever the OS
    // returns first.
    let resolved = tokio::task::spawn_blocking(move || {
        format!("{host}:{port}")
            .to_socket_addrs()
            .ok()
            .and_then(|mut it| it.next())
    })
    .await
    .ok()
    .flatten();

    let Some(addr) = resolved else {
        return ProbeResult {
            id:         target.id.clone(),
            label:      target.label.clone(),
            continent:  target.continent.clone(),
            host:       target.host.clone(),
            port:       target.port,
            latency_ms: None,
            error:      Some("DNS resolution failed".into()),
        };
    };

    let started = Instant::now();
    match timeout(PROBE_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => ProbeResult {
            id:         target.id.clone(),
            label:      target.label.clone(),
            continent:  target.continent.clone(),
            host:       target.host.clone(),
            port:       target.port,
            latency_ms: Some(started.elapsed().as_millis() as u32),
            error:      None,
        },
        Ok(Err(e)) => ProbeResult {
            id:         target.id.clone(),
            label:      target.label.clone(),
            continent:  target.continent.clone(),
            host:       target.host.clone(),
            port:       target.port,
            latency_ms: None,
            error:      Some(e.to_string()),
        },
        Err(_) => ProbeResult {
            id:         target.id.clone(),
            label:      target.label.clone(),
            continent:  target.continent.clone(),
            host:       target.host.clone(),
            port:       target.port,
            latency_ms: None,
            error:      Some("timeout".into()),
        },
    }
}

/// Probe every target concurrently (with a cap so we don't open a flood
/// of sockets at once). Returns results sorted by latency, fastest first.
pub async fn probe_all(targets: &[ProbeTarget]) -> Vec<ProbeResult> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let mut in_flight: FuturesUnordered<_> = targets.iter().map(probe_one).collect();
    let mut results = Vec::with_capacity(targets.len());
    while let Some(r) = in_flight.next().await {
        results.push(r);
    }
    results.sort_by_key(|r| (r.latency_ms.is_none(), r.latency_ms.unwrap_or(u32::MAX)));
    results
}

/// Pick the lowest-latency reachable region.
pub fn recommend(results: &[ProbeResult]) -> Option<RecommendedRegion> {
    results
        .iter()
        .filter_map(|r| r.latency_ms.map(|ms| (r, ms)))
        .min_by_key(|(_, ms)| *ms)
        .map(|(r, ms)| RecommendedRegion {
            id:         r.id.clone(),
            label:      r.label.clone(),
            latency_ms: ms,
        })
}

/// Cross-player recommender. Given two latency profiles (one per player),
/// pick the region that minimises `max(p1, p2)` — best worst-case round
/// trip for the pair.
pub fn recommend_pair(
    p1: &LatencyProfile,
    p2: &LatencyProfile,
    targets: &[ProbeTarget],
) -> Option<RecommendedRegion> {
    let mut best: Option<(String, String, u32)> = None;
    for t in targets {
        let m1 = p1.measurements.get(&t.id).copied();
        let m2 = p2.measurements.get(&t.id).copied();
        if let (Some(a), Some(b)) = (m1, m2) {
            let worst = a.max(b);
            if best.as_ref().is_none_or(|(_, _, current)| worst < *current) {
                best = Some((t.id.clone(), t.label.clone(), worst));
            }
        }
    }
    best.map(|(id, label, latency_ms)| RecommendedRegion { id, label, latency_ms })
}

/// Convert a `ProbeResult[]` into a serialisable `LatencyProfile`, dropping
/// failed probes. Useful when Phase 6 lets players exchange profiles for
/// pair-wise recommendations.
pub fn profile_from_results(results: &[ProbeResult]) -> LatencyProfile {
    let mut m = BTreeMap::new();
    for r in results {
        if let Some(ms) = r.latency_ms { m.insert(r.id.clone(), ms); }
    }
    LatencyProfile { measurements: m }
}
