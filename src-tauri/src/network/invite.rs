//! Invite codes — paste-able strings that bundle a Tailscale pre-auth
//! key plus the host's display name. Friends paste one, Abyss redeems
//! it, and their mesh sidecar restarts authenticating against the
//! host's tailnet — no browser dance on the friend's side.
//!
//! Wire format: base64url(JSON{v,ak,name}). Short enough to copy from
//! a chat message, opaque enough that a casual onlooker can't tell at
//! a glance that it embeds a Tailscale key.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};

/// Wire shape inside the base64 envelope.
#[derive(Debug, Serialize, Deserialize)]
struct InvitePayload {
    /// Schema version. Bump on any breaking change so older clients can
    /// refuse loudly instead of silently misinterpreting fields.
    v: u8,
    /// The Tailscale pre-auth key. Real Tailscale keys start with
    /// `tskey-auth-` so we soft-validate that shape on redeem.
    ak: String,
    /// Display name for the host — surfaced in the redeemer's UI so they
    /// can see "Joined Bob's tailnet" instead of an opaque key.
    name: String,
}

/// Public-shaped info we expose to the frontend after redeeming.
#[derive(Debug, Clone, Serialize)]
pub struct InviteInfo {
    pub host_name: String,
    pub authkey:   String,
}

const CURRENT_VERSION: u8 = 1;
const TAILSCALE_KEY_PREFIX: &str = "tskey-auth-";

/// Encode a host-side auth key + display name into a paste-able invite
/// code. The caller is responsible for already having validated the key
/// shape (we re-validate as a safety net).
pub fn encode(authkey: &str, host_name: &str) -> Result<String> {
    let ak = authkey.trim();
    if !looks_like_tailscale_key(ak) {
        return Err(anyhow!(
            "auth key doesn't look like a Tailscale pre-auth key (expected to start with '{}')",
            TAILSCALE_KEY_PREFIX
        ));
    }
    let name = host_name.trim();
    if name.is_empty() {
        return Err(anyhow!("host display name is empty"));
    }
    let payload = InvitePayload {
        v:    CURRENT_VERSION,
        ak:   ak.to_string(),
        name: name.to_string(),
    };
    let bytes = serde_json::to_vec(&payload).context("serialising invite payload")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Decode an invite code into the embedded info. Validates version + key shape.
pub fn decode(code: &str) -> Result<InviteInfo> {
    let code = code.trim();
    if code.is_empty() {
        return Err(anyhow!("invite code is empty"));
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(code)
        .map_err(|e| anyhow!("invite code is not valid base64: {e}"))?;
    let payload: InvitePayload =
        serde_json::from_slice(&bytes).map_err(|e| anyhow!("invite code payload malformed: {e}"))?;
    if payload.v > CURRENT_VERSION {
        return Err(anyhow!(
            "invite code is from a newer Abyss version (v{}); upgrade to redeem it",
            payload.v
        ));
    }
    if !looks_like_tailscale_key(&payload.ak) {
        return Err(anyhow!(
            "invite code's embedded key isn't a Tailscale pre-auth key"
        ));
    }
    Ok(InviteInfo {
        host_name: payload.name,
        authkey:   payload.ak,
    })
}

/// Soft shape check — Tailscale auth keys are `tskey-auth-<id>-<secret>`
/// and run ~70-90 chars. We don't strictly validate length so future
/// Tailscale changes don't break us, just the prefix + presence.
pub fn looks_like_tailscale_key(s: &str) -> bool {
    s.starts_with(TAILSCALE_KEY_PREFIX) && s.len() >= TAILSCALE_KEY_PREFIX.len() + 10
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_KEY: &str = "tskey-auth-kxxxxxxxxxxCNTRL-xxxxxxxxxxxxxxxxxxxxxxxxxx";

    #[test]
    fn round_trip_through_invite_code() {
        let code = encode(SAMPLE_KEY, "Bob").unwrap();
        let info = decode(&code).unwrap();
        assert_eq!(info.host_name, "Bob");
        assert_eq!(info.authkey, SAMPLE_KEY);
    }

    #[test]
    fn refuses_to_encode_a_non_tailscale_key() {
        let err = encode("not-a-real-key", "Bob").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("tailscale"),
            "{err}");
    }

    #[test]
    fn refuses_to_decode_a_v2_payload_from_a_newer_build() {
        // Hand-craft a payload claiming version 99 — older client should refuse.
        let future = serde_json::json!({
            "v": 99, "ak": SAMPLE_KEY, "name": "Future Bob"
        });
        let bytes = serde_json::to_vec(&future).unwrap();
        let code = URL_SAFE_NO_PAD.encode(bytes);
        let err = decode(&code).unwrap_err();
        assert!(err.to_string().contains("newer"));
    }

    #[test]
    fn refuses_obvious_garbage() {
        assert!(decode("").is_err());
        assert!(decode("not base64 at all!!!").is_err());
        // Valid base64 but not JSON.
        assert!(decode(&URL_SAFE_NO_PAD.encode(b"not json")).is_err());
    }

    #[test]
    fn trims_whitespace_around_the_code() {
        let code = encode(SAMPLE_KEY, "Bob").unwrap();
        let padded = format!("   {code}\n");
        let info = decode(&padded).unwrap();
        assert_eq!(info.host_name, "Bob");
    }

    #[test]
    fn rejects_blank_display_name() {
        assert!(encode(SAMPLE_KEY, "   ").is_err());
    }
}
