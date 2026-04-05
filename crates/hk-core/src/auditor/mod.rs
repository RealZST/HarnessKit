pub mod rules;

use crate::models::{AuditFinding, AuditResult, Severity};
use chrono::Utc;

/// Content to audit — the raw text + metadata
#[derive(Clone)]
pub struct AuditInput {
    pub extension_id: String,
    pub kind: crate::models::ExtensionKind,
    pub name: String,
    pub content: String,
    pub source: crate::models::Source,
    pub file_path: String,
    pub mcp_command: Option<String>,
    pub mcp_args: Vec<String>,
    pub mcp_env: std::collections::HashMap<String, String>,
    pub installed_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub permissions: Vec<crate::models::Permission>,
    // Parent CLI link (for child skills/MCPs)
    pub cli_parent_id: Option<String>,
    // CLI-specific fields
    pub cli_meta: Option<crate::models::CliMeta>,
    pub child_permissions: Vec<crate::models::Permission>,
    /// Repo-based source group (e.g. "owner/repo"), if known
    pub pack: Option<String>,
}

pub trait AuditRule: Send + Sync {
    fn id(&self) -> &str;
    fn severity(&self) -> Severity;
    fn check(&self, input: &AuditInput) -> Vec<AuditFinding>;
}

/// Strip invisible Unicode characters that could hide malicious content.
/// Inspired by AgentSeal's deobfuscation layer.
fn deobfuscate(input: &str) -> String {
    input.chars().filter(|c| {
        !matches!(c,
            '\u{200B}'..='\u{200F}' | // zero-width spaces, directional marks
            '\u{202A}'..='\u{202E}' | // directional formatting
            '\u{2060}'..='\u{2064}' | // word joiner, invisible operators
            '\u{2066}'..='\u{2069}' | // isolate formatting
            '\u{FEFF}'              | // byte order mark
            '\u{00AD}'              | // soft hyphen
            '\u{180E}'              | // Mongolian vowel separator
            '\u{FE00}'..='\u{FE0F}' | // variation selectors
            '\u{E0100}'..='\u{E01EF}'  // variation selectors supplement
        )
    }).collect()
}

pub struct Auditor {
    pub rules: Vec<Box<dyn AuditRule>>,
}

impl Default for Auditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Auditor {
    pub fn new() -> Self {
        Self {
            rules: rules::all_rules(),
        }
    }

    pub fn audit(&self, input: &AuditInput) -> AuditResult {
        // Deobfuscate content to detect hidden malicious instructions
        let clean_input = AuditInput {
            content: deobfuscate(&input.content),
            ..input.clone()
        };
        let mut findings = Vec::new();
        for rule in &self.rules {
            findings.extend(rule.check(&clean_input));
        }
        let trust_score = compute_trust_score(&findings);
        AuditResult {
            extension_id: input.extension_id.clone(),
            findings,
            trust_score,
            audited_at: Utc::now(),
        }
    }

    /// Audit multiple extensions, with batch-level duplicate detection.
    pub fn audit_batch(&self, inputs: &[AuditInput]) -> Vec<AuditResult> {
        let mut results: Vec<AuditResult> = inputs.iter().map(|input| self.audit(input)).collect();

        // Batch pass: detect name collisions across extensions of the same kind.
        // Skip child skills that share a CLI parent — they're meant to have the same name.
        let mut name_map: std::collections::HashMap<(&str, crate::models::ExtensionKind), Vec<usize>> = std::collections::HashMap::new();
        for (idx, input) in inputs.iter().enumerate() {
            name_map.entry((input.name.as_str(), input.kind)).or_default().push(idx);
        }
        for ((name, kind), indices) in &name_map {
            // If all extensions in this group share the same CLI parent, it's not a conflict
            let parents: std::collections::HashSet<_> = indices.iter()
                .filter_map(|&idx| inputs[idx].cli_parent_id.as_deref())
                .collect();
            if parents.len() == 1 && indices.iter().all(|&idx| inputs[idx].cli_parent_id.is_some()) {
                continue;
            }
            if indices.len() > 1 {
                for &idx in indices {
                    results[idx].findings.push(AuditFinding {
                        rule_id: "duplicate-conflict".into(),
                        severity: Severity::Low,
                        message: format!(
                            "Name collision: {} other {}(s) share the name \"{}\"",
                            indices.len() - 1,
                            kind.as_str(),
                            name
                        ),
                        location: inputs[idx].file_path.clone(),
                    });
                    results[idx].trust_score = compute_trust_score(&results[idx].findings);
                }
            }
        }
        results
    }
}

/// Compute trust score with same-rule deduplication.
/// First finding per rule_id deducts the full severity amount.
/// Subsequent findings of the same rule_id deduct only 1 point each.
pub fn compute_trust_score(findings: &[AuditFinding]) -> u8 {
    let mut seen_rules = std::collections::HashSet::new();
    let mut total_deduction: u32 = 0;
    for f in findings {
        if seen_rules.contains(f.rule_id.as_str()) {
            // Repeated hit of same rule — minimal deduction
            total_deduction += 1;
        } else {
            seen_rules.insert(f.rule_id.as_str());
            total_deduction += f.severity.deduction() as u32;
        }
    }
    100u8.saturating_sub(total_deduction.min(100) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn test_compute_trust_score_no_findings() {
        assert_eq!(compute_trust_score(&[]), 100);
    }

    #[test]
    fn test_compute_trust_score_one_critical() {
        let findings = vec![AuditFinding {
            rule_id: "test".into(),
            severity: Severity::Critical,
            message: "bad".into(),
            location: "file:1".into(),
        }];
        assert_eq!(compute_trust_score(&findings), 75);
    }

    #[test]
    fn test_compute_trust_score_floors_at_zero() {
        let findings: Vec<AuditFinding> = (0..5)
            .map(|i| AuditFinding {
                rule_id: format!("test-{i}"),
                severity: Severity::Critical,
                message: "bad".into(),
                location: "file:1".into(),
            })
            .collect();
        assert_eq!(compute_trust_score(&findings), 0);
    }

    #[test]
    fn test_compute_trust_score_mixed() {
        let findings = vec![
            AuditFinding { rule_id: "a".into(), severity: Severity::Critical, message: "".into(), location: "".into() },
            AuditFinding { rule_id: "b".into(), severity: Severity::Low, message: "".into(), location: "".into() },
        ];
        // 100 - 25 - 3 = 72
        assert_eq!(compute_trust_score(&findings), 72);
    }

    #[test]
    fn test_auditor_runs_all_enabled_rules() {
        let auditor = Auditor::new();
        assert_eq!(auditor.rules.len(), 18);
    }

    #[test]
    fn test_compute_trust_score_same_rule_dedup() {
        // 3 findings from the same rule: first = -25, next two = -1 each
        let findings: Vec<AuditFinding> = (0..3)
            .map(|_| AuditFinding {
                rule_id: "prompt-injection".into(),
                severity: Severity::Critical,
                message: "bad".into(),
                location: "file:1".into(),
            })
            .collect();
        // 100 - 25 - 1 - 1 = 73 (not 100 - 75 = 25)
        assert_eq!(compute_trust_score(&findings), 73);
    }

    #[test]
    fn test_compute_trust_score_different_rules_no_dedup() {
        // 3 findings from different rules: each deducts full amount
        let findings = vec![
            AuditFinding { rule_id: "prompt-injection".into(), severity: Severity::Critical, message: "".into(), location: "".into() },
            AuditFinding { rule_id: "rce".into(), severity: Severity::Critical, message: "".into(), location: "".into() },
            AuditFinding { rule_id: "safety-bypass".into(), severity: Severity::Critical, message: "".into(), location: "".into() },
        ];
        // 100 - 25 - 25 - 25 = 25
        assert_eq!(compute_trust_score(&findings), 25);
    }
}
