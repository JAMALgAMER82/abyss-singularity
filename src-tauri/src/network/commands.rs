//! Tauri commands for the networking subsystem (Phase 4).

use super::latency::{probe_all, profile_from_results, recommend, recommend_pair};
use super::regions::default_targets;
use super::tailscale;
use super::types::{
    LatencyProfile, ProbeReport, RecommendedRegion, TailscaleStatus,
};

#[tauri::command]
pub async fn net_tailscale_status() -> Result<TailscaleStatus, String> {
    Ok(tailscale::status().await)
}

#[tauri::command]
pub async fn net_probe_regions() -> Result<ProbeReport, String> {
    let started = std::time::Instant::now();
    let targets = default_targets();
    let results = probe_all(&targets).await;
    let recommended = recommend(&results);
    Ok(ProbeReport {
        results,
        recommended,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

#[tauri::command]
pub async fn net_my_profile() -> Result<LatencyProfile, String> {
    let targets = default_targets();
    let results = probe_all(&targets).await;
    Ok(profile_from_results(&results))
}

#[tauri::command]
pub fn net_recommend_pair(
    p1: LatencyProfile,
    p2: LatencyProfile,
) -> Result<Option<RecommendedRegion>, String> {
    Ok(recommend_pair(&p1, &p2, &default_targets()))
}
