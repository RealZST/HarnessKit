use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

fn default_true() -> bool {
    true
}

// --- Extension ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    pub id: String,
    pub kind: ExtensionKind,
    pub name: String,
    pub description: String,
    pub source: Source,
    pub agents: Vec<String>,
    pub tags: Vec<String>,
    pub pack: Option<String>,
    pub permissions: Vec<Permission>,
    pub enabled: bool,
    pub trust_score: Option<u8>,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source_path: Option<String>,
    pub cli_parent_id: Option<String>,
    pub cli_meta: Option<CliMeta>,
    pub install_meta: Option<InstallMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionKind {
    Skill,
    Mcp,
    Plugin,
    Hook,
    Cli,
}

impl ExtensionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Mcp => "mcp",
            Self::Plugin => "plugin",
            Self::Hook => "hook",
            Self::Cli => "cli",
        }
    }
}

impl FromStr for ExtensionKind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "skill" => Ok(Self::Skill),
            "mcp" => Ok(Self::Mcp),
            "plugin" => Ok(Self::Plugin),
            "hook" => Ok(Self::Hook),
            "cli" => Ok(Self::Cli),
            _ => Err(anyhow::anyhow!("unknown extension kind: {s}")),
        }
    }
}

// --- Source ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub origin: SourceOrigin,
    pub url: Option<String>,
    pub version: Option<String>,
    pub commit_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceOrigin {
    Git,
    Registry,
    Agent,
    Local,
}

impl SourceOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Registry => "registry",
            Self::Agent => "agent",
            Self::Local => "local",
        }
    }
}

// --- Permission ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Permission {
    FileSystem { paths: Vec<String> },
    Network { domains: Vec<String> },
    Shell { commands: Vec<String> },
    Database { engines: Vec<String> },
    Env { keys: Vec<String> },
}

impl Permission {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FileSystem { .. } => "filesystem",
            Self::Network { .. } => "network",
            Self::Shell { .. } => "shell",
            Self::Database { .. } => "database",
            Self::Env { .. } => "env",
        }
    }

    /// Get mutable reference to the inner values vec, regardless of variant.
    fn values_mut(&mut self) -> &mut Vec<String> {
        match self {
            Self::FileSystem { paths } => paths,
            Self::Network { domains } => domains,
            Self::Shell { commands } => commands,
            Self::Database { engines } => engines,
            Self::Env { keys } => keys,
        }
    }

    /// Get reference to the inner values vec, regardless of variant.
    fn values(&self) -> &[String] {
        match self {
            Self::FileSystem { paths } => paths,
            Self::Network { domains } => domains,
            Self::Shell { commands } => commands,
            Self::Database { engines } => engines,
            Self::Env { keys } => keys,
        }
    }
}

/// Merge `source` permissions into `target`, deduplicating by dimension.
/// For each permission in source, if target already has the same variant,
/// merge the values (dedup); otherwise push a new entry.
pub fn merge_permissions(target: &mut Vec<Permission>, source: &[Permission]) {
    for src in source {
        if let Some(existing) = target.iter_mut().find(|t| t.label() == src.label()) {
            let dst = existing.values_mut();
            for val in src.values() {
                if !dst.contains(val) {
                    dst.push(val.clone());
                }
            }
        } else {
            target.push(src.clone());
        }
    }
}

// --- CLI Metadata ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CliMeta {
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub install_method: Option<String>,
    pub credentials_path: Option<String>,
    pub version: Option<String>,
    pub api_domains: Vec<String>,
}

// --- Install Metadata ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallMeta {
    pub install_type: String,
    pub url: Option<String>,
    pub url_resolved: Option<String>,
    pub branch: Option<String>,
    pub subpath: Option<String>,
    pub revision: Option<String>,
    pub remote_revision: Option<String>,
    pub checked_at: Option<DateTime<Utc>>,
    pub check_error: Option<String>,
}

// --- Audit ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    pub extension_id: String,
    pub findings: Vec<AuditFinding>,
    pub trust_score: u8,
    pub audited_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub location: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

impl Severity {
    pub fn deduction(&self) -> u8 {
        match self {
            Self::Critical => 25,
            Self::High => 15,
            Self::Medium => 8,
            Self::Low => 3,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

// --- Trust Tier ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustTier {
    Safe,
    LowRisk,
    NeedsReview,
}

impl TrustTier {
    pub fn from_score(score: u8) -> Self {
        match score {
            80..=100 => Self::Safe,
            60..=79 => Self::LowRisk,
            0..=59 => Self::NeedsReview,
            _ => Self::NeedsReview,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Safe => "Safe",
            Self::LowRisk => "Low Risk",
            Self::NeedsReview => "Needs Review",
        }
    }
}

// --- Project ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: DateTime<Utc>,
    /// Whether the project path exists on disk.
    #[serde(default = "default_true")]
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProject {
    pub name: String,
    pub path: String,
}

// --- Update Status ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UpdateStatus {
    UpToDate { remote_hash: String },
    UpdateAvailable { remote_hash: String },
    RemovedFromRepo,
    Error { message: String },
}

// --- New Repo Skills ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewRepoSkill {
    pub repo_url: String,
    pub pack: Option<String>,
    pub skill_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckUpdatesResult {
    pub statuses: Vec<(String, UpdateStatus)>,
    pub new_skills: Vec<NewRepoSkill>,
}

// --- Agent Info ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub detected: bool,
    pub extension_count: usize,
    pub path: String,
    pub enabled: bool,
}

// --- Dashboard Stats ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_extensions: usize,
    pub skill_count: usize,
    pub mcp_count: usize,
    pub plugin_count: usize,
    pub hook_count: usize,
    pub cli_count: usize,
    pub critical_issues: usize,
    pub high_issues: usize,
    pub medium_issues: usize,
    pub low_issues: usize,
    pub updates_available: usize,
}

// --- Agent Config File ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigFile {
    pub path: String,
    pub agent: String,
    pub category: ConfigCategory,
    pub scope: ConfigScope,
    pub file_name: String,
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    /// Whether this path is a directory.
    #[serde(default)]
    pub is_dir: bool,
    /// Whether the path exists on disk.
    #[serde(default = "default_true")]
    pub exists: bool,
    /// If set, this is a user-added custom config path (value is the DB row ID).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<i64>,
    /// User-defined label for custom config paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigCategory {
    Rules,
    Memory,
    Settings,
    Workflow,
    Ignore,
}

impl ConfigCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Memory => "memory",
            Self::Settings => "settings",
            Self::Workflow => "workflow",
            Self::Ignore => "ignore",
        }
    }

    pub fn order(&self) -> u8 {
        match self {
            Self::Rules => 0,
            Self::Memory => 1,
            Self::Settings => 2,
            Self::Workflow => 3,
            Self::Ignore => 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConfigScope {
    Global,
    Project { name: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDetail {
    pub name: String,
    pub detected: bool,
    pub config_files: Vec<AgentConfigFile>,
    pub extension_counts: ExtensionCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCounts {
    pub skill: usize,
    pub mcp: usize,
    pub plugin: usize,
    pub hook: usize,
    pub cli: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_kind_display() {
        assert_eq!(ExtensionKind::Skill.as_str(), "skill");
        assert_eq!(ExtensionKind::Mcp.as_str(), "mcp");
        assert_eq!(ExtensionKind::Plugin.as_str(), "plugin");
        assert_eq!(ExtensionKind::Hook.as_str(), "hook");
    }

    #[test]
    fn test_extension_kind_from_str() {
        assert_eq!(
            "skill".parse::<ExtensionKind>().unwrap(),
            ExtensionKind::Skill
        );
        assert_eq!("mcp".parse::<ExtensionKind>().unwrap(), ExtensionKind::Mcp);
        assert!("invalid".parse::<ExtensionKind>().is_err());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
    }

    #[test]
    fn test_severity_deduction() {
        assert_eq!(Severity::Critical.deduction(), 25);
        assert_eq!(Severity::High.deduction(), 15);
        assert_eq!(Severity::Medium.deduction(), 8);
        assert_eq!(Severity::Low.deduction(), 3);
    }

    #[test]
    fn test_trust_tier() {
        assert_eq!(TrustTier::from_score(95), TrustTier::Safe);
        assert_eq!(TrustTier::from_score(80), TrustTier::Safe);
        assert_eq!(TrustTier::from_score(79), TrustTier::LowRisk);
        assert_eq!(TrustTier::from_score(60), TrustTier::LowRisk);
        assert_eq!(TrustTier::from_score(59), TrustTier::NeedsReview);
        assert_eq!(TrustTier::from_score(40), TrustTier::NeedsReview);
        assert_eq!(TrustTier::from_score(39), TrustTier::NeedsReview);
        assert_eq!(TrustTier::from_score(0), TrustTier::NeedsReview);
    }

    #[test]
    fn test_source_origin_display() {
        assert_eq!(SourceOrigin::Git.as_str(), "git");
        assert_eq!(SourceOrigin::Registry.as_str(), "registry");
        assert_eq!(SourceOrigin::Agent.as_str(), "agent");
        assert_eq!(SourceOrigin::Local.as_str(), "local");
    }

    #[test]
    fn test_config_category_as_str() {
        assert_eq!(ConfigCategory::Rules.as_str(), "rules");
        assert_eq!(ConfigCategory::Memory.as_str(), "memory");
        assert_eq!(ConfigCategory::Settings.as_str(), "settings");
        assert_eq!(ConfigCategory::Workflow.as_str(), "workflow");
        assert_eq!(ConfigCategory::Ignore.as_str(), "ignore");
    }

    #[test]
    fn test_config_scope_serialization() {
        let global = ConfigScope::Global;
        let json = serde_json::to_string(&global).unwrap();
        assert!(json.contains("\"type\":\"global\""));

        let project = ConfigScope::Project {
            name: "myapp".into(),
            path: "/Users/test/myapp".into(),
        };
        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("\"type\":\"project\""));
        assert!(json.contains("\"name\":\"myapp\""));
    }

    #[test]
    fn test_extension_kind_cli() {
        assert_eq!(ExtensionKind::Cli.as_str(), "cli");
        assert_eq!("cli".parse::<ExtensionKind>().unwrap(), ExtensionKind::Cli);
    }

    #[test]
    fn test_cli_meta_serde() {
        let meta = CliMeta {
            binary_name: "wecom-cli".into(),
            binary_path: Some("/usr/local/bin/wecom-cli".into()),
            install_method: Some("npm".into()),
            credentials_path: Some("~/.config/wecom/bot.enc".into()),
            version: Some("1.2.3".into()),
            api_domains: vec!["qyapi.weixin.qq.com".into()],
        };
        let json = serde_json::to_string(&meta).unwrap();
        let round_trip: CliMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip.binary_name, "wecom-cli");
        assert_eq!(round_trip.api_domains.len(), 1);
    }
}
