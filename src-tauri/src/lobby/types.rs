use serde::{Deserialize, Serialize};

use crate::library::types::Platform;

/// What role this Abyss is playing in the currently-active room.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoomRole {
    /// We're hosting — we picked the game and own the netplay listener.
    Host,
    /// We joined someone else's room.
    Member,
}

/// One peer in a room, identified by the address chat already uses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomMember {
    pub addr:         String,
    pub display_name: Option<String>,
}

/// The fields the UI needs to render the active-room banner.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RoomSnapshot {
    /// `None` when we're not in any room.
    pub role:       Option<RoomRole>,
    /// Address of the host. Equal to "self" when we are the host.
    pub host_addr:  Option<String>,
    pub host_name:  Option<String>,
    pub platform:   Option<Platform>,
    pub game_name:  Option<String>,
    pub members:    Vec<RoomMember>,
}

/// Result of attempting to launch a game for a room (host or join).
#[derive(Debug, Clone, Serialize)]
pub struct LobbyLaunchReport {
    pub run_id:       String,
    pub command_line: String,
    pub role:         RoomRole,
}

pub const LOBBY_EVENT: &str = "abyss://lobby/state";
pub const LOBBY_INCOMING_EVENT: &str = "abyss://lobby/incoming-invite";
