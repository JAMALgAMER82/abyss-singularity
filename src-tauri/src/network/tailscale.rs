//! Tailscale-status façade.
//!
//! Phase 7 onward, the source of truth is the embedded `abyss-mesh`
//! sidecar (Tailscale userspace stack via `tsnet`). This module projects
//! the sidecar's [`crate::mesh::types::MeshStatus`] into the shape the
//! frontend was already consuming so no React change was needed when we
//! swapped the transport.

use super::types::{TailscalePeer, TailscaleStatus};

pub async fn status() -> TailscaleStatus {
    let ports = crate::mesh::types::MeshPorts::default();
    match crate::mesh::control::status(ports).await {
        Ok(m) => TailscaleStatus {
            installed:     m.installed,
            version:       (!m.version.is_empty()).then_some(m.version),
            backend_state: (!m.backend_state.is_empty()).then_some(m.backend_state),
            self_ip:       (!m.self_ip.is_empty()).then_some(m.self_ip),
            self_dns:      (!m.self_dns.is_empty()).then_some(m.self_dns),
            peers:         m.peers.into_iter().map(|p| TailscalePeer {
                host_name: p.host_name,
                dns_name:  (!p.dns_name.is_empty()).then_some(p.dns_name),
                addrs:     p.addrs,
                online:    p.online,
                os:        (!p.os.is_empty()).then_some(p.os),
            }).collect(),
            needs_auth:    m.needs_auth,
            auth_url:      (!m.auth_url.is_empty()).then_some(m.auth_url),
            error:         None,
        },
        Err(e) => TailscaleStatus {
            installed: false,
            error:     Some(format!("{e:#}")),
            ..Default::default()
        },
    }
}
