//! `streaming` — Phase 5 of the Abyss roadmap.
//!
//! Owns the orchestration of two external binaries the user has
//! installed:
//!   * **Sunshine** — the open-source GameStream-compatible host that
//!     captures the gaming PC's display and serves it to clients.
//!   * **Moonlight** — the client that connects to Sunshine and renders
//!     the stream remotely.
//!
//! Both run as long-lived child processes managed by the same
//! `ProcessRegistry` we built in Phase 3 — meaning their stdout/stderr
//! land on the same `abyss://orchestration/event` channel the UI is
//! already listening on for emulator runs.

pub mod commands;
pub mod config;
pub mod types;

#[cfg(test)]
mod tests {
    use super::types::{KnownHost, StreamingConfig};

    #[test]
    fn streaming_config_round_trips_through_json_with_missing_optionals() {
        let cfg = StreamingConfig::default();
        let s = serde_json::to_string(&cfg).unwrap();
        let back: StreamingConfig = serde_json::from_str(&s).unwrap();
        assert!(back.sunshine_exe.is_none());
        assert!(back.moonlight_exe.is_none());
        assert!(back.known_hosts.is_empty());
    }

    #[test]
    fn streaming_config_persists_known_hosts() {
        let cfg = StreamingConfig {
            sunshine_exe:       Some(std::path::PathBuf::from("C:/sunshine.exe")),
            sunshine_admin_url: Some("https://localhost:47990".into()),
            moonlight_exe:      Some(std::path::PathBuf::from("C:/moonlight.exe")),
            known_hosts: vec![
                KnownHost { id: "h1".into(), name: "Mesh host".into(), host: "100.64.0.5".into() },
            ],
        };
        let s = serde_json::to_string(&cfg).unwrap();
        let back: StreamingConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(back.known_hosts.len(), 1);
        assert_eq!(back.known_hosts[0].host, "100.64.0.5");
        assert_eq!(back.sunshine_exe.as_ref().map(|p| p.to_string_lossy().to_string()),
                   Some("C:/sunshine.exe".to_string()));
    }
}
