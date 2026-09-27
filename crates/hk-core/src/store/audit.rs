//! Audit result persistence and queries.

use super::*;

impl Store {
    pub fn insert_audit_result(&self, result: &AuditResult) -> Result<(), HkError> {
        self.conn.execute(
            "INSERT INTO audit_results (extension_id, findings_json, trust_score, audited_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                result.extension_id,
                serde_json::to_string(&result.findings)?,
                result.trust_score as i32,
                result.audited_at.to_rfc3339(),
            ],
        )?;
        self.update_trust_score(&result.extension_id, result.trust_score)?;
        Ok(())
    }
    pub fn get_audit_results(&self, extension_id: &str) -> Result<Vec<AuditResult>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT extension_id, findings_json, trust_score, audited_at
             FROM audit_results WHERE extension_id = ?1 ORDER BY audited_at DESC",
        )?;
        let rows = stmt.query_map(params![extension_id], |row| {
            let findings_json: String = row.get(1)?;
            let audited_at_str: String = row.get(3)?;
            Ok(AuditResult {
                extension_id: row.get(0)?,
                findings: serde_json::from_str(&findings_json).unwrap_or_default(),
                trust_score: row.get::<_, i32>(2)? as u8,
                audited_at: DateTime::parse_from_rfc3339(&audited_at_str)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    /// Get the latest audit result for every non-hidden extension (one per extension_id).
    pub fn list_latest_audit_results(&self) -> Result<Vec<AuditResult>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT a.extension_id, a.findings_json, a.trust_score, a.audited_at
             FROM audit_results a
             INNER JOIN (
                 SELECT extension_id, MAX(audited_at) AS max_at
                 FROM audit_results GROUP BY extension_id
             ) latest ON a.extension_id = latest.extension_id AND a.audited_at = latest.max_at
             INNER JOIN extensions e ON a.extension_id = e.id",
        )?;
        let rows = stmt.query_map([], |row| {
            let findings_json: String = row.get(1)?;
            let audited_at_str: String = row.get(3)?;
            Ok(AuditResult {
                extension_id: row.get(0)?,
                findings: serde_json::from_str(&findings_json).unwrap_or_default(),
                trust_score: row.get::<_, i32>(2)? as u8,
                audited_at: DateTime::parse_from_rfc3339(&audited_at_str)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    /// Count audit findings by severity across all latest audit results.
    /// Uses a single SQL query (list_latest_audit_results) then aggregates
    /// in Rust, replacing the previous N+1 pattern of querying per-extension.
    pub fn count_latest_findings_by_severity(&self) -> Result<std::collections::HashMap<String, usize>, HkError> {
        let results = self.list_latest_audit_results()?;
        let mut counts = std::collections::HashMap::new();
        for result in &results {
            for finding in &result.findings {
                *counts.entry(finding.severity.as_str().to_string()).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;
    #[test]
    fn test_insert_and_get_audit_result() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        let audit = AuditResult {
            extension_id: ext.id.clone(),
            findings: vec![AuditFinding {
                rule_id: "prompt-injection".into(),
                severity: Severity::Critical,
                message: "Found prompt injection pattern".into(),
                location: "SKILL.md:5".into(),
            }],
            trust_score: 75,
            audited_at: Utc::now(),
        };
        store.insert_audit_result(&audit).unwrap();

        let results = store.get_audit_results(&ext.id).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].trust_score, 75);
        assert_eq!(results[0].findings.len(), 1);
        assert_eq!(results[0].findings[0].rule_id, "prompt-injection");
    }
    #[test]
    fn test_count_latest_findings_by_severity() {
        let (store, _dir) = test_store();

        // Create two extensions
        let mut ext1 = sample_extension();
        ext1.id = "ext-1".into();
        ext1.name = "ext-one".into();
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = "ext-2".into();
        ext2.name = "ext-two".into();
        store.insert_extension(&ext2).unwrap();

        // Insert audit results for ext1 (2 findings: 1 critical, 1 high)
        let audit1 = AuditResult {
            extension_id: "ext-1".into(),
            findings: vec![
                AuditFinding {
                    rule_id: "rule-a".into(),
                    severity: Severity::Critical,
                    message: "bad".into(),
                    location: "file:1".into(),
                },
                AuditFinding {
                    rule_id: "rule-b".into(),
                    severity: Severity::High,
                    message: "also bad".into(),
                    location: "file:2".into(),
                },
            ],
            trust_score: 60,
            audited_at: Utc::now(),
        };
        store.insert_audit_result(&audit1).unwrap();

        // Insert audit results for ext2 (1 finding: medium)
        let audit2 = AuditResult {
            extension_id: "ext-2".into(),
            findings: vec![AuditFinding {
                rule_id: "rule-c".into(),
                severity: Severity::Medium,
                message: "meh".into(),
                location: "file:3".into(),
            }],
            trust_score: 80,
            audited_at: Utc::now(),
        };
        store.insert_audit_result(&audit2).unwrap();

        let counts = store.count_latest_findings_by_severity().unwrap();
        assert_eq!(counts.get("critical").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("high").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("medium").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("low").copied().unwrap_or(0), 0);
    }
    #[test]
    fn test_count_latest_findings_uses_only_latest_audit() {
        let (store, _dir) = test_store();

        let mut ext = sample_extension();
        ext.id = "ext-latest".into();
        ext.name = "ext-latest".into();
        store.insert_extension(&ext).unwrap();

        // Insert an old audit with 1 critical finding
        let old_audit = AuditResult {
            extension_id: "ext-latest".into(),
            findings: vec![AuditFinding {
                rule_id: "rule-old".into(),
                severity: Severity::Critical,
                message: "old issue".into(),
                location: "file:1".into(),
            }],
            trust_score: 50,
            audited_at: chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap().with_timezone(&Utc),
        };
        store.insert_audit_result(&old_audit).unwrap();

        // Insert a newer audit with 0 findings (resolved)
        let new_audit = AuditResult {
            extension_id: "ext-latest".into(),
            findings: vec![],
            trust_score: 100,
            audited_at: chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap().with_timezone(&Utc),
        };
        store.insert_audit_result(&new_audit).unwrap();

        // Only the latest audit (with 0 findings) should be counted
        let counts = store.count_latest_findings_by_severity().unwrap();
        assert_eq!(counts.get("critical").copied().unwrap_or(0), 0);
    }
}
