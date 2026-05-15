//! Wire types — exactly mirror the JSON shapes the Worker emits.

use serde::{Deserialize, Serialize};

/// One online user as returned by `GET /v1/online`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineUser {
    pub id:          String,
    pub handle:      String,
    pub country:     Option<String>,
    pub last_seen:   i64,
    pub app_version: String,
}

/// One row from the friend-request inbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id:          i64,
    pub from_id:     String,
    pub from_handle: String,
    pub message:     Option<String>,
    pub invite_code: Option<String>,
    pub created_at:  i64,
}

/// Outcome row for requests *we* sent, surfaced by `/v1/friend-responses`
/// so the UI can show "Bob accepted your friend request" and — when an
/// accept-side invite code is included — auto-offer to redeem it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendResponse {
    pub id:                 i64,
    pub to_id:              String,
    pub status:             String,                 // "accepted" | "rejected"
    pub accept_invite_code: Option<String>,
    pub responded_at:       Option<i64>,
    pub created_at:         i64,
}

/// One row from `/v1/friends`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Friend {
    pub id:             String,
    pub handle:         String,
    pub country:        Option<String>,
    pub last_seen:      i64,
    pub hidden:         i64,
    pub established_at: i64,
}

/// One direct message — same shape for both inbound and outbound (UI
/// distinguishes via `from_id == me` vs `to_id == me`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessage {
    pub id:      i64,
    pub from_id: String,
    pub to_id:   String,
    pub body:    String,
    pub sent_at: i64,
}

/// One global-chat row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalChatMessage {
    pub id:      i64,
    pub user_id: String,
    pub handle:  String,
    pub body:    String,
    pub sent_at: i64,
}

/// Tauri event names — pushed to the frontend when the heartbeat or
/// inbox poll notices something new.
pub const DIR_FRIEND_REQUEST_EVENT:    &str = "abyss://directory/friend-request";
pub const DIR_FRIEND_RESPONSE_EVENT:   &str = "abyss://directory/friend-response";
pub const DIR_DM_EVENT:                &str = "abyss://directory/dm";
#[allow(dead_code)] // Reserved for the global-chat push path once the heartbeat starts polling it.
pub const DIR_GLOBAL_CHAT_EVENT:       &str = "abyss://directory/global-chat";
