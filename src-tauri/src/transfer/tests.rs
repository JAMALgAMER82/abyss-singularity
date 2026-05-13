use super::protocol::{read_transfer_id, write_transfer_id};
use super::state::{new_transfer_id, TransferState};
use super::types::{Direction, PendingOffer};
use crate::library::types::Platform;
use chrono::Utc;

fn sample_offer(tid: &str, dir: Direction) -> PendingOffer {
    PendingOffer {
        transfer_id: tid.into(),
        peer_addr:   "100.64.0.5".into(),
        direction:   dir,
        file_name:   "Sonic.md".into(),
        file_size:   64 * 1024,
        platform:    Platform::Genesis,
        sha256:      "deadbeef".into(),
        source_path: None,
        offered_at:  Utc::now(),
    }
}

#[test]
fn transfer_ids_are_unique_across_calls() {
    use std::collections::HashSet;
    let ids: HashSet<String> = (0..1024).map(|_| new_transfer_id()).collect();
    assert_eq!(ids.len(), 1024);
}

#[test]
fn registry_separates_incoming_from_outgoing() {
    let s = TransferState::default();
    s.record_outgoing(sample_offer("a", Direction::Outgoing));
    s.record_incoming(sample_offer("b", Direction::Incoming));
    assert_eq!(s.list_incoming().len(), 1);
    assert!(s.take_outgoing("a").is_some());
    assert!(s.take_outgoing("a").is_none());
    assert!(s.take_incoming("b").is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn protocol_round_trip() {
    let (mut a, mut b) = tokio::io::duplex(1024);
    let tid = "xf-deadbeef";
    write_transfer_id(&mut a, tid).await.unwrap();
    let got = read_transfer_id(&mut b).await.unwrap();
    assert_eq!(got, tid);
}

#[tokio::test(flavor = "current_thread")]
async fn protocol_rejects_oversized_tid_len() {
    use tokio::io::AsyncWriteExt;
    let (mut a, mut b) = tokio::io::duplex(64);
    // Claim a 1 MB tid — should be refused before alloc.
    a.write_u32(1_000_000).await.unwrap();
    let err = read_transfer_id(&mut b).await.unwrap_err();
    assert!(err.to_string().contains("too long"));
}
