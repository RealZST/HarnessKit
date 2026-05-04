use hk_core::HkError;

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
