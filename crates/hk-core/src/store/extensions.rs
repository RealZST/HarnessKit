//! Extension rows: upsert, query, toggle, metadata (tags/pack/install-meta), row mapping.

use super::*;

impl Store {
    /// Upsert an extension: insert if new, update scanner-derived fields if existing.
    /// Preserves user-set fields: enabled, tags, pack, trust_score, and install meta.
    pub fn insert_extension(&self, ext: &Extension) -> Result<(), HkError> {
        let im = ext.install_meta.as_ref();
        self.conn.execute(
            UPSERT_EXTENSION_FULL_SQL,
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
                ext.cli_meta.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                im.map(|m| m.install_type.as_str()),
                im.and_then(|m| m.url.as_deref()),
                im.and_then(|m| m.url_resolved.as_deref()),
                im.and_then(|m| m.branch.as_deref()),
                im.and_then(|m| m.subpath.as_deref()),
                im.and_then(|m| m.revision.as_deref()),
                im.and_then(|m| m.remote_revision.as_deref()),
                im.and_then(|m| m.checked_at.map(|t| t.to_rfc3339())),
                im.and_then(|m| m.check_error.as_deref()),
                ext.pack,
                serde_json::to_string(&ext.scope)?,
                ext.mcp_transport.map(|t| t.as_str()),
            ],
        )?;
        // Keep extension_agents join table in sync
        Self::sync_extension_agents(&self.conn, &ext.id, &ext.agents)?;
        Ok(())
    }
    pub fn get_extension(&self, id: &str) -> Result<Option<Extension>, HkError> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {EXTENSION_COLUMNS} FROM extensions WHERE id = ?1"))?;
        let mut rows = stmt.query_map(params![id], |row| Ok(self.row_to_extension(row)))?;
        match rows.next() {
            Some(Ok(Ok(ext))) => Ok(Some(ext)),
            Some(Ok(Err(e))) => Err(e),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
    pub fn list_extensions(
        &self,
        kind: Option<ExtensionKind>,
        agent: Option<&str>,
    ) -> Result<Vec<Extension>, HkError> {
        // Unqualified names stay unambiguous in the join: extension_agents
        // has no columns in common with extensions.
        let ext_cols = EXTENSION_COLUMNS;

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // Build FROM/WHERE depending on whether we filter by agent
        let mut sql = if let Some(agent_val) = agent {
            param_values.push(Box::new(agent_val.to_string()));
            format!(
                "SELECT DISTINCT {} FROM extensions e INNER JOIN extension_agents ea ON e.id = ea.extension_id WHERE ea.agent_name = ?1",
                ext_cols
            )
        } else {
            format!("SELECT {} FROM extensions e WHERE 1=1", ext_cols)
        };

        if let Some(k) = kind {
            sql.push_str(&format!(" AND e.kind = ?{}", param_values.len() + 1));
            param_values.push(Box::new(k.as_str().to_string()));
        }

        sql.push_str(" ORDER BY e.name ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| Ok(self.row_to_extension(row)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row??);
        }
        Ok(results)
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET enabled = ?1 WHERE id = ?2",
            params![enabled as i32, id],
        )?;
        Ok(())
    }
    pub fn get_disabled_config(&self, id: &str) -> Result<Option<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT disabled_config FROM extensions WHERE id = ?1")?;
        let result = stmt.query_row(params![id], |row| row.get::<_, Option<String>>(0));
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn set_disabled_config(&self, id: &str, config: Option<&str>) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET disabled_config = ?1 WHERE id = ?2",
            params![config, id],
        )?;
        Ok(())
    }
    /// Persist install source metadata for an extension.
    pub fn set_install_meta(&self, id: &str, meta: &InstallMeta) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET install_type = ?1, install_url = ?2, install_url_resolved = ?3, install_branch = ?4, install_subpath = ?5, install_revision = ?6, remote_revision = ?7, checked_at = ?8, check_error = ?9 WHERE id = ?10",
            params![
                meta.install_type,
                meta.url,
                meta.url_resolved,
                meta.branch,
                meta.subpath,
                meta.revision,
                meta.remote_revision,
                meta.checked_at.map(|t| t.to_rfc3339()),
                meta.check_error,
                id,
            ],
        )?;
        Ok(())
    }
    /// Clear every install_meta column for an extension. Used by the manual
    /// source-binding flow when a user unbinds (clears the pack field) — only
    /// rows with `install_type = "manual"` should be passed here; rows with
    /// real "git" / "marketplace" install_meta must be preserved.
    pub fn clear_install_meta(&self, id: &str) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET install_type = NULL, install_url = NULL, install_url_resolved = NULL, install_branch = NULL, install_subpath = NULL, install_revision = NULL, remote_revision = NULL, checked_at = NULL, check_error = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
    /// Update remote revision check state for an extension.
    pub fn update_check_state(
        &self,
        id: &str,
        remote_revision: Option<&str>,
        checked_at: DateTime<Utc>,
        check_error: Option<&str>,
    ) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET remote_revision = ?1, checked_at = ?2, check_error = ?3 WHERE id = ?4",
            params![remote_revision, checked_at.to_rfc3339(), check_error, id],
        )?;
        Ok(())
    }
    pub fn update_trust_score(&self, id: &str, score: u8) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET trust_score = ?1 WHERE id = ?2",
            params![score as i32, id],
        )?;
        Ok(())
    }
    pub fn update_tags(&self, id: &str, tags: &[String]) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET tags_json = ?1 WHERE id = ?2",
            params![serde_json::to_string(tags)?, id],
        )?;
        Ok(())
    }
    pub fn batch_update_tags(&self, ids: &[String], tags: &[String]) -> Result<(), HkError> {
        let tags_json = serde_json::to_string(tags)?;
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE extensions SET tags_json = ?1 WHERE id = ?2",
                params![tags_json, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn get_all_tags(&self) -> Result<Vec<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT tags_json FROM extensions WHERE tags_json != '[]'")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut all_tags = std::collections::BTreeSet::new();
        for row in rows {
            let json: String = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&json) {
                for tag in tags {
                    all_tags.insert(tag);
                }
            }
        }
        Ok(all_tags.into_iter().collect())
    }
    pub fn update_pack(&self, id: &str, pack: Option<&str>) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET pack = ?1 WHERE id = ?2",
            params![pack, id],
        )?;
        Ok(())
    }
    pub fn batch_update_pack(&self, ids: &[String], pack: Option<&str>) -> Result<(), HkError> {
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE extensions SET pack = ?1 WHERE id = ?2",
                params![pack, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn get_all_packs(&self) -> Result<Vec<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT pack FROM extensions WHERE pack IS NOT NULL ORDER BY pack")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    /// Find all extension IDs with the same name and kind.
    pub fn find_ids_by_name_and_kind(&self, name: &str, kind: &str) -> Result<Vec<String>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM extensions WHERE name = ?1 AND kind = ?2",
        )?;
        let rows = stmt.query_map(params![name, kind], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    /// Find all extension IDs that share the same source_path as the given extension.
    pub fn find_siblings_by_source_path(&self, id: &str) -> Result<Vec<String>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT e2.id FROM extensions e1
             JOIN extensions e2 ON e1.source_path = e2.source_path
             WHERE e1.id = ?1 AND e1.source_path IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    /// Get all child skills linked to a CLI extension
    pub fn get_child_skills(&self, cli_id: &str) -> Result<Vec<Extension>, HkError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {EXTENSION_COLUMNS} FROM extensions WHERE cli_parent_id = ?1"
        ))?;
        let rows = stmt.query_map(params![cli_id], |row| Ok(self.row_to_extension(row)))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row??);
        }
        Ok(results)
    }
    /// Link child skills to a CLI parent
    pub fn link_skills_to_cli(&self, cli_id: &str, skill_ids: &[String]) -> Result<(), HkError> {
        for skill_id in skill_ids {
            self.conn.execute(
                "UPDATE extensions SET cli_parent_id = ?1 WHERE id = ?2",
                params![cli_id, skill_id],
            )?;
        }
        Ok(())
    }
    /// Unlink all children from a CLI (set cli_parent_id to NULL)
    pub fn unlink_cli_children(&self, cli_id: &str) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET cli_parent_id = NULL WHERE cli_parent_id = ?1",
            params![cli_id],
        )?;
        Ok(())
    }
    /// Sync the extension_agents join table for a single extension.
    /// Deletes existing rows and re-inserts from the provided agent list.
    pub(super) fn sync_extension_agents(conn: &rusqlite::Connection, ext_id: &str, agents: &[String]) -> Result<(), HkError> {
        conn.execute("DELETE FROM extension_agents WHERE extension_id = ?1", params![ext_id])?;
        for agent in agents {
            conn.execute(
                "INSERT INTO extension_agents (extension_id, agent_name) VALUES (?1, ?2)",
                params![ext_id, agent],
            )?;
        }
        Ok(())
    }
    pub fn delete_extension(&self, id: &str) -> Result<(), HkError> {
        // `PRAGMA secure_delete` (set in `open`) already zeroes the row's pages,
        // including the `disabled_config` snapshot, so no separate scrub pass is
        // needed — and a scrub in its own autocommit statement would be worse
        // than useless: a crash between the two would leave a disabled row whose
        // snapshot is gone, failing re-enable with "No saved config".
        self.conn.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
        Ok(())
    }
    fn row_to_extension(&self, row: &rusqlite::Row) -> Result<Extension, HkError> {
        let kind_str: String = row.get(1)?;
        let source_json: String = row.get(4)?;
        let agents_json: String = row.get(5)?;
        let tags_json: String = row.get(6)?;
        let permissions_json: String = row.get(7)?;
        let installed_at_str: String = row.get(10)?;
        let updated_at_str: String = row.get(11)?;
        let cli_meta_json: Option<String> = row.get::<_, Option<String>>(15).ok().flatten();

        // Install meta columns (16-24)
        let install_type: Option<String> = row.get::<_, Option<String>>(16).ok().flatten();
        let install_meta = install_type.map(|it| {
            let checked_at_str: Option<String> = row.get::<_, Option<String>>(23).ok().flatten();
            InstallMeta {
                install_type: it,
                url: row.get::<_, Option<String>>(17).ok().flatten(),
                url_resolved: row.get::<_, Option<String>>(18).ok().flatten(),
                branch: row.get::<_, Option<String>>(19).ok().flatten(),
                subpath: row.get::<_, Option<String>>(20).ok().flatten(),
                revision: row.get::<_, Option<String>>(21).ok().flatten(),
                remote_revision: row.get::<_, Option<String>>(22).ok().flatten(),
                checked_at: checked_at_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                check_error: row.get::<_, Option<String>>(24).ok().flatten(),
            }
        });

        let scope_json: Option<String> = row.get::<_, Option<String>>(26).ok().flatten();
        let scope = scope_json
            .and_then(|s| serde_json::from_str::<ConfigScope>(&s).ok())
            .unwrap_or(ConfigScope::Global);

        let mcp_transport = row
            .get::<_, Option<String>>(27)
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok());

        Ok(Extension {
            id: row.get(0)?,
            kind: kind_str
                .parse()
                .map_err(|e: anyhow::Error| HkError::Internal(e.to_string()))?,
            name: row.get(2)?,
            description: row.get(3)?,
            source: serde_json::from_str(&source_json)?,
            agents: serde_json::from_str(&agents_json)?,
            tags: serde_json::from_str(&tags_json)?,
            pack: row.get::<_, Option<String>>(25).ok().flatten(),
            permissions: serde_json::from_str(&permissions_json)?,
            enabled: row.get::<_, i32>(8)? != 0,
            trust_score: row.get::<_, Option<i32>>(9)?.map(|s| s as u8),
            installed_at: DateTime::parse_from_rfc3339(&installed_at_str)
                .map_err(|e| HkError::Internal(format!("Invalid installed_at timestamp: {e}")))?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| HkError::Internal(format!("Invalid updated_at timestamp: {e}")))?
                .with_timezone(&Utc),
            source_path: row.get::<_, Option<String>>(13).ok().flatten(),
            cli_parent_id: row.get::<_, Option<String>>(14).ok().flatten(),
            cli_meta: cli_meta_json.and_then(|s| serde_json::from_str::<CliMeta>(&s).ok()),
            install_meta,
            scope,
            mcp_transport,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;
    #[test]
    fn test_insert_and_get_extension() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.name, "test-skill");
        assert_eq!(fetched.kind, ExtensionKind::Skill);
        assert_eq!(fetched.agents, vec!["claude"]);
        assert_eq!(fetched.tags, vec!["test"]);
    }
    #[test]
    fn test_extension_mcp_transport_round_trip() {
        let (store, _dir) = test_store();

        let mut remote = sample_extension();
        remote.id = "remote-mcp".into();
        remote.kind = ExtensionKind::Mcp;
        remote.mcp_transport = Some(crate::adapter::McpTransport::Sse);
        store.insert_extension(&remote).unwrap();
        let fetched = store.get_extension("remote-mcp").unwrap().unwrap();
        assert_eq!(
            fetched.mcp_transport,
            Some(crate::adapter::McpTransport::Sse)
        );

        // Non-MCP rows (and legacy NULLs) come back as None.
        let skill = sample_extension();
        let skill_id = skill.id.clone();
        store.insert_extension(&skill).unwrap();
        assert_eq!(
            store.get_extension(&skill_id).unwrap().unwrap().mcp_transport,
            None
        );

        // sync_extensions (the scanner upsert) must persist it too.
        let mut synced = sample_extension();
        synced.id = "synced-mcp".into();
        synced.kind = ExtensionKind::Mcp;
        synced.mcp_transport = Some(crate::adapter::McpTransport::Http);
        store.sync_extensions(std::slice::from_ref(&synced)).unwrap();
        assert_eq!(
            store
                .get_extension("synced-mcp")
                .unwrap()
                .unwrap()
                .mcp_transport,
            Some(crate::adapter::McpTransport::Http)
        );
    }
    #[test]
    fn test_extension_scope_round_trip() {
        let (store, _dir) = test_store();

        let mut global = sample_extension();
        global.id = "global-skill".into();
        store.insert_extension(&global).unwrap();

        let mut project = sample_extension();
        project.id = "project-skill".into();
        project.scope = ConfigScope::Project {
            name: "myapp".into(),
            path: "/Users/test/myapp".into(),
        };
        store.insert_extension(&project).unwrap();

        let g = store.get_extension("global-skill").unwrap().unwrap();
        assert!(matches!(g.scope, ConfigScope::Global));

        let p = store.get_extension("project-skill").unwrap().unwrap();
        match p.scope {
            ConfigScope::Project { name, path } => {
                assert_eq!(name, "myapp");
                assert_eq!(path, "/Users/test/myapp");
            }
            _ => panic!("expected project scope"),
        }
    }
    #[test]
    fn test_extension_scope_null_legacy_row_is_global() {
        // Rows that predate the scope_json column have NULL scope. The reader
        // must default these to Global so existing databases keep working.
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        // Simulate a legacy row by clearing scope_json after insert
        store
            .conn
            .execute(
                "UPDATE extensions SET scope_json = NULL WHERE id = ?1",
                params![ext.id],
            )
            .unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert!(matches!(fetched.scope, ConfigScope::Global));
    }
    #[test]
    fn test_list_extensions_filter_by_kind() {
        let (store, _dir) = test_store();
        let mut skill = sample_extension();
        skill.name = "my-skill".into();
        store.insert_extension(&skill).unwrap();

        let mut mcp = sample_extension();
        mcp.id = uuid::Uuid::new_v4().to_string();
        mcp.kind = ExtensionKind::Mcp;
        mcp.name = "my-mcp".into();
        store.insert_extension(&mcp).unwrap();

        let skills = store
            .list_extensions(Some(ExtensionKind::Skill), None)
            .unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
    }
    #[test]
    fn test_list_extensions_filter_by_agent() {
        let (store, _dir) = test_store();
        let mut ext1 = sample_extension();
        ext1.agents = vec!["claude".into()];
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = uuid::Uuid::new_v4().to_string();
        ext2.name = "cursor-skill".into();
        ext2.agents = vec!["cursor".into()];
        store.insert_extension(&ext2).unwrap();

        let claude_exts = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude_exts.len(), 1);
        assert_eq!(claude_exts[0].name, "test-skill");
    }
    #[test]
    fn test_update_extension_toggle() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        store.set_enabled(&ext.id, false).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }
    #[test]
    fn test_delete_extension() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        store.delete_extension(&ext.id).unwrap();
        assert!(store.get_extension(&ext.id).unwrap().is_none());
    }
    #[test]
    fn test_update_trust_score() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        store.update_trust_score(&ext.id, 85).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.trust_score, Some(85));
    }
    #[test]
    fn test_update_tags() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        store
            .update_tags(&ext.id, &["security".into(), "audit".into()])
            .unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.tags, vec!["security", "audit"]);
    }
    #[test]
    fn test_get_all_tags() {
        let (store, _dir) = test_store();
        let mut ext1 = sample_extension();
        ext1.tags = vec!["security".into(), "audit".into()];
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = uuid::Uuid::new_v4().to_string();
        ext2.tags = vec!["audit".into(), "testing".into()];
        store.insert_extension(&ext2).unwrap();

        let tags = store.get_all_tags().unwrap();
        assert_eq!(tags, vec!["audit", "security", "testing"]);
    }
    #[test]
    fn test_update_pack() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        assert_eq!(store.get_extension(&ext.id).unwrap().unwrap().pack, None);

        store.update_pack(&ext.id, Some("alice/repo")).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.pack, Some("alice/repo".to_string()));

        store.update_pack(&ext.id, None).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.pack, None);
    }
    #[test]
    fn test_disabled_config_roundtrip() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        assert!(store.get_disabled_config(&ext.id).unwrap().is_none());

        let config = r#"{"command":"npx","args":["-y","@mcp/server"]}"#;
        store.set_disabled_config(&ext.id, Some(config)).unwrap();
        assert_eq!(store.get_disabled_config(&ext.id).unwrap().unwrap(), config);

        store.set_disabled_config(&ext.id, None).unwrap();
        assert!(store.get_disabled_config(&ext.id).unwrap().is_none());
    }
    #[test]
    fn test_find_siblings_by_source_path() {
        let (store, _dir) = test_store();
        let shared_path = "/home/.agents/skills/my-skill/SKILL.md";

        let mut ext1 = sample_extension();
        ext1.id = "ext-cursor".into();
        ext1.agents = vec!["cursor".into()];
        ext1.source_path = Some(shared_path.to_string());
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = "ext-codex".into();
        ext2.agents = vec!["codex".into()];
        ext2.source_path = Some(shared_path.to_string());
        store.insert_extension(&ext2).unwrap();

        let mut ext3 = sample_extension();
        ext3.id = "ext-claude".into();
        ext3.agents = vec!["claude".into()];
        ext3.source_path = Some("/home/.claude/skills/other/SKILL.md".to_string());
        store.insert_extension(&ext3).unwrap();

        let siblings = store.find_siblings_by_source_path("ext-cursor").unwrap();
        assert_eq!(siblings.len(), 2);
        assert!(siblings.contains(&"ext-cursor".to_string()));
        assert!(siblings.contains(&"ext-codex".to_string()));
    }
    #[test]
    fn test_cli_extension_roundtrip() {
        let (store, _dir) = test_store();
        let meta = CliMeta {
            binary_name: "wecom-cli".into(),
            binary_path: Some("/usr/local/bin/wecom-cli".into()),
            install_method: Some("npm".into()),
            credentials_path: Some("~/.config/wecom/bot.enc".into()),
            version: Some("1.2.3".into()),
            api_domains: vec!["qyapi.weixin.qq.com".into()],
        };
        let mut ext = sample_extension();
        ext.kind = ExtensionKind::Cli;
        ext.name = "wecom-cli".into();
        ext.cli_meta = Some(meta.clone());
        store.insert_extension(&ext).unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.kind, ExtensionKind::Cli);
        assert_eq!(fetched.name, "wecom-cli");
        let fetched_meta = fetched.cli_meta.unwrap();
        assert_eq!(fetched_meta.binary_name, "wecom-cli");
        assert_eq!(
            fetched_meta.binary_path,
            Some("/usr/local/bin/wecom-cli".into())
        );
        assert_eq!(fetched_meta.install_method, Some("npm".into()));
        assert_eq!(
            fetched_meta.credentials_path,
            Some("~/.config/wecom/bot.enc".into())
        );
        assert_eq!(fetched_meta.version, Some("1.2.3".into()));
        assert_eq!(fetched_meta.api_domains, vec!["qyapi.weixin.qq.com"]);
        assert!(fetched.cli_parent_id.is_none());
    }
    #[test]
    fn test_cli_parent_child_link() {
        let (store, _dir) = test_store();

        // Create CLI parent
        let mut cli = sample_extension();
        cli.id = "cli-parent".into();
        cli.kind = ExtensionKind::Cli;
        cli.name = "my-cli".into();
        cli.cli_meta = Some(CliMeta {
            binary_name: "my-cli".into(),
            binary_path: None,
            install_method: None,
            credentials_path: None,
            version: None,
            api_domains: vec![],
        });
        store.insert_extension(&cli).unwrap();

        // Create 2 child skills
        let mut child1 = sample_extension();
        child1.id = "child-skill-1".into();
        child1.name = "skill-one".into();
        child1.cli_parent_id = Some("cli-parent".into());
        store.insert_extension(&child1).unwrap();

        let mut child2 = sample_extension();
        child2.id = "child-skill-2".into();
        child2.name = "skill-two".into();
        child2.cli_parent_id = Some("cli-parent".into());
        store.insert_extension(&child2).unwrap();

        // Verify get_child_skills returns both
        let children = store.get_child_skills("cli-parent").unwrap();
        assert_eq!(children.len(), 2);
        let child_ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        assert!(child_ids.contains(&"child-skill-1"));
        assert!(child_ids.contains(&"child-skill-2"));

        // Verify parent_id roundtrips
        let fetched = store.get_extension("child-skill-1").unwrap().unwrap();
        assert_eq!(fetched.cli_parent_id, Some("cli-parent".to_string()));

        // Unlink, verify empty
        store.unlink_cli_children("cli-parent").unwrap();
        let children = store.get_child_skills("cli-parent").unwrap();
        assert!(children.is_empty());

        // Verify child still exists but has no parent
        let fetched = store.get_extension("child-skill-1").unwrap().unwrap();
        assert!(fetched.cli_parent_id.is_none());
    }
    #[test]
    fn test_link_skills_to_cli() {
        let (store, _dir) = test_store();

        // Create CLI parent
        let mut cli = sample_extension();
        cli.id = "cli-parent".into();
        cli.kind = ExtensionKind::Cli;
        cli.name = "my-cli".into();
        store.insert_extension(&cli).unwrap();

        // Create children without parent initially
        let mut child1 = sample_extension();
        child1.id = "orphan-1".into();
        child1.name = "orphan-one".into();
        store.insert_extension(&child1).unwrap();

        let mut child2 = sample_extension();
        child2.id = "orphan-2".into();
        child2.name = "orphan-two".into();
        store.insert_extension(&child2).unwrap();

        // Link them
        store
            .link_skills_to_cli("cli-parent", &["orphan-1".into(), "orphan-2".into()])
            .unwrap();

        let children = store.get_child_skills("cli-parent").unwrap();
        assert_eq!(children.len(), 2);
    }
    #[test]
    fn test_install_meta_roundtrip() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        // Initially no install meta
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert!(fetched.install_meta.is_none());

        // Set install meta
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/user/repo".into()),
            url_resolved: Some("https://github.com/user/repo.git".into()),
            branch: Some("main".into()),
            subpath: Some("skills/my-skill".into()),
            revision: Some("abc123".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        store.set_install_meta(&ext.id, &meta).unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "git");
        assert_eq!(im.url.as_deref(), Some("https://github.com/user/repo"));
        assert_eq!(
            im.url_resolved.as_deref(),
            Some("https://github.com/user/repo.git")
        );
        assert_eq!(im.branch.as_deref(), Some("main"));
        assert_eq!(im.subpath.as_deref(), Some("skills/my-skill"));
        assert_eq!(im.revision.as_deref(), Some("abc123"));
        assert!(im.remote_revision.is_none());
        assert!(im.checked_at.is_none());
        assert!(im.check_error.is_none());
    }
    #[test]
    fn test_update_check_state_roundtrip() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        // Set initial install meta
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/user/repo".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("abc123".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        store.set_install_meta(&ext.id, &meta).unwrap();

        // Update check state
        let now = Utc::now();
        store
            .update_check_state(&ext.id, Some("def456"), now, None)
            .unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "git");
        assert_eq!(im.revision.as_deref(), Some("abc123"));
        assert_eq!(im.remote_revision.as_deref(), Some("def456"));
        assert!(im.checked_at.is_some());
        assert!(im.check_error.is_none());

        // Update check state with error
        store
            .update_check_state(&ext.id, None, now, Some("network timeout"))
            .unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert!(im.remote_revision.is_none());
        assert_eq!(im.check_error.as_deref(), Some("network timeout"));
    }
    #[test]
    fn test_insert_extension_with_install_meta() {
        let (store, _dir) = test_store();
        let mut ext = sample_extension();
        ext.install_meta = Some(InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://marketplace.example.com/skill/42".into()),
            url_resolved: None,
            branch: None,
            subpath: Some("42".into()),
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "marketplace");
        assert_eq!(im.subpath.as_deref(), Some("42"));
    }
    #[test]
    fn test_list_extensions_agent_filter_escapes_wildcards() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.db")).unwrap();

        // Insert extension for "claude" agent
        let ext_claude = Extension {
            id: "ext-claude".into(),
            kind: ExtensionKind::Skill,
            name: "claude-skill".into(),
            description: "".into(),
            source: Source {
                origin: SourceOrigin::Local,
                url: None,
                version: None,
                commit_hash: None,
                from_manifest: false,
            },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
            scope: ConfigScope::Global,
            mcp_transport: None,
        };
        store.insert_extension(&ext_claude).unwrap();

        // A wildcard agent filter should NOT match everything
        let results = store.list_extensions(None, Some("%")).unwrap();
        assert_eq!(results.len(), 0, "Wildcard '%' should not match any agent");
    }
    #[test]
    fn test_extension_agents_join_table_populated_by_insert() {
        let (store, _dir) = test_store();

        let mut ext = sample_extension();
        ext.agents = vec!["claude".into(), "cursor".into()];
        store.insert_extension(&ext).unwrap();

        // Verify join table rows exist
        let count: i64 = store.conn.query_row(
            "SELECT COUNT(*) FROM extension_agents WHERE extension_id = ?1",
            params![ext.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 2);

        // Verify agent filter uses the join table correctly
        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 1);
        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
        let codex = store.list_extensions(None, Some("codex")).unwrap();
        assert!(codex.is_empty());
    }
}
