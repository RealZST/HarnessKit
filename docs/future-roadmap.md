# Future Roadmap

Features planned for future versions. Items here have been discussed and designed but are not yet implemented.

---

## New Agent Support

### OpenClaw Adapter
- Add `openclaw` to adapter list
- Config path: `~/.openclaw/`
- Skills dir: `~/.openclaw/skills/`
- Add to `AGENT_ORDER` and `AGENT_DISPLAY_NAMES` in `types.ts`

### Additional Agents to Evaluate
- Kiro, Vercel Skills, Qoder, Augment, Junie, iFlow, CommandCode, CodeBuddy, Cline, Crush, Pi, Droid, Qwen Code, ZenCoder, OpenCode, TRAE, Roo Code, Continue, Goose
- Reference: competitor supports 27 agents (see `~/.skills-manager/config.json` for full list with paths)

### Universal Agents and `~/.agents/skills/`

`~/.agents/skills/` is an emerging cross-agent convention (see agentskills.io). Some agents natively scan this directory — these are called "universal agents." When adding a new adapter, check whether it supports `~/.agents/skills/` and add it to `skill_dirs()` if so.

**Currently implemented** (have `~/.agents/skills` in their `skill_dirs()`):
- Codex (`adapter/codex.rs`)
- Cursor (`adapter/cursor.rs`)
- Gemini (`adapter/gemini.rs`)

**Known universal agents not yet added** (from skills.sh `src/agents.ts` `getUniversalAgents()`):
- Amp, Cline, Deep Agents, Firebender, Kimi CLI, OpenCode, Replit, Warp

**Known non-universal agents** (use their own paths, need symlinks from skills.sh):
- Claude Code (`~/.claude/skills/`)
- Windsurf (`~/.windsurf/skills/`)
- Goose (`~/.config/goose/skills/`)
- Roo Code (`~/.roo/skills/`)

**How to check**: Look at the agent's source code for references to `.agents/skills`, or check skills.sh `src/agents.ts` for the `getUniversalAgents()` list. The `~/.agents/` directory itself has no metadata indicating which agents read from it.

---

## Scenarios / Profiles

Context-based skill grouping — switch between skill sets for different workflows (e.g., "work" vs "personal", "ML research" vs "web dev").

### Design Notes
- Tags (backend already supports `tags_json` in DB, store has `updateTags`/`fetchTags`/`allTags`) can serve as the grouping mechanism for scenarios
- Scenario = a named filter that activates/deactivates skills by tag
- Switching a scenario batch-toggles `enabled` on matching extensions
- Competitor reference: skills-manager has `scenarios` + `scenario_skills` + `scenario_skill_tools` tables with per-scenario per-agent toggles

### Implementation Approach
1. Re-enable Tags UI in detail panel (removed from frontend, backend intact)
2. Add `scenarios` table to SQLite
3. Add scenario switcher in sidebar or header
4. Batch toggle extensions when switching scenarios

---

## Tags UI (Re-enable)

Tags editing was removed from the frontend to reduce clutter. Backend is fully intact:
- Store: `tagFilter`, `allTags`, `updateTags`, `fetchTags`, `setTagFilter`
- API: `update_tags`, `get_all_tags`
- DB: `tags_json` column in extensions table
- Filters component: `tagColor()`, `TAG_COLORS` still exported

Re-enable when Scenarios feature is implemented, as tags become the primary grouping mechanism.

---

## Enable/Disable Improvements

Followup items from the real enable/disable implementation (2026-03-29).

### Per-Agent Toggle

Currently toggling an extension affects all agents at once (the frontend iterates all instances in a group). The backend already supports per-agent granularity — each agent has its own extension ID and its own physical file/config entry. To enable per-agent toggle:

1. Add a per-agent toggle button in the Detail Panel (e.g., next to each agent badge)
2. Call `api.toggleExtension(singleInstanceId, enabled)` for just that one instance
3. Shared skills (`~/.agents/skills/`) will still link all agents (Strategy A) — only agent-specific skills support independent toggle

### Refactor: Eliminate Toggle Logic Duplication

Toggle logic is currently duplicated between `manager.rs` and `commands.rs` (the Tauri command inlines the same logic because `Manager` owns `Store` but `AppState` holds it behind a `Mutex`).

Fix options:
- (a) Refactor `Manager` to borrow `&Store` instead of owning it
- (b) Extract toggle logic into standalone functions that accept `&Store`, called from both Manager and commands
- (c) Change `AppState` to hold `Manager` instead of raw `Store`

### restore_hook Deduplication

`deployer::restore_hook` appends the hook entry without checking for duplicates. `deploy_hook` has dedup logic but `restore_hook` doesn't. Low risk since the disable path removes the hook first, but adding a dedup check would be more robust.

### Filesystem + DB Transactional Safety

Currently, if `toggle_skill` renames `SKILL.md` successfully but the subsequent `set_enabled` DB call fails, the system is in an inconsistent state. A compensating pattern (try DB first, then filesystem, rollback DB on filesystem failure) would be more robust.

### Multi-Agent Config Overwrite

In `toggle_mcp` / `toggle_hook` / `toggle_plugin`, the adapter loop calls `set_disabled_config` per adapter. If an extension somehow had multiple agents, the second adapter's saved config would overwrite the first. In practice this doesn't happen (scanned extensions always have one agent), but adding `debug_assert!(ext.agents.len() == 1)` or per-agent config storage would make the assumption explicit.

### Additional Edge Case Tests

The following scenarios are handled correctly in code but lack explicit test coverage:
- Toggle an already-disabled skill (should be a no-op)
- Toggle an already-enabled skill (should be a no-op)
- Toggle a skill with `source_path: None` (should error)
- Re-enable an MCP with no saved `disabled_config` (should error)
- Toggle when filesystem rename fails (permissions, disk full)

---

## CLI Marketplace — Automated GitHub Discovery

Daily background scan to discover new agent-oriented CLI tools on GitHub, supplementing the hardcoded CLI_REGISTRY.

### Design (from spec)

**Trigger:** On app startup, check `last_discovery_at` timestamp in SQLite. If > 24 hours ago, run discovery in background.

**GitHub Search API query:**
```
GET https://api.github.com/search/repositories?q=SKILL.md+in:path+cli+in:name+pushed:>{30_days_ago}&sort=stars&order=desc
```

**Filter rules:**

| Rule | Signal Strength |
|------|----------------|
| Repo contains SKILL.md with `requires.bins` referencing its own binary | Strong |
| Repo has `install-skills.sh` or equivalent | Strong |
| Repo topics include `agent-skills`, `agent-cli`, `claude-code` | Medium |
| README mentions "CLI" + ("Agent Skills" or "SKILL.md" or "Claude Code" or "Cursor") | Medium |

Match logic: 1 strong signal, or 2+ medium signals → candidate.

**Storage:** New `cli_candidates` table in SQLite (id, repo, name, stars, discovered_at, status: pending/approved/dismissed).

**UI:** "Discovered" section in Marketplace CLI Tools tab, below the curated trending list. User can dismiss or approve candidates. Approved entries are promoted to the main trending list.

**Rate limits:** GitHub API unauthenticated = 60 req/hour, authenticated = 5,000 req/hour. One daily scan is well within limits.

### Implementation Steps
1. Add `cli_candidates` table + `last_discovery_at` to SQLite schema
2. Add `discover_cli_candidates()` function in marketplace.rs
3. Add Tauri command + background task on app startup
4. Add "Discovered" UI section in marketplace CLI tab
5. Add approve/dismiss actions

---

## Cloud Sync / Backup

Git-based backup of skill library (similar to competitor's approach):
- Configure a remote Git repo for backup
- Push/pull skill metadata + configuration
- Version history with snapshot tags
- Device ID tracking for multi-device sync
