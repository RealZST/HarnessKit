# CLI Extension Type Design

**Date:** 2026-03-30
**Status:** Approved

## Context

A wave of agent-oriented CLI tools (wecom-cli, lark-cli, dingtalk dws, meitu-cli, OfficeCLI, CLI-Anything) has emerged as a new extension paradigm for AI coding agents. These CLIs are distinct from MCP servers and pure skills: they are installable binaries that ship with SKILL.md files teaching agents how to invoke them. HarnessKit currently tracks four extension kinds (Skill, Mcp, Plugin, Hook) and needs a fifth — CLI — to manage this growing category.

## Design Decisions

| Decision | Outcome |
|----------|---------|
| What CLI represents | The binary itself, as a parent entity of its associated skills |
| Discovery mechanism | Dual-path: reverse discovery from SKILL.md `requires.bins` + known CLI registry scan |
| CLI-Skill relationship | Hard link via `cli_parent_id` field; cascade enable/disable/delete |
| Inclusion criteria | Only agent-oriented CLIs — the CLI vendor must ship official agent integration (see Inclusion Criteria section) |
| Audit dimensions | Credential storage, network access, binary source, permission scope, aggregate risk |
| List UI treatment | Uniform columns with all other kinds; CLI-specific info in detail view only |

## Inclusion Criteria: What Makes a CLI "Agent-Oriented"

The criterion is not "does an agent use this CLI" but **"did the CLI vendor build agent integration"**.

An agent-oriented CLI is one where the vendor themselves provides:
- Official SKILL.md files (with `requires.bins` pointing to themselves), OR
- An agent skills installation mechanism (e.g., `install-skills.sh`, `npx skills add`), OR
- Agent-specific features (e.g., `dws schema` for runtime tool discovery), OR
- A built-in MCP server mode alongside the CLI

Traditional CLIs like `gh`, `aws`, `docker`, `kubectl` are NOT agent-oriented CLIs even though agents use them daily. The key difference: their vendors (GitHub, AWS, Google, Kubernetes) have not shipped official SKILL.md files or agent integration layers. If a third party writes a skill that references `gh` via `requires.bins`, that does not make `gh` an agent CLI — the skill author is not the CLI vendor.

**The KNOWN_CLIS registry codifies this editorial judgment.** The `requires.bins` reverse-discovery mechanism helps surface new candidates, but a CLI is only recognized as an agent-oriented extension when it meets the vendor-ships-agent-integration criterion (either by being in KNOWN_CLIS or by shipping its own SKILL.md with self-referencing `requires.bins`).

## 1. Data Model

### Rust (models.rs)

**ExtensionKind** gains a `Cli` variant:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionKind {
    Skill,
    Mcp,
    Plugin,
    Hook,
    Cli,
}
```

**New struct — CliMeta:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliMeta {
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub install_method: Option<String>,   // npm | pip | curl | brew | manual
    pub credentials_path: Option<String>,
    pub version: Option<String>,
    pub api_domains: Vec<String>,
}
```

**Extension struct** adds two fields:

```rust
pub struct Extension {
    // ... existing fields unchanged ...
    pub cli_parent_id: Option<String>,  // child skill → parent CLI
    pub cli_meta: Option<CliMeta>,      // only populated when kind == Cli
}
```

### TypeScript (types.ts)

```typescript
export type ExtensionKind = "skill" | "mcp" | "plugin" | "hook" | "cli";

export interface CliMeta {
  binary_name: string;
  binary_path: string | null;
  install_method: string | null;
  credentials_path: string | null;
  version: string | null;
  api_domains: string[];
}

export interface Extension {
  // ... existing fields unchanged ...
  cli_parent_id: string | null;
  cli_meta: CliMeta | null;
}
```

### DashboardStats

Both Rust `DashboardStats` and TypeScript `ExtensionCounts` gain a `cli_count` / `cli` field.

## 2. Scanner / Discovery

### New function: scan_cli_binaries()

Runs once per scan cycle (global, not per-adapter).

**Phase 1 — Reverse discovery from installed skills:**

Parse SKILL.md frontmatter for `metadata.requires.bins` (per the Agent Skills spec used by wecom-cli, lark-cli, dws, meitu). Also regex-match skill content for known binary invocation patterns as fallback.

Example frontmatter:
```yaml
---
name: wecomcli-send-message
description: Send messages via WeChat Work
metadata:
  requires:
    bins: ["wecom-cli"]
---
```

**Phase 2 — Known CLI registry scan:**

Hardcoded registry of agent-oriented CLIs:

```rust
struct KnownCli {
    binary_name: &'static str,
    display_name: &'static str,
    api_domains: &'static [&'static str],
    credentials_path: Option<&'static str>,
}

const KNOWN_CLIS: &[KnownCli] = &[
    KnownCli {
        binary_name: "wecom-cli",
        display_name: "WeChat Work CLI",
        api_domains: &["qyapi.weixin.qq.com"],
        credentials_path: Some("~/.config/wecom/bot.enc"),
    },
    KnownCli {
        binary_name: "lark-cli",
        display_name: "Lark / Feishu CLI",
        api_domains: &["open.feishu.cn", "open.larksuite.com"],
        credentials_path: Some("~/.config/lark/credentials"),
    },
    KnownCli {
        binary_name: "dws",
        display_name: "DingTalk Workspace CLI",
        api_domains: &["api.dingtalk.com"],
        credentials_path: Some("~/.config/dws/auth.json"),
    },
    KnownCli {
        binary_name: "meitu",
        display_name: "Meitu CLI",
        api_domains: &["openapi.mtlab.meitu.com"],
        credentials_path: Some("~/.meitu/credentials.json"),
    },
    KnownCli {
        binary_name: "officecli",
        display_name: "OfficeCLI",
        api_domains: &[],
        credentials_path: None,
    },
];
```

**Combined flow:**

1. Collect candidate binary names (Phase 1 `requires.bins` ∪ Phase 2 `KNOWN_CLIS`)
2. For each candidate:
   - `which <binary>` → binary_path (None if not found)
   - `<binary> --version` → version (None on failure)
   - Look up KNOWN_CLIS for supplementary metadata
   - Build CliMeta
   - Auto-derive permissions from CliMeta:
     - `api_domains` → `Permission::Network { domains }`
     - `credentials_path` → `Permission::FileSystem { paths }`
     - binary exists → `Permission::Shell { commands: [binary_name] }`
   - Create Extension with `kind: Cli`
3. Back-fill: set `cli_parent_id` on all skills whose `requires.bins` matches

**Stable ID:** `cli_stable_id(binary_name)` uses `"cli::{binary_name}"` as the hash input (not agent-scoped, since CLIs are global).

### scan_all modification

```rust
pub fn scan_all(adapters, store) -> Vec<Extension> {
    let mut all = vec![];

    // Existing: scan each adapter
    for adapter in adapters {
        all.extend(scan_skill_dir(...));
        all.extend(scan_mcp_servers(...));
        all.extend(scan_hooks(...));
        all.extend(scan_plugins(...));
    }

    // New: scan CLIs (global, once)
    let clis = scan_cli_binaries(&all);

    // Back-fill cli_parent_id on child skills
    for ext in &mut all {
        if ext.kind == ExtensionKind::Skill {
            if let Some(cli) = find_parent_cli(&clis, ext) {
                ext.cli_parent_id = Some(cli.id.clone());
            }
        }
    }

    all.extend(clis);
    all
}
```

### SKILL.md frontmatter parsing enhancement

Existing `scan_skill_dir` parses `name` and `description` from frontmatter. Add parsing of `metadata.requires.bins` (a string array). If the field is absent, fall back to regex content matching against known binary names.

## 3. Store (SQLite)

### Schema changes

```sql
ALTER TABLE extensions ADD COLUMN cli_parent_id TEXT REFERENCES extensions(id) ON DELETE SET NULL;
ALTER TABLE extensions ADD COLUMN cli_meta_json TEXT;
```

`ON DELETE SET NULL`: deleting a CLI orphans child skills (sets their cli_parent_id to null) rather than cascading a database delete. Actual cascade deletion is handled by the Manager layer.

### New methods

```rust
fn get_child_skills(&self, cli_id: &str) -> Result<Vec<Extension>>
fn link_skills_to_cli(&self, cli_id: &str, skill_ids: &[String]) -> Result<()>
fn unlink_cli_children(&self, cli_id: &str) -> Result<()>
```

### Existing method adaptations

- **insert_extension**: write `cli_parent_id` and `cli_meta_json`; preserve user-set fields on conflict
- **list_extensions**: kind filter supports `"cli"`; add optional `cli_parent_id` filter parameter
- **sync_extensions**: CLI participates in sync (scanned → keep, not scanned → stale)
- **get_dashboard_stats**: add `cli_count`

### Migration

On Store initialization, check whether `cli_parent_id` column exists. If not, run ALTER TABLE. Consistent with the existing `CREATE TABLE IF NOT EXISTS` pattern.

## 4. Auditor

### AuditInput extension

```rust
pub struct AuditInput {
    // ... existing fields ...
    pub cli_meta: Option<CliMeta>,
    pub child_skills: Vec<Extension>,
}
```

### 5 new audit rules (CLI-only)

**CliCredentialStorage (High)**
- Check credentials file permissions (should be 0600, not 0644+)
- Detect plaintext vs encrypted credential files
- Flag unknown credential storage location for CLIs known to require auth

**CliNetworkAccess (Medium)**
- Report connected external API domains from `api_domains`
- Flag when domain count > 3 (broad network surface)

**CliBinarySource (High)**
- npm/pip from official registry → more trusted
- curl (direct binary download) → higher risk
- binary_path outside standard locations → flag anomaly
- Unknown install method → flag "unknown source"

**CliPermissionScope (Medium)**
- Analyze child skills' descriptions to count business domains covered (contacts, messaging, calendar, docs, etc.)
- Flag when domain count > 5 (broad business permissions)

**CliAggregateRisk (Medium/High)**
- Collect all child skills' permissions (Network + Shell + FileSystem)
- Detect risky combinations: "read contacts" skill + "send messages" skill → potential data exfiltration channel
- Flag when individual child trust_scores are high but combined permission scope is broad

### Scoring

Standard deduction model: base 100, subtract by severity (Critical -25, High -15, Medium -8, Low -3). Maps to existing TrustTier (Safe 80+, LowRisk 60-79, NeedsReview 0-59).

## 5. Manager (Lifecycle)

### toggle_cli

```rust
fn toggle_cli(&self, ext: &Extension, enabled: bool) -> Result<()> {
    let children = self.store.get_child_skills(&ext.id)?;
    for child in &children {
        self.toggle_skill(child, enabled)?;
        self.store.set_enabled(&child.id, enabled)?;
    }
    self.store.set_enabled(&ext.id, enabled)?;
    Ok(())
}
```

Disabling a CLI cascades to all child skills. The binary itself is not touched — agents lose awareness because the SKILL.md files are disabled.

### delete_cli

Deletes all child skills (reusing existing skill deletion logic), then removes the CLI record from the database. Does NOT uninstall the binary — that is the user's responsibility via their package manager.

### install_cli

Executes the install command (e.g., `npm install -g @wecom/cli`), then installs skills via `install_from_git` or `npx skills add`. Requires user confirmation in UI since it runs a system-level command. Triggers `scan_and_sync` after completion.

## 6. Tauri Commands (IPC)

### New commands

```rust
get_cli_with_children(cli_id: String) -> (Extension, Vec<Extension>)
toggle_cli(cli_id: String, enabled: bool) -> ()
delete_cli(cli_id: String) -> ()
install_cli(binary_name: String, install_command: String, skills_repo: String, target_agents: Vec<String>) -> ()
```

### Existing commands

Zero or minimal changes. `toggle_extension` and `delete_extension` delegate to Manager which handles CLI cascade internally. `list_extensions`, `get_dashboard_stats`, `run_audit`, `scan_and_sync` all work through the existing kind-aware plumbing.

## 7. Frontend

### KindBadge

Add CLI entry: orange color scheme, label "CLI", title "Agent-oriented CLI tool — binary + skills bundle".

CSS variable `--kind-cli` defined in `index.css` (orange, filling the gap in the existing blue/purple/green/red palette).

### Extensions page — list view

CLI entries use the exact same column structure as all other kinds:
- Kind badge (CLI, orange)
- Name
- Permission tags (Network / FileSystem / Shell — auto-derived from CliMeta)
- Trust score badge
- Toggle switch

No separate section or special columns.

### Extensions page — detail view

When a CLI entry is selected, the detail panel shows CLI-specific info:
- CliMeta: binary_path, version, install_method, credentials_path, api_domains
- Child skills list (individually toggleable only when the parent CLI is enabled; when CLI is disabled, all children are force-disabled)
- Audit findings
- Cascade toggle hint: "This will also disable N associated skills"

### Overview page

New CLI count stat card alongside existing skill/mcp/plugin/hook cards.

### Audit page

No structural changes. CLI audit findings enter the same results list.

### invoke.ts

New wrappers for `get_cli_with_children` and `install_cli`. Existing `toggleExtension` and `deleteExtension` unchanged — backend handles cascade.

### Extension store

- Kind filter gains `"cli"` option
- CLI extensions do not participate in cross-agent grouping (they are global)
- New getter: `childSkillsOf(cliId)` filters skills by `cli_parent_id`
