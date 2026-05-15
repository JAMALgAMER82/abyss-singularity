//! Download + silently install the Sunshine streaming host and the
//! Moonlight client. Unlike emulator installs (zip / 7z extracts into
//! Abyss's data dir), these are NSIS installers that put themselves
//! under `C:\Program Files\` and — for Sunshine — register a Local
//! System service. Sunshine therefore requires an admin elevation
//! (one UAC prompt); Moonlight is per-user and silent.
//!
//! After each install we probe the standard install path so the
//! caller can write it into the streaming config.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;

use super::download::fetch_to_file;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingInstallReport {
    pub sunshine_installed:  bool,
    pub sunshine_path:       Option<PathBuf>,
    pub moonlight_installed: bool,
    pub moonlight_path:      Option<PathBuf>,
    /// Standalone Tailscale Windows client (system-tray app + service).
    /// Abyss has its own embedded Tailscale stack inside abyss-mesh.exe,
    /// so the standalone is optional — but a friend whose abyss-mesh.exe
    /// got quarantined by AV can use the standalone as a fallback that
    /// also gives them tailnet status / device list in the system tray.
    pub tailscale_installed: bool,
    pub tailscale_path:      Option<PathBuf>,
    /// Auto-generated credentials we set on Sunshine via `sunshine.exe --creds`
    /// after a fresh install, so the user never has to visit the localhost:47990
    /// admin page. `None` when we left Sunshine's existing creds alone (because
    /// it was already installed) or the auto-setup step failed.
    pub auto_creds_user: Option<String>,
    pub auto_creds_pass: Option<String>,
    pub messages:            Vec<String>,
}

/// Look in the standard install locations for the three external apps,
/// skipping any download for the ones already present.
pub fn detect_existing() -> StreamingInstallReport {
    let sun_paths = [
        r"C:\Program Files\Sunshine\sunshine.exe",
        r"C:\Program Files (x86)\Sunshine\sunshine.exe",
    ];
    let moon_paths = [
        r"C:\Program Files\Moonlight Game Streaming\Moonlight.exe",
        r"C:\Program Files (x86)\Moonlight Game Streaming\Moonlight.exe",
    ];
    let ts_paths = [
        r"C:\Program Files\Tailscale\tailscale.exe",
        r"C:\Program Files (x86)\Tailscale\tailscale.exe",
        r"C:\Program Files\Tailscale IPN\tailscale-ipn.exe",
    ];
    let sunshine_path  = sun_paths.iter().map(PathBuf::from).find(|p| p.exists());
    let moonlight_path = moon_paths.iter().map(PathBuf::from).find(|p| p.exists());
    let tailscale_path = ts_paths.iter().map(PathBuf::from).find(|p| p.exists());
    StreamingInstallReport {
        sunshine_installed:  sunshine_path.is_some(),
        sunshine_path,
        moonlight_installed: moonlight_path.is_some(),
        moonlight_path,
        tailscale_installed: tailscale_path.is_some(),
        tailscale_path,
        auto_creds_user: None,
        auto_creds_pass: None,
        messages:            Vec::new(),
    }
}

/// Fetch the latest upstream installer .exe and run it silently for
/// every external app we ship setup for (Sunshine + Moonlight + the
/// standalone Tailscale Windows client). Sunshine + Tailscale trigger
/// a UAC prompt each (they both register system services); Moonlight
/// is silent per-user. Already-installed copies are detected + skipped.
pub async fn install_both() -> Result<StreamingInstallReport> {
    let mut report = detect_existing();
    if report.sunshine_installed && report.moonlight_installed && report.tailscale_installed {
        report.messages.push("Sunshine + Moonlight + Tailscale already installed.".into());
        return Ok(report);
    }

    let tmp = std::env::temp_dir();

    if !report.sunshine_installed {
        let url = sunshine_installer_url().await
            .context("resolving Sunshine installer URL")?;
        let dest = tmp.join("abyss-Sunshine-installer.exe");
        log::info!("streaming_apps: downloading Sunshine -> {}", dest.display());
        fetch_to_file(&url, &dest, |_, _| {}).await
            .with_context(|| format!("downloading Sunshine from {url}"))?;
        // /S is the silent flag for NSIS; -Verb RunAs gives UAC elevation
        // because Sunshine's installer registers a Local System service.
        run_installer_elevated(&dest, "/S").await
            .context("running Sunshine installer")?;
        let _ = std::fs::remove_file(&dest);
        let recheck = detect_existing();
        if recheck.sunshine_installed {
            report.sunshine_installed = true;
            report.sunshine_path      = recheck.sunshine_path.clone();
            report.messages.push("Sunshine installed.".into());

            // Auto-set admin credentials so the user never has to open the
            // localhost:47990 admin page. Sunshine ships a top-level
            // `--creds <user> <pass>` flag that updates sunshine_state.json
            // and exits without starting the server, perfect for headless
            // post-install setup.
            if let Some(exe) = &recheck.sunshine_path {
                let user = "abyss".to_string();
                let pass = generate_password();
                match autoset_sunshine_creds(exe, &user, &pass).await {
                    Ok(()) => {
                        report.auto_creds_user = Some(user);
                        report.auto_creds_pass = Some(pass);
                        report.messages.push(
                            "Sunshine credentials configured silently — no browser dance needed.".into(),
                        );
                    }
                    Err(e) => {
                        log::warn!("sunshine creds auto-set failed: {e:#}");
                        report.messages.push(format!(
                            "Sunshine installed but auto-credential setup failed: {e}. \
                             You can set them manually at https://localhost:47990 or via \
                             Settings → Streaming."
                        ));
                    }
                }
            }
        } else {
            report.messages.push(
                "Sunshine installer ran but the binary isn't where expected — \
                 user may have cancelled the UAC prompt.".into(),
            );
        }
    } else {
        report.messages.push("Sunshine was already installed; skipped credential setup to \
                              preserve any existing config.".into());
    }

    if !report.moonlight_installed {
        let url = moonlight_installer_url().await
            .context("resolving Moonlight installer URL")?;
        let dest = tmp.join("abyss-Moonlight-installer.exe");
        log::info!("streaming_apps: downloading Moonlight -> {}", dest.display());
        fetch_to_file(&url, &dest, |_, _| {}).await
            .with_context(|| format!("downloading Moonlight from {url}"))?;
        run_installer_silent(&dest, "/S").await
            .context("running Moonlight installer")?;
        let _ = std::fs::remove_file(&dest);
        let recheck = detect_existing();
        if recheck.moonlight_installed {
            report.moonlight_installed = true;
            report.moonlight_path      = recheck.moonlight_path;
            report.messages.push("Moonlight installed.".into());
        } else {
            report.messages.push(
                "Moonlight installer ran but the binary isn't where expected.".into(),
            );
        }
    } else {
        report.messages.push("Moonlight was already installed; skipped.".into());
    }

    if !report.tailscale_installed {
        // Tailscale publishes a stable `latest` redirect on their CDN —
        // no need to chase release IDs.
        let url = "https://pkgs.tailscale.com/stable/tailscale-setup-latest.exe";
        let dest = tmp.join("abyss-Tailscale-setup.exe");
        log::info!("streaming_apps: downloading Tailscale -> {}", dest.display());
        fetch_to_file(url, &dest, |_, _| {}).await
            .with_context(|| format!("downloading Tailscale from {url}"))?;
        // Tailscale's NSIS wrapper accepts /S for silent install, and
        // installs to %ProgramFiles%\Tailscale\ . System service +
        // tray app both register, so elevation is required.
        run_installer_elevated(&dest, "/S").await
            .context("running Tailscale installer")?;
        let _ = std::fs::remove_file(&dest);
        let recheck = detect_existing();
        if recheck.tailscale_installed {
            report.tailscale_installed = true;
            report.tailscale_path      = recheck.tailscale_path;
            report.messages.push("Tailscale installed.".into());
        } else {
            report.messages.push(
                "Tailscale installer ran but the binary isn't where expected — \
                 user may have cancelled the UAC prompt.".into(),
            );
        }
    } else {
        report.messages.push("Tailscale was already installed; skipped.".into());
    }

    Ok(report)
}

#[cfg(target_os = "windows")]
async fn run_installer_elevated(path: &std::path::Path, args: &str) -> Result<()> {
    // PowerShell's Start-Process -Verb RunAs is the simplest UAC-elevating
    // launcher we can drive from Rust without pulling in winapi for
    // ShellExecuteEx. Wait=$true blocks until the installer exits so the
    // post-install file probe sees the real state.
    let ps = format!(
        "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait",
        path.display().to_string().replace('\'', "''"),
        args,
    );
    let out = crate::util::silent_cmd_std("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
        .context("spawning powershell to elevate installer")?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "elevated installer failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
async fn run_installer_silent(path: &std::path::Path, args: &str) -> Result<()> {
    // No elevation needed — Moonlight is per-user NSIS. Silent + no console
    // flash so a /S install is invisible to the user as intended.
    let mut cmd = crate::util::silent_cmd_tokio(path);
    cmd.arg(args);
    let status = cmd.status().await.context("spawning silent installer")?;
    if !status.success() {
        return Err(anyhow::anyhow!("installer exited {}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
async fn run_installer_elevated(_p: &std::path::Path, _a: &str) -> Result<()> {
    Err(anyhow::anyhow!("Sunshine auto-install is Windows-only"))
}
#[cfg(not(target_os = "windows"))]
async fn run_installer_silent(_p: &std::path::Path, _a: &str) -> Result<()> {
    Err(anyhow::anyhow!("Moonlight auto-install is Windows-only"))
}

/// Resolve the current Sunshine Windows installer URL via GitHub API.
async fn sunshine_installer_url() -> Result<String> {
    pick_release_asset("LizardByte", "Sunshine", "windows-amd64-installer.exe").await
}

/// Resolve the current Moonlight Qt Windows installer URL via GitHub API.
async fn moonlight_installer_url() -> Result<String> {
    pick_release_asset("moonlight-stream", "moonlight-qt", "MoonlightSetup-").await
}

// ---------------------------------------------------------------------------
// Silent post-install credential setup for Sunshine.
//
// Sunshine v0.21+ exposes `sunshine.exe --creds <user> <pass>` as a top-level
// flag — it updates the credentials file at
// %ProgramFiles%\Sunshine\config\sunshine_state.json and exits without
// starting the streaming server. Requires admin elevation because the
// install dir is under Program Files. We bracket the call with stop/start
// of SunshineService so the running service can't lock the credential
// file or cache stale creds in memory.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub(crate) async fn autoset_sunshine_creds(exe: &std::path::Path, user: &str, pass: &str) -> Result<()> {
    // 1. Stop the service. `sc stop` is idempotent: stops if running,
    //    returns a "service not started" error otherwise — which we ignore.
    let _ = crate::util::silent_cmd_std("sc").args(["stop", "SunshineService"]).output();
    // Small sleep so the service process actually exits before we touch creds.
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    // 2. Run sunshine.exe --creds with elevation. Build a single PowerShell
    //    invocation so the admin prompt happens once for this whole flow.
    let ps_cmd = format!(
        "Start-Process -FilePath '{}' -ArgumentList @('--creds', '{}', '{}') -Verb RunAs -Wait",
        exe.display().to_string().replace('\'', "''"),
        user.replace('\'', "''"),
        pass.replace('\'', "''"),
    );
    let out = crate::util::silent_cmd_std("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
        .output()
        .context("invoking powershell for elevated --creds call")?;
    if !out.status.success() {
        return Err(anyhow::anyhow!(
            "sunshine --creds failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    // 3. Restart the service so it picks up the new credentials.
    let start = crate::util::silent_cmd_std("sc")
        .args(["start", "SunshineService"])
        .output()
        .context("starting SunshineService after credential update")?;
    if !start.status.success() {
        // Non-fatal: the user can start it manually or via diagnostics.
        log::warn!(
            "SunshineService didn't restart cleanly: {}",
            String::from_utf8_lossy(&start.stderr).trim()
        );
    }

    log::info!("sunshine: auto-set admin credentials (user={user})");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) async fn autoset_sunshine_creds(_exe: &std::path::Path, _user: &str, _pass: &str) -> Result<()> {
    Err(anyhow::anyhow!("Sunshine auto-credential setup is Windows-only"))
}

/// Generate a 24-character alphanumeric password. We don't pull in the
/// `rand` crate — sha2 + the high-res timer + the process ID is more than
/// enough entropy for a host-local credential nobody outside this box
/// ever sees in cleartext.
pub(crate) fn generate_password() -> String {
    use sha2::{Digest, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(pid.to_le_bytes());
    hasher.update(b"abyss-sunshine-credential-salt-v1");
    let bytes = hasher.finalize();
    // Map each byte to a printable alphanumeric. Base36-ish without lookalikes.
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789abcdefghjkmnpqrstuvwxyz";
    bytes.iter()
        .take(24)
        .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
        .collect()
}

async fn pick_release_asset(owner: &str, repo: &str, name_contains: &str) -> Result<String> {
    use serde::Deserialize;
    #[derive(Deserialize)] struct Asset { name: String, browser_download_url: String }
    #[derive(Deserialize)] struct Release { assets: Vec<Asset> }
    let api = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("AbyssSingularity/0.1 installer")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let rel: Release = client.get(&api)
        .header("Accept", "application/vnd.github+json")
        .send().await?.error_for_status()?.json().await?;
    let needle = name_contains.to_ascii_lowercase();
    let pick = rel.assets.into_iter()
        .find(|a| a.name.to_ascii_lowercase().contains(&needle))
        .ok_or_else(|| anyhow::anyhow!("no asset matching {:?} in {owner}/{repo}", needle))?;
    Ok(pick.browser_download_url)
}
