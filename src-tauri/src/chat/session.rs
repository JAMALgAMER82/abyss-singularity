//! Bidirectional connection session — splits a TcpStream into reader +
//! writer tasks and routes frames through the [`ChatState`].
//!
//! Used identically for inbound (server-accepted, pre-split BufReader for
//! PROXY v1 parsing) and outbound (client-initiated TcpStream) connections.

use std::sync::Arc;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::protocol::{now_ms, read_frame, write_frame};
use super::state::ChatState;
use super::types::{
    ChatHistoryEntry, ChatProtocol, Direction, CHAT_MESSAGE_EVENT, CHAT_PEER_EVENT,
};

/// Run a session over a `TcpStream`. Convenience wrapper used by the
/// outbound client where no PROXY-v1 parse is needed.
pub async fn run<RT: Runtime>(
    app:        AppHandle<RT>,
    state:      Arc<ChatState>,
    stream:     TcpStream,
    peer_label: String,
    we_started: bool,
) {
    let (r, w) = stream.into_split();
    run_split(app, state, r, w, peer_label, we_started).await;
}

/// Run a session over arbitrary read/write halves. The inbound server
/// uses this with a `BufReader` that already consumed the PROXY-v1
/// header; the outbound client uses it with raw split halves.
pub async fn run_split<RT, R, W>(
    app:        AppHandle<RT>,
    state:      Arc<ChatState>,
    mut read_half:  R,
    mut write_half: W,
    peer_label: String,
    we_started: bool,
)
where
    RT: Runtime,
    R:  AsyncRead  + Unpin + Send + 'static,
    W:  AsyncWrite + Unpin + Send + 'static,
{
    let (tx, mut rx) = mpsc::unbounded_channel::<ChatProtocol>();

    state.upsert_peer(&peer_label, |slot| {
        slot.tx = Some(tx.clone());
    });
    emit_peer_update(&app, &state, &peer_label);

    // Writer task: drain mpsc -> socket.
    let writer_label = peer_label.clone();
    let writer_state = state.clone();
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = write_frame(&mut write_half, &msg).await {
                log::warn!("chat: write to {writer_label} failed: {e:#}");
                break;
            }
        }
        writer_state.clear_peer_sender(&writer_label);
    });

    // If we initiated, send Hello + initial Presence immediately.
    if we_started {
        let (status, activity) = state.presence();
        let _ = tx.send(ChatProtocol::Hello {
            name:        state.self_name(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            sent_at_ms:  now_ms(),
        });
        let _ = tx.send(ChatProtocol::Presence { status, activity, sent_at_ms: now_ms() });
    }

    // Reader loop.
    loop {
        match read_frame(&mut read_half).await {
            Ok(frame) => handle_frame(&app, &state, &peer_label, &tx, frame),
            Err(e) => {
                log::info!("chat: read from {peer_label} ended: {e:#}");
                break;
            }
        }
    }

    drop(tx);            // closes the writer channel
    let _ = writer.await;
    state.clear_peer_sender(&peer_label);
    emit_peer_update(&app, &state, &peer_label);
}

fn handle_frame<RT: Runtime>(
    app:        &AppHandle<RT>,
    state:      &Arc<ChatState>,
    peer_label: &str,
    tx:         &mpsc::UnboundedSender<ChatProtocol>,
    frame:      ChatProtocol,
) {
    match frame {
        ChatProtocol::Hello { name, .. } => {
            state.upsert_peer(peer_label, |slot| {
                slot.display_name = Some(name);
                slot.last_seen    = Some(Utc::now());
            });
            // Reciprocate with our Hello + current presence so they get
            // our state too. (Idempotent if they sent us one first.)
            let (status, activity) = state.presence();
            let _ = tx.send(ChatProtocol::Hello {
                name:        state.self_name(),
                app_version: env!("CARGO_PKG_VERSION").into(),
                sent_at_ms:  now_ms(),
            });
            let _ = tx.send(ChatProtocol::Presence { status, activity, sent_at_ms: now_ms() });
            emit_peer_update(app, state, peer_label);
        }
        ChatProtocol::Chat { id, body, .. } => {
            let entry = ChatHistoryEntry {
                id,
                peer_addr: peer_label.to_string(),
                direction: Direction::Inbound,
                body,
                at:        Utc::now(),
            };
            let stored = state.append_history(entry);
            state.upsert_peer(peer_label, |slot| { slot.last_seen = Some(Utc::now()); });
            let _ = app.emit(CHAT_MESSAGE_EVENT, stored);
        }
        ChatProtocol::Presence { status, activity, .. } => {
            state.upsert_peer(peer_label, |slot| {
                slot.presence  = Some(status);
                slot.activity  = activity;
                slot.last_seen = Some(Utc::now());
            });
            emit_peer_update(app, state, peer_label);
        }
        ChatProtocol::Ping { sent_at_ms } => {
            let _ = tx.send(ChatProtocol::Pong { sent_at_ms });
        }
        ChatProtocol::Pong { .. } => {
            state.upsert_peer(peer_label, |slot| { slot.last_seen = Some(Utc::now()); });
        }
        ChatProtocol::FileOffer { transfer_id, file_name, file_size, platform, sha256, .. } => {
            crate::transfer::commands::on_inbound_offer(
                app, peer_label, transfer_id, file_name, file_size, platform, sha256,
            );
        }
        ChatProtocol::FileAccept { transfer_id, accept, resume_offset, .. } => {
            crate::transfer::commands::on_inbound_accept(app, &transfer_id, accept, resume_offset);
        }
    }
}

fn emit_peer_update<RT: Runtime>(app: &AppHandle<RT>, state: &Arc<ChatState>, _peer: &str) {
    // Send the whole peer list — clients can diff if they care. Cheap
    // enough at our scale (peers ≤ tens), and avoids a per-peer payload
    // shape proliferation.
    let _ = app.emit(CHAT_PEER_EVENT, state.list_peers());
}

// ----------------------------------------------------------------------------
// Tiny no-dep short-id helper. Used for stable message IDs without pulling
// in the full `uuid` crate.
// ----------------------------------------------------------------------------
mod uuid_lite {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    pub fn short_id() -> String {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{t:x}-{n:x}")
    }
}

pub use uuid_lite::short_id as new_message_id;
