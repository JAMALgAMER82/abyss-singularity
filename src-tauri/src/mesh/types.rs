use serde::{Deserialize, Serialize};

/// Mirror of the sidecar's HTTP `/status` JSON. Frontend consumers — the
/// `network::commands::net_tailscale_status` shim re-projects this into the
/// existing `TailscaleStatus` shape so the React side doesn't need to know
/// about the sidecar transport.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeshStatus {
    #[serde(default)] pub installed:     bool,
    #[serde(default)] pub backend_state: String,
    #[serde(default)] pub version:       String,
    #[serde(default)] pub self_ip:       String,
    #[serde(default)] pub self_dns:      String,
    #[serde(default)] pub needs_auth:    bool,
    #[serde(default)] pub auth_url:      String,
    #[serde(default)] pub peers:         Vec<MeshPeer>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeshPeer {
    #[serde(default)] pub host_name: String,
    #[serde(default)] pub dns_name:  String,
    #[serde(default)] pub addrs:     Vec<String>,
    #[serde(default)] pub online:    bool,
    #[serde(default)] pub os:        String,
}

/// Ports the sidecar binds on localhost. We set these explicitly when
/// spawning the child so Rust can dial them without an extra
/// "where did the sidecar end up?" round-trip.
#[derive(Debug, Clone, Copy)]
pub struct MeshPorts {
    pub control: u16,
    pub socks5:  u16,
    pub chat:    u16,
}

impl Default for MeshPorts {
    fn default() -> Self {
        Self { control: 7080, socks5: 1080, chat: 47992 }
    }
}
