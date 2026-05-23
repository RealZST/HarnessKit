//! Shared internals for the kits module.

use std::path::Path;

/// RAII guard that deletes a file on drop unless disarmed. Used to clean up
/// partially-written files (e.g., `<target>.tmp` in pack_kit, imported zip in
/// import_kit) when a subsequent operation fails before commit.
pub struct RemovePathGuard<'a> {
    path: &'a Path,
    armed: bool,
}

impl<'a> RemovePathGuard<'a> {
    pub fn new(p: &'a Path) -> Self {
        Self { path: p, armed: true }
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl<'a> Drop for RemovePathGuard<'a> {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

/// Walk a directory tree, calling `cb(absolute_path, relative_path_string)` for
/// each regular file. Skips symlinks and any directory named `.git`.
/// Relative path separators are normalized to `/` for cross-platform stability.
pub fn walk_dir(
    base: &Path,
    cb: &mut dyn FnMut(&Path, &str) -> std::io::Result<()>,
) -> std::io::Result<()> {
    walk_dir_inner(base, base, cb)
}

fn walk_dir_inner(
    base: &Path,
    cur: &Path,
    cb: &mut dyn FnMut(&Path, &str) -> std::io::Result<()>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(cur)? {
        let entry = entry?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            walk_dir_inner(base, &path, cb)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            cb(&path, &rel)?;
        }
    }
    Ok(())
}
