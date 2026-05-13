//! In-memory chat state: history + peer slots + presence + listener handle.
//!
//! Wrapped in `Mutex` (sync, not `tokio::sync::Mutex`) because all access
//! is short and non-async. Long ops (TCP I/O) happen outside the lock.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::types::{
    ChatHistoryEntry, ChatProtocol, PeerSnapshot, PresenceStatus,
};

const HISTORY_CAP: usize = 1_000;

#[derive(Debug)]
pub struct PeerSlot {
    pub display_name: Option<String>,
    pub last_seen:    Option<DateTime<Utc>>,
    pub presence:     Option<PresenceStatus>,
    pub activity:     Option<String>,
    /// `Some` while a live writer task exists. Dropping clears it via
    /// `clear_peer_sender`.
    pub tx:           Option<mpsc::UnboundedSender<ChatProtocol>>,
}

#[derive(Default)]
pub struct ChatState {
    history:    Mutex<Vec<ChatHistoryEntry>>,
    peers:      Mutex<HashMap<String, PeerSlot>>,
    self_name:  Mutex<Option<String>>,
    presence:   Mutex<(PresenceStatus, Option<String>)>,
    server:     Mutex<Option<JoinHandle<()>>>,
}

impl ChatState {
    pub fn append_history(&self, entry: ChatHistoryEntry) -> ChatHistoryEntry {
        let mut g = self.history.lock().expect("history poisoned");
        g.push(entry.clone());
        if g.len() > HISTORY_CAP {
            let overflow = g.len() - HISTORY_CAP;
            g.drain(..overflow);
        }
        entry
    }

    pub fn history_for(&self, peer: Option<&str>) -> Vec<ChatHistoryEntry> {
        let g = self.history.lock().expect("history poisoned");
        match peer {
            Some(p) => g.iter().filter(|e| e.peer_addr == p).cloned().collect(),
            None    => g.clone(),
        }
    }

    pub fn upsert_peer(&self, addr: &str, mutate: impl FnOnce(&mut PeerSlot)) {
        let mut g = self.peers.lock().expect("peers poisoned");
        let slot = g.entry(addr.to_string()).or_insert_with(|| PeerSlot {
            display_name: None,
            last_seen:    None,
            presence:     None,
            activity:     None,
            tx:           None,
        });
        mutate(slot);
    }

    pub fn clear_peer_sender(&self, addr: &str) {
        let mut g = self.peers.lock().expect("peers poisoned");
        if let Some(slot) = g.get_mut(addr) {
            slot.tx = None;
        }
    }

    pub fn peer_sender(&self, addr: &str) -> Option<mpsc::UnboundedSender<ChatProtocol>> {
        let g = self.peers.lock().expect("peers poisoned");
        g.get(addr).and_then(|p| p.tx.clone())
    }

    pub fn list_peers(&self) -> Vec<PeerSnapshot> {
        let g = self.peers.lock().expect("peers poisoned");
        g.iter()
            .map(|(addr, slot)| PeerSnapshot {
                addr:         addr.clone(),
                display_name: slot.display_name.clone(),
                connected:    slot.tx.is_some(),
                presence:     slot.presence,
                activity:     slot.activity.clone(),
                last_seen:    slot.last_seen,
            })
            .collect()
    }

    pub fn set_self_name(&self, name: Option<String>) {
        *self.self_name.lock().expect("self_name poisoned") = name;
    }

    pub fn self_name(&self) -> String {
        self.self_name
            .lock()
            .expect("self_name poisoned")
            .clone()
            .unwrap_or_else(|| "Abyss user".to_string())
    }

    pub fn set_presence(&self, status: PresenceStatus, activity: Option<String>) {
        *self.presence.lock().expect("presence poisoned") = (status, activity);
    }

    pub fn presence(&self) -> (PresenceStatus, Option<String>) {
        self.presence.lock().expect("presence poisoned").clone()
    }

    pub fn install_server(&self, handle: JoinHandle<()>) {
        let prev = self
            .server
            .lock()
            .expect("server poisoned")
            .replace(handle);
        if let Some(h) = prev {
            h.abort();
        }
    }

    pub fn server_running(&self) -> bool {
        self.server
            .lock()
            .expect("server poisoned")
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }

    pub fn stop_server(&self) {
        if let Some(h) = self.server.lock().expect("server poisoned").take() {
            h.abort();
        }
        // Drop all peer senders so writer tasks exit cleanly.
        let mut g = self.peers.lock().expect("peers poisoned");
        for slot in g.values_mut() {
            slot.tx = None;
        }
    }
}

/// Process-global chat state. Stored in a OnceLock so both Tauri-managed
/// state and free functions (e.g. the listener task) can reach it
/// without threading an Arc through every callsite.
static GLOBAL: OnceLock<std::sync::Arc<ChatState>> = OnceLock::new();
pub fn global() -> std::sync::Arc<ChatState> {
    GLOBAL.get_or_init(|| std::sync::Arc::new(ChatState::default())).clone()
}

