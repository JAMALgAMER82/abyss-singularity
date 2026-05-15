//! Thin HTTP client over the Worker's `/v1/*` endpoints.
//!
//! Every method is async, takes ownership of the inputs (no &str arena
//! fiddling) and returns an [`anyhow::Result`] for clean propagation
//! into Tauri command error strings. We don't cache state here — the
//! Worker is the source of truth and is cheap to ping.

use anyhow::{anyhow, Context, Result};
use reqwest::Client as Http;
use serde_json::{json, Value};

use super::types::{
    DirectMessage, Friend, FriendRequest, FriendResponse, GlobalChatMessage, OnlineUser,
};

#[derive(Clone)]
pub struct Directory {
    http:    Http,
    base:    String,
    user_id: String,
}

impl Directory {
    pub fn new(base: impl Into<String>, user_id: impl Into<String>) -> Result<Self> {
        let http = Http::builder()
            .user_agent(format!("AbyssSingularity/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("building http client")?;
        let mut base = base.into();
        // Trim trailing slashes so we can `format!("{base}/v1/...")` consistently.
        while base.ends_with('/') { base.pop(); }
        Ok(Self { http, base, user_id: user_id.into() })
    }

    /// Heartbeat — keep us on the online list.
    pub async fn presence(
        &self,
        handle:      &str,
        app_version: &str,
        country:     Option<&str>,
    ) -> Result<()> {
        let body = json!({
            "user_id":     self.user_id,
            "handle":      handle,
            "app_version": app_version,
            "country":     country,
        });
        self.post("/v1/presence", body).await.map(|_| ())
    }

    /// Toggle hidden / appear-offline.
    pub async fn set_hidden(&self, hidden: bool) -> Result<()> {
        let body = json!({ "user_id": self.user_id, "hidden": hidden });
        self.post("/v1/hidden", body).await.map(|_| ())
    }

    /// List users seen in the last `since_ms` (default 5 min if 0).
    pub async fn online(&self, since_ms: u64) -> Result<Vec<OnlineUser>> {
        let path = format!("/v1/online?since_ms={since_ms}&viewer_id={}", self.user_id);
        let v = self.get(&path).await?;
        Ok(serde_json::from_value(v.get("users").cloned().unwrap_or_default())?)
    }

    /// Send a friend request. `invite_code` is optional — when None,
    /// it's a directory-only friendship (chat/DM, no mesh peering).
    pub async fn send_friend_request(
        &self,
        to_id:       &str,
        from_handle: &str,
        invite_code: Option<&str>,
        message:     Option<&str>,
    ) -> Result<i64> {
        let body = json!({
            "from_id":     self.user_id,
            "from_handle": from_handle,
            "to_id":       to_id,
            "invite_code": invite_code,
            "message":     message,
        });
        let v = self.post("/v1/friend-request", body).await?;
        let id = v.get("request_id").and_then(|x| x.as_i64())
            .ok_or_else(|| anyhow!("server omitted request_id"))?;
        Ok(id)
    }

    /// Pending requests addressed to us.
    pub async fn friend_requests(&self) -> Result<Vec<FriendRequest>> {
        let path = format!("/v1/friend-requests?user_id={}", self.user_id);
        let v = self.get(&path).await?;
        Ok(serde_json::from_value(v.get("requests").cloned().unwrap_or_default())?)
    }

    /// Responses to requests *we* sent (accept / reject), so the UI can
    /// auto-surface the inbound accept-side invite code if present.
    pub async fn friend_responses(&self) -> Result<Vec<FriendResponse>> {
        let path = format!("/v1/friend-responses?user_id={}", self.user_id);
        let v = self.get(&path).await?;
        Ok(serde_json::from_value(v.get("responses").cloned().unwrap_or_default())?)
    }

    pub async fn accept_request(&self, request_id: i64, invite_code: Option<&str>) -> Result<()> {
        let body = json!({
            "user_id":     self.user_id,
            "request_id":  request_id,
            "invite_code": invite_code,
        });
        self.post("/v1/friend-accept", body).await.map(|_| ())
    }

    pub async fn reject_request(&self, request_id: i64) -> Result<()> {
        let body = json!({ "user_id": self.user_id, "request_id": request_id });
        self.post("/v1/friend-reject", body).await.map(|_| ())
    }

    pub async fn friends(&self) -> Result<Vec<Friend>> {
        let path = format!("/v1/friends?user_id={}", self.user_id);
        let v = self.get(&path).await?;
        Ok(serde_json::from_value(v.get("friends").cloned().unwrap_or_default())?)
    }

    pub async fn send_dm(&self, to_id: &str, body_text: &str) -> Result<i64> {
        let body = json!({
            "from_id": self.user_id,
            "to_id":   to_id,
            "body":    body_text,
        });
        let v = self.post("/v1/dm", body).await?;
        Ok(v.get("message_id").and_then(|x| x.as_i64()).unwrap_or_default())
    }

    pub async fn get_dms(&self, since_ms: u64) -> Result<Vec<DirectMessage>> {
        let path = format!("/v1/dm?user_id={}&since_ms={since_ms}", self.user_id);
        let v = self.get(&path).await?;
        Ok(serde_json::from_value(v.get("messages").cloned().unwrap_or_default())?)
    }

    pub async fn send_global_chat(&self, handle: &str, body_text: &str) -> Result<i64> {
        let body = json!({
            "user_id": self.user_id,
            "handle":  handle,
            "body":    body_text,
        });
        let v = self.post("/v1/global-chat", body).await?;
        Ok(v.get("message_id").and_then(|x| x.as_i64()).unwrap_or_default())
    }

    pub async fn get_global_chat(&self, since_ms: u64) -> Result<Vec<GlobalChatMessage>> {
        let path = format!("/v1/global-chat?since_ms={since_ms}");
        let v = self.get(&path).await?;
        Ok(serde_json::from_value(v.get("messages").cloned().unwrap_or_default())?)
    }

    pub async fn block(&self, target_id: &str) -> Result<()> {
        let body = json!({ "user_id": self.user_id, "target_id": target_id });
        self.post("/v1/block", body).await.map(|_| ())
    }

    pub async fn unblock(&self, target_id: &str) -> Result<()> {
        let body = json!({ "user_id": self.user_id, "target_id": target_id });
        self.post("/v1/unblock", body).await.map(|_| ())
    }

    // ---------------- internals ----------------------------------------------

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let resp = self.http.post(&url).json(&body).send().await
            .with_context(|| format!("POST {url}"))?;
        Self::interpret(resp).await
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let resp = self.http.get(&url).send().await
            .with_context(|| format!("GET {url}"))?;
        Self::interpret(resp).await
    }

    async fn interpret(resp: reqwest::Response) -> Result<Value> {
        let status = resp.status();
        let text   = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            // Try to surface the Worker's `{ ok:false, error:"..." }` message.
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                if let Some(msg) = v.get("error").and_then(|x| x.as_str()) {
                    return Err(anyhow!("directory: {msg} (HTTP {status})"));
                }
            }
            return Err(anyhow!("directory: HTTP {status}: {}", text.trim()));
        }
        Ok(serde_json::from_str(&text).unwrap_or_else(|_| Value::Null))
    }
}
