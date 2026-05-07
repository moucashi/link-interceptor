use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchRequest {
    pub raw_url: String,
    pub received_at: DateTime<Utc>,
}

impl LaunchRequest {
    pub fn new(raw_url: String) -> Self {
        Self {
            raw_url,
            received_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub url: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub open_count: u64,
}

impl HistoryEntry {
    pub fn new(url: String, now: DateTime<Utc>) -> Self {
        Self {
            url,
            first_seen_at: now,
            last_seen_at: now,
            open_count: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FavoriteEntry {
    pub url: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomApp {
    pub name: String,
    pub executable: String,
    pub args_template: String,
}

impl Default for CustomApp {
    fn default() -> Self {
        Self {
            name: "自定义应用".to_owned(),
            executable: String::new(),
            args_template: "{url}".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainRule {
    pub pattern: String,
    pub app_name: String,
}

impl Default for DomainRule {
    fn default() -> Self {
        Self {
            pattern: "example.com".to_owned(),
            app_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "default_bring_new_windows_to_front")]
    pub bring_new_windows_to_front: bool,
    pub custom_apps: Vec<CustomApp>,
    pub domain_rules: Vec<DomainRule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bring_new_windows_to_front: default_bring_new_windows_to_front(),
            custom_apps: Vec::new(),
            domain_rules: Vec::new(),
        }
    }
}

fn default_bring_new_windows_to_front() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    Browser,
    ProtocolHandler,
    DomainApp,
    CustomApp,
    ShellFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCandidate {
    pub name: String,
    pub kind: CandidateKind,
    pub command: Option<String>,
    pub args_template: String,
    pub available: bool,
    pub reason: String,
}

impl OpenCandidate {
    pub fn new(
        name: impl Into<String>,
        kind: CandidateKind,
        command: Option<String>,
        args_template: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let command = command.filter(|value| !value.trim().is_empty());
        let available = command.is_some() || matches!(kind, CandidateKind::ShellFallback);
        Self {
            name: name.into(),
            kind,
            command,
            args_template: args_template.into(),
            available,
            reason: reason.into(),
        }
    }
}
