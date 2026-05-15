use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Wire protocol exchanged between two Abyss Singularity nodes over the
/// Tailscale mesh. Internally-tagged so a single channel can carry
/// heterogeneous events without per-message framing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatProtocol {
    /// First frame on a fresh connection — peer identifies itself.
    Hello {
        name:        String,
        app_version: String,
        sent_at_ms:  i64,
    },
    /// A text chat message.
    Chat {
        id:         String,
        body:       String,
        sent_at_ms: i64,
    },
    /// Periodic broadcast of "what am I doing right now."
    Presence {
        status:     PresenceStatus,
        activity:   Option<String>,
        sent_at_ms: i64,
    },
    /// Liveness probe.
    Ping { sent_at_ms: i64 },
    Pong { sent_at_ms: i64 },

    /// Phase 9 — peer wants to send us a game file. Receiver replies
    /// with [`Self::FileAccept`] either way.
    FileOffer {
        transfer_id: String,
        file_name:   String,
        file_size:   u64,
        platform:    crate::library::types::Platform,
        sha256:      String,
        sent_at_ms:  i64,
    },
    FileAccept {
        transfer_id: String,
        accept:      bool,
        /// Phase 10 — when the receiver already has an N-byte `.part`
        /// from a prior aborted attempt, it asks the sender to seek to
        /// that offset before streaming. `None` or `0` = fresh start.
        #[serde(default)]
        resume_offset: Option<u64>,
        sent_at_ms:  i64,
    },

    // ---------------- Phase 12 — in-app GameRanger-style lobby ----------------
    /// Host broadcasts: "I'm hosting a game right now." Sent to every
    /// connected chat peer when a room is created and on each membership
    /// change. Receivers surface it in the Friends/Lobby panel.
    LobbyAdvertise {
        host_name:  String,
        platform:   String,
        game_name:  String,
        members:    Vec<String>,
        sent_at_ms: i64,
    },
    /// Member asks the host to join the current room.
    LobbyJoinRequest {
        display_name: String,
        sent_at_ms:   i64,
    },
    /// Host accepted the join. Carries the canonical game identifier so the
    /// joiner can confirm they have a local copy before the host hits Start.
    LobbyJoinAccepted {
        platform:   String,
        game_name:  String,
        sent_at_ms: i64,
    },
    /// Member voluntarily leaves the room (or host kicks them).
    LobbyLeave { sent_at_ms: i64 },
    /// Host shuts the room down. Receivers clear any in-room state.
    LobbyClose { sent_at_ms: i64 },
    /// Host hit "Start game" — all members launch their copy of the game
    /// configured as netplay-client pointing at `host_addr`. Sent once,
    /// fan-out is N copies (one per member). Members reply via their local
    /// launch; success/failure is observed via orchestration events.
    LobbyStartGame {
        platform:    String,
        game_name:   String,
        host_addr:   String,
        sent_at_ms:  i64,
    },

    // ---------------- Phase 13 — silent Sunshine/Moonlight pairing -----------
    /// Friend → host: "I'm about to start a Moonlight pair with the PIN
    /// below. Accept it on your Sunshine for me." Eliminates the
    /// "Moonlight shows a PIN, host pastes it into Abyss" out-of-band
    /// step — both sides agree on the PIN via this trusted channel.
    StreamPairOffer {
        pin:        String,
        sent_at_ms: i64,
    },
    /// Host → friend: result of the auto-accept. `ok=true` means Sunshine
    /// accepted; `ok=false` carries an actionable error string.
    StreamPairResult {
        ok:         bool,
        error:      Option<String>,
        sent_at_ms: i64,
    },
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresenceStatus {
    #[default]
    Idle,
    Playing,
    Streaming,
    Away,
}

/// Persisted chat config — backed by the same `tauri-plugin-store`
/// `settings.json` everything else uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    /// Defaults to the OS hostname; freely renamable by the user.
    #[serde(default)]
    pub display_name: Option<String>,
    /// TCP port the listener binds on. Default 47992 — unprivileged,
    /// outside the standard service-port range so it shouldn't collide
    /// with Sunshine (47984-47990) or other common gaming tools.
    #[serde(default = "default_port")]
    pub listen_port: u16,
    /// Auto-start the listener on app launch. Off by default — user must
    /// opt in via Friends > "Go online" so the network surface stays
    /// closed unless they actively want it.
    #[serde(default)]
    pub enabled: bool,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            display_name: None,
            listen_port:  default_port(),
            enabled:      false,
        }
    }
}

fn default_port() -> u16 { 47992 }

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction { Inbound, Outbound }

#[derive(Debug, Clone, Serialize)]
pub struct ChatHistoryEntry {
    pub id:        String,
    pub peer_addr: String,
    pub direction: Direction,
    pub body:      String,
    pub at:        DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerSnapshot {
    pub addr:         String,
    pub display_name: Option<String>,
    pub connected:    bool,
    pub presence:     Option<PresenceStatus>,
    pub activity:     Option<String>,
    pub last_seen:    Option<DateTime<Utc>>,
}

/// Tauri event payloads.
pub const CHAT_MESSAGE_EVENT: &str = "abyss://chat/message";
pub const CHAT_PEER_EVENT:    &str = "abyss://chat/peer-update";
