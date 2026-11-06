//! Store operations for ignored repository skills and repository scan states.

use super::*;

impl Store {
    /// Add or update an ignored skill from a repository.
    pub fn ignore_repo_skill(
        &self,
        repo_url: &str,
        skill_id: &str,
        skill_name: &str,
    ) -> Result<(), HkError> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO ignored_repo_skills (repo_url, skill_id, skill_name, ignored_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(repo_url, skill_id) DO UPDATE SET
               skill_name = excluded.skill_name,
               ignored_at = excluded.ignored_at",
            params![repo_url, skill_id, skill_name, now],
        )?;
        Ok(())
    }

    /// Remove a skill from the ignored repository skills list.
    pub fn unignore_repo_skill(&self, repo_url: &str, skill_id: &str) -> Result<(), HkError> {
        self.conn.execute(
            "DELETE FROM ignored_repo_skills WHERE repo_url = ?1 AND skill_id = ?2",
            params![repo_url, skill_id],
        )?;
        Ok(())
    }

    /// List all currently ignored repository skills.
    pub fn list_ignored_repo_skills(&self) -> Result<Vec<IgnoredRepoSkill>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, repo_url, skill_id, skill_name, ignored_at
             FROM ignored_repo_skills
             ORDER BY repo_url ASC, skill_name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let repo_url: String = row.get(1)?;
                let skill_id: String = row.get(2)?;
                let skill_name: String = row.get(3)?;
                let ignored_at_str: String = row.get(4)?;
                Ok(IgnoredRepoSkill {
                    id,
                    repo_url,
                    skill_id,
                    skill_name,
                    ignored_at: parse_dt(ignored_at_str),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Clear all ignored repository skills.
    pub fn clear_ignored_repo_skills(&self) -> Result<(), HkError> {
        self.conn.execute("DELETE FROM ignored_repo_skills", [])?;
        Ok(())
    }

    /// Get the last scanned revision for a repository.
    pub fn get_repo_scan_state(&self, repo_url: &str) -> Result<Option<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT last_scanned_revision FROM repo_scan_states WHERE repo_url = ?1")?;
        let res = stmt.query_row(params![repo_url], |row| row.get::<_, String>(0));
        match res {
            Ok(rev) => Ok(Some(rev)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Record the last scanned revision for a repository.
    pub fn set_repo_scan_state(&self, repo_url: &str, revision: &str) -> Result<(), HkError> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO repo_scan_states (repo_url, last_scanned_revision, last_scanned_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(repo_url) DO UPDATE SET
               last_scanned_revision = excluded.last_scanned_revision,
               last_scanned_at = excluded.last_scanned_at",
            params![repo_url, revision, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::*;

    #[test]
    fn test_ignored_repo_skills_crud() {
        let (store, _dir) = test_store();

        let list = store.list_ignored_repo_skills().unwrap();
        assert!(list.is_empty());

        store
            .ignore_repo_skill(
                "https://github.com/foo/bar",
                "weather-skill",
                "Weather Checker",
            )
            .unwrap();
        store
            .ignore_repo_skill("https://github.com/foo/bar", "news-skill", "News Reader")
            .unwrap();

        let list = store.list_ignored_repo_skills().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].skill_id, "news-skill");
        assert_eq!(list[1].skill_id, "weather-skill");

        // Upsert update
        store
            .ignore_repo_skill(
                "https://github.com/foo/bar",
                "weather-skill",
                "Updated Weather",
            )
            .unwrap();
        let list = store.list_ignored_repo_skills().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].skill_name, "Updated Weather");

        // Unignore one
        store
            .unignore_repo_skill("https://github.com/foo/bar", "news-skill")
            .unwrap();
        let list = store.list_ignored_repo_skills().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].skill_id, "weather-skill");

        // Clear all
        store.clear_ignored_repo_skills().unwrap();
        let list = store.list_ignored_repo_skills().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_repo_scan_state() {
        let (store, _dir) = test_store();

        let state = store
            .get_repo_scan_state("https://github.com/foo/bar")
            .unwrap();
        assert!(state.is_none());

        store
            .set_repo_scan_state("https://github.com/foo/bar", "commit_abc123")
            .unwrap();
        let state = store
            .get_repo_scan_state("https://github.com/foo/bar")
            .unwrap();
        assert_eq!(state.as_deref(), Some("commit_abc123"));

        store
            .set_repo_scan_state("https://github.com/foo/bar", "commit_def456")
            .unwrap();
        let state = store
            .get_repo_scan_state("https://github.com/foo/bar")
            .unwrap();
        assert_eq!(state.as_deref(), Some("commit_def456"));
    }
}
