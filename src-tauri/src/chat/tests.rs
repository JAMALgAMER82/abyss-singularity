use super::protocol::{read_frame, write_frame, MAX_FRAME_BYTES};
use super::state::ChatState;
use super::types::{
    ChatConfig, ChatHistoryEntry, ChatProtocol, Direction, PresenceStatus,
};

// ---------- types: defaults + serde round trips ----------------------------

#[test]
fn default_chat_config_listens_on_47992_and_is_disabled() {
    let cfg = ChatConfig::default();
    assert_eq!(cfg.listen_port, 47992);
    assert!(!cfg.enabled, "chat must default to OFF so we don't open a port silently");
    assert!(cfg.display_name.is_none());
}

#[test]
fn chat_protocol_chat_round_trips_through_json() {
    let frame = ChatProtocol::Chat {
        id:         "abc".into(),
        body:       "hello mesh".into(),
        sent_at_ms: 1234,
    };
    let s = serde_json::to_string(&frame).unwrap();
    assert!(s.contains(r#""kind":"chat""#));
    let back: ChatProtocol = serde_json::from_str(&s).unwrap();
    assert_eq!(frame, back);
}

#[test]
fn chat_protocol_presence_round_trips_through_json() {
    let frame = ChatProtocol::Presence {
        status:     PresenceStatus::Playing,
        activity:   Some("Chrono Trigger".into()),
        sent_at_ms: 9999,
    };
    let s = serde_json::to_string(&frame).unwrap();
    assert!(s.contains(r#""kind":"presence""#));
    assert!(s.contains(r#""status":"playing""#));
    let back: ChatProtocol = serde_json::from_str(&s).unwrap();
    assert_eq!(frame, back);
}

// ---------- protocol framing: write+read in-memory --------------------------

#[tokio::test(flavor = "current_thread")]
async fn frame_write_then_read_recovers_the_original() {
    let (mut a, mut b) = tokio::io::duplex(4096);
    let msg = ChatProtocol::Hello {
        name: "alice".into(),
        app_version: "0.1.0".into(),
        sent_at_ms: 42,
    };
    write_frame(&mut a, &msg).await.unwrap();
    let back = read_frame(&mut b).await.unwrap();
    assert_eq!(msg, back);
}

#[tokio::test(flavor = "current_thread")]
async fn read_frame_rejects_oversized_length_prefix() {
    use tokio::io::AsyncWriteExt;
    let (mut a, mut b) = tokio::io::duplex(4096);
    // Length far above MAX_FRAME_BYTES — should be refused before alloc.
    let big = (MAX_FRAME_BYTES as u32) + 1;
    a.write_u32(big).await.unwrap();
    let err = read_frame(&mut b).await.expect_err("must reject");
    assert!(err.to_string().contains("too large"));
}

// ---------- state: history + peers -----------------------------------------

#[test]
fn history_caps_at_thousand_entries() {
    use chrono::Utc;
    let state = ChatState::default();
    for i in 0..1_500 {
        state.append_history(ChatHistoryEntry {
            id: i.to_string(),
            peer_addr: "100.64.0.5".into(),
            direction: Direction::Outbound,
            body: format!("msg {i}"),
            at: Utc::now(),
        });
    }
    let all = state.history_for(None);
    assert_eq!(all.len(), 1_000);
    assert_eq!(all.first().unwrap().body, "msg 500"); // oldest 500 evicted
    assert_eq!(all.last().unwrap().body,  "msg 1499");
}

#[test]
fn upsert_peer_then_list_returns_the_snapshot_we_wrote() {
    let state = ChatState::default();
    state.upsert_peer("100.64.0.5", |slot| {
        slot.display_name = Some("buddy".into());
        slot.presence     = Some(PresenceStatus::Playing);
        slot.activity     = Some("Halo".into());
    });
    let peers = state.list_peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].addr, "100.64.0.5");
    assert_eq!(peers[0].display_name.as_deref(), Some("buddy"));
    assert!(!peers[0].connected, "no live tx yet");
    assert_eq!(peers[0].presence, Some(PresenceStatus::Playing));
}

#[test]
fn history_for_filters_by_peer() {
    use chrono::Utc;
    let state = ChatState::default();
    for (peer, body) in [("a", "1"), ("b", "2"), ("a", "3")] {
        state.append_history(ChatHistoryEntry {
            id: body.into(),
            peer_addr: peer.into(),
            direction: Direction::Inbound,
            body: body.into(),
            at: Utc::now(),
        });
    }
    let a_only = state.history_for(Some("a"));
    assert_eq!(a_only.len(), 2);
    assert_eq!(a_only[0].body, "1");
    assert_eq!(a_only[1].body, "3");
}
