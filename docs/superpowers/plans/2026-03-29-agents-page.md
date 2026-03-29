# Agents Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an Agents page providing a per-agent unified view of config files (Rules, Memory, Settings, Ignore) with read-only preview and external editor integration.

**Architecture:** Extend the `AgentAdapter` trait with config file discovery methods (global paths + project patterns). Add a `scan_agent_configs` function that collects files from adapters and stats them. Expose via new Tauri commands. Build a master-detail React page with a Zustand store.

**Tech Stack:** Rust (hk-core adapter/scanner), Tauri commands, React 19, Zustand, Tailwind CSS 4, lucide-react icons

---

## File Structure

### Rust (hk-core)

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `crates/hk-core/src/adapter/mod.rs` | Add 7 new default methods to `AgentAdapter` trait |
| Modify | `crates/hk-core/src/adapter/claude.rs` | Implement config file methods for Claude |
| Modify | `crates/hk-core/src/adapter/cursor.rs` | Implement config file methods for Cursor |
| Modify | `crates/hk-core/src/adapter/codex.rs` | Implement config file methods for Codex |
| Modify | `crates/hk-core/src/adapter/gemini.rs` | Implement config file methods for Gemini |
| Modify | `crates/hk-core/src/adapter/copilot.rs` | Implement config file methods for Copilot |
| Modify | `crates/hk-core/src/adapter/antigravity.rs` | Implement config file methods for Antigravity |
| Modify | `crates/hk-core/src/models.rs` | Add `AgentConfigFile`, `ConfigCategory`, `ConfigScope`, `AgentDetail` |
| Modify | `crates/hk-core/src/scanner.rs` | Add `scan_agent_configs` function |

### Rust (hk-desktop)

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `crates/hk-desktop/src/commands.rs` | Add `list_agent_configs` and `read_config_file_preview` commands |
| Modify | `crates/hk-desktop/src/main.rs` | Register new commands |

### Frontend

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src/lib/types.ts` | Add `AgentConfigFile`, `ConfigCategory`, `ConfigScope`, `AgentDetail` types |
| Modify | `src/lib/invoke.ts` | Add `listAgentConfigs` and `readConfigFilePreview` API wrappers |
| Create | `src/stores/agent-config-store.ts` | Zustand store for agent config state |
| Create | `src/pages/agents.tsx` | Agents page with master-detail layout |
| Create | `src/components/agents/agent-list.tsx` | Left panel agent list |
| Create | `src/components/agents/agent-detail.tsx` | Right panel detail view |
| Create | `src/components/agents/config-section.tsx` | Collapsible config category section |
| Create | `src/components/agents/config-file-entry.tsx` | File row with expand/collapse and preview |
| Create | `src/components/agents/extensions-summary-card.tsx` | Compact extension counts with link |
| Modify | `src/App.tsx` | Add `/agents` route |
| Modify | `src/components/layout/sidebar.tsx` | Add Agents nav item |

---

## Task 1: Data Models (Rust)

**Files:**
- Modify: `crates/hk-core/src/models.rs:241` (before the `#[cfg(test)]` block)

- [ ] **Step 1: Write tests for new models**

Add at the end of the existing `mod tests` block in `crates/hk-core/src/models.rs`:

```rust
#[test]
fn test_config_category_as_str() {
    assert_eq!(ConfigCategory::Rules.as_str(), "rules");
    assert_eq!(ConfigCategory::Memory.as_str(), "memory");
    assert_eq!(ConfigCategory::Settings.as_str(), "settings");
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core test_config_category_as_str test_config_scope_serialization 2>&1`
Expected: FAIL — `ConfigCategory` and `ConfigScope` not defined

- [ ] **Step 3: Implement models**

Insert before the `#[cfg(test)]` line (line 243) in `crates/hk-core/src/models.rs`:

```rust
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigCategory {
    Rules,
    Memory,
    Settings,
    Ignore,
}

impl ConfigCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Memory => "memory",
            Self::Settings => "settings",
            Self::Ignore => "ignore",
        }
    }

    pub fn order(&self) -> u8 {
        match self {
            Self::Rules => 0,
            Self::Memory => 1,
            Self::Settings => 2,
            Self::Ignore => 3,
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
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core test_config_category_as_str test_config_scope_serialization 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add crates/hk-core/src/models.rs
git commit -m "feat(models): add AgentConfigFile, ConfigCategory, ConfigScope, AgentDetail types"
```

---

## Task 2: Extend AgentAdapter Trait

**Files:**
- Modify: `crates/hk-core/src/adapter/mod.rs:36-47` (the trait definition)

- [ ] **Step 1: Write test for new trait methods**

Add inside the existing `mod tests` block in `crates/hk-core/src/adapter/mod.rs`:

```rust
#[test]
fn test_default_config_methods_return_empty() {
    // All adapters should have the new methods available with empty defaults
    let adapters = all_adapters();
    for a in &adapters {
        // Default implementations return empty vecs — the test
        // just verifies the methods exist and don't panic
        let _ = a.global_rules_files();
        let _ = a.global_memory_files();
        let _ = a.global_settings_files();
        let _ = a.project_rules_patterns();
        let _ = a.project_memory_patterns();
        let _ = a.project_settings_patterns();
        let _ = a.project_ignore_patterns();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core test_default_config_methods_return_empty 2>&1`
Expected: FAIL — methods not defined on trait

- [ ] **Step 3: Add methods to trait**

In `crates/hk-core/src/adapter/mod.rs`, add these default methods inside the `AgentAdapter` trait (after the `read_plugins` line, before the closing `}`):

```rust
    // --- Config file discovery (for Agents page) ---

    /// Global rule/instruction files (absolute paths, e.g. ~/.claude/CLAUDE.md)
    fn global_rules_files(&self) -> Vec<PathBuf> { vec![] }

    /// Global memory files (absolute paths)
    fn global_memory_files(&self) -> Vec<PathBuf> { vec![] }

    /// Global settings files (absolute paths, e.g. ~/.claude/settings.json)
    fn global_settings_files(&self) -> Vec<PathBuf> { vec![] }

    /// Relative paths/globs for rules within a project dir (e.g. "CLAUDE.md")
    fn project_rules_patterns(&self) -> Vec<String> { vec![] }

    /// Relative paths/globs for memory within a project dir
    fn project_memory_patterns(&self) -> Vec<String> { vec![] }

    /// Relative paths/globs for settings within a project dir
    fn project_settings_patterns(&self) -> Vec<String> { vec![] }

    /// Relative paths/globs for ignore files within a project dir
    fn project_ignore_patterns(&self) -> Vec<String> { vec![] }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core test_default_config_methods_return_empty 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add crates/hk-core/src/adapter/mod.rs
git commit -m "feat(adapter): add config file discovery methods to AgentAdapter trait"
```

---

## Task 3: Implement Config Methods for All 6 Adapters

**Files:**
- Modify: `crates/hk-core/src/adapter/claude.rs`
- Modify: `crates/hk-core/src/adapter/cursor.rs`
- Modify: `crates/hk-core/src/adapter/codex.rs`
- Modify: `crates/hk-core/src/adapter/gemini.rs`
- Modify: `crates/hk-core/src/adapter/copilot.rs`
- Modify: `crates/hk-core/src/adapter/antigravity.rs`

- [ ] **Step 1: Write test for Claude adapter config methods**

Add inside the existing test module of `crates/hk-core/src/adapter/claude.rs` (or create one if none exists):

```rust
#[test]
fn test_claude_config_methods() {
    let tmp = tempfile::tempdir().unwrap();
    let adapter = ClaudeAdapter::with_home(tmp.path().to_path_buf());

    let global_rules = adapter.global_rules_files();
    assert_eq!(global_rules.len(), 1);
    assert!(global_rules[0].ends_with("CLAUDE.md"));

    let global_settings = adapter.global_settings_files();
    assert_eq!(global_settings.len(), 1);
    assert!(global_settings[0].ends_with("settings.json"));

    let project_rules = adapter.project_rules_patterns();
    assert!(project_rules.contains(&"CLAUDE.md".to_string()));
    assert!(project_rules.contains(&".claude/CLAUDE.md".to_string()));

    let project_settings = adapter.project_settings_patterns();
    assert!(project_settings.contains(&".claude/settings.json".to_string()));
    assert!(project_settings.contains(&".claude/settings.local.json".to_string()));

    let project_ignore = adapter.project_ignore_patterns();
    assert!(project_ignore.contains(&".claudeignore".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core test_claude_config_methods 2>&1`
Expected: FAIL — returns empty vecs (default implementations)

- [ ] **Step 3: Implement Claude adapter config methods**

Add to the `impl AgentAdapter for ClaudeAdapter` block in `crates/hk-core/src/adapter/claude.rs`:

```rust
    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("CLAUDE.md")]
    }

    fn global_memory_files(&self) -> Vec<PathBuf> {
        // Claude stores memory per-project in ~/.claude/projects/<hash>/memory/*.md
        let projects_dir = self.base_dir().join("projects");
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.flatten() {
                let memory_dir = entry.path().join("memory");
                if memory_dir.is_dir() {
                    if let Ok(mem_entries) = std::fs::read_dir(&memory_dir) {
                        for mem_entry in mem_entries.flatten() {
                            let p = mem_entry.path();
                            if p.extension().is_some_and(|e| e == "md") {
                                files.push(p);
                            }
                        }
                    }
                }
            }
        }
        files
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("settings.json")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            "CLAUDE.md".into(),
            ".claude/CLAUDE.md".into(),
        ]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        vec![]  // Claude memory is global (~/.claude/projects/), not in project dir
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![
            ".claude/settings.json".into(),
            ".claude/settings.local.json".into(),
        ]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".claudeignore".into()]
    }
```

- [ ] **Step 4: Implement Cursor adapter config methods**

Add to the `impl AgentAdapter for CursorAdapter` block in `crates/hk-core/src/adapter/cursor.rs`:

```rust
    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("settings.json")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            ".cursorrules".into(),
            ".cursor/rules/*.mdc".into(),
        ]
    }

    fn project_memory_patterns(&self) -> Vec<String> {
        vec![".cursor/notepads/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![".cursor/settings.json".into()]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".cursorignore".into()]
    }
```

- [ ] **Step 5: Implement Codex adapter config methods**

Add to the `impl AgentAdapter for CodexAdapter` block in `crates/hk-core/src/adapter/codex.rs`:

```rust
    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("instructions.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("config.yaml")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec!["AGENTS.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![]
    }
```

- [ ] **Step 6: Implement Gemini adapter config methods**

Add to the `impl AgentAdapter for GeminiAdapter` block in `crates/hk-core/src/adapter/gemini.rs`:

```rust
    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("settings.json")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec!["GEMINI.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![]
    }
```

- [ ] **Step 7: Implement Copilot adapter config methods**

Add to the `impl AgentAdapter for CopilotAdapter` block in `crates/hk-core/src/adapter/copilot.rs`:

```rust
    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            ".github/copilot-instructions.md".into(),
            ".github/copilot/*.md".into(),
        ]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![".copilotignore".into()]
    }
```

- [ ] **Step 8: Implement Antigravity adapter config methods**

Add to the `impl AgentAdapter for AntigravityAdapter` block in `crates/hk-core/src/adapter/antigravity.rs`:

```rust
    fn global_settings_files(&self) -> Vec<PathBuf> {
        vec![self.base_dir().join("settings.json")]
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![".antigravity/rules/*.md".into()]
    }

    fn project_settings_patterns(&self) -> Vec<String> {
        vec![]
    }

    fn project_ignore_patterns(&self) -> Vec<String> {
        vec![]
    }
```

- [ ] **Step 9: Run all tests**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core 2>&1`
Expected: All tests PASS

- [ ] **Step 10: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add crates/hk-core/src/adapter/
git commit -m "feat(adapter): implement config file discovery for all 6 agents"
```

---

## Task 4: Scanner — `scan_agent_configs`

**Files:**
- Modify: `crates/hk-core/src/scanner.rs` (add new function at the end, before tests)

- [ ] **Step 1: Write test for scan_agent_configs**

Add to the test module (or create one) at the end of `crates/hk-core/src/scanner.rs`:

```rust
#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::adapter::claude::ClaudeAdapter;
    use std::fs;

    #[test]
    fn test_scan_agent_configs_global_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        // Create a global CLAUDE.md
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("CLAUDE.md"), "# Rules\nUse Rust.").unwrap();
        fs::write(claude_dir.join("settings.json"), "{}").unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let configs = scan_agent_configs(&adapter, &[]);

        // Should find CLAUDE.md and settings.json
        let rules: Vec<_> = configs.iter().filter(|c| c.category == ConfigCategory::Rules).collect();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].file_name, "CLAUDE.md");
        assert!(matches!(rules[0].scope, ConfigScope::Global));

        let settings: Vec<_> = configs.iter().filter(|c| c.category == ConfigCategory::Settings).collect();
        assert_eq!(settings.len(), 1);
    }

    #[test]
    fn test_scan_agent_configs_project_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let project = tmp.path().join("myproject");

        // Create global dir so adapter is valid
        fs::create_dir_all(home.join(".claude")).unwrap();

        // Create project with CLAUDE.md and .claudeignore
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::write(project.join("CLAUDE.md"), "# Project rules").unwrap();
        fs::write(project.join(".claudeignore"), "node_modules/").unwrap();
        fs::write(project.join(".claude").join("settings.json"), "{}").unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let projects = vec![(
            "myproject".to_string(),
            project.to_string_lossy().to_string(),
        )];
        let configs = scan_agent_configs(&adapter, &projects);

        let project_rules: Vec<_> = configs.iter()
            .filter(|c| c.category == ConfigCategory::Rules && matches!(&c.scope, ConfigScope::Project { .. }))
            .collect();
        assert_eq!(project_rules.len(), 1);

        let ignores: Vec<_> = configs.iter()
            .filter(|c| c.category == ConfigCategory::Ignore)
            .collect();
        assert_eq!(ignores.len(), 1);
        assert_eq!(ignores[0].file_name, ".claudeignore");
    }

    #[test]
    fn test_scan_agent_configs_skips_missing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        // Create base dir but NO config files
        fs::create_dir_all(home.join(".claude")).unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let configs = scan_agent_configs(&adapter, &[]);

        // Nothing should be returned since files don't exist
        assert!(configs.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core config_tests 2>&1`
Expected: FAIL — `scan_agent_configs` not defined

- [ ] **Step 3: Implement scan_agent_configs**

Add to `crates/hk-core/src/scanner.rs` (before any `#[cfg(test)]` block):

```rust
use crate::models::{AgentConfigFile, ConfigCategory, ConfigScope};

/// Scan an agent adapter for config files (rules, memory, settings, ignore).
/// `projects` is a list of (project_name, project_path) pairs.
pub fn scan_agent_configs(
    adapter: &dyn AgentAdapter,
    projects: &[(String, String)],
) -> Vec<AgentConfigFile> {
    let mut configs = Vec::new();

    // --- Global files ---
    let global_groups: [(ConfigCategory, Vec<std::path::PathBuf>); 3] = [
        (ConfigCategory::Rules, adapter.global_rules_files()),
        (ConfigCategory::Memory, adapter.global_memory_files()),
        (ConfigCategory::Settings, adapter.global_settings_files()),
    ];

    for (category, paths) in &global_groups {
        for path in paths {
            if let Some(cf) = stat_config_file(path, adapter.name(), *category, ConfigScope::Global) {
                configs.push(cf);
            }
        }
    }

    // --- Project files ---
    let project_groups: [(ConfigCategory, Vec<String>); 4] = [
        (ConfigCategory::Rules, adapter.project_rules_patterns()),
        (ConfigCategory::Memory, adapter.project_memory_patterns()),
        (ConfigCategory::Settings, adapter.project_settings_patterns()),
        (ConfigCategory::Ignore, adapter.project_ignore_patterns()),
    ];

    for (project_name, project_path) in projects {
        let project_root = std::path::Path::new(project_path);
        if !project_root.is_dir() { continue; }

        let scope = ConfigScope::Project {
            name: project_name.clone(),
            path: project_path.clone(),
        };

        for (category, patterns) in &project_groups {
            for pattern in patterns {
                let resolved = resolve_pattern(project_root, pattern);
                for path in resolved {
                    if let Some(cf) = stat_config_file(&path, adapter.name(), *category, scope.clone()) {
                        configs.push(cf);
                    }
                }
            }
        }
    }

    // Sort by category order, then by scope (global first), then by file name
    configs.sort_by(|a, b| {
        a.category.order().cmp(&b.category.order())
            .then_with(|| {
                let a_is_global = matches!(a.scope, ConfigScope::Global);
                let b_is_global = matches!(b.scope, ConfigScope::Global);
                b_is_global.cmp(&a_is_global) // global first
            })
            .then_with(|| a.file_name.cmp(&b.file_name))
    });

    configs
}

/// Stat a file and build an AgentConfigFile if it exists.
fn stat_config_file(
    path: &std::path::Path,
    agent: &str,
    category: ConfigCategory,
    scope: ConfigScope,
) -> Option<AgentConfigFile> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() { return None; }

    let modified_at = metadata.modified().ok().map(|t| {
        let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0).unwrap_or_default()
    });

    Some(AgentConfigFile {
        path: path.to_string_lossy().to_string(),
        agent: agent.to_string(),
        category,
        scope,
        file_name: path.file_name()?.to_string_lossy().to_string(),
        size_bytes: metadata.len(),
        modified_at,
    })
}

/// Resolve a pattern (possibly with glob `*`) against a project root.
/// Returns concrete file paths that exist on disk.
fn resolve_pattern(root: &std::path::Path, pattern: &str) -> Vec<std::path::PathBuf> {
    if pattern.contains('*') {
        // Glob expansion
        let full_pattern = root.join(pattern).to_string_lossy().to_string();
        glob::glob(&full_pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default()
    } else {
        // Direct path
        let path = root.join(pattern);
        if path.exists() { vec![path] } else { vec![] }
    }
}
```

- [ ] **Step 4: Add `glob` dependency to hk-core Cargo.toml**

Add to `crates/hk-core/Cargo.toml` under `[dependencies]`:

```toml
glob = "0.3"
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test -p hk-core config_tests 2>&1`
Expected: All 3 tests PASS

- [ ] **Step 6: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add crates/hk-core/src/scanner.rs crates/hk-core/Cargo.toml Cargo.lock
git commit -m "feat(scanner): add scan_agent_configs for config file discovery"
```

---

## Task 5: Tauri Commands

**Files:**
- Modify: `crates/hk-desktop/src/commands.rs`
- Modify: `crates/hk-desktop/src/main.rs:17-47` (invoke_handler registration)

- [ ] **Step 1: Add `list_agent_configs` command**

Add to `crates/hk-desktop/src/commands.rs`:

```rust
#[tauri::command]
pub fn list_agent_configs(state: State<AppState>) -> Result<Vec<AgentDetail>, String> {
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;

    // Collect registered projects as (name, path) pairs
    let projects: Vec<(String, String)> = store.list_projects()
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.name, p.path))
        .collect();

    let mut results = Vec::new();
    for a in &adapters {
        let (_, enabled) = store.get_agent_setting(a.name()).unwrap_or((None, true));
        let config_files = if a.detect() {
            scanner::scan_agent_configs(a.as_ref(), &projects)
        } else {
            vec![]
        };

        // Count extensions for this agent
        let extensions = store.list_extensions(None, Some(a.name())).unwrap_or_default();
        let extension_counts = ExtensionCounts {
            skill: extensions.iter().filter(|e| e.kind == ExtensionKind::Skill).count(),
            mcp: extensions.iter().filter(|e| e.kind == ExtensionKind::Mcp).count(),
            plugin: extensions.iter().filter(|e| e.kind == ExtensionKind::Plugin).count(),
            hook: extensions.iter().filter(|e| e.kind == ExtensionKind::Hook).count(),
        };

        results.push(AgentDetail {
            name: a.name().to_string(),
            detected: a.detect(),
            config_files,
            extension_counts,
        });
    }
    Ok(results)
}
```

- [ ] **Step 2: Add `read_config_file_preview` command**

Add to `crates/hk-desktop/src/commands.rs`:

```rust
#[tauri::command]
pub fn read_config_file_preview(path: String, max_lines: Option<usize>) -> Result<String, String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err("File not found".into());
    }
    if !file_path.is_file() {
        return Err("Path is not a file".into());
    }

    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let limit = max_lines.unwrap_or(30);
    let preview: String = content
        .lines()
        .take(limit)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(preview)
}
```

- [ ] **Step 3: Add imports in commands.rs**

Ensure the imports at the top of `crates/hk-desktop/src/commands.rs` include the new model types. Update line 1:

```rust
use hk_core::{adapter, auditor::{self, Auditor}, deployer, manager, models::*, scanner, store::Store};
```

This already imports `*` from models, so `AgentDetail`, `AgentConfigFile`, `ExtensionCounts`, `ConfigCategory`, and `ConfigScope` will be available.

- [ ] **Step 4: Register new commands in main.rs**

In `crates/hk-desktop/src/main.rs`, add to the `invoke_handler` list (inside `tauri::generate_handler![]`):

```rust
            commands::list_agent_configs,
            commands::read_config_file_preview,
```

- [ ] **Step 5: Verify it compiles**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo build -p hk-desktop 2>&1`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add crates/hk-desktop/src/commands.rs crates/hk-desktop/src/main.rs
git commit -m "feat(desktop): add list_agent_configs and read_config_file_preview commands"
```

---

## Task 6: Frontend Types and API

**Files:**
- Modify: `src/lib/types.ts:92` (after `AgentInfo` interface)
- Modify: `src/lib/invoke.ts:119` (before closing `}`)

- [ ] **Step 1: Add TypeScript types**

Add after line 92 (after `AgentInfo` interface) in `src/lib/types.ts`:

```typescript
export type ConfigCategory = "rules" | "memory" | "settings" | "ignore";

export type ConfigScope =
  | { type: "global" }
  | { type: "project"; name: string; path: string };

export interface AgentConfigFile {
  path: string;
  agent: string;
  category: ConfigCategory;
  scope: ConfigScope;
  file_name: string;
  size_bytes: number;
  modified_at: string | null;
}

export interface ExtensionCounts {
  skill: number;
  mcp: number;
  plugin: number;
  hook: number;
}

export interface AgentDetail {
  name: string;
  detected: boolean;
  config_files: AgentConfigFile[];
  extension_counts: ExtensionCounts;
}

/** Display labels for config categories. */
export const CONFIG_CATEGORY_LABELS: Record<ConfigCategory, string> = {
  rules: "Rules",
  memory: "Memory",
  settings: "Settings",
  ignore: "Ignore",
};
```

- [ ] **Step 2: Add API wrappers**

Add before the closing `};` in `src/lib/invoke.ts`:

```typescript
  listAgentConfigs(): Promise<AgentDetail[]> {
    return invoke("list_agent_configs");
  },

  readConfigFilePreview(path: string, maxLines?: number): Promise<string> {
    return invoke("read_config_file_preview", { path, maxLines });
  },
```

Update the import at the top of `src/lib/invoke.ts` to include the new type:

```typescript
import type { Extension, ExtensionContent, AgentInfo, AgentDetail, DashboardStats, AuditResult, UpdateStatus, MarketplaceItem, SkillAuditInfo, Project, DiscoveredProject, InstallResult, FileEntry } from "./types";
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd /Users/zoe/Documents/code/harnesskit && npx tsc --noEmit 2>&1`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add src/lib/types.ts src/lib/invoke.ts
git commit -m "feat(frontend): add AgentDetail types and API wrappers"
```

---

## Task 7: Zustand Store

**Files:**
- Create: `src/stores/agent-config-store.ts`

- [ ] **Step 1: Create the store**

Create `src/stores/agent-config-store.ts`:

```typescript
import { create } from "zustand";
import type { AgentDetail } from "@/lib/types";
import { api } from "@/lib/invoke";
import { toast } from "@/stores/toast-store";

interface AgentConfigState {
  agentDetails: AgentDetail[];
  selectedAgent: string | null;
  expandedFiles: Set<string>;
  previewCache: Map<string, string>;
  loading: boolean;

  fetch: () => Promise<void>;
  selectAgent: (name: string) => void;
  toggleFile: (path: string) => void;
  fetchPreview: (path: string) => Promise<string>;
  openInEditor: (path: string) => Promise<void>;
  copyPath: (path: string) => Promise<void>;
}

export const useAgentConfigStore = create<AgentConfigState>((set, get) => ({
  agentDetails: [],
  selectedAgent: null,
  expandedFiles: new Set(),
  previewCache: new Map(),
  loading: false,

  async fetch() {
    set({ loading: true });
    try {
      const agentDetails = await api.listAgentConfigs();
      const current = get().selectedAgent;
      // Auto-select first detected agent if nothing is selected
      const firstDetected = agentDetails.find((a) => a.detected)?.name ?? null;
      set({
        agentDetails,
        selectedAgent: current && agentDetails.some((a) => a.name === current) ? current : firstDetected,
        loading: false,
      });
    } catch {
      set({ loading: false });
    }
  },

  selectAgent(name: string) {
    set({ selectedAgent: name, expandedFiles: new Set() });
  },

  toggleFile(path: string) {
    const expanded = new Set(get().expandedFiles);
    if (expanded.has(path)) {
      expanded.delete(path);
    } else {
      expanded.add(path);
      // Trigger preview fetch if not cached
      if (!get().previewCache.has(path)) {
        get().fetchPreview(path);
      }
    }
    set({ expandedFiles: expanded });
  },

  async fetchPreview(path: string) {
    if (get().previewCache.has(path)) {
      return get().previewCache.get(path)!;
    }
    try {
      const content = await api.readConfigFilePreview(path, 30);
      const cache = new Map(get().previewCache);
      cache.set(path, content);
      set({ previewCache: cache });
      return content;
    } catch {
      return "";
    }
  },

  async openInEditor(path: string) {
    try {
      await api.openInSystem(path);
    } catch {
      toast.error("Failed to open file");
    }
  },

  async copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      toast.success("Path copied");
    } catch {
      toast.error("Failed to copy path");
    }
  },
}));
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd /Users/zoe/Documents/code/harnesskit && npx tsc --noEmit 2>&1`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add src/stores/agent-config-store.ts
git commit -m "feat(store): add agent-config-store for Agents page state"
```

---

## Task 8: Frontend Components — Agent List and Detail

**Files:**
- Create: `src/components/agents/agent-list.tsx`
- Create: `src/components/agents/agent-detail.tsx`
- Create: `src/components/agents/config-section.tsx`
- Create: `src/components/agents/config-file-entry.tsx`
- Create: `src/components/agents/extensions-summary-card.tsx`

- [ ] **Step 1: Create agent-list.tsx**

Create `src/components/agents/agent-list.tsx`:

```tsx
import { clsx } from "clsx";
import { agentDisplayName } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";

export function AgentList() {
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);
  const selectAgent = useAgentConfigStore((s) => s.selectAgent);

  return (
    <div className="flex flex-col gap-0.5 p-2">
      <div className="px-3 py-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
        Agents
      </div>
      {agentDetails.map((agent) => {
        const isSelected = agent.name === selectedAgent;
        const itemCount = agent.config_files.length;

        return (
          <button
            key={agent.name}
            onClick={() => selectAgent(agent.name)}
            className={clsx(
              "flex flex-col items-start rounded-lg px-3 py-2.5 text-left transition-colors",
              isSelected
                ? "bg-accent text-accent-foreground"
                : agent.detected
                  ? "text-foreground/80 hover:bg-accent/50"
                  : "text-muted-foreground/50 cursor-default"
            )}
            disabled={!agent.detected}
          >
            <span className="text-[13px] font-medium">
              {agentDisplayName(agent.name)}
            </span>
            <span className="text-[10px] text-muted-foreground">
              {agent.detected ? `${itemCount} items` : "Not detected"}
            </span>
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Create config-file-entry.tsx**

Create `src/components/agents/config-file-entry.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ChevronRight, ExternalLink, Copy } from "lucide-react";
import { clsx } from "clsx";
import type { AgentConfigFile } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";

export function ConfigFileEntry({ file }: { file: AgentConfigFile }) {
  const expandedFiles = useAgentConfigStore((s) => s.expandedFiles);
  const toggleFile = useAgentConfigStore((s) => s.toggleFile);
  const fetchPreview = useAgentConfigStore((s) => s.fetchPreview);
  const openInEditor = useAgentConfigStore((s) => s.openInEditor);
  const copyPath = useAgentConfigStore((s) => s.copyPath);
  const previewCache = useAgentConfigStore((s) => s.previewCache);

  const isExpanded = expandedFiles.has(file.path);
  const [preview, setPreview] = useState<string | null>(null);

  useEffect(() => {
    if (isExpanded && !preview) {
      fetchPreview(file.path).then(setPreview);
    }
  }, [isExpanded, file.path, fetchPreview, preview]);

  // Also update from cache
  useEffect(() => {
    const cached = previewCache.get(file.path);
    if (cached !== undefined) setPreview(cached);
  }, [previewCache, file.path]);

  const scopeLabel = file.scope.type === "global"
    ? "Global"
    : file.scope.name;

  const scopePath = file.scope.type === "global"
    ? file.path.replace(file.file_name, "")
    : file.scope.path;

  const sizeLabel = file.size_bytes < 1024
    ? `${file.size_bytes} B`
    : `${(file.size_bytes / 1024).toFixed(1)} KB`;

  return (
    <div className="border-b border-border/50 last:border-b-0">
      <button
        onClick={() => toggleFile(file.path)}
        className={clsx(
          "flex w-full items-center justify-between px-4 py-2.5 text-left transition-colors hover:bg-accent/30",
          isExpanded && "bg-accent/20"
        )}
      >
        <div className="flex items-center gap-2 min-w-0">
          <ChevronRight
            size={14}
            className={clsx(
              "shrink-0 text-muted-foreground transition-transform",
              isExpanded && "rotate-90"
            )}
          />
          <span className="text-[13px] font-medium truncate">{file.file_name}</span>
          <span className="text-[11px] text-muted-foreground truncate">
            {scopeLabel} · {scopePath}
          </span>
        </div>
        <span className="text-[11px] text-muted-foreground shrink-0 ml-2">
          {sizeLabel}
        </span>
      </button>

      {isExpanded && (
        <div className="border-t border-border/30 bg-muted/30 px-4 py-3">
          {preview !== null ? (
            <pre className="text-[11px] leading-relaxed text-muted-foreground font-mono whitespace-pre-wrap max-h-[200px] overflow-y-auto mb-3">
              {preview || "(empty file)"}
            </pre>
          ) : (
            <div className="text-[11px] text-muted-foreground mb-3">Loading...</div>
          )}
          <div className="flex gap-2">
            <button
              onClick={(e) => { e.stopPropagation(); openInEditor(file.path); }}
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
            >
              <ExternalLink size={12} />
              Open in Editor
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); copyPath(file.path); }}
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
            >
              <Copy size={12} />
              Copy Path
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Create config-section.tsx**

Create `src/components/agents/config-section.tsx`:

```tsx
import type { AgentConfigFile, ConfigCategory } from "@/lib/types";
import { CONFIG_CATEGORY_LABELS } from "@/lib/types";
import { ConfigFileEntry } from "./config-file-entry";
import { FileText, Brain, Settings, EyeOff } from "lucide-react";

const CATEGORY_ICONS: Record<ConfigCategory, React.ElementType> = {
  rules: FileText,
  memory: Brain,
  settings: Settings,
  ignore: EyeOff,
};

export function ConfigSection({
  category,
  files,
}: {
  category: ConfigCategory;
  files: AgentConfigFile[];
}) {
  if (files.length === 0) return null;

  const Icon = CATEGORY_ICONS[category];
  const label = CONFIG_CATEGORY_LABELS[category];

  return (
    <div className="mb-5">
      <div className="flex items-center gap-2 mb-2 px-1">
        <Icon size={14} className="text-muted-foreground" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </span>
        <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded-full text-muted-foreground">
          {files.length}
        </span>
      </div>
      <div className="rounded-lg border border-border overflow-hidden">
        {files.map((file) => (
          <ConfigFileEntry key={file.path} file={file} />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Create extensions-summary-card.tsx**

Create `src/components/agents/extensions-summary-card.tsx`:

```tsx
import { useNavigate } from "react-router-dom";
import { Package, ArrowRight } from "lucide-react";
import type { ExtensionCounts } from "@/lib/types";

export function ExtensionsSummaryCard({
  counts,
  agentName,
}: {
  counts: ExtensionCounts;
  agentName: string;
}) {
  const navigate = useNavigate();
  const total = counts.skill + counts.mcp + counts.plugin + counts.hook;
  if (total === 0) return null;

  return (
    <div className="mb-5">
      <div className="flex items-center gap-2 mb-2 px-1">
        <Package size={14} className="text-muted-foreground" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          Extensions
        </span>
        <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded-full text-muted-foreground">
          {total}
        </span>
      </div>
      <button
        onClick={() => navigate(`/extensions?agent=${agentName}`)}
        className="w-full rounded-lg border border-border p-3.5 flex items-center justify-between transition-colors hover:bg-accent/30"
      >
        <div className="flex gap-4 text-[13px]">
          {counts.skill > 0 && <span><strong>{counts.skill}</strong> <span className="text-muted-foreground">Skills</span></span>}
          {counts.mcp > 0 && <span><strong>{counts.mcp}</strong> <span className="text-muted-foreground">MCP</span></span>}
          {counts.plugin > 0 && <span><strong>{counts.plugin}</strong> <span className="text-muted-foreground">Plugins</span></span>}
          {counts.hook > 0 && <span><strong>{counts.hook}</strong> <span className="text-muted-foreground">Hooks</span></span>}
        </div>
        <span className="flex items-center gap-1 text-[12px] font-medium text-primary">
          View in Extensions <ArrowRight size={14} />
        </span>
      </button>
    </div>
  );
}
```

- [ ] **Step 5: Create agent-detail.tsx**

Create `src/components/agents/agent-detail.tsx`:

```tsx
import { agentDisplayName, type ConfigCategory } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { ConfigSection } from "./config-section";
import { ExtensionsSummaryCard } from "./extensions-summary-card";

const CATEGORY_ORDER: ConfigCategory[] = ["rules", "memory", "settings", "ignore"];

export function AgentDetail() {
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);

  const agent = agentDetails.find((a) => a.name === selectedAgent);
  if (!agent) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
        Select an agent to view its configuration
      </div>
    );
  }

  // Group config files by category
  const byCategory = new Map<ConfigCategory, typeof agent.config_files>();
  for (const cat of CATEGORY_ORDER) {
    byCategory.set(cat, []);
  }
  for (const file of agent.config_files) {
    const list = byCategory.get(file.category);
    if (list) list.push(file);
  }

  // Collect unique scopes for badges
  const scopes = new Set<string>();
  for (const file of agent.config_files) {
    scopes.add(file.scope.type === "global" ? "Global" : file.scope.name);
  }

  return (
    <div className="flex-1 overflow-y-auto p-5">
      {/* Header */}
      <div className="flex items-start justify-between mb-6">
        <div>
          <h2 className="text-xl font-bold">{agentDisplayName(agent.name)}</h2>
          <p className="text-[12px] text-muted-foreground mt-0.5">
            {agent.detected ? "Detected" : "Not detected"}
          </p>
        </div>
        {scopes.size > 0 && (
          <div className="flex gap-1.5">
            {[...scopes].map((scope) => (
              <span
                key={scope}
                className="text-[11px] px-2 py-0.5 rounded-md border border-border bg-muted/50"
              >
                {scope}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Config sections */}
      {CATEGORY_ORDER.map((cat) => (
        <ConfigSection
          key={cat}
          category={cat}
          files={byCategory.get(cat) ?? []}
        />
      ))}

      {/* Extensions summary */}
      <ExtensionsSummaryCard
        counts={agent.extension_counts}
        agentName={agent.name}
      />
    </div>
  );
}
```

- [ ] **Step 6: Verify TypeScript compiles**

Run: `cd /Users/zoe/Documents/code/harnesskit && npx tsc --noEmit 2>&1`
Expected: No errors

- [ ] **Step 7: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add src/components/agents/
git commit -m "feat(ui): add Agents page components — list, detail, config sections"
```

---

## Task 9: Agents Page and Routing

**Files:**
- Create: `src/pages/agents.tsx`
- Modify: `src/App.tsx:9-13` (imports), `src/App.tsx:78-85` (routes)
- Modify: `src/components/layout/sidebar.tsx:1-10` (imports + nav items)

- [ ] **Step 1: Create agents page**

Create `src/pages/agents.tsx`:

```tsx
import { useEffect } from "react";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { AgentList } from "@/components/agents/agent-list";
import { AgentDetail } from "@/components/agents/agent-detail";

export default function AgentsPage() {
  const fetch = useAgentConfigStore((s) => s.fetch);
  const loading = useAgentConfigStore((s) => s.loading);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return (
    <div className="flex h-full">
      {/* Left panel — agent list */}
      <div className="w-[200px] shrink-0 border-r border-border overflow-y-auto">
        <AgentList />
      </div>

      {/* Right panel — agent detail */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
          Loading...
        </div>
      ) : (
        <AgentDetail />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Add route in App.tsx**

In `src/App.tsx`, add the import after line 13 (after the MarketplacePage import):

```typescript
import AgentsPage from "./pages/agents";
```

Then add the route after line 80 (after the `<Route index element={<OverviewPage />} />` line):

```tsx
          <Route path="agents" element={<AgentsPage />} />
```

- [ ] **Step 3: Add sidebar nav item**

In `src/components/layout/sidebar.tsx`, add the `Bot` icon import (line 2):

```typescript
import { LayoutDashboard, Bot, Package, Shield, Settings, ShoppingBag } from "lucide-react";
```

Then add the Agents item to `mainNavItems` array, between Overview and Extensions (after line 6):

```typescript
  { to: "/agents", icon: Bot, label: "Agents" },
```

So `mainNavItems` becomes:

```typescript
const mainNavItems = [
  { to: "/", icon: LayoutDashboard, label: "Overview" },
  { to: "/agents", icon: Bot, label: "Agents" },
  { to: "/extensions", icon: Package, label: "Extensions" },
  { to: "/audit", icon: Shield, label: "Audit" },
  { to: "/marketplace", icon: ShoppingBag, label: "Marketplace" },
];
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `cd /Users/zoe/Documents/code/harnesskit && npx tsc --noEmit 2>&1`
Expected: No errors

- [ ] **Step 5: Verify the app builds**

Run: `cd /Users/zoe/Documents/code/harnesskit && npm run build 2>&1`
Expected: Build succeeds

- [ ] **Step 6: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add src/pages/agents.tsx src/App.tsx src/components/layout/sidebar.tsx
git commit -m "feat: wire up Agents page with routing and sidebar navigation"
```

---

## Task 10: Handle Extensions Page Agent Filter from URL

**Files:**
- Modify: `src/pages/extensions.tsx` (read `?agent=` query param and apply filter)
- Modify: `src/stores/extension-store.ts` (ensure agent filter works with URL param)

- [ ] **Step 1: Add URL param reading to extensions page**

In `src/pages/extensions.tsx`, add at the top of the component function:

```typescript
import { useSearchParams } from "react-router-dom";
```

Then inside the `ExtensionsPage` component, add near the top:

```typescript
const [searchParams] = useSearchParams();
const agentFromUrl = searchParams.get("agent");
```

And use it to initialize the agent filter. In the `useEffect` that runs on mount, add:

```typescript
useEffect(() => {
  if (agentFromUrl) {
    setAgentFilter(agentFromUrl);
  }
}, [agentFromUrl]);
```

Where `setAgentFilter` is the existing store action for changing the agent filter. Check the existing extension store for the exact method name and wire it in.

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd /Users/zoe/Documents/code/harnesskit && npx tsc --noEmit 2>&1`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
cd /Users/zoe/Documents/code/harnesskit
git add src/pages/extensions.tsx
git commit -m "feat(extensions): support ?agent= URL param for cross-page navigation from Agents"
```

---

## Task 11: Full Build Verification

- [ ] **Step 1: Run all Rust tests**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo test --workspace 2>&1`
Expected: All tests PASS

- [ ] **Step 2: Run full Cargo build**

Run: `cd /Users/zoe/Documents/code/harnesskit && cargo build --workspace 2>&1`
Expected: Compiles successfully

- [ ] **Step 3: Run frontend build**

Run: `cd /Users/zoe/Documents/code/harnesskit && npm run build 2>&1`
Expected: Build succeeds with no errors

- [ ] **Step 4: Final commit if any fixes were needed**

If any compilation fixes were applied, commit them:

```bash
cd /Users/zoe/Documents/code/harnesskit
git add -A
git commit -m "fix: address compilation issues from Agents page integration"
```
