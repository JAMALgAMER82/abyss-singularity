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
pub mod pairing;
pub mod types;

#[cfg(test)]
mod tests {
    use super::types::{HostStatus, KnownHost, StreamingConfig};

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
            sunshine_exe:        Some(std::path::PathBuf::from("C:/sunshine.exe")),
            sunshine_admin_url:  Some("https://localhost:47990".into()),
            sunshine_admin_user: None,
            sunshine_admin_pass: None,
            moonlight_exe:       Some(std::path::PathBuf::from("C:/moonlight.exe")),
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

    #[test]
    fn streaming_config_accepts_legacy_payload_without_admin_creds() {
        // Configs written by Abyss builds before the in-app pairing feature
        // landed don't carry sunshine_admin_user / sunshine_admin_pass.
        // Loading one of those must not error — the user pairs through
        // Sunshine's web UI in the legacy path until they enter creds.
        let legacy = r#"{
            "sunshine_exe": "C:/sunshine.exe",
            "moonlight_exe": "C:/moonlight.exe",
            "known_hosts": []
        }"#;
        let cfg: StreamingConfig = serde_json::from_str(legacy).expect("legacy load");
        assert!(cfg.sunshine_admin_user.is_none());
        assert!(cfg.sunshine_admin_pass.is_none());
        assert!(cfg.sunshine_admin_url.is_none());
    }

    #[test]
    fn host_status_serialises_with_camel_field_names_for_the_frontend() {
        // The TS bridge in src/lib/streaming.ts declares `HostStatus` with
        // snake_case field names (run_id, admin_url, …). Make sure serde
        // doesn't accidentally start renaming if anyone adds a top-level
        // `#[serde(rename_all = "camelCase")]` to the struct later — this
        // test pins the wire shape.
        let status = HostStatus {
            configured: true,
            running:    true,
            pid:        Some(4321),
            admin_url:  Some("https://localhost:47990".into()),
            run_id:     Some("run-abc".into()),
        };
        let s = serde_json::to_string(&status).unwrap();
        for needle in ["\"configured\":true", "\"running\":true", "\"pid\":4321",
                       "\"admin_url\":", "\"run_id\":"] {
            assert!(s.contains(needle), "HostStatus JSON missing {needle}: {s}");
        }
    }

    #[test]
    fn host_status_omits_pid_and_run_id_when_not_running() {
        // Down-state HostStatus must still emit the fields even when None,
        // so the TS side's nullability hint stays accurate. (We DON'T use
        // skip_serializing_if on these — the UI distinguishes "missing key"
        // from "key with null value" today.)
        let status = HostStatus {
            configured: false,
            running:    false,
            pid:        None,
            admin_url:  None,
            run_id:     None,
        };
        let s = serde_json::to_string(&status).unwrap();
        assert!(s.contains("\"pid\":null"));
        assert!(s.contains("\"run_id\":null"));
        assert!(s.contains("\"admin_url\":null"));
    }
}
