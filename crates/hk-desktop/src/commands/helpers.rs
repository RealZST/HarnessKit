use hk_core::{adapter, auditor::{self, Auditor}, models::*, scanner};
use std::path::PathBuf;

/// Information about a skill's physical location on disk.
pub(super) struct SkillLocation {
    /// The filesystem entry (directory or .md file).
    pub(super) entry_path: PathBuf,
    /// The SKILL.md file (same as entry_path for standalone .md files).
    pub(super) skill_file: PathBuf,
    /// The parent skill directory that contained this entry.
    pub(super) skill_dir: PathBuf,
}

/// Scan all `adapters` for the skill whose stable ID matches `ext_id`,
/// restricting to adapters whose names appear in `agent_filter`.
///
/// Returns `Some(SkillLocation)` on the first match, or `None`.
pub(super) fn find_skill_by_id(
    adapters: &[Box<dyn adapter::AgentAdapter>],
    ext_id: &str,
    agent_filter: &[String],
) -> Option<SkillLocation> {
    for a in adapters {
        if !agent_filter.contains(&a.name().to_string()) { continue; }
        for skill_dir in a.skill_dirs() {
            let Ok(entries) = std::fs::read_dir(&skill_dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let skill_file = if path.is_dir() {
                    let md = path.join("SKILL.md");
                    if md.exists() { md } else { path.join("SKILL.md.disabled") }
                } else if path.extension().is_some_and(|e| e == "md" || e == "disabled") {
                    path.clone()
                } else { continue };
                if !skill_file.exists() { continue; }
                let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(||
                    path.file_stem().unwrap_or_default().to_string_lossy().to_string()
                );
                if scanner::stable_id_for(&name, "skill", a.name()) == ext_id {
                    return Some(SkillLocation { entry_path: path, skill_file, skill_dir: skill_dir.clone() });
                }
            }
        }
    }
    None
}

/// Audit a single extension by name (best-effort).
/// Takes owned extensions list so the caller can release the Mutex before calling this.
/// Returns audit results to be stored by the caller.
pub(super) fn audit_extension_by_name(name: &str, extensions: &[Extension], adapters: &[Box<dyn adapter::AgentAdapter>]) -> Vec<AuditResult> {
    let auditor = Auditor::new();
    let mut results = Vec::new();
    for ext in extensions {
        if ext.name != name { continue; }
        let input = match ext.kind {
            ExtensionKind::Skill => {
                let (content, file_path) = if let Some(loc) = find_skill_by_id(adapters, &ext.id, &ext.agents) {
                    (
                        std::fs::read_to_string(&loc.skill_file).unwrap_or_default(),
                        loc.skill_file.to_string_lossy().to_string(),
                    )
                } else {
                    (String::new(), ext.name.clone())
                };
                auditor::AuditInput {
                    extension_id: ext.id.clone(),
                    kind: ext.kind,
                    name: ext.name.clone(),
                    content,
                    source: ext.source.clone(),
                    file_path,
                    mcp_command: None,
                    mcp_args: vec![],
                    mcp_env: Default::default(),
                    installed_at: ext.installed_at,
                    updated_at: ext.updated_at,
                    permissions: ext.permissions.clone(),
                    cli_parent_id: ext.cli_parent_id.clone(),
                    cli_meta: ext.cli_meta.clone(),
                    child_permissions: vec![],
                }
            }
            _ => continue,
        };
        results.push(auditor.audit(&input));
    }
    results
}

/// Validate that a path is within a known agent directory, registered project, or the app data dir.
pub(super) fn is_path_within_allowed_dirs(path: &std::path::Path, state: &super::AppState) -> Result<bool, String> {
    let canonical = path.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let adapters = adapter::all_adapters();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let projects = store.list_projects().unwrap_or_default();
    let custom_paths = store.list_all_custom_config_paths().unwrap_or_default();

    let allowed = adapters.iter().any(|a| {
            a.base_dir().canonicalize().is_ok_and(|d| canonical.starts_with(&d))
                || a.skill_dirs().iter().any(|sd| sd.canonicalize().is_ok_and(|d| canonical.starts_with(&d)))
                || a.mcp_config_path().canonicalize().is_ok_and(|d| canonical == d)
                || a.global_settings_files().iter().any(|f| f.canonicalize().is_ok_and(|d| canonical == d))
        })
        || projects.iter().any(|p| {
            std::path::Path::new(&p.path).canonicalize().is_ok_and(|d| canonical.starts_with(&d))
        })
        || custom_paths.iter().any(|p| {
            std::path::Path::new(p).canonicalize().is_ok_and(|d| canonical.starts_with(&d))
        })
        || dirs::home_dir().map(|h| h.join(".harnesskit"))
            .and_then(|d| d.canonicalize().ok())
            .is_some_and(|d| canonical.starts_with(&d));
    Ok(allowed)
}

#[derive(serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// Children of a directory. `None` for files.
    pub children: Option<Vec<FileEntry>>,
}

pub fn list_dir_entries(dir: &std::path::Path, depth: u8) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    let mut read = std::fs::read_dir(dir).map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();
    // Sort: SKILL.md first, then directories, then files, alphabetically within each group
    read.sort_by(|a, b| {
        let a_name = a.file_name();
        let b_name = b.file_name();
        let a_skill = a_name == "SKILL.md";
        let b_skill = b_name == "SKILL.md";
        if a_skill != b_skill { return b_skill.cmp(&a_skill); }
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| a_name.cmp(&b_name))
    });
    for entry in read {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs
        if name.starts_with('.') { continue; }
        let path = entry.path();
        let is_dir = path.is_dir();
        let children = if is_dir && depth < 1 {
            Some(list_dir_entries(&path, depth + 1)?)
        } else if is_dir {
            // Beyond depth limit, return empty children (frontend knows it's a dir)
            Some(vec![])
        } else {
            None
        };
        entries.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            children,
        });
    }
    Ok(entries)
}
