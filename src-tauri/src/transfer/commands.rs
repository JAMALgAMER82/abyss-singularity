//! Tauri commands for peer-to-peer game transfer (Phase 9).

use chrono::Utc;
use tauri::{AppHandle, Emitter, Runtime};

use super::client::{send_file, sha256_of_file};
use super::server;
use super::state::{global, new_transfer_id};
use super::types::{
    Direction, PendingOffer, TransferEvent, TRANSFER_EVENT,
};
use crate::chat::{self, types::ChatProtocol};
use crate::library::cache;

/// Start the inbound listener. Called once during app setup so a peer
/// can send us a file at any time. Mirrors `chat_start`'s lifecycle.
#[tauri::command]
pub async fn transfer_start<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let state = global();
    let app_c = app.clone();
    tokio::spawn(async move {
        if let Err(e) = server::run(app_c, state).await {
            log::error!("transfer: listener task ended: {e:#}");
        }
    });
    Ok(())
}

/// Send `entry_id` from our library to `peer_addr`. Constructs a FileOffer,
/// ships it over the chat channel, and stages the source path so when
/// FileAccept comes back, [`accept_my_offer`] can launch the actual
/// byte-streaming task.
#[tauri::command]
pub async fn transfer_send<R: Runtime>(
    app:       AppHandle<R>,
    entry_id:  String,
    peer_addr: String,
) -> Result<String, String> {
    let dir = app
        .path_resolver_inline()
        .ok_or_else(|| "resolving app data dir".to_string())?;
    let snapshot = cache::load(&dir).map_err(|e| format!("{e:#}"))?;
    let entry = snapshot
        .entries
        .into_iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| format!("library entry not found: {entry_id}"))?;

    let path = entry.path.clone();
    if !path.exists() {
        return Err(format!("source file missing on disk: {}", path.display()));
    }
    let file_size = path
        .metadata()
        .map_err(|e| format!("stat {}: {e}", path.display()))?
        .len();

    let sha256 = sha256_of_file(&path).await.map_err(|e| format!("{e:#}"))?;
    let transfer_id = new_transfer_id();

    let offer = PendingOffer {
        transfer_id: transfer_id.clone(),
        peer_addr:   peer_addr.clone(),
        direction:   Direction::Outgoing,
        file_name:   entry.file_name.clone(),
        file_size,
        platform:    entry.platform,
        sha256:      sha256.clone(),
        source_path: Some(path),
        offered_at:  Utc::now(),
    };
    global().record_outgoing(offer.clone());

    // Send a FileOffer frame over the chat channel.
    let chat_state = chat::state::global();
    let tx = chat_state
        .peer_sender(&peer_addr)
        .ok_or_else(|| format!("no live chat connection to {peer_addr} — connect first"))?;
    let frame = ChatProtocol::FileOffer {
        transfer_id: transfer_id.clone(),
        file_name:   offer.file_name.clone(),
        file_size,
        platform:    offer.platform,
        sha256:      offer.sha256.clone(),
        sent_at_ms:  chat::protocol::now_ms(),
    };
    tx.send(frame)
        .map_err(|_| "chat connection dropped while sending offer".to_string())?;

    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Offered { offer });
    Ok(transfer_id)
}

/// Receiver-side accept. Sends `FileAccept{accept=true}` to peer, leaving
/// the offer registered so the inbound TCP handler can use it.
#[tauri::command]
pub fn transfer_accept<R: Runtime>(
    app:         AppHandle<R>,
    transfer_id: String,
) -> Result<(), String> {
    let state = global();
    // Find the offer (must already be registered as incoming).
    let g = state.incoming.lock().expect("incoming poisoned");
    let offer = g.get(&transfer_id).cloned()
        .ok_or_else(|| format!("no incoming offer with id {transfer_id}"))?;
    drop(g);

    let chat_state = chat::state::global();
    let tx = chat_state
        .peer_sender(&offer.peer_addr)
        .ok_or_else(|| format!("no live chat connection to {}", offer.peer_addr))?;
    // If a `.part` file exists from a previous aborted attempt, resume
    // from where it left off. Saves re-downloading multi-GB ROMs.
    let part_size = part_file_size(&app, &offer.file_name).unwrap_or(0);
    let resume_offset = if part_size > 0 && part_size < offer.file_size {
        Some(part_size)
    } else { None };

    tx.send(ChatProtocol::FileAccept {
        transfer_id: transfer_id.clone(),
        accept:      true,
        resume_offset,
        sent_at_ms:  chat::protocol::now_ms(),
    })
    .map_err(|_| "chat connection dropped while sending accept".to_string())?;

    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Accepted {
        transfer_id, peer_addr: offer.peer_addr,
    });
    Ok(())
}

#[tauri::command]
pub fn transfer_reject<R: Runtime>(
    app:         AppHandle<R>,
    transfer_id: String,
) -> Result<(), String> {
    let state = global();
    let offer = state.take_incoming(&transfer_id)
        .ok_or_else(|| format!("no incoming offer with id {transfer_id}"))?;
    if let Some(tx) = chat::state::global().peer_sender(&offer.peer_addr) {
        let _ = tx.send(ChatProtocol::FileAccept {
            transfer_id:   transfer_id.clone(),
            accept:        false,
            resume_offset: None,
            sent_at_ms:    chat::protocol::now_ms(),
        });
    }
    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Rejected {
        transfer_id, peer_addr: offer.peer_addr,
    });
    Ok(())
}

#[tauri::command]
pub fn transfer_list_incoming() -> Vec<PendingOffer> {
    global().list_incoming()
}

/// Called from `chat::session::handle_frame` when a `FileOffer` arrives.
/// Registers the incoming offer and emits an event so the UI can show
/// the accept dialog. Game transfers always require explicit user
/// consent via the Friends-view dialog — there's no auto-accept path.
#[allow(clippy::too_many_arguments)] // matches the FileOffer frame shape one-for-one
pub fn on_inbound_offer<R: Runtime>(
    app:         &AppHandle<R>,
    peer_addr:   &str,
    transfer_id: String,
    file_name:   String,
    file_size:   u64,
    platform:    crate::library::types::Platform,
    sha256:      String,
) {
    let offer = PendingOffer {
        transfer_id,
        peer_addr:   peer_addr.to_string(),
        direction:   Direction::Incoming,
        file_name,
        file_size,
        platform,
        sha256,
        source_path: None,
        offered_at:  Utc::now(),
    };
    global().record_incoming(offer.clone());
    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Offered { offer });
}

/// Called from `chat::session::handle_frame` when a `FileAccept` arrives.
/// Looks up our staged outgoing offer and kicks off `send_file`.
pub fn on_inbound_accept<R: Runtime>(
    app:           &AppHandle<R>,
    transfer_id:   &str,
    accept:        bool,
    resume_offset: Option<u64>,
) {
    let state = global();
    let offer = match state.take_outgoing(transfer_id) {
        Some(o) => o,
        None => {
            log::warn!("transfer: ignored stale FileAccept {transfer_id}");
            return;
        }
    };
    if !accept {
        let _ = app.emit(TRANSFER_EVENT, TransferEvent::Rejected {
            transfer_id: transfer_id.to_string(),
            peer_addr:   offer.peer_addr.clone(),
        });
        return;
    }
    let Some(source) = offer.source_path.clone() else {
        log::warn!("transfer: outgoing offer {transfer_id} has no source_path");
        return;
    };
    let start = resume_offset.unwrap_or(0);
    if start > 0 {
        log::info!("transfer: resuming {transfer_id} at offset {start}");
    }
    let app_c = app.clone();
    let tid_c = transfer_id.to_string();
    tokio::spawn(async move {
        let file_size = offer.file_size;
        if let Err(e) = send_file(app_c.clone(), offer.peer_addr.clone(), tid_c.clone(), &source, file_size, start).await {
            log::warn!("transfer: send {tid_c} failed: {e:#}");
            let _ = app_c.emit(TRANSFER_EVENT, TransferEvent::Failed {
                transfer_id: tid_c, message: format!("{e:#}"),
            });
        }
    });
}

/// Return the size in bytes of `incoming/<file>.part`, or None if it
/// doesn't exist. Used by the receiver to decide whether to ask for a
/// resume.
fn part_file_size<R: Runtime>(app: &AppHandle<R>, file_name: &str) -> Option<u64> {
    let dir = super::server::incoming_dir(app).ok()?;
    let dest = dir.join(file_name);
    let tmp = dest.with_extension(format!(
        "{}.part",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("bin"),
    ));
    std::fs::metadata(&tmp).ok().map(|m| m.len())
}

// ----------------------------------------------------------------------------
// `path_resolver_inline` — tiny helper so the command doesn't need to depend
// on `tauri::Manager` import (and so we get a `PathBuf` straight back).
// ----------------------------------------------------------------------------
trait AppHandleExt {
    fn path_resolver_inline(&self) -> Option<std::path::PathBuf>;
}
impl<R: Runtime> AppHandleExt for AppHandle<R> {
    fn path_resolver_inline(&self) -> Option<std::path::PathBuf> {
        use tauri::Manager as _;
        self.path().app_data_dir().ok()
    }
}
