use crate::auditor::{AuditInput, AuditRule};
use crate::models::{AuditFinding, ExtensionKind, Permission, Severity};

pub struct CliCredentialStorage;

impl AuditRule for CliCredentialStorage {
    fn id(&self) -> &str {
        "cli-credential-storage"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Cli {
            return vec![];
        }
        let Some(meta) = &input.cli_meta else {
            return vec![];
        };

        if let Some(cred_path) = &meta.credentials_path {
            let expanded = if let Some(rest) = cred_path.strip_prefix("~/") {
                crate::home_dir().join(rest).to_string_lossy().to_string()
            } else {
                cred_path.clone()
            };

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(&expanded) {
                    let mode = metadata.permissions().mode() & 0o777;
                    if mode > 0o600 {
                        return vec![AuditFinding {
                            rule_id: self.id().into(),
                            severity: self.severity(),
                            message: format!(
                                "Credential file {} has permissions {:04o} (should be 0600)",
                                cred_path, mode
                            ),
                            location: input.file_path.clone(),
                        }];
                    }
                }
            }
            vec![]
        } else if !meta.api_domains.is_empty() {
            vec![AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: format!(
                    "CLI accesses {} API domain(s) but has no known credentials_path — unknown credential storage",
                    meta.api_domains.len()
                ),
                location: input.file_path.clone(),
            }]
        } else {
            vec![]
        }
    }
}

pub struct CliNetworkAccess;

impl AuditRule for CliNetworkAccess {
    fn id(&self) -> &str {
        "cli-network-access"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Cli {
            return vec![];
        }
        let Some(meta) = &input.cli_meta else {
            return vec![];
        };
        if meta.api_domains.len() > 3 {
            vec![AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: format!(
                    "CLI contacts {} API domains — broad network surface ({})",
                    meta.api_domains.len(),
                    meta.api_domains.join(", ")
                ),
                location: input.file_path.clone(),
            }]
        } else {
            vec![]
        }
    }
}

pub struct CliBinarySource;

impl AuditRule for CliBinarySource {
    fn id(&self) -> &str {
        "cli-binary-source"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Cli {
            return vec![];
        }
        let Some(meta) = &input.cli_meta else {
            return vec![];
        };

        match meta.install_method.as_deref() {
            Some(m) if m == "curl" || m == "wget" || m == "curl|sh" || m == "wget|sh" => {
                vec![AuditFinding {
                    rule_id: self.id().into(),
                    severity: Severity::High,
                    message: format!("CLI installed via {} — high risk (unverified binary)", m),
                    location: input.file_path.clone(),
                }]
            }
            Some(m) if m == "npm" || m == "pip" || m == "brew" || m == "cargo" => vec![],
            Some(m) => {
                vec![AuditFinding {
                    rule_id: self.id().into(),
                    severity: Severity::Medium,
                    message: format!("CLI installed via unknown method: {} — medium risk", m),
                    location: input.file_path.clone(),
                }]
            }
            None => {
                if input.pack.is_some() || input.source.url.is_some() {
                    vec![]
                } else {
                    vec![AuditFinding {
                        rule_id: self.id().into(),
                        severity: Severity::Medium,
                        message: "CLI has no known install method — medium risk".into(),
                        location: input.file_path.clone(),
                    }]
                }
            }
        }
    }
}

pub struct CliPermissionScope;

impl AuditRule for CliPermissionScope {
    fn id(&self) -> &str {
        "cli-permission-scope"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Cli {
            return vec![];
        }
        let mut types = std::collections::HashSet::new();
        for perm in &input.child_permissions {
            types.insert(std::mem::discriminant(perm));
        }
        if types.len() > 3 {
            vec![AuditFinding {
                rule_id: self.id().into(),
                severity: self.severity(),
                message: format!(
                    "CLI child skills request {} distinct permission types — broad capability surface",
                    types.len()
                ),
                location: input.file_path.clone(),
            }]
        } else {
            vec![]
        }
    }
}

pub struct CliAggregateRisk;

impl AuditRule for CliAggregateRisk {
    fn id(&self) -> &str {
        "cli-aggregate-risk"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn check(&self, input: &AuditInput) -> Vec<AuditFinding> {
        if input.kind != ExtensionKind::Cli {
            return vec![];
        }
        let has_network = input
            .child_permissions
            .iter()
            .any(|p| matches!(p, Permission::Network { .. }));
        let has_fs = input
            .child_permissions
            .iter()
            .any(|p| matches!(p, Permission::FileSystem { .. }));
        let has_shell = input
            .child_permissions
            .iter()
            .any(|p| matches!(p, Permission::Shell { .. }));

        if has_network && has_fs && has_shell {
            let severity = if input.pack.is_some() || input.source.url.is_some() {
                Severity::Low
            } else {
                Severity::High
            };
            vec![AuditFinding {
                rule_id: self.id().into(),
                severity,
                message: "CLI child skills collectively have network + filesystem + shell — potential data exfiltration path".into(),
                location: input.file_path.clone(),
            }]
        } else {
            vec![]
        }
    }
}
