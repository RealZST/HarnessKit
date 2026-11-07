//! Store operations for dedicated repositories and repository items.

use super::*;

/// Derive a human-readable repository name from its URL or Git reference.
pub fn derive_repo_name(url: &str) -> String {
    if let Some(pack) = crate::scanner::extract_pack_from_url(url) {
        return pack;
    }
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");
    trimmed
        .rsplit('/')
        .next()
        .unwrap_or(trimmed)
        .to_string()
}

impl Store {
    /// Add a new repository to the database.
    pub fn add_repository(&self, repo: &NewRepository) -> Result<Repository, HkError> {
        let trimmed_url = repo.url.trim().to_string();
        if trimmed_url.is_empty() {
            return Err(HkError::Validation("Repository URL cannot be empty".into()));
        }

        let id = repo
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let name = repo
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| derive_repo_name(&trimmed_url));
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        let res = self.conn.execute(
            "INSERT INTO repositories (id, url, name, branch, auth_token, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                trimmed_url,
                name,
                repo.branch,
                repo.auth_token,
                now_str
            ],
        );

        match res {
            Ok(_) => Ok(Repository {
                id,
                url: trimmed_url,
                name,
                branch: repo.branch.clone(),
                auth_token: repo.auth_token.clone(),
                last_synced_at: None,
                last_commit_hash: None,
                last_commit_message: None,
                last_sync_error: None,
                created_at: now,
            }),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                Err(HkError::Conflict(format!(
                    "Repository with URL '{}' already exists",
                    trimmed_url
                )))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Retrieve a repository by its primary key ID.
    pub fn get_repository(&self, id: &str) -> Result<Option<Repository>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, name, branch, auth_token, last_synced_at, last_commit_hash, last_commit_message, last_sync_error, created_at
             FROM repositories
             WHERE id = ?1",
        )?;

        let res = stmt.query_row(params![id], row_to_repository);
        match res {
            Ok(repo) => Ok(Some(repo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Retrieve a repository by its URL.
    pub fn get_repository_by_url(&self, url: &str) -> Result<Option<Repository>, HkError> {
        let trimmed_url = url.trim();
        let mut stmt = self.conn.prepare(
            "SELECT id, url, name, branch, auth_token, last_synced_at, last_commit_hash, last_commit_message, last_sync_error, created_at
             FROM repositories
             WHERE url = ?1",
        )?;

        let res = stmt.query_row(params![trimmed_url], row_to_repository);
        match res {
            Ok(repo) => Ok(Some(repo)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all repositories, sorted by name ascending.
    pub fn list_repositories(&self) -> Result<Vec<Repository>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, name, branch, auth_token, last_synced_at, last_commit_hash, last_commit_message, last_sync_error, created_at
             FROM repositories
             ORDER BY name ASC, created_at DESC",
        )?;

        let rows = stmt
            .query_map([], row_to_repository)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Delete a repository by ID. Cascades to repository_items.
    pub fn delete_repository(&self, id: &str) -> Result<bool, HkError> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM repositories WHERE id = ?1", params![id])?;
        Ok(rows_affected > 0)
    }

    /// Update repository metadata following a successful sync.
    pub fn update_repository_sync(&self, id: &str, sync_data: &RepoSyncResult) -> Result<(), HkError> {
        let synced_at_str = sync_data.synced_at.to_rfc3339();
        self.conn.execute(
            "UPDATE repositories
             SET last_synced_at = ?1,
                 last_commit_hash = ?2,
                 last_commit_message = ?3,
                 last_sync_error = NULL
             WHERE id = ?4",
            params![
                synced_at_str,
                sync_data.commit_hash,
                sync_data.commit_message,
                id
            ],
        )?;
        Ok(())
    }

    /// Record a sync failure on a repository.
    pub fn set_repository_sync_error(&self, id: &str, error: &str) -> Result<(), HkError> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE repositories
             SET last_synced_at = ?1,
                 last_sync_error = ?2
             WHERE id = ?3",
            params![now, error, id],
        )?;
        Ok(())
    }

    /// Upsert a repository item. Preserves existing `status` on update.
    pub fn upsert_repository_item(&self, item: &RepositoryItem) -> Result<(), HkError> {
        self.conn.execute(
            "INSERT INTO repository_items (repo_id, kind, item_id, name, description, subpath, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(repo_id, kind, item_id) DO UPDATE SET
               name = excluded.name,
               description = excluded.description,
               subpath = excluded.subpath",
            params![
                item.repo_id,
                item.kind.as_str(),
                item.item_id,
                item.name,
                item.description,
                item.subpath,
                item.status,
            ],
        )?;
        Ok(())
    }

    /// Remove items belonging to a repository that are no longer present in upstream scan.
    pub fn prune_repository_items(
        &self,
        repo_id: &str,
        kind: ExtensionKind,
        active_item_ids: &[String],
    ) -> Result<(), HkError> {
        if active_item_ids.is_empty() {
            self.conn.execute(
                "DELETE FROM repository_items WHERE repo_id = ?1 AND kind = ?2",
                params![repo_id, kind.as_str()],
            )?;
            return Ok(());
        }

        let placeholders: Vec<String> = (0..active_item_ids.len()).map(|_| "?".to_string()).collect();
        let query = format!(
            "DELETE FROM repository_items
             WHERE repo_id = ?1 AND kind = ?2 AND item_id NOT IN ({})",
            placeholders.join(", ")
        );

        let mut stmt = self.conn.prepare(&query)?;
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        params_vec.push(Box::new(repo_id.to_string()));
        params_vec.push(Box::new(kind.as_str().to_string()));
        for id in active_item_ids {
            params_vec.push(Box::new(id.clone()));
        }

        let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        stmt.execute(rusqlite_params.as_slice())?;
        Ok(())
    }

    /// Set an item's status ('available' or 'ignored').
    pub fn set_repository_item_status(
        &self,
        repo_id: &str,
        kind: ExtensionKind,
        item_id: &str,
        status: &str,
    ) -> Result<(), HkError> {
        if status != "available" && status != "ignored" {
            return Err(HkError::Validation(format!(
                "Invalid status '{}'; must be 'available' or 'ignored'",
                status
            )));
        }

        let rows = self.conn.execute(
            "UPDATE repository_items
             SET status = ?1
             WHERE repo_id = ?2 AND kind = ?3 AND item_id = ?4",
            params![status, repo_id, kind.as_str(), item_id],
        )?;

        if rows == 0 {
            return Err(HkError::NotFound(format!(
                "Repository item '{}/{}' not found",
                repo_id, item_id
            )));
        }

        Ok(())
    }

    /// List all items for a given repository.
    pub fn list_repository_items(&self, repo_id: &str) -> Result<Vec<RepositoryItem>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT repo_id, kind, item_id, name, description, subpath, status
             FROM repository_items
             WHERE repo_id = ?1
             ORDER BY name ASC",
        )?;

        let rows = stmt
            .query_map(params![repo_id], row_to_repository_item)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    /// Retrieve a single repository with all its items and dynamic install statuses.
    pub fn get_repository_with_items(&self, id: &str) -> Result<Option<RepositoryWithItems>, HkError> {
        let repo = match self.get_repository(id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let items = self.list_repository_items(&repo.id)?;
        let extensions = self.list_extensions(None, None).unwrap_or_default();
        let items_with_status = correlate_items_with_extensions(&repo, items, &extensions);

        Ok(Some(RepositoryWithItems {
            repository: repo,
            items: items_with_status,
        }))
    }

    /// List all repositories with their items and live install statuses.
    pub fn list_repositories_with_items(&self) -> Result<Vec<RepositoryWithItems>, HkError> {
        let repos = self.list_repositories()?;
        if repos.is_empty() {
            return Ok(Vec::new());
        }

        let extensions = self.list_extensions(None, None).unwrap_or_default();
        let mut results = Vec::with_capacity(repos.len());

        for repo in repos {
            let items = self.list_repository_items(&repo.id)?;
            let items_with_status = correlate_items_with_extensions(&repo, items, &extensions);
            results.push(RepositoryWithItems {
                repository: repo,
                items: items_with_status,
            });
        }

        Ok(results)
    }

    /// Auto-seed repositories table from existing installed Git extensions.
    pub fn seed_repositories_from_extensions(&self) -> Result<(), HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT
                COALESCE(install_url_resolved, install_url, json_extract(source_json, '$.url')) AS repo_url,
                install_branch,
                install_revision
             FROM extensions
             WHERE (install_type = 'git' OR json_extract(source_json, '$.origin') = 'git')
               AND COALESCE(install_url_resolved, install_url, json_extract(source_json, '$.url')) IS NOT NULL
               AND COALESCE(install_url_resolved, install_url, json_extract(source_json, '$.url')) != ''",
        )?;

        let rows: Vec<(String, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let now = chrono::Utc::now().to_rfc3339();
        for (repo_url, branch, revision) in rows {
            let trimmed = repo_url.trim().to_string();
            let name = derive_repo_name(&trimmed);
            let id = uuid::Uuid::new_v4().to_string();

            let _ = self.conn.execute(
                "INSERT OR IGNORE INTO repositories (id, url, name, branch, last_commit_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, trimmed, name, branch, revision, now],
            );
        }

        Ok(())
    }
}

fn row_to_repository(row: &rusqlite::Row) -> rusqlite::Result<Repository> {
    let id: String = row.get(0)?;
    let url: String = row.get(1)?;
    let name: String = row.get(2)?;
    let branch: Option<String> = row.get(3)?;
    let auth_token: Option<String> = row.get(4)?;
    let last_synced_at: Option<String> = row.get(5)?;
    let last_commit_hash: Option<String> = row.get(6)?;
    let last_commit_message: Option<String> = row.get(7)?;
    let last_sync_error: Option<String> = row.get(8)?;
    let created_at: String = row.get(9)?;

    Ok(Repository {
        id,
        url,
        name,
        branch,
        auth_token,
        last_synced_at: last_synced_at.map(parse_dt),
        last_commit_hash,
        last_commit_message,
        last_sync_error,
        created_at: parse_dt(created_at),
    })
}

fn row_to_repository_item(row: &rusqlite::Row) -> rusqlite::Result<RepositoryItem> {
    let repo_id: String = row.get(0)?;
    let kind_str: String = row.get(1)?;
    let item_id: String = row.get(2)?;
    let name: String = row.get(3)?;
    let description: String = row.get(4)?;
    let subpath: String = row.get(5)?;
    let status: String = row.get(6)?;

    let kind = kind_str
        .parse::<ExtensionKind>()
        .unwrap_or(ExtensionKind::Skill);

    Ok(RepositoryItem {
        repo_id,
        kind,
        item_id,
        name,
        description,
        subpath,
        status,
    })
}

/// Helper to correlate repository items against installed extensions.
fn correlate_items_with_extensions(
    repo: &Repository,
    items: Vec<RepositoryItem>,
    extensions: &[Extension],
) -> Vec<RepositoryItemWithInstallStatus> {
    let repo_pack = crate::scanner::extract_pack_from_url(&repo.url).map(|p| p.to_lowercase());

    items
        .into_iter()
        .map(|item| {
            let mut installed_agents: Vec<InstalledAgentInfo> = Vec::new();

            for ext in extensions {
                if ext.kind != item.kind {
                    continue;
                }

                // Check URL match or pack match
                let ext_url = ext
                    .install_meta
                    .as_ref()
                    .and_then(|m| m.url_resolved.as_deref().or(m.url.as_deref()))
                    .or(ext.source.url.as_deref());

                let url_matches = ext_url.is_some_and(|u| u.trim() == repo.url.trim());
                let pack_matches = repo_pack.is_some()
                    && ext.pack.as_ref().map(|p| p.to_lowercase()) == repo_pack;

                if !url_matches && !pack_matches {
                    continue;
                }

                // Check item name or subpath match
                let ext_subpath = ext.install_meta.as_ref().and_then(|m| m.subpath.as_deref());
                let subpath_matches = ext_subpath.is_some_and(|sp| {
                    sp == item.subpath || sp == item.item_id || sp.ends_with(&item.item_id)
                });
                let name_matches = ext.name.eq_ignore_ascii_case(&item.name);

                if subpath_matches || name_matches {
                    let revision = ext
                        .install_meta
                        .as_ref()
                        .and_then(|m| m.revision.clone())
                        .or_else(|| ext.source.commit_hash.clone());

                    for agent in &ext.agents {
                        installed_agents.push(InstalledAgentInfo {
                            agent: agent.clone(),
                            scope: ext.scope.clone(),
                            extension_id: ext.id.clone(),
                            revision: revision.clone(),
                        });
                    }
                }
            }

            RepositoryItemWithInstallStatus {
                repo_id: item.repo_id,
                kind: item.kind,
                item_id: item.item_id,
                name: item.name,
                description: item.description,
                subpath: item.subpath,
                status: item.status,
                installed_agents,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;

    #[test]
    fn test_repository_crud() {
        let (store, _dir) = test_store();

        // Add
        let new_repo = NewRepository {
            id: None,
            url: "https://github.com/foo/bar.git".into(),
            name: None,
            branch: Some("main".into()),
            auth_token: None,
        };
        let repo = store.add_repository(&new_repo).unwrap();
        assert_eq!(repo.name, "foo/bar");
        assert_eq!(repo.branch.as_deref(), Some("main"));

        // Duplicate URL rejects
        let dup = store.add_repository(&new_repo);
        assert!(matches!(dup, Err(HkError::Conflict(_))));

        // Get by ID & URL
        let found = store.get_repository(&repo.id).unwrap().unwrap();
        assert_eq!(found.id, repo.id);
        let found_url = store.get_repository_by_url(&repo.url).unwrap().unwrap();
        assert_eq!(found_url.id, repo.id);

        // List
        let all = store.list_repositories().unwrap();
        assert_eq!(all.len(), 1);

        // Sync update
        let sync_res = RepoSyncResult {
            repo_id: repo.id.clone(),
            commit_hash: "abc1234".into(),
            commit_message: "Initial sync".into(),
            items_count: 2,
            synced_at: chrono::Utc::now(),
        };
        store.update_repository_sync(&repo.id, &sync_res).unwrap();
        let updated = store.get_repository(&repo.id).unwrap().unwrap();
        assert_eq!(updated.last_commit_hash.as_deref(), Some("abc1234"));
        assert_eq!(updated.last_commit_message.as_deref(), Some("Initial sync"));

        // Delete
        let deleted = store.delete_repository(&repo.id).unwrap();
        assert!(deleted);
        assert!(store.get_repository(&repo.id).unwrap().is_none());
    }

    #[test]
    fn test_repository_items_and_pruning() {
        let (store, _dir) = test_store();

        let repo = store
            .add_repository(&NewRepository {
                id: None,
                url: "https://github.com/org/skills.git".into(),
                name: Some("Skills".into()),
                branch: None,
                auth_token: None,
            })
            .unwrap();

        let item1 = RepositoryItem {
            repo_id: repo.id.clone(),
            kind: ExtensionKind::Skill,
            item_id: "weather".into(),
            name: "Weather".into(),
            description: "Check weather".into(),
            subpath: "skills/weather".into(),
            status: "available".into(),
        };
        let item2 = RepositoryItem {
            repo_id: repo.id.clone(),
            kind: ExtensionKind::Skill,
            item_id: "calculator".into(),
            name: "Calculator".into(),
            description: "Math tool".into(),
            subpath: "skills/calculator".into(),
            status: "available".into(),
        };

        store.upsert_repository_item(&item1).unwrap();
        store.upsert_repository_item(&item2).unwrap();

        let items = store.list_repository_items(&repo.id).unwrap();
        assert_eq!(items.len(), 2);

        // Set status to ignored
        store
            .set_repository_item_status(&repo.id, ExtensionKind::Skill, "weather", "ignored")
            .unwrap();
        let items = store.list_repository_items(&repo.id).unwrap();
        let weather = items.iter().find(|i| i.item_id == "weather").unwrap();
        assert_eq!(weather.status, "ignored");

        // Re-upsert weather: status MUST remain 'ignored'
        let mut item1_updated = item1.clone();
        item1_updated.description = "Updated weather description".into();
        store.upsert_repository_item(&item1_updated).unwrap();
        let items = store.list_repository_items(&repo.id).unwrap();
        let weather = items.iter().find(|i| i.item_id == "weather").unwrap();
        assert_eq!(weather.description, "Updated weather description");
        assert_eq!(weather.status, "ignored");

        // Pruning: remove calculator
        store
            .prune_repository_items(&repo.id, ExtensionKind::Skill, &["weather".into()])
            .unwrap();
        let items = store.list_repository_items(&repo.id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_id, "weather");

        // Cascade delete on repo deletion
        store.delete_repository(&repo.id).unwrap();
        let items = store.list_repository_items(&repo.id).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_correlate_items_with_extensions() {
        let (store, _dir) = test_store();

        let repo = store
            .add_repository(&NewRepository {
                id: None,
                url: "https://github.com/anthropics/anthropic-quickstarts.git".into(),
                name: None,
                branch: None,
                auth_token: None,
            })
            .unwrap();

        let item = RepositoryItem {
            repo_id: repo.id.clone(),
            kind: ExtensionKind::Skill,
            item_id: "computer-use-demo".into(),
            name: "computer-use-demo".into(),
            description: "Anthropic Computer Use".into(),
            subpath: "computer-use-demo".into(),
            status: "available".into(),
        };
        store.upsert_repository_item(&item).unwrap();

        // Initially no extension installed
        let with_items = store.get_repository_with_items(&repo.id).unwrap().unwrap();
        assert_eq!(with_items.items.len(), 1);
        assert!(with_items.items[0].installed_agents.is_empty());

        // Install extension into store
        let mut ext = sample_extension();
        ext.name = "computer-use-demo".into();
        ext.kind = ExtensionKind::Skill;
        ext.agents = vec!["claude".into()];
        ext.scope = ConfigScope::Global;
        ext.install_meta = Some(InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/anthropics/anthropic-quickstarts.git".into()),
            url_resolved: None,
            branch: Some("main".into()),
            subpath: Some("computer-use-demo".into()),
            revision: Some("commit-aaa".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        // Correlate: installed_agents now populated
        let with_items = store.get_repository_with_items(&repo.id).unwrap().unwrap();
        assert_eq!(with_items.items[0].installed_agents.len(), 1);
        assert_eq!(with_items.items[0].installed_agents[0].agent, "claude");
        assert_eq!(
            with_items.items[0].installed_agents[0].revision.as_deref(),
            Some("commit-aaa")
        );
    }
}
