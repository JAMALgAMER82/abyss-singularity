//! Process-global lobby state. Mirrors the pattern used by `chat::state` —
//! a single `OnceLock` so both Tauri commands and incoming chat frames
//! can reach the same room snapshot without threading an Arc everywhere.

use std::sync::{Mutex, OnceLock};

use crate::library::types::Platform;

use super::types::{RoomMember, RoomRole, RoomSnapshot};

#[derive(Default)]
pub struct LobbyState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    role:      Option<RoomRole>,
    host_addr: Option<String>,
    host_name: Option<String>,
    platform:  Option<Platform>,
    game_name: Option<String>,
    members:   Vec<RoomMember>,
}

impl LobbyState {
    pub fn snapshot(&self) -> RoomSnapshot {
        let g = self.inner.lock().expect("lobby state poisoned");
        RoomSnapshot {
            role:      g.role,
            host_addr: g.host_addr.clone(),
            host_name: g.host_name.clone(),
            platform:  g.platform,
            game_name: g.game_name.clone(),
            members:   g.members.clone(),
        }
    }

    pub fn become_host(&self, platform: Platform, game_name: String, host_name: String) {
        let mut g = self.inner.lock().expect("lobby state poisoned");
        g.role      = Some(RoomRole::Host);
        g.host_addr = Some("self".to_string());
        g.host_name = Some(host_name);
        g.platform  = Some(platform);
        g.game_name = Some(game_name);
        g.members.clear();
    }

    pub fn become_member(
        &self,
        host_addr: String,
        host_name: Option<String>,
        platform: Platform,
        game_name: String,
    ) {
        let mut g = self.inner.lock().expect("lobby state poisoned");
        g.role      = Some(RoomRole::Member);
        g.host_addr = Some(host_addr);
        g.host_name = host_name;
        g.platform  = Some(platform);
        g.game_name = Some(game_name);
        g.members.clear();
    }

    pub fn clear(&self) {
        let mut g = self.inner.lock().expect("lobby state poisoned");
        *g = Inner::default();
    }

    /// Idempotent member upsert. Returns true if the membership actually
    /// changed (insert or display-name update) so the caller can decide
    /// whether to re-broadcast.
    pub fn add_member(&self, addr: &str, display_name: Option<String>) -> bool {
        let mut g = self.inner.lock().expect("lobby state poisoned");
        if let Some(m) = g.members.iter_mut().find(|m| m.addr == addr) {
            if m.display_name != display_name {
                m.display_name = display_name;
                return true;
            }
            return false;
        }
        g.members.push(RoomMember { addr: addr.to_string(), display_name });
        true
    }

    pub fn remove_member(&self, addr: &str) -> bool {
        let mut g = self.inner.lock().expect("lobby state poisoned");
        let before = g.members.len();
        g.members.retain(|m| m.addr != addr);
        g.members.len() != before
    }

    pub fn is_host(&self) -> bool {
        self.inner.lock().expect("lobby state poisoned").role == Some(RoomRole::Host)
    }

    pub fn current_room(&self) -> Option<(Platform, String, Vec<RoomMember>)> {
        let g = self.inner.lock().expect("lobby state poisoned");
        match (g.platform, g.game_name.clone()) {
            (Some(p), Some(n)) => Some((p, n, g.members.clone())),
            _ => None,
        }
    }
}

static GLOBAL: OnceLock<std::sync::Arc<LobbyState>> = OnceLock::new();
pub fn global() -> std::sync::Arc<LobbyState> {
    GLOBAL.get_or_init(|| std::sync::Arc::new(LobbyState::default())).clone()
}
