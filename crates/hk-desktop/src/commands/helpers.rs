use hk_core::HkError;

/// Validate that a path is within a known agent directory, registered project, or the app data dir.
pub(super) fn is_path_within_allowed_dirs(
    path: &std::path::Path,
    state: &super::AppState,
) -> Result<bool, HkError> {
    let canonical = path.canonicalize()?;
    let adapters = &*state.adapters;
    let store = state.store.lock();
    let projects = store.list_projects().unwrap_or_default();
    let custom_paths = store.list_all_custom_config_paths().unwrap_or_default();

    let allowed = adapters.iter().any(|a| {
        a.base_dir()
            .canonicalize()
            .is_ok_and(|d| canonical.starts_with(&d))
            || a.skill_dirs()
                .iter()
                .any(|sd| sd.canonicalize().is_ok_and(|d| canonical.starts_with(&d)))
            || a.plugin_dirs()
                .iter()
                .any(|pd| pd.canonicalize().is_ok_and(|d| canonical.starts_with(&d)))
            || a.mcp_config_path()
                .canonicalize()
                .is_ok_and(|d| canonical == d)
            || a.global_settings_files()
                .iter()
                .any(|f| f.canonicalize().is_ok_and(|d| canonical == d))
    }) || projects.iter().any(|p| {
        std::path::Path::new(&p.path)
            .canonicalize()
            .is_ok_and(|d| canonical.starts_with(&d))
    }) || custom_paths.iter().any(|p| {
        std::path::Path::new(p)
            .canonicalize()
            .is_ok_and(|d| canonical.starts_with(&d))
    }) || dirs::home_dir()
        .map(|h| h.join(".harnesskit"))
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

pub fn list_dir_entries(dir: &std::path::Path, depth: u8) -> Result<Vec<FileEntry>, HkError> {
    let mut entries = Vec::new();
    let mut read = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect::<Vec<_>>();
    // Sort: SKILL.md first, then directories, then files, alphabetically within each group
    read.sort_by(|a, b| {
        let a_name = a.file_name();
        let b_name = b.file_name();
        let a_skill = a_name == "SKILL.md";
        let b_skill = b_name == "SKILL.md";
        if a_skill != b_skill {
            return b_skill.cmp(&a_skill);
        }
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| a_name.cmp(&b_name))
    });
    for entry in read {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }
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
