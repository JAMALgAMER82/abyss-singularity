//! In-process transfer registry. Holds:
//!  * Outgoing offers we've sent — keyed by `transfer_id`, holding the
//!    source file path so the sender task can find it after `FileAccept`
//!    comes back.
//!  * Incoming offers we've received — keyed by `transfer_id`, holding
//!    the metadata so the inbound TCP handler can look up the transfer
//!    when bytes start arriving.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::types::PendingOffer;

#[derive(Default)]
pub struct TransferState {
    pub outgoing: Mutex<HashMap<String, PendingOffer>>,
    pub incoming: Mutex<HashMap<String, PendingOffer>>,
}

impl TransferState {
    pub fn record_outgoing(&self, offer: PendingOffer) {
        self.outgoing
            .lock()
            .expect("outgoing poisoned")
            .insert(offer.transfer_id.clone(), offer);
    }

    pub fn record_incoming(&self, offer: PendingOffer) {
        self.incoming
            .lock()
            .expect("incoming poisoned")
            .insert(offer.transfer_id.clone(), offer);
    }

    pub fn take_outgoing(&self, transfer_id: &str) -> Option<PendingOffer> {
        self.outgoing
            .lock()
            .expect("outgoing poisoned")
            .remove(transfer_id)
    }

    pub fn take_incoming(&self, transfer_id: &str) -> Option<PendingOffer> {
        self.incoming
            .lock()
            .expect("incoming poisoned")
            .remove(transfer_id)
    }

    pub fn list_incoming(&self) -> Vec<PendingOffer> {
        self.incoming
            .lock()
            .expect("incoming poisoned")
            .values()
            .cloned()
            .collect()
    }
}

static GLOBAL: OnceLock<std::sync::Arc<TransferState>> = OnceLock::new();
pub fn global() -> std::sync::Arc<TransferState> {
    GLOBAL.get_or_init(|| std::sync::Arc::new(TransferState::default())).clone()
}

/// Tiny no-dep transfer-id helper. Reuses the same shape as `chat::session`
/// so we have one fewer crate to pull in.
pub fn new_transfer_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("xf-{t:x}-{n:x}")
}
