//! Scan synchronization: bulk upsert + stale-row pruning, install-meta heal/backfill.

use super::*;
use crate::scanner::ScanResult;

impl Store {
    /// Does this row's file still exist on disk, in either toggle state?
    ///
    /// HarnessKit disables file-backed extensions by renaming them with a
    /// `.disabled` suffix, and for a disabled skill the scanner deliberately
    /// records the *enabled* filename so the row's id survives toggling (see
    /// `scanner::scan_skill_dir`). Testing `source_path` alone would therefore
    /// report every disabled skill as missing. Check both names so "still on
    /// disk" means what it says.
    fn source_still_on_disk(source_path: &str) -> bool {
        Path::new(source_path).exists() || Path::new(&format!("{source_path}.disabled")).exists()
    }
    /// Decide whether a stale extension row (one absent from the latest scan)
    /// should be pruned from the store.
    ///
    /// `agent_scanned` says whether the scan visited the row's agent at all.
    /// When it did not (the agent failed detection, or HarnessKit no longer
    /// knows it), every exemption below except store-held disabled state is
    /// void and the row goes. Keeping it would show extensions for an agent
    /// that is not there, fed by files in directories shared with other
    /// agents such as `~/.agents/skills`.
    ///
    /// Kept (returns false):
    /// - disabled rows whose state the store alone holds — `disabled_config` is
    ///   where an MCP/hook entry goes once it is removed from the agent's config
    ///   file, and where a renamed plugin manifest's path is recorded. Pruning
    ///   those would destroy the only copy, so they are exempt even when the
    ///   agent was not visited (detection can fail transiently). This is the
    ///   same predicate `UPSERT_EXTENSION_SQL` uses to stop a scan from
    ///   overwriting `enabled`;
    /// - CLI extensions with install_meta — their binary can transiently fail
    ///   detection on startup, so one missing scan isn't proof of removal;
    /// - file-backed install_meta rows still on disk (or whose path is unknown)
    ///   — a momentary scan gap, not a real uninstall.
    ///
    /// Pruned (returns true): everything else that is gone, including skill and
    /// plugin rows with install_meta whose files the user deleted (e.g.
    /// `rm -rf ~/.claude`) — otherwise they linger forever as ghost rows.
    ///
    /// Disabled *skills* fall in that last group by design: they carry no
    /// `disabled_config`, and a disabled skill still on disk is reported by the
    /// scanner as `SKILL.md.disabled` with `enabled = false`, so it is never
    /// stale in the first place. Reaching this function means both filenames are
    /// gone. Scanned MCP/hook entries carry no install_meta, so they take the
    /// normal no-meta prune path rather than the `has_install_meta` branch.
    fn stale_row_should_prune(
        agent_scanned: bool,
        enabled: bool,
        has_disabled_config: bool,
        has_install_meta: bool,
        kind: &str,
        source_path: Option<&str>,
    ) -> bool {
        if !enabled && has_disabled_config {
            return false;
        }
        if !agent_scanned {
            return true;
        }
        if has_install_meta {
            if kind == ExtensionKind::Cli.as_str() {
                return false;
            }
            if source_path.is_none_or(Self::source_still_on_disk) {
                return false;
            }
        }
        true
    }
    /// Was the row's agent among the ones the scan visited? `None` means the
    /// caller does not know (a partial sync), which counts as visited. A row
    /// with no agent (a CLI binary found without a skill naming it) is not
    /// agent-bound, so a scan always counts as having looked for it.
    fn row_agent_scanned(agents_json: &str, scanned_agents: Option<&[String]>) -> bool {
        let Some(scanned) = scanned_agents else {
            return true;
        };
        let agents: Vec<String> = serde_json::from_str(agents_json).unwrap_or_default();
        agents.is_empty() || agents.iter().any(|a| scanned.contains(a))
    }
    /// Sync one full scan in a single transaction: upsert every extension and
    /// remove stale rows, including every row of an agent the scan did not
    /// visit (see `stale_row_should_prune`).
    pub fn sync_scan(&self, scan: &ScanResult) -> Result<(), HkError> {
        self.sync_rows(&scan.extensions, Some(&scan.scanned_agents))
    }
    /// Sync a list of extensions without knowing which agents were visited,
    /// so every exemption in `stale_row_should_prune` applies to every row.
    /// A full scan must go through `sync_scan` instead, or the rows of an
    /// agent that has gone away are never pruned. Only tests use this now.
    pub fn sync_extensions(&self, extensions: &[Extension]) -> Result<(), HkError> {
        self.sync_rows(extensions, None)
    }
    /// Shared body of `sync_scan` and `sync_extensions`.
    /// Much faster than individual insert_extension calls (one fsync instead of N).
    /// NOTE: The ON CONFLICT clause intentionally does NOT touch install meta columns
    /// so that install source metadata survives re-scans.
    fn sync_rows(
        &self,
        extensions: &[Extension],
        scanned_agents: Option<&[String]>,
    ) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;

        for ext in extensions {
            tx.execute(
                UPSERT_EXTENSION_SQL,
                params![
                    ext.id,
                    ext.kind.as_str(),
                    ext.name,
                    ext.description,
                    serde_json::to_string(&ext.source)?,
                    serde_json::to_string(&ext.agents)?,
                    serde_json::to_string(&ext.tags)?,
                    serde_json::to_string(&ext.permissions)?,
                    ext.enabled as i32,
                    ext.trust_score.map(|s| s as i32),
                    ext.installed_at.to_rfc3339(),
                    ext.updated_at.to_rfc3339(),
                    Option::<String>::None,
                    ext.source_path,
                    ext.cli_parent_id,
                    ext.cli_meta
                        .as_ref()
                        .map(|m| serde_json::to_string(m).unwrap_or_default()),
                    ext.pack,
                    serde_json::to_string(&ext.scope)?,
                    ext.mcp_transport.map(|t| t.as_str()),
                ],
            )?;
            // Keep extension_agents join table in sync
            Self::sync_extension_agents(&tx, &ext.id, &ext.agents)?;
        }

        // Drop any extension rows scoped to a project that no longer exists.
        // delete_project now cascades, but pre-1.3.1 deletions left orphans
        // behind that the stale-cleanup below would preserve (because they
        // carry install_meta from a marketplace install). Self-heal here so
        // upgrading users don't have to manually clear them.
        tx.execute(
            "DELETE FROM extensions \
             WHERE json_extract(scope_json, '$.type') = 'project' \
               AND json_extract(scope_json, '$.path') NOT IN \
                   (SELECT path FROM projects)",
            [],
        )?;

        // Remove stale extensions no longer on disk. The keep/prune decision
        // lives in `stale_row_should_prune`: rows whose disabled state the store
        // alone holds, and CLI binaries with install_meta, are always kept;
        // file-backed install_meta rows are kept only while their file is still
        // there, so a manual delete (e.g. `rm -rf ~/.claude`) no longer leaves
        // ghost rows behind.
        let scanned_ids: std::collections::HashSet<&str> =
            extensions.iter().map(|e| e.id.as_str()).collect();
        // (id, enabled, has_disabled_config, has_install_meta, kind, source_path, agents_json)
        type StaleRow = (String, bool, bool, bool, String, Option<String>, String);
        let stale_ids: Vec<StaleRow> = {
            let mut stmt = tx.prepare(
                "SELECT id, enabled, (disabled_config IS NOT NULL) as has_disabled_config,
                        (install_type IS NOT NULL) as has_meta, kind, source_path, agents_json
                 FROM extensions",
            )?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect()
        };
        for (id, enabled, has_disabled_config, has_install_meta, kind, source_path, agents_json) in
            &stale_ids
        {
            if !scanned_ids.contains(id.as_str())
                && Self::stale_row_should_prune(
                    Self::row_agent_scanned(agents_json, scanned_agents),
                    *enabled,
                    *has_disabled_config,
                    *has_install_meta,
                    kind,
                    source_path.as_deref(),
                )
            {
                tx.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
            }
        }

        // Backfill install_meta from scanner-detected git source for extensions
        // that have no install metadata yet. This covers:
        // - Skills that existed before harnesskit was installed (user git-cloned them)
        // - Skills from previous versions before install tracking was added
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'git',
                 install_url = json_extract(source_json, '$.url'),
                 install_revision = json_extract(source_json, '$.commit_hash')
             WHERE install_type IS NULL
               AND json_extract(source_json, '$.origin') = 'git'
               AND json_extract(source_json, '$.url') IS NOT NULL",
        )?;

        // Backfill install_type for CLI extensions that were installed before
        // install_meta tracking was added to install_cli
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'cli-registry'
             WHERE install_type IS NULL
               AND kind = 'cli'
               AND cli_meta_json IS NOT NULL",
        )?;

        // Undo bogus `git` install_meta the backfill above stamped onto skills
        // reached via a symlink inside an agent-home dotfiles repo. Run before
        // pack backfill so the cleared rows don't re-acquire a pack.
        Self::heal_symlinked_git_install_meta(&tx, extensions)?;

        // Realign git install_meta the backfill stamped from a since-corrected
        // source (e.g. plugins re-attributed to their marketplace repo). Run
        // before pack backfill so pack re-derives from the refreshed URL.
        Self::refresh_stale_git_install_meta(&tx)?;

        // Backfill pack from install_url or source_json URL for deployed extensions
        // that lost their git context after being copied to agent directories
        Self::backfill_packs(&tx)?;

        tx.commit()?;
        Ok(())
    }
    /// Sync extensions for a specific agent only — upsert scanned extensions and remove stale ones.
    /// Only deletes stale extensions that belong to the specified agent.
    pub fn sync_extensions_for_agent(
        &self,
        agent: &str,
        extensions: &[Extension],
    ) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;
        for ext in extensions {
            tx.execute(
                UPSERT_EXTENSION_SQL,
                params![
                    ext.id,
                    ext.kind.as_str(),
                    ext.name,
                    ext.description,
                    serde_json::to_string(&ext.source)?,
                    serde_json::to_string(&ext.agents)?,
                    serde_json::to_string(&ext.tags)?,
                    serde_json::to_string(&ext.permissions)?,
                    ext.enabled as i32,
                    ext.trust_score.map(|s| s as i32),
                    ext.installed_at.to_rfc3339(),
                    ext.updated_at.to_rfc3339(),
                    Option::<String>::None,
                    ext.source_path,
                    ext.cli_parent_id,
                    ext.cli_meta
                        .as_ref()
                        .map(|m| serde_json::to_string(m).unwrap_or_default()),
                    ext.pack,
                    serde_json::to_string(&ext.scope)?,
                    ext.mcp_transport.map(|t| t.as_str()),
                ],
            )?;
            // Keep extension_agents join table in sync
            Self::sync_extension_agents(&tx, &ext.id, &ext.agents)?;
        }

        // Remove stale extensions for THIS agent only, using the same keep/prune
        // rule as sync_scan (see `stale_row_should_prune`, with the agent
        // counted as visited): rows whose disabled state the store alone
        // holds, and CLI binaries with install_meta, stay; file-backed
        // install_meta rows stay only while their file is still on disk.
        let scanned_ids: std::collections::HashSet<&str> =
            extensions.iter().map(|e| e.id.as_str()).collect();
        let stale_ids: Vec<(String, bool, bool, bool, String, Option<String>)> = {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT e.id, e.enabled,
                        (e.disabled_config IS NOT NULL) as has_disabled_config,
                        (e.install_type IS NOT NULL) as has_meta,
                        e.kind, e.source_path
                 FROM extensions e
                 INNER JOIN extension_agents ea ON e.id = ea.extension_id
                 WHERE ea.agent_name = ?1",
            )?;
            stmt.query_map(params![agent], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect()
        };
        for (id, enabled, has_disabled_config, has_install_meta, kind, source_path) in &stale_ids {
            if !scanned_ids.contains(id.as_str())
                && Self::stale_row_should_prune(
                    true,
                    *enabled,
                    *has_disabled_config,
                    *has_install_meta,
                    kind,
                    source_path.as_deref(),
                )
            {
                tx.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
            }
        }

        // Backfill install_meta from scanner-detected git source
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'git',
                 install_url = json_extract(source_json, '$.url'),
                 install_revision = json_extract(source_json, '$.commit_hash')
             WHERE install_type IS NULL
               AND json_extract(source_json, '$.origin') = 'git'
               AND json_extract(source_json, '$.url') IS NOT NULL",
        )?;

        // Backfill install_type for CLI extensions missing install_meta
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'cli-registry'
             WHERE install_type IS NULL
               AND kind = 'cli'
               AND cli_meta_json IS NOT NULL",
        )?;

        Self::heal_symlinked_git_install_meta(&tx, extensions)?;

        Self::refresh_stale_git_install_meta(&tx)?;

        Self::backfill_packs(&tx)?;

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;
    #[test]
    fn test_sync_extensions_purges_orphan_project_rows() {
        // Pre-1.3.1 delete_project did not cascade, leaving extension rows
        // pointing at a project that no longer existed. Simulate that state
        // by inserting an extension scoped to a project we never inserted,
        // then verify the next sync_extensions clears it out.
        let (store, _dir) = test_store();

        let mut orphan = sample_extension();
        orphan.id = "ext-orphan".into();
        orphan.scope = ConfigScope::Project {
            name: "ghost".into(),
            path: "/tmp/ghost".into(),
        };
        store.insert_extension(&orphan).unwrap();

        let mut keep = sample_extension();
        keep.id = "ext-keep".into();
        store.insert_extension(&keep).unwrap();

        store.sync_extensions(&[keep.clone()]).unwrap();

        let remaining: Vec<String> = store
            .list_extensions(None, None)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert!(!remaining.contains(&"ext-orphan".to_string()));
        assert!(remaining.contains(&"ext-keep".to_string()));
    }
    #[test]
    fn test_sync_preserves_disabled_extensions() {
        let (store, _dir) = test_store();

        // Insert an extension and disable it the way `manager::toggle_mcp` does:
        // the entry is lifted out of the agent's config file and parked in
        // `disabled_config`, which makes this row the only copy of it. That is
        // what earns the row its exemption from stale pruning — flipping the
        // `enabled` flag alone would not, and must not.
        let mut ext = sample_extension();
        ext.id = "disabled-mcp".into();
        ext.kind = ExtensionKind::Mcp;
        ext.name = "my-mcp".into();
        store.insert_extension(&ext).unwrap();
        store.set_enabled("disabled-mcp", false).unwrap();
        store
            .set_disabled_config("disabled-mcp", Some(r#"{"command":"my-mcp"}"#))
            .unwrap();

        // Sync with an empty scan result (simulating MCP removed from config)
        store.sync_extensions(&[]).unwrap();

        // Disabled extension should survive the sync
        let fetched = store.get_extension("disabled-mcp").unwrap();
        assert!(
            fetched.is_some(),
            "Disabled extension should not be deleted by sync"
        );
        assert!(!fetched.unwrap().enabled);
    }
    #[test]
    fn test_stale_row_should_prune_decision() {
        // Args: (agent_scanned, enabled, has_disabled_config, has_install_meta, kind, source_path)
        let cli = ExtensionKind::Cli.as_str();
        let skill = ExtensionKind::Skill.as_str();
        let exists = env!("CARGO_MANIFEST_DIR"); // guaranteed to exist
        let missing = "/nonexistent/harnesskit/ghost/SKILL.md";

        // Disabled row whose state only the store holds (MCP/hook entry pulled
        // out of the agent's config, or a renamed plugin manifest) — kept.
        assert!(!Store::stale_row_should_prune(
            true,
            false,
            true,
            true,
            skill,
            Some(missing)
        ));
        // Disabled row with no such state — a skill whose SKILL.md and
        // SKILL.md.disabled are both gone — is a ghost and gets pruned.
        assert!(Store::stale_row_should_prune(
            true,
            false,
            false,
            true,
            skill,
            Some(missing)
        ));
        // Sourceless rows (no install_meta) are pruned when gone — prior behavior.
        assert!(Store::stale_row_should_prune(
            true, true, false, false, skill, None
        ));
        // CLI with install_meta is kept even when absent (flaky binary detection).
        assert!(!Store::stale_row_should_prune(
            true,
            true,
            false,
            true,
            cli,
            Some(missing)
        ));
        // File-backed install_meta row whose file is gone → pruned (the ghost fix).
        assert!(Store::stale_row_should_prune(
            true,
            true,
            false,
            true,
            skill,
            Some(missing)
        ));
        // File-backed install_meta row whose file still exists → kept (scan gap).
        assert!(!Store::stale_row_should_prune(
            true,
            true,
            false,
            true,
            skill,
            Some(exists)
        ));
        // Unknown source_path → kept (can't prove removal).
        assert!(!Store::stale_row_should_prune(
            true, true, false, true, skill, None
        ));
        // Agent not visited by the scan: the on-disk exemptions are void, so a
        // file-backed install_meta row is pruned even though its file exists…
        assert!(Store::stale_row_should_prune(
            false,
            true,
            false,
            true,
            skill,
            Some(exists)
        ));
        // …and so is a CLI row with install_meta; only store-held disabled
        // state survives.
        assert!(Store::stale_row_should_prune(
            false, true, false, true, cli, None
        ));
        assert!(!Store::stale_row_should_prune(
            false, false, true, false, skill, None
        ));
    }
    #[test]
    fn test_row_agent_scanned() {
        let visited = vec!["claude".to_string()];
        // A partial sync knows nothing about agents: always counts as visited.
        assert!(Store::row_agent_scanned(r#"["windsurf"]"#, None));
        assert!(!Store::row_agent_scanned(r#"["windsurf"]"#, Some(&visited)));
        // One visited agent is enough (CLI rows list every agent of their skills).
        assert!(Store::row_agent_scanned(
            r#"["windsurf","claude"]"#,
            Some(&visited)
        ));
        // Agentless rows (a CLI binary no skill names) are not agent-bound.
        assert!(Store::row_agent_scanned("[]", Some(&visited)));
    }
    #[test]
    fn test_scan_then_sync_prunes_rows_of_an_agent_that_is_no_longer_detected() {
        // A skill in a shared directory such as ~/.agents/skills has one row
        // per agent that reads it. Once one of those agents is uninstalled its
        // row is never produced again, yet the file is still there, so the
        // install_meta exemption alone kept it forever. Store-held disabled
        // state is the one thing that must survive.
        use crate::adapter::{AgentAdapter, codex::CodexAdapter, windsurf::WindsurfAdapter};
        use crate::scanner;

        let (store, dir) = test_store();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::create_dir_all(home.join(".codeium/windsurf")).unwrap();
        let skill = home.join(".agents/skills/shared");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: shared\ndescription: read by both agents\n---\n",
        )
        .unwrap();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![
            Box::new(CodexAdapter::with_home(home.to_path_buf())),
            Box::new(WindsurfAdapter::with_home(home.to_path_buf())),
        ];

        let scan = scanner::scan_all(&adapters, &[]);
        assert_eq!(scan.scanned_agents, ["codex", "windsurf"]);
        store.sync_scan(&scan).unwrap();
        let windsurf_skill = &store.list_extensions(None, Some("windsurf")).unwrap()[0];
        store
            .set_install_meta(
                &windsurf_skill.id,
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/octo/skills.git".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: None,
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        // An MCP entry HarnessKit disabled lives only in disabled_config.
        let mut off = sample_extension();
        off.id = "windsurf-mcp-off".into();
        off.kind = ExtensionKind::Mcp;
        off.agents = vec!["windsurf".into()];
        off.enabled = false;
        store.insert_extension(&off).unwrap();
        store
            .set_disabled_config(&off.id, Some(r#"{"command":"npx"}"#))
            .unwrap();

        // Devin Desktop is uninstalled; the shared skill stays where it is.
        std::fs::remove_dir_all(home.join(".codeium")).unwrap();
        let scan = scanner::scan_all(&adapters, &[]);
        assert_eq!(scan.scanned_agents, ["codex"]);
        store.sync_scan(&scan).unwrap();

        let windsurf_ids: Vec<String> = store
            .list_extensions(None, Some("windsurf"))
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(
            windsurf_ids,
            ["windsurf-mcp-off"],
            "the shared skill's row goes, the disabled MCP's row stays"
        );
        assert_eq!(
            store.list_extensions(None, Some("codex")).unwrap().len(),
            1,
            "the detected agent's row for the same file stays"
        );
    }
    #[test]
    fn test_sync_prunes_ghost_skill_with_install_meta_when_files_deleted() {
        // Regression: a marketplace/git-installed skill whose files the user
        // deleted (e.g. `rm -rf ~/.claude`) used to linger forever because any
        // install_meta row was exempt from stale cleanup. It should now be
        // pruned once its source_path is gone, while a CLI with install_meta and
        // a skill whose file still exists are both kept.
        let (store, dir) = test_store();
        let meta = || {
            Some(InstallMeta {
                install_type: "marketplace".into(),
                url: Some("https://github.com/tw93/waza".into()),
                url_resolved: None,
                branch: None,
                subpath: None,
                revision: None,
                remote_revision: None,
                checked_at: None,
                check_error: None,
            })
        };

        // Skill installed from marketplace, but its file no longer exists.
        let mut ghost = sample_extension();
        ghost.id = "ghost-skill".into();
        ghost.name = "ghost-skill".into();
        ghost.source_path = Some("/nonexistent/harnesskit/ghost/SKILL.md".into());
        ghost.install_meta = meta();
        store.insert_extension(&ghost).unwrap();

        // Skill whose file still exists on disk (simulate a transient scan gap).
        let live_path = dir.path().join("live-SKILL.md");
        std::fs::write(&live_path, "x").unwrap();
        let mut live = sample_extension();
        live.id = "live-skill".into();
        live.name = "live-skill".into();
        live.source_path = Some(live_path.to_string_lossy().into_owned());
        live.install_meta = meta();
        store.insert_extension(&live).unwrap();

        // CLI with install_meta — binary detection is flaky, must stay.
        let mut cli = sample_extension();
        cli.id = "cli-tool".into();
        cli.name = "cli-tool".into();
        cli.kind = ExtensionKind::Cli;
        cli.source_path = Some("/nonexistent/harnesskit/cli-bin".into());
        cli.install_meta = meta();
        store.insert_extension(&cli).unwrap();

        // Disabled skill the user then deleted outside HarnessKit. Disabling a
        // skill only renames its file, so nothing is parked in disabled_config
        // and the store holds no state worth saving — once both filenames are
        // gone the row is a ghost like any other.
        let mut disabled_ghost = sample_extension();
        disabled_ghost.id = "disabled-ghost".into();
        disabled_ghost.name = "disabled-ghost".into();
        disabled_ghost.enabled = false;
        disabled_ghost.source_path = Some("/nonexistent/harnesskit/off/SKILL.md".into());
        disabled_ghost.install_meta = meta();
        store.insert_extension(&disabled_ghost).unwrap();

        // Disabled skill still on disk as SKILL.md.disabled — a scan gap must
        // not take it, even though its recorded source_path names SKILL.md.
        let off_dir = dir.path().join("off-skill");
        std::fs::create_dir_all(&off_dir).unwrap();
        std::fs::write(off_dir.join("SKILL.md.disabled"), "x").unwrap();
        let mut disabled_live = sample_extension();
        disabled_live.id = "disabled-live".into();
        disabled_live.name = "disabled-live".into();
        disabled_live.enabled = false;
        disabled_live.source_path = Some(off_dir.join("SKILL.md").to_string_lossy().into_owned());
        disabled_live.install_meta = meta();
        store.insert_extension(&disabled_live).unwrap();

        // Empty scan = nothing found on disk this round.
        store.sync_extensions(&[]).unwrap();

        let ids: Vec<String> = store
            .list_extensions(None, None)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert!(
            !ids.contains(&"ghost-skill".to_string()),
            "ghost skill with deleted files should be pruned"
        );
        assert!(
            !ids.contains(&"disabled-ghost".to_string()),
            "disabled skill whose files are gone should be pruned too"
        );
        assert!(
            ids.contains(&"disabled-live".to_string()),
            "disabled skill still on disk as .disabled should be kept"
        );
        assert!(
            ids.contains(&"live-skill".to_string()),
            "skill whose file still exists should be kept"
        );
        assert!(
            ids.contains(&"cli-tool".to_string()),
            "CLI with install_meta should be kept"
        );
    }
    #[test]
    fn test_extension_agents_join_table_synced_by_sync_extensions() {
        let (store, _dir) = test_store();

        let mut ext1 = sample_extension();
        ext1.id = "ext-1".into();
        ext1.name = "ext-one".into();
        ext1.agents = vec!["claude".into()];

        let mut ext2 = sample_extension();
        ext2.id = "ext-2".into();
        ext2.name = "ext-two".into();
        ext2.agents = vec!["cursor".into(), "claude".into()];

        store
            .sync_extensions(&[ext1.clone(), ext2.clone()])
            .unwrap();

        // Verify both come back for claude
        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 2);

        // Only ext2 for cursor
        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
        assert_eq!(cursor[0].id, "ext-2");

        // Re-sync ext1 with changed agents (now also cursor)
        ext1.agents = vec!["claude".into(), "cursor".into()];
        store.sync_extensions(&[ext1, ext2]).unwrap();

        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 2);
    }
    #[test]
    fn test_sync_extensions_for_agent_uses_join_table() {
        let (store, _dir) = test_store();

        let mut ext1 = sample_extension();
        ext1.id = "agent-ext-1".into();
        ext1.name = "agent-ext-one".into();
        ext1.agents = vec!["claude".into()];

        let mut ext2 = sample_extension();
        ext2.id = "agent-ext-2".into();
        ext2.name = "agent-ext-two".into();
        ext2.agents = vec!["cursor".into()];

        // Sync for claude agent
        store
            .sync_extensions_for_agent("claude", &[ext1.clone()])
            .unwrap();
        // Sync for cursor agent separately
        store
            .sync_extensions_for_agent("cursor", &[ext2.clone()])
            .unwrap();

        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].id, "agent-ext-1");

        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
        assert_eq!(cursor[0].id, "agent-ext-2");

        // Remove ext1 from claude scan — it should be deleted
        store.sync_extensions_for_agent("claude", &[]).unwrap();
        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert!(claude.is_empty());

        // cursor extension should still exist
        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
    }
    #[test]
    fn test_sync_for_agent_prunes_ghost_skill_with_install_meta() {
        // Same ghost-pruning rule as sync_extensions, exercised through the
        // per-agent path (which has its own JOIN query) to guard the column
        // mapping there.
        let (store, dir) = test_store();
        let meta = || {
            Some(InstallMeta {
                install_type: "marketplace".into(),
                url: Some("https://github.com/tw93/waza".into()),
                url_resolved: None,
                branch: None,
                subpath: None,
                revision: None,
                remote_revision: None,
                checked_at: None,
                check_error: None,
            })
        };

        let mut ghost = sample_extension();
        ghost.id = "agent-ghost".into();
        ghost.agents = vec!["claude".into()];
        ghost.source_path = Some("/nonexistent/harnesskit/ghost/SKILL.md".into());
        ghost.install_meta = meta();
        store.insert_extension(&ghost).unwrap();

        let live_path = dir.path().join("agent-live-SKILL.md");
        std::fs::write(&live_path, "x").unwrap();
        let mut live = sample_extension();
        live.id = "agent-live".into();
        live.agents = vec!["claude".into()];
        live.source_path = Some(live_path.to_string_lossy().into_owned());
        live.install_meta = meta();
        store.insert_extension(&live).unwrap();

        // Disabled row whose entry the store alone holds — exercises the
        // disabled_config column in this query's own SELECT list.
        let mut agent_off = sample_extension();
        agent_off.id = "agent-off".into();
        agent_off.kind = ExtensionKind::Mcp;
        agent_off.agents = vec!["claude".into()];
        agent_off.enabled = false;
        agent_off.source_path = Some("/nonexistent/harnesskit/off/.mcp.json".into());
        store.insert_extension(&agent_off).unwrap();
        store
            .set_disabled_config("agent-off", Some(r#"{"command":"x"}"#))
            .unwrap();

        store.sync_extensions_for_agent("claude", &[]).unwrap();

        let ids: Vec<String> = store
            .list_extensions(None, Some("claude"))
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert!(
            !ids.contains(&"agent-ghost".to_string()),
            "ghost skill should be pruned via the per-agent path"
        );
        assert!(
            ids.contains(&"agent-live".to_string()),
            "skill whose file still exists should be kept"
        );
        assert!(
            ids.contains(&"agent-off".to_string()),
            "row whose disabled entry lives only in the store should be kept"
        );
    }
}
