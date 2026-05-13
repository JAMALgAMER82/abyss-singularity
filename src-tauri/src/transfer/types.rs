use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::PathBuf;

use crate::library::types::Platform;

/// Direction of a transfer from the local node's perspective.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction { Outgoing, Incoming }

/// A pending transfer that's been *offered* but not yet started.
#[derive(Debug, Clone, Serialize)]
pub struct PendingOffer {
    pub transfer_id: String,
    pub peer_addr:   String,
    pub direction:   Direction,
    pub file_name:   String,
    pub file_size:   u64,
    pub platform:    Platform,
    pub sha256:      String,
    /// For outgoing offers, the local file we'll be sending.
    #[serde(skip_serializing)]
    pub source_path: Option<PathBuf>,
    pub offered_at:  DateTime<Utc>,
}

/// A transfer that's actively shovelling bytes.
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)] // reserved for the per-transfer status command we'll add when the UI needs it
pub struct ActiveTransfer {
    pub transfer_id: String,
    pub peer_addr:   String,
    pub direction:   Direction,
    pub file_name:   String,
    pub file_size:   u64,
    pub bytes_done:  u64,
    pub started_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)] // reserved for transfer_history command
pub struct TransferReport {
    pub transfer_id: String,
    pub file_name:   String,
    pub bytes_done:  u64,
    pub elapsed_ms:  u64,
    pub final_path:  Option<PathBuf>,
    pub sha256_ok:   bool,
}

/// Event payload streamed to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransferEvent {
    Offered  { offer: PendingOffer },
    Accepted { transfer_id: String, peer_addr: String },
    Rejected { transfer_id: String, peer_addr: String },
    Started  { transfer_id: String, direction: Direction },
    Progress { transfer_id: String, bytes_done: u64, bytes_total: u64 },
    Completed{ transfer_id: String, final_path: Option<PathBuf>, sha256_ok: bool },
    Failed   { transfer_id: String, message: String },
}

pub const TRANSFER_EVENT: &str = "abyss://transfer/event";

/// Default port the file-transfer forwarder uses. Matches the Go
/// sidecar's `--transfer` flag.
pub const TRANSFER_PORT: u16 = 47993;
