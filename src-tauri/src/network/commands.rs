//! Tauri commands for the networking subsystem (Phase 4 + Phase 12 lobby).

use tauri::{AppHandle, Runtime};

use super::config::{self, NetworkConfig};
use super::invite;
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

// ---------------------- Phase 12 — invite codes ----------------------

#[tauri::command]
pub fn net_get_config<R: Runtime>(app: AppHandle<R>) -> Result<NetworkConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

/// Persist the host-side Tailscale pre-auth key the user has generated in
/// their tailnet admin console + the display name shown on invite codes.
/// Passing an empty string for either clears it.
#[tauri::command]
pub fn net_set_invite_config<R: Runtime>(
    app:          AppHandle<R>,
    authkey:      String,
    display_name: String,
) -> Result<(), String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let ak = authkey.trim();
    let dn = display_name.trim();
    if !ak.is_empty() && !invite::looks_like_tailscale_key(ak) {
        return Err(
            "That doesn't look like a Tailscale pre-auth key. Get one at \
             admin.tailscale.com → Settings → Keys → Generate auth key (reusable). \
             Keys start with 'tskey-auth-'.".to_string()
        );
    }
    cfg.host_invite_authkey = (!ak.is_empty()).then(|| ak.to_string());
    cfg.host_display_name   = (!dn.is_empty()).then(|| dn.to_string());
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))
}

/// Generate an invite code from the persisted host invite config. Errors
/// if the user hasn't set their auth key yet.
#[tauri::command]
pub fn net_create_invite<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let authkey = cfg.host_invite_authkey.ok_or_else(|| {
        "No Tailscale auth key set yet. Go to Settings → Network → 'Invite a friend' \
         and paste an auth key first.".to_string()
    })?;
    let display = cfg.host_display_name
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "Abyss host".to_string());
    invite::encode(&authkey, &display).map_err(|e| format!("{e:#}"))
}

/// Decode an invite code, persist its auth key under
/// [`NetworkConfig::redeemed_authkey`], then respawn the mesh sidecar
/// so tsnet picks up the new identity. Reports who the invite was from.
#[tauri::command]
pub async fn net_redeem_invite<R: Runtime>(
    app:  AppHandle<R>,
    code: String,
) -> Result<String, String> {
    let info = invite::decode(&code).map_err(|e| format!("{e:#}"))?;
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    cfg.redeemed_authkey = Some(info.authkey.clone());
    cfg.redeemed_from    = Some(info.host_name.clone());
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    // Respawn the sidecar so tsnet picks up the new identity now —
    // otherwise the user has to restart the whole app for the join to
    // take effect, which would be a terrible UX.
    crate::mesh::sidecar::respawn_with_authkey(&app, &info.authkey)
        .map_err(|e| format!("respawning mesh sidecar with new auth key: {e:#}"))?;
    Ok(info.host_name)
}

/// Forget the redeemed auth key — useful if the user wants to leave the
/// host's tailnet and sign back into their own. Respawns the sidecar
/// without an auth key so it falls back to interactive sign-in.
#[tauri::command]
pub async fn net_clear_redeemed_invite<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    cfg.redeemed_authkey = None;
    cfg.redeemed_from    = None;
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    // Pass an empty key so the helper wipes state + respawns without --authkey.
    crate::mesh::sidecar::respawn_with_authkey(&app, "")
        .map_err(|e| format!("respawning mesh sidecar without auth key: {e:#}"))
}
