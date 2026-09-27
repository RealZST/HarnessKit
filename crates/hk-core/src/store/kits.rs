//! Kits, kit assets, kit config files, and kit sync records.

use super::*;

impl Store {
    pub fn insert_kit(&self, row: &KitRow) -> Result<(), HkError> {
        self.conn.execute(
            "INSERT INTO kits (id, name, description, zip_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                row.id, row.name, row.description, row.zip_path,
                row.created_at.to_rfc3339(),
                row.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
    pub fn update_kit_meta(
        &self,
        id: &str,
        name: &str,
        description: &str,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE kits
                SET name = ?2,
                    description = ?3,
                    updated_at = ?4
              WHERE id = ?1",
            params![id, name, description, updated_at.to_rfc3339()],
        )?;
        Ok(())
    }
    pub fn delete_kit(&self, id: &str) -> Result<(), HkError> {
        self.conn.execute("DELETE FROM kits WHERE id = ?1", params![id])?;
        Ok(())
    }
    pub fn list_kit_rows(&self) -> Result<Vec<KitRow>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, zip_path, created_at, updated_at
               FROM kits
              ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(KitRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                zip_path: row.get(3)?,
                created_at: parse_dt(row.get::<_, String>(4)?),
                updated_at: parse_dt(row.get::<_, String>(5)?),
            })
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }
    pub fn get_kit_row(&self, id: &str) -> Result<Option<KitRow>, HkError> {
        let r = self
            .conn
            .query_row(
                "SELECT id, name, description, zip_path, created_at, updated_at
                   FROM kits WHERE id = ?1",
                params![id],
                |row| {
                    Ok(KitRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        zip_path: row.get(3)?,
                        created_at: parse_dt(row.get::<_, String>(4)?),
                        updated_at: parse_dt(row.get::<_, String>(5)?),
                    })
                },
            )
            .optional()?;
        Ok(r)
    }
    pub fn replace_kit_assets(
        &self,
        kit_id: &str,
        rows: &[KitAssetRow],
    ) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM kit_assets WHERE kit_id = ?1", params![kit_id])?;
        for r in rows {
            tx.execute(
                "INSERT INTO kit_assets (kit_id, extension_id, asset_name, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params![r.kit_id, r.extension_id, r.asset_name, r.position],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn list_kit_assets(&self, kit_id: &str) -> Result<Vec<KitAssetRow>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT kit_id, extension_id, asset_name, position
               FROM kit_assets WHERE kit_id = ?1
              ORDER BY position",
        )?;
        let rows = stmt.query_map(params![kit_id], |row| {
            Ok(KitAssetRow {
                kit_id: row.get(0)?,
                extension_id: row.get(1)?,
                asset_name: row.get(2)?,
                position: row.get(3)?,
            })
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }
    pub fn replace_kit_config_files(
        &self,
        kit_id: &str,
        rows: &[KitConfigFileRow],
    ) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM kit_config_files WHERE kit_id = ?1", params![kit_id])?;
        for r in rows {
            tx.execute(
                "INSERT INTO kit_config_files (kit_id, agent, category, source_path, source_file_name, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    r.kit_id, r.agent, r.category.as_str(),
                    r.source_path, r.source_file_name, r.position,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn list_kit_config_files(&self, kit_id: &str) -> Result<Vec<KitConfigFileRow>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT kit_id, agent, category, source_path, source_file_name, position
               FROM kit_config_files WHERE kit_id = ?1
              ORDER BY position",
        )?;
        let rows = stmt.query_map(params![kit_id], |row| {
            let cat_s: String = row.get(2)?;
            let category = cat_s.parse::<ConfigCategory>().unwrap_or_else(|_| {
                eprintln!(
                    "[hk] list_kit_config_files: unknown ConfigCategory {:?}, falling back to Settings",
                    cat_s
                );
                ConfigCategory::Settings
            });
            Ok(KitConfigFileRow {
                kit_id: row.get(0)?,
                agent: row.get(1)?,
                category,
                source_path: row.get(3)?,
                source_file_name: row.get(4)?,
                position: row.get(5)?,
            })
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }
    pub fn upsert_sync_record(&self, row: &SyncRecordRow) -> Result<(), HkError> {
        let paths_json = serde_json::to_string(&row.written_paths)
            .map_err(|e| HkError::Internal(format!("written_paths serialize: {e}")))?;
        self.conn.execute(
            "INSERT INTO kit_sync_records (id, kit_id, project_path, agent_name, written_paths, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(kit_id, project_path, agent_name) DO UPDATE SET
                 id = excluded.id,
                 written_paths = excluded.written_paths,
                 synced_at = excluded.synced_at",
            params![
                row.id, row.kit_id, row.project_path, row.agent_name,
                paths_json, row.synced_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
    pub fn get_sync_record(
        &self,
        kit_id: &str,
        project_path: &str,
        agent_name: &str,
    ) -> Result<Option<SyncRecordRow>, HkError> {
        let row = self
            .conn
            .query_row(
                "SELECT id, kit_id, project_path, agent_name, written_paths, synced_at
                   FROM kit_sync_records
                  WHERE kit_id = ?1 AND project_path = ?2 AND agent_name = ?3",
                params![kit_id, project_path, agent_name],
                |row| {
                    let paths_json: String = row.get(4)?;
                    let written_paths: Vec<String> =
                        serde_json::from_str(&paths_json).unwrap_or_default();
                    Ok(SyncRecordRow {
                        id: row.get(0)?,
                        kit_id: row.get(1)?,
                        project_path: row.get(2)?,
                        agent_name: row.get(3)?,
                        written_paths,
                        synced_at: parse_dt(row.get::<_, String>(5)?),
                    })
                },
            )
            .optional()?;
        Ok(row)
    }
    pub fn delete_sync_record(
        &self,
        kit_id: &str,
        project_path: &str,
        agent_name: &str,
    ) -> Result<(), HkError> {
        self.conn.execute(
            "DELETE FROM kit_sync_records
              WHERE kit_id = ?1 AND project_path = ?2 AND agent_name = ?3",
            params![kit_id, project_path, agent_name],
        )?;
        Ok(())
    }
    pub fn list_sync_records_for_kit(&self, kit_id: &str) -> Result<Vec<SyncRecordRow>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kit_id, project_path, agent_name, written_paths, synced_at
               FROM kit_sync_records WHERE kit_id = ?1
              ORDER BY synced_at DESC",
        )?;
        let rows = stmt.query_map(params![kit_id], |row| {
            let paths_json: String = row.get(4)?;
            let written_paths: Vec<String> =
                serde_json::from_str(&paths_json).unwrap_or_default();
            Ok(SyncRecordRow {
                id: row.get(0)?,
                kit_id: row.get(1)?,
                project_path: row.get(2)?,
                agent_name: row.get(3)?,
                written_paths,
                synced_at: parse_dt(row.get::<_, String>(5)?),
            })
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }
    /// All sync records across every kit, ordered by sync time descending.
    /// Powers the per-project install view in the Kits UI.
    pub fn list_all_sync_records(&self) -> Result<Vec<SyncRecordRow>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kit_id, project_path, agent_name, written_paths, synced_at
               FROM kit_sync_records
              ORDER BY synced_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let paths_json: String = row.get(4)?;
            let written_paths: Vec<String> =
                serde_json::from_str(&paths_json).unwrap_or_default();
            Ok(SyncRecordRow {
                id: row.get(0)?,
                kit_id: row.get(1)?,
                project_path: row.get(2)?,
                agent_name: row.get(3)?,
                written_paths,
                synced_at: parse_dt(row.get::<_, String>(5)?),
            })
        })?;
        let out: Result<Vec<_>, _> = rows.collect();
        Ok(out?)
    }
}
