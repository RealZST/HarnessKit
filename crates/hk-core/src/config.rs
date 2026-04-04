use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub audit: AuditConfig,
    #[serde(default)]
    pub agent_paths: AgentPathOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub theme: String,
    pub update_check_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub outdated_days: u32,
    pub rules_enabled: RulesEnabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesEnabled {
    pub prompt_injection: bool,
    pub rce: bool,
    pub credential_theft: bool,
    pub plaintext_secrets: bool,
    pub safety_bypass: bool,
    pub dangerous_commands: bool,
    pub broad_permissions: bool,

    pub supply_chain: bool,
    pub outdated: bool,
    pub unknown_source: bool,
    pub duplicate_conflict: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPathOverrides {
    pub claude: Option<String>,
    pub cursor: Option<String>,
    pub codex: Option<String>,
    pub gemini: Option<String>,
    pub antigravity: Option<String>,
    pub copilot: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                theme: "dark".into(),
                update_check_hours: 24,
            },
            audit: AuditConfig {
                outdated_days: 90,
                rules_enabled: RulesEnabled::default(),
            },
            agent_paths: AgentPathOverrides::default(),
        }
    }
}

impl Default for RulesEnabled {
    fn default() -> Self {
        Self {
            prompt_injection: true,
            rce: true,
            credential_theft: true,
            plaintext_secrets: true,
            safety_bypass: true,
            dangerous_commands: true,
            broad_permissions: true,

            supply_chain: true,
            outdated: true,
            unknown_source: true,
            duplicate_conflict: true,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let cfg: Config = toml::from_str(&content)?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert!(cfg.audit.rules_enabled.prompt_injection);
        assert_eq!(cfg.audit.outdated_days, 90);
        assert_eq!(cfg.general.theme, "dark");
    }

    #[test]
    fn test_load_creates_default_if_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config::load(&path).unwrap();
        assert!(path.exists());
        assert_eq!(cfg.general.theme, "dark");
    }

    #[test]
    fn test_save_and_reload() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.general.theme = "light".into();
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.general.theme, "light");
    }
}
