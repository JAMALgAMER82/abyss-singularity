use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// Subsystem was already healthy, no action taken.
    Ok,
    /// We detected a problem and fixed it on the spot.
    Repaired,
    /// Real problem detected but it needs the user to do something
    /// Abyss legally / technically can't do (BIOS dump, AV exception).
    NeedsUser,
    /// Tried to fix and failed — usually transient (network down,
    /// service permission denied).
    Failed,
    /// Step skipped because a prerequisite isn't met.
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// Short id like `"mesh"` / `"sunshine"` — frontend can key off
    /// this if it wants to render an icon per row.
    pub id:       String,
    /// Human-readable title shown in the row.
    pub title:    String,
    pub status:   CheckStatus,
    /// One-line description of what was found / what to do.
    pub message:  String,
    /// Optional path that's worth showing the user verbatim
    /// (e.g. the abyss-mesh.exe path to whitelist in AV).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_path: Option<String>,
    /// Optional URL the UI can render as a "learn more" link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_url:  Option<String>,
}

impl CheckResult {
    pub fn ok(id: &str, title: &str, message: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), status: CheckStatus::Ok,
               message: message.into(), action_path: None, action_url: None }
    }
    pub fn repaired(id: &str, title: &str, message: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), status: CheckStatus::Repaired,
               message: message.into(), action_path: None, action_url: None }
    }
    pub fn needs_user(id: &str, title: &str, message: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), status: CheckStatus::NeedsUser,
               message: message.into(), action_path: None, action_url: None }
    }
    pub fn failed(id: &str, title: &str, message: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), status: CheckStatus::Failed,
               message: message.into(), action_path: None, action_url: None }
    }
    #[allow(dead_code)]
    pub fn skipped(id: &str, title: &str, message: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), status: CheckStatus::Skipped,
               message: message.into(), action_path: None, action_url: None }
    }
    pub fn with_path(mut self, p: impl Into<String>) -> Self {
        self.action_path = Some(p.into()); self
    }
    pub fn with_url(mut self, u: impl Into<String>) -> Self {
        self.action_url = Some(u.into()); self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub checks:        Vec<CheckResult>,
    pub elapsed_ms:    u64,
    pub repaired_count: usize,
    pub needs_user_count: usize,
    pub failed_count:  usize,
}
