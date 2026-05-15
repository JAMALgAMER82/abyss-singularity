//! Auto-pairing flow — eliminates the "Moonlight shows PIN, host pastes it
//! into Sunshine" out-of-band step.
//!
//! Two-actor protocol over the existing chat channel:
//!
//! * **Friend (initiator)**
//!   1. Generate a random 4-digit PIN.
//!   2. Spawn `moonlight pair <host_ip> --pin <pin>` — Moonlight starts the
//!      GameStream pairing handshake against Sunshine at `<host_ip>:47984`.
//!   3. Send `ChatProtocol::StreamPairOffer { pin }` to the host so they
//!      know which PIN to accept.
//!   4. Wait for `ChatProtocol::StreamPairResult { ok }` from the host.
//!      If ok, launch `moonlight stream <host_ip>`.
//!
//! * **Host (responder)**
//!   1. Receive `StreamPairOffer { pin }` from a chat-known peer (i.e. a
//!      peer on our tailnet that already shares a session with us).
//!   2. Retry-loop POST to Sunshine's `/api/pin` with the PIN, using the
//!      auto-generated admin credentials in [`StreamingConfig`]. We retry
//!      because Sunshine only accepts a PIN once Moonlight has actually
//!      reached the pairing-pending state, which can take a couple of
//!      seconds after the friend invokes `moonlight pair`.
//!   3. Send `StreamPairResult` back so the friend's UI can flip from
//!      "pairing…" to "streaming".
//!
//! Security note: we don't auto-accept from arbitrary internet peers —
//! the chat layer only sees frames from peers we already have a TCP
//! connection with, which over the embedded mesh means they're on the
//! same tailnet (host's, after invite redemption). So the trust boundary
//! is "anyone on the tailnet can ask to pair Moonlight" — same shape as
//! "anyone on the tailnet can connect to Sunshine directly anyway."

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Emitter, Runtime};

use crate::chat::state as chat_state;
use crate::chat::types::ChatProtocol;

use super::config;

/// Tauri event payload friend-side UI listens for to know when the
/// pair-and-stream sequence finishes (or fails). Carries the host
/// address so the UI can disambiguate concurrent attempts.
pub const STREAM_PAIR_PROGRESS_EVENT: &str = "abyss://stream/pair-progress";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "phase")]
pub enum PairProgress {
    /// Host accepted our PIN and Sunshine is now paired with us.
    Accepted { host_addr: String },
    /// Host rejected or Sunshine returned an error.
    Rejected { host_addr: String, error: String },
    /// Moonlight pair subprocess never reached Sunshine (timeout / network).
    Timeout  { host_addr: String },
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Generate a cryptographically-mediocre 4-digit PIN. Good enough — the
/// PIN is consumed by two endpoints we both control within seconds.
fn random_pin() -> String {
    use sha2::{Digest, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(b"abyss-pair-pin-salt");
    let bytes = h.finalize();
    // Take first 4 bytes, fold into a 4-digit decimal.
    let n = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 10_000;
    format!("{:04}", n)
}

// ---------------------------------------------------------------------------
// Friend side — `stream_request_pair_and_launch` Tauri command.
// ---------------------------------------------------------------------------

/// One-shot helper friend's UI hits to pair-and-stream against a host
/// they can already see in chat. Orchestrates:
///   * generate PIN
///   * `moonlight pair <host> --pin <pin>` subprocess
///   * `StreamPairOffer { pin }` over chat
///   * wait for `StreamPairResult` (delivered as an emit event)
///   * `moonlight stream <host>`
///
/// On the happy path, the friend just sees Moonlight pop up streaming the
/// host's desktop — no PINs read aloud, no IPs typed.
#[tauri::command]
pub async fn stream_request_pair_and_launch<R: Runtime>(
    app:       AppHandle<R>,
    host_addr: String,
) -> Result<(), String> {
    let host_addr = host_addr.trim().to_string();
    if host_addr.is_empty() {
        return Err("host address is empty".into());
    }

    // Load streaming config — we need Moonlight's exe path to spawn the
    // subprocess. (The host's creds live on the host side, not ours.)
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let moonlight = cfg.moonlight_exe.clone().ok_or_else(|| {
        "Moonlight isn't installed yet — open Settings → Streaming and click 'Install Sunshine + Moonlight'.".to_string()
    })?;

    // Make sure we have a live chat session to the host. Without one the
    // PIN offer can't be delivered, so we'd just timeout silently.
    let chat = chat_state::global();
    let host_tx = chat.peer_sender(&host_addr).ok_or_else(|| {
        format!(
            "no live chat session to {host_addr}. Open the Friends tab and click 'link' on \
             that peer first so the host can accept our pairing offer."
        )
    })?;

    let pin = random_pin();

    // Spawn `moonlight pair <host> --pin <pin>` in the background. We don't
    // .wait on it here — Moonlight will eventually exit on its own when
    // Sunshine accepts the PIN (driven by the host's side of this flow).
    // silent_cmd_tokio suppresses Moonlight's console window flash; the
    // Qt GUI windows it opens are unaffected.
    let pair_cmd = crate::util::silent_cmd_tokio(&moonlight)
        .args(["pair", &host_addr, "--pin", &pin])
        .spawn()
        .map_err(|e| format!("spawning moonlight pair: {e}"))?;
    let pair_pid = pair_cmd.id().unwrap_or(0);
    log::info!("stream-pair: moonlight pair {host_addr} --pin {pin} (pid {pair_pid})");

    // Send the offer to the host. Brief delay first so Moonlight has time
    // to reach Sunshine's pending-pair state before the host POSTs the PIN.
    tokio::time::sleep(Duration::from_millis(400)).await;
    host_tx
        .send(ChatProtocol::StreamPairOffer { pin: pin.clone(), sent_at_ms: now_ms() })
        .map_err(|e| format!("dispatching pair offer to host: {e}"))?;

    // The `StreamPairResult` arrives asynchronously via the chat handler,
    // which forwards it as a Tauri event the frontend listens for. We
    // also stash a per-host wait token so a successful pair triggers the
    // stream launch from the chat handler side. Implemented as a simple
    // ID-keyed flag in the global pairing state.
    pending::insert(&host_addr, pair_pid);

    // Defensive timeout: if the host doesn't reply within 30s, surface
    // it to the UI so the user knows to retry / check that the host is
    // running Abyss.
    let app_for_timeout = app.clone();
    let host_for_timeout = host_addr.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        if pending::take(&host_for_timeout).is_some() {
            log::warn!("stream-pair: timeout waiting for {host_for_timeout} to accept");
            let _ = app_for_timeout.emit(STREAM_PAIR_PROGRESS_EVENT, PairProgress::Timeout {
                host_addr: host_for_timeout,
            });
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Chat-layer entry points called by `chat::session::handle_frame`.
// ---------------------------------------------------------------------------

/// Host received `StreamPairOffer { pin }` from a peer. Auto-accept by
/// retry-POSTing to Sunshine's `/api/pin` until either it succeeds or we
/// hit the retry budget. Report the outcome back to the peer.
pub fn on_pair_offer<R: Runtime>(app: &AppHandle<R>, peer_addr: &str, pin: String) {
    let app = app.clone();
    let peer = peer_addr.to_string();
    tauri::async_runtime::spawn(async move {
        let res = accept_pin_with_retries(&app, &pin).await;
        let frame = match &res {
            Ok(()) => {
                log::info!("stream-pair: accepted PIN from {peer} on Sunshine");
                ChatProtocol::StreamPairResult {
                    ok: true,
                    error: None,
                    sent_at_ms: now_ms(),
                }
            }
            Err(e) => {
                let msg = format!("{e:#}");
                log::warn!("stream-pair: rejecting PIN from {peer}: {msg}");
                ChatProtocol::StreamPairResult {
                    ok: false,
                    error: Some(msg),
                    sent_at_ms: now_ms(),
                }
            }
        };
        if let Some(tx) = chat_state::global().peer_sender(&peer) {
            let _ = tx.send(frame);
        }
    });
}

/// Friend received `StreamPairResult` from the host. If ok, fire off the
/// `moonlight stream <host>` subprocess we'd been waiting for. Either
/// way, emit the result to the frontend so the UI can flip its banner.
pub fn on_pair_result<R: Runtime>(
    app:        &AppHandle<R>,
    host_addr:  &str,
    ok:         bool,
    error:      Option<String>,
) {
    let host_owned = host_addr.to_string();
    let pending_removed = pending::take(host_addr).is_some();
    if !pending_removed {
        // We aren't waiting on a pair from this host. Stale message —
        // ignore. (Defensive: the host might dupe a frame after a reconnect.)
        return;
    }

    if !ok {
        let _ = app.emit(STREAM_PAIR_PROGRESS_EVENT, PairProgress::Rejected {
            host_addr: host_owned,
            error: error.unwrap_or_else(|| "host rejected the pair offer".into()),
        });
        return;
    }

    // Success — kick off the stream. We do this fire-and-forget so the
    // chat handler returns quickly; orchestration will surface launch
    // errors via its own event stream.
    let app_owned = app.clone();
    tauri::async_runtime::spawn(async move {
        match super::commands::stream_launch_client_internal(app_owned.clone(), Some(host_owned.clone())).await {
            Ok(_)  => {
                let _ = app_owned.emit(STREAM_PAIR_PROGRESS_EVENT, PairProgress::Accepted {
                    host_addr: host_owned,
                });
            }
            Err(e) => {
                let _ = app_owned.emit(STREAM_PAIR_PROGRESS_EVENT, PairProgress::Rejected {
                    host_addr: host_owned,
                    error: format!("pair succeeded but launching Moonlight failed: {e}"),
                });
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Host-side retry loop against Sunshine's REST endpoint.
// ---------------------------------------------------------------------------

async fn accept_pin_with_retries<R: Runtime>(app: &AppHandle<R>, pin: &str) -> Result<()> {
    let cfg = config::load(app).context("loading streaming config")?;
    let user = cfg.sunshine_admin_user.clone().ok_or_else(|| {
        anyhow!(
            "Sunshine admin username not set — re-run the streaming installer or set it under \
             Settings → Streaming."
        )
    })?;
    let pass = cfg.sunshine_admin_pass.clone().ok_or_else(|| {
        anyhow!("Sunshine admin password not set.")
    })?;
    let base = cfg
        .sunshine_admin_url
        .clone()
        .unwrap_or_else(|| "https://localhost:47990".to_string());

    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
        .build()
        .context("building http client for sunshine /api/pin")?;
    let body = serde_json::json!({ "pin": pin, "name": "AbyssPaired" });

    // Sunshine's pairing window: Moonlight typically reaches the pending
    // state within 1-3s of `moonlight pair` starting. Allow 12s of retry,
    // 500ms between tries → 24 attempts is plenty of headroom.
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..24u32 {
        let resp = http
            .post(format!("{base}/api/pin"))
            .basic_auth(&user, Some(&pass))
            .json(&body)
            .send()
            .await;
        match resp {
            Ok(r) => {
                let status = r.status();
                let txt = r.text().await.unwrap_or_default();
                if status.as_u16() == 401 {
                    return Err(anyhow!(
                        "Sunshine rejected the admin credentials (401). \
                         Reset them via Settings → Streaming → 'Reset Sunshine credentials'."
                    ));
                }
                if status.is_success() {
                    // Sunshine signals PIN mismatch with body `{"status":"false"}`.
                    if let Ok(j) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if j.get("status").and_then(|v| v.as_str()) == Some("false") {
                            // Not necessarily fatal — Moonlight may not yet
                            // have reached the pending state. Keep retrying.
                            last_err = Some(anyhow!(
                                "Sunshine: {}",
                                j.get("error").and_then(|v| v.as_str()).unwrap_or("PIN not yet pending")
                            ));
                        } else {
                            return Ok(());
                        }
                    } else {
                        return Ok(());
                    }
                } else {
                    last_err = Some(anyhow!("Sunshine /api/pin {status}: {}", txt.trim()));
                }
            }
            Err(e) => {
                last_err = Some(anyhow!("Sunshine /api/pin {attempt} request failed: {e}"));
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(last_err.unwrap_or_else(|| anyhow!("Sunshine never accepted the pair within 12s")))
}

// ---------------------------------------------------------------------------
// Per-process pending-pair table so the chat handler knows which hosts
// we're actively waiting on.
// ---------------------------------------------------------------------------

mod pending {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static MAP: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

    fn map() -> &'static Mutex<HashMap<String, u32>> {
        MAP.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn insert(host_addr: &str, pair_pid: u32) {
        let mut g = map().lock().expect("pending pair map poisoned");
        g.insert(host_addr.to_string(), pair_pid);
    }

    pub fn take(host_addr: &str) -> Option<u32> {
        let mut g = map().lock().expect("pending pair map poisoned");
        g.remove(host_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_pin_is_always_four_decimal_digits() {
        for _ in 0..20 {
            let p = random_pin();
            assert_eq!(p.len(), 4);
            assert!(p.chars().all(|c| c.is_ascii_digit()), "pin {p} has non-digits");
        }
    }

    #[test]
    fn random_pin_varies_across_calls() {
        // The PIN includes nanosecond entropy — two back-to-back calls
        // should almost always differ. We allow a single accidental match
        // before declaring the entropy source dead.
        let a = random_pin();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = random_pin();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let c = random_pin();
        assert!(a != b || b != c, "got {a}/{b}/{c} — entropy looks broken");
    }

    #[test]
    fn pending_insert_take_roundtrips() {
        pending::insert("100.1.2.3", 1234);
        assert_eq!(pending::take("100.1.2.3"), Some(1234));
        assert_eq!(pending::take("100.1.2.3"), None);
    }
}
