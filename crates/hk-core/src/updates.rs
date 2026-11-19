//! Finding and applying updates for skills installed from a Git repository
//! or the Marketplace.

use crate::{
    HkError,
    manager::{self, InstallResult},
    marketplace,
    models::*,
    service,
    store::Store,
};
use parking_lot::Mutex;

/// The update status each skill was left with by the last check.
pub fn cached_update_statuses(store: &Store) -> Result<Vec<(String, UpdateStatus)>, HkError> {
    let extensions = store.list_extensions(None, None)?;
    let mut results = Vec::new();
    for ext in extensions {
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        let Some(meta) = ext.install_meta else {
            continue;
        };
        if meta.checked_at.is_none() {
            continue;
        }
        let status = match (meta.revision.as_deref(), meta.remote_revision.as_deref()) {
            (Some(local), Some(remote)) => {
                if local.starts_with(remote) || remote.starts_with(local) {
                    UpdateStatus::UpToDate {
                        remote_hash: remote.to_string(),
                    }
                } else {
                    UpdateStatus::UpdateAvailable {
                        remote_hash: remote.to_string(),
                    }
                }
            }
            (None, Some(remote)) => {
                // No local revision (pre-existing skill) — treat as update available
                UpdateStatus::UpdateAvailable {
                    remote_hash: remote.to_string(),
                }
            }
            _ => {
                if let Some(ref err) = meta.check_error {
                    if err == "removed_from_repo" {
                        UpdateStatus::RemovedFromRepo
                    } else {
                        UpdateStatus::Error {
                            message: err.clone(),
                        }
                    }
                } else {
                    continue; // No remote_revision and no error — nothing to report
                }
            }
        };
        results.push((ext.id, status));
    }
    Ok(results)
}

/// Record a Marketplace source on each skill, given as `(id, name)`, whose
/// name matches exactly one Marketplace skill. Returns whether any did.
pub fn link_to_marketplace(store: &Mutex<Store>, unlinked: &[(String, String)]) -> bool {
    let names: std::collections::HashSet<&str> =
        unlinked.iter().map(|(_, name)| name.as_str()).collect();
    let mut matched = std::collections::HashMap::new();
    for name in names {
        let Ok(results) = marketplace::search_skills(name, 5) else {
            continue;
        };
        let mut exact = results.iter().filter(|r| r.name.eq_ignore_ascii_case(name));
        if let (Some(item), None) = (exact.next(), exact.next()) {
            let git_url = marketplace::git_url_for_source(&item.source);
            let remote_rev = manager::get_remote_head(&git_url).ok();
            matched.insert(name, (git_url, item.skill_id.clone(), remote_rev));
        }
    }

    let store = store.lock();
    let now = chrono::Utc::now();
    for (id, name) in unlinked {
        let Some((git_url, skill_id, remote_rev)) = matched.get(name.as_str()) else {
            continue;
        };
        let meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some(format!("{}/{}", git_url.trim_end_matches(".git"), skill_id)),
            url_resolved: Some(git_url.clone()),
            branch: None,
            subpath: Some(skill_id.clone()).filter(|id| !id.is_empty()),
            revision: remote_rev.clone(),
            remote_revision: remote_rev.clone(),
            checked_at: Some(now),
            check_error: None,
        };
        if let Err(e) = store.set_install_meta(id, &meta) {
            eprintln!("[hk] warning: {e}");
        }
    }
    !matched.is_empty()
}

/// Ask every skill's repository for its latest commit and record the answer.
/// Skills with no recorded source are first linked to the Marketplace by name.
pub fn check_updates(store: &Mutex<Store>) -> Result<Vec<(String, UpdateStatus)>, HkError> {
    // Read all extensions and release the lock before doing slow network calls
    type Updatable = Vec<(String, String, InstallMeta)>; // (id, name, meta)
    type Unlinked = Vec<(String, String)>;
    let (updatable, unlinked): (Updatable, Unlinked) = {
        let store = store.lock();
        // One-shot migration: bring pre-feature rows (pack present, no
        // install_meta) into the manual-bound state so they get checked
        // here without the user having to re-touch the pack input.
        if let Err(e) = service::migrate_pack_to_manual_meta(&store) {
            eprintln!("[hk] warning: pack→manual migration failed: {e}");
        }
        let extensions = store.list_extensions(None, None)?;
        let mut has_meta = Vec::new();
        let mut no_meta = Vec::new();
        for e in extensions {
            if !service::is_update_eligible(&e) {
                continue;
            }
            if let Some(meta) = e.install_meta {
                match meta.install_type.as_str() {
                    "git" | "marketplace" | "manual" => has_meta.push((e.id, e.name, meta)),
                    _ => {}
                }
            } else {
                no_meta.push((e.id, e.name));
            }
        }
        (has_meta, no_meta)
    };

    link_to_marketplace(store, &unlinked);

    // Check each extension for updates — cache git ls-remote results per URL
    // so extensions sharing the same repo only trigger one network call.
    let mut remote_cache = std::collections::HashMap::new();
    let mut statuses: Vec<_> = updatable
        .iter()
        .map(|(id, name, meta)| {
            let status = manager::check_update_with_cache(meta, &mut remote_cache);
            (id.clone(), name.clone(), meta.clone(), status)
        })
        .collect();

    // A skill with a subpath, or bound by hand, may be gone from a repository
    // that still has new commits. Clone each such repository once to tell.
    let mut by_url: std::collections::HashMap<String, Vec<usize>> = Default::default();
    for (idx, (_, _, meta, status)) in statuses.iter().enumerate() {
        if matches!(status, UpdateStatus::UpdateAvailable { .. })
            && (meta.subpath.is_some() || meta.install_type == "manual")
            && let Some(url) = meta.url_resolved.as_deref().or(meta.url.as_deref())
            && !url.is_empty()
        {
            by_url.entry(url.to_string()).or_default().push(idx);
        }
    }
    for (url, indices) in &by_url {
        let Ok(temp) = tempfile::tempdir() else {
            continue;
        };
        let clone_path = temp.path().join("repo");
        if manager::shallow_clone(url, &clone_path).is_err() {
            continue;
        }
        for &idx in indices {
            let name = &statuses[idx].1;
            if manager::find_skill_in_repo(&clone_path, name).is_none() {
                eprintln!("[hk] Skill '{}' no longer exists in repository", name);
                statuses[idx].3 = UpdateStatus::RemovedFromRepo;
            }
        }
    }

    let store = store.lock();
    let now = chrono::Utc::now();
    for (id, _name, _meta, status) in &statuses {
        let (remote_rev, check_err) = match status {
            UpdateStatus::UpToDate { remote_hash } => (Some(remote_hash.as_str()), None),
            UpdateStatus::UpdateAvailable { remote_hash } => (Some(remote_hash.as_str()), None),
            UpdateStatus::RemovedFromRepo => (None, Some("removed_from_repo")),
            UpdateStatus::Error { message } => (None, Some(message.as_str())),
        };
        if let Err(e) = store.update_check_state(id, remote_rev, now, check_err) {
            eprintln!("[hk] warning: {e}");
        }
    }

    Ok(statuses
        .into_iter()
        .map(|(id, _, _, status)| (id, status))
        .collect())
}

/// Replace a skill, in every agent that holds it at the same scope, with the
/// repository's latest version.
pub fn update_extension(store: &Mutex<Store>, id: &str) -> Result<InstallResult, HkError> {
    let (ext, install_meta) = {
        let store = store.lock();
        let ext = store
            .get_extension(id)?
            .ok_or_else(|| HkError::NotFound(format!("Extension '{}' not found", id)))?;
        // The update path clones a repo and deploys it as a skill, so guard
        // against non-skill kinds (e.g. a plugin now carrying install_type
        // 'git') reaching it — same gate the bulk update path uses.
        if !service::is_update_eligible(&ext) {
            return Err(HkError::Validation(format!(
                "Extension '{}' is not eligible for update",
                ext.name
            )));
        }
        let meta = ext.install_meta.clone().ok_or_else(|| {
            HkError::NotFound("Extension has no install metadata — cannot update".into())
        })?;
        match meta.install_type.as_str() {
            "git" | "marketplace" | "manual" => {}
            _ => {
                return Err(HkError::Validation(format!(
                    "Extensions with install type '{}' cannot be updated",
                    meta.install_type
                )));
            }
        }
        (ext, meta)
    };

    // Skills-CLI-managed skills update via that CLI; the caller's rescan
    // reflects it. Falls through to HK's clone+deploy when delegation declines.
    if service::try_delegate_skill_update(store, &ext)? {
        return Ok(InstallResult {
            name: ext.name.clone(),
            was_update: true,
            revision: None,
            skipped: false,
        });
    }

    let url = install_meta
        .url_resolved
        .as_deref()
        .or(install_meta.url.as_deref())
        .ok_or_else(|| HkError::NotFound("Extension has no remote URL".into()))?;

    let temp = tempfile::tempdir().map_err(|e| HkError::Internal(e.to_string()))?;
    let clone_dir = temp.path().join("repo");
    manager::shallow_clone(url, &clone_dir)?;
    let revision = manager::capture_git_revision(&clone_dir);

    let skill_name = &ext.name;
    let skill_source = match manager::find_skill_in_repo(&clone_dir, skill_name) {
        Some(path) => path,
        None => {
            eprintln!(
                "[hk] Skill '{}' no longer exists in repository — skipping update",
                skill_name
            );
            // Persist removed_from_repo state so UI shows it after restart
            let store = store.lock();
            let now = chrono::Utc::now();
            if let Err(e) = store.update_check_state(id, None, now, Some("removed_from_repo")) {
                eprintln!("[hk] warning: {e}");
            }
            return Ok(InstallResult {
                name: skill_name.clone(),
                was_update: false,
                revision,
                skipped: true,
            });
        }
    };

    // Find all installed paths (deduplicated) and copy the latest version
    // to each. Restrict to siblings in the same scope as the trigger ext
    // so a Global update doesn't clobber a project copy and vice versa.
    let all_siblings: Vec<Extension> = {
        let store = store.lock();
        let all = store.list_extensions(Some(ext.kind), None)?;
        all.into_iter()
            .filter(|e| {
                e.name == ext.name
                    && e.source_path.is_some()
                    && service::same_scope(&e.scope, &ext.scope)
            })
            .collect()
    };

    let mut updated_dirs = std::collections::HashSet::new();
    for sibling in &all_siblings {
        let source_path = sibling
            .source_path
            .as_deref()
            .ok_or_else(|| HkError::Internal("Sibling extension has no source_path".into()))?;
        let source_path = std::path::Path::new(source_path);
        // A skill kept as one loose file has no folder of its own to update.
        if !source_path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("SKILL.md"))
        {
            continue;
        }
        let skill_dir = source_path.parent().ok_or_else(|| {
            HkError::Internal("Cannot determine skill directory from source path".into())
        })?;
        if !updated_dirs.insert(skill_dir.to_string_lossy().to_string()) {
            continue;
        }
        // Into the folder the skill was installed as, which need not be
        // named like its folder in the repository.
        manager::copy_dir_contents(&skill_source, skill_dir)?;
    }

    {
        let store = store.lock();
        let updated_meta = InstallMeta {
            revision: revision.clone().or(install_meta.revision.clone()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
            ..install_meta
        };
        for sibling in &all_siblings {
            if let Err(e) = store.set_install_meta(&sibling.id, &updated_meta) {
                eprintln!("[hk] warning: {e}");
            }
        }
    }

    // No partial rescan here: the caller runs a full one, which keeps CLI
    // sub-extension merging consistent.
    Ok(InstallResult {
        name: skill_name.clone(),
        was_update: true,
        revision,
        skipped: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::tests::Fixture;
    use crate::repositories::tests::{commit_all, write_skill};

    #[test]
    fn a_skill_follows_its_repository_until_it_is_removed_there() {
        let fx = Fixture::new(&["alpha"]);
        fx.scan();
        let upstream = fx.home.path().join("upstream");
        let status = || check_updates(&fx.store).unwrap().remove(0);
        let (id, fresh) = status();
        assert!(matches!(fresh, UpdateStatus::UpToDate { .. }));

        std::fs::write(upstream.join("skills/alpha/notes.md"), "v2").unwrap();
        commit_all(&upstream, false);
        assert!(matches!(status().1, UpdateStatus::UpdateAvailable { .. }));

        let updated = update_extension(&fx.store, &id).unwrap();
        assert!(updated.was_update);
        assert!(fx.installed("alpha").join("notes.md").exists());
        assert!(matches!(status().1, UpdateStatus::UpToDate { .. }));

        std::fs::remove_dir_all(upstream.join("skills/alpha")).unwrap();
        write_skill(&upstream, "beta");
        commit_all(&upstream, false);
        assert!(matches!(status().1, UpdateStatus::RemovedFromRepo));
    }

    #[test]
    fn a_skill_kept_as_one_loose_file_is_left_alone_by_an_update() {
        let fx = Fixture::new(&["alpha"]);
        let skills = fx.installed("alpha").parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(skills.join("alpha.md"), "loose").unwrap();
        let scanned = crate::scanner::scan_adapter(fx.adapters[0].as_ref());
        let store = fx.store.lock();
        store.sync_extensions_for_agent("claude", &scanned).unwrap();
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some(fx.url()),
            url_resolved: None,
            branch: None,
            subpath: Some("alpha".into()),
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        store.set_install_meta(&scanned[0].id, &meta).unwrap();
        drop(store);

        update_extension(&fx.store, &scanned[0].id).unwrap();

        assert_eq!(std::fs::read_dir(&skills).unwrap().count(), 1);
    }

    #[test]
    fn a_skill_at_the_root_of_its_repository_is_updated_in_place() {
        let fx = Fixture::new(&[]);
        let upstream = fx.home.path().join("upstream");
        std::fs::create_dir_all(&upstream).unwrap();
        let skill_md = "---\nname: solo\ndescription: demo\n---\n";
        std::fs::write(upstream.join("SKILL.md"), skill_md).unwrap();
        commit_all(&upstream, true);
        fx.scan();
        std::fs::write(upstream.join("notes.md"), "v2").unwrap();
        commit_all(&upstream, false);
        let (id, _) = check_updates(&fx.store).unwrap().remove(0);

        update_extension(&fx.store, &id).unwrap();

        assert!(fx.installed("solo").join("notes.md").exists());
        let skills = std::fs::read_dir(fx.installed("solo").parent().unwrap()).unwrap();
        assert_eq!(skills.count(), 1);
    }
}
