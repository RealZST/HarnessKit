//! Git repositories tracked as catalogs of skills.
//!
//! A tracked repository keeps a shallow checkout under `{cache_root}/{id}`.
//! Syncing refreshes the checkout and records the skills it holds; installing
//! copies a skill out of the checkout the same way a one-off Git install
//! copies it out of a temporary clone.
//!
//! The UI calls tracking a repository following it, a sync a refresh, and an
//! ignored skill a hidden one.

use crate::adapter::AgentAdapter;
use crate::manager;
use crate::models::{InstallRepositoryItemRequest, Repository};
use crate::store::Store;
use crate::{HkError, git, install, sanitize, service};
use fs2::FileExt;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};

/// `~/.harnesskit/repos`, the directory holding every repository's checkout.
pub fn cache_root() -> PathBuf {
    crate::home_dir().join(".harnesskit").join("repos")
}

/// Identity of a repository URL: `host/path`, lowercased and without userinfo
/// or `.git`, so the HTTPS, SSH and `.git` spellings of one repository
/// compare equal. `owner/repo` shorthand counts as GitHub, and a GitHub URL
/// that points inside a repository counts as that repository.
pub(crate) fn repo_key(url: &str) -> String {
    let (authority, path) = split_url(url);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
        .to_lowercase();
    let path = if host == "github.com" {
        github_repo_path(path)
    } else {
        path.to_string()
    };
    format!("{host}/{}", path.to_lowercase())
}

/// The authority and path of a Git URL, the path without `.git`. The
/// authority keeps any userinfo, except the `git@` of the scp-like form;
/// `owner/repo` shorthand is on GitHub.
fn split_url(url: &str) -> (&str, &str) {
    let url = url.trim().trim_end_matches('/');
    let (authority, path) = if let Some(rest) = url.strip_prefix("git@") {
        rest.split_once(':').unwrap_or((rest, ""))
    } else if let Some((_, rest)) = url.split_once("://") {
        rest.split_once('/').unwrap_or((rest, ""))
    } else if service::is_valid_pack_format(url) {
        ("github.com", url)
    } else {
        ("", url)
    };
    (authority, path.strip_suffix(".git").unwrap_or(path))
}

/// `owner/repo` from the path of a GitHub URL, which may go on into the
/// repository (`owner/repo/tree/main/skills/pdf`).
fn github_repo_path(path: &str) -> String {
    path.split('/').take(2).collect::<Vec<_>>().join("/")
}

/// Build the record for a repository the user wants to track. Accepts a Git
/// URL, `owner/repo` shorthand for GitHub, or a GitHub page inside the
/// repository.
pub fn new_repository(url: &str) -> Result<Repository, HkError> {
    let url = url.trim().trim_end_matches('/');
    let url = if service::is_valid_pack_format(url) {
        format!("https://github.com/{url}")
    } else if let Some(path) = url.strip_prefix("https://github.com/") {
        format!("https://github.com/{}", github_repo_path(path))
    } else {
        url.to_string()
    };
    sanitize::validate_git_url(&url).map_err(|e| HkError::Validation(e.to_string()))?;
    let (authority, path) = split_url(&url);
    let userinfo = authority.rsplit_once('@').map(|(userinfo, _)| userinfo);
    if userinfo.is_some_and(|userinfo| !url.starts_with("ssh://") || userinfo.contains(':')) {
        return Err(HkError::Validation(
            "Repository URL must not contain credentials".into(),
        ));
    }
    let name = path.to_string();
    Ok(Repository {
        id: uuid::Uuid::new_v4().to_string(),
        url,
        name,
        last_synced_at: None,
        last_commit_hash: None,
        last_commit_message: None,
        last_sync_error: None,
    })
}

pub fn can_track(url: &str) -> bool {
    new_repository(url).is_ok()
}

/// Start tracking a repository: record it and run its first sync, and return
/// its id. If that sync fails the repository is not kept, so a mistyped URL
/// leaves nothing behind.
pub fn add_repository(
    store: &Mutex<Store>,
    cache_root: &Path,
    url: &str,
) -> Result<String, HkError> {
    track(store, cache_root, url, Checkout::refresh)
}

/// Track the repository a one-off install has just cloned to `clone_dir`,
/// taking that clone over as its checkout. Nothing changes for a repository
/// that is already tracked. Best effort: the install is done either way, so
/// a failure is only logged, and a URL that cannot be tracked, which may
/// carry credentials, is not even reported.
pub fn track_clone(store: &Mutex<Store>, cache_root: &Path, url: &str, clone_dir: &Path) {
    if !can_track(url) {
        return;
    }
    let tracked = track(store, cache_root, url, |checkout, _| {
        checkout.adopt(clone_dir)
    });
    if let Err(e) = tracked
        && !matches!(e, HkError::Conflict(_))
    {
        eprintln!("[harnesskit] Could not track the installed repository: {e}");
    }
}

/// Record a new repository and run its first sync, with `prepare` putting
/// its checkout in place.
fn track(
    store: &Mutex<Store>,
    cache_root: &Path,
    url: &str,
    prepare: impl FnOnce(&Checkout, &str) -> Result<(), HkError>,
) -> Result<String, HkError> {
    let repo = new_repository(url)?;
    let checkout = Checkout::lock(cache_root, &repo.id)?;
    let tracked = insert_unless_tracked(store, &repo).and_then(|()| {
        prepare(&checkout, &repo.url)
            .and_then(|()| record_checkout(store, &repo.id, &checkout))
            .inspect_err(|_| {
                // Undone under the lock, so a sync started from the list
                // meanwhile finds the repository gone. Best effort: the
                // sync's error is the one to report.
                let _ = std::fs::remove_dir_all(&checkout.dir);
                let _ = store.lock().delete_repository(&repo.id);
            })
    });
    match tracked {
        Ok(()) => Ok(repo.id),
        Err(e) => {
            checkout.unlock_and_remove();
            Err(e)
        }
    }
}

fn insert_unless_tracked(store: &Mutex<Store>, repo: &Repository) -> Result<(), HkError> {
    let store = store.lock();
    let key = repo_key(&repo.url);
    if store
        .list_repositories()?
        .iter()
        .any(|existing| repo_key(&existing.url) == key)
    {
        return Err(HkError::Conflict(format!(
            "Repository '{}' is already followed",
            repo.name
        )));
    }
    store.insert_repository(repo)
}

/// Refresh a repository's checkout and record the skills it holds. A failure
/// is recorded on the repository and returned; the last good state stays.
pub fn sync_repository(store: &Mutex<Store>, cache_root: &Path, id: &str) -> Result<(), HkError> {
    let synced = lock_repository(store, cache_root, id).and_then(|(repo, checkout)| {
        checkout.refresh(&repo.url)?;
        record_checkout(store, id, &checkout)
    });
    if let Err(e) = &synced {
        let _ = store
            .lock()
            .record_repository_sync_error(id, &e.to_string());
    }
    synced
}

/// Record the commit a checkout is at and the skills it holds.
fn record_checkout(store: &Mutex<Store>, id: &str, checkout: &Checkout) -> Result<(), HkError> {
    let head = git::run(Some(&checkout.dir), &["log", "-1", "--format=%H%n%s"])?;
    let (commit_hash, commit_message) = head.split_once('\n').unwrap_or((&head, ""));
    let skills = manager::scan_repo_skills(&checkout.dir);
    store
        .lock()
        .record_repository_sync(id, commit_hash, commit_message, &skills)
}

/// Install one skill of a synced repository for the requested agents.
pub fn install_item(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    cache_root: &Path,
    req: &InstallRepositoryItemRequest,
) -> Result<(), HkError> {
    let (repo, checkout) = lock_repository(store, cache_root, &req.repo_id)?;
    install::install_from_clone(
        store,
        adapters,
        &checkout.dir,
        &install::Origin::git(&repo.url),
        &req.item_id,
        &install::Target {
            agents: &req.target_agents,
            scope: &req.target_scope,
            hermes_category: req.hermes_category.as_deref(),
        },
    )?;
    // A skill the user installs is one they want to see.
    store
        .lock()
        .set_repository_item_ignored(&req.repo_id, &req.item_id, false)
}

/// The SKILL.md of one skill of a synced repository. Read without the
/// checkout's lock, so it does not wait behind a refresh; the page asks again
/// once the refresh has moved the checkout.
pub fn item_content(
    store: &Mutex<Store>,
    cache_root: &Path,
    repo_id: &str,
    item_id: &str,
) -> Result<String, HkError> {
    let not_found = || HkError::NotFound(format!("Skill '{item_id}' not found"));
    store
        .lock()
        .get_repository(repo_id)?
        .ok_or_else(not_found)?;
    let checkout = cache_root.join(repo_id);
    let skill = manager::scan_repo_skills(&checkout)
        .into_iter()
        .find(|skill| skill.skill_id == item_id)
        .ok_or_else(not_found)?;
    let skill_md = checkout.join(skill.path).join("SKILL.md");
    // An install leaves symlinks out, so one is not shown either.
    if skill_md.symlink_metadata()?.is_symlink() {
        return Err(not_found());
    }
    Ok(std::fs::read_to_string(skill_md)?)
}

/// Stop tracking a repository and remove its checkout. Skills installed from
/// it stay installed.
pub fn delete_repository(store: &Mutex<Store>, cache_root: &Path, id: &str) -> Result<(), HkError> {
    let (_, checkout) = lock_repository(store, cache_root, id)?;
    // The checkout goes first: if it cannot be removed, the repository stays
    // listed and the user can try again.
    if checkout.dir.exists() {
        std::fs::remove_dir_all(&checkout.dir)?;
    }
    store.lock().delete_repository(id)?;
    checkout.unlock_and_remove();
    Ok(())
}

/// Look up a repository and lock its checkout. The lookup runs on both sides
/// of the lock: before, so an unknown id never touches the disk; after, so an
/// operation that waited behind a delete does not bring the checkout back.
fn lock_repository(
    store: &Mutex<Store>,
    cache_root: &Path,
    id: &str,
) -> Result<(Repository, Checkout), HkError> {
    let require = || {
        store
            .lock()
            .get_repository(id)?
            .ok_or_else(|| HkError::NotFound(format!("Repository '{id}' not found")))
    };
    require()?;
    let checkout = Checkout::lock(cache_root, id)?;
    match require() {
        Ok(repo) => Ok((repo, checkout)),
        Err(e) => {
            // Only for a deleted repository: others may still be waiting on
            // the lock of one that exists.
            if matches!(e, HkError::NotFound(_)) {
                checkout.unlock_and_remove();
            }
            Err(e)
        }
    }
}

/// The checkout directory of a repository, held under an exclusive file lock
/// so no two operations, in one process or across the desktop app and
/// `hk serve`, run git in it at once.
struct Checkout {
    dir: PathBuf,
    _lock: std::fs::File,
}

impl Checkout {
    fn lock(cache_root: &Path, id: &str) -> Result<Self, HkError> {
        std::fs::create_dir_all(cache_root)?;
        let dir = cache_root.join(id);
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dir.with_extension("lock"))?;
        lock.lock_exclusive()?;
        Ok(Self { dir, _lock: lock })
    }

    fn unlock_and_remove(self) {
        let lock_path = self.dir.with_extension("lock");
        drop(self);
        let _ = std::fs::remove_file(lock_path);
    }

    /// Make a one-off install's clone the checkout: moved, or copied when it
    /// sits on another filesystem.
    fn adopt(&self, clone_dir: &Path) -> Result<(), HkError> {
        if std::fs::rename(clone_dir, &self.dir).is_ok() {
            return Ok(());
        }
        let (from, to) = (clone_dir.to_string_lossy(), self.dir.to_string_lossy());
        git::run(None, &["clone", "--", &from, &to]).map(drop)
    }

    /// Bring the checkout to the remote's default branch, cloning it first
    /// when it is missing or broken.
    fn refresh(&self, url: &str) -> Result<(), HkError> {
        let git_dir = self.dir.join(".git");
        if !git_dir.join("HEAD").exists() {
            if self.dir.exists() {
                std::fs::remove_dir_all(&self.dir)?;
            }
            let dir = self.dir.to_string_lossy();
            git::run(None, &["clone", "--depth", "1", "--", url, &dir])?;
            return Ok(());
        }
        // Left by a git that was killed mid-sync; under the checkout lock no
        // other git owns them.
        for stale in ["index.lock", "shallow.lock"] {
            let _ = std::fs::remove_file(git_dir.join(stale));
        }
        // By URL: the origin of a checkout taken over from an install is
        // whatever that install cloned from.
        git::run(
            Some(&self.dir),
            &["fetch", "--depth", "1", "--", url, "HEAD"],
        )?;
        git::run(Some(&self.dir), &["reset", "--hard", "FETCH_HEAD"])?;
        git::run(Some(&self.dir), &["clean", "-fdx"])?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::adapter::claude::ClaudeAdapter;
    use tempfile::TempDir;

    /// A home with a Claude adapter and store, plus an upstream repository
    /// holding the skills `alpha` and `beta`, tracked and synced as `repo`.
    struct Fixture {
        home: TempDir,
        store: Mutex<Store>,
        adapters: Vec<Box<dyn AgentAdapter>>,
        repo: Repository,
    }

    impl Fixture {
        fn new() -> Self {
            let home = TempDir::new().unwrap();
            let upstream = home.path().join("upstream");
            for skill in ["alpha", "beta"] {
                write_skill(&upstream, skill);
            }
            commit_all(&upstream, true);
            let store = Mutex::new(Store::open(&home.path().join("test.db")).unwrap());
            let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(ClaudeAdapter::with_home(
                home.path().to_path_buf(),
            ))];
            let id =
                add_repository(&store, &home.path().join("repos"), &file_url(&upstream)).unwrap();
            let repo = store.lock().get_repository(&id).unwrap().unwrap();
            Self {
                home,
                store,
                adapters,
                repo,
            }
        }

        fn upstream(&self) -> PathBuf {
            self.home.path().join("upstream")
        }

        fn cache(&self) -> PathBuf {
            self.home.path().join("repos")
        }

        fn sync(&self) -> Result<(), HkError> {
            sync_repository(&self.store, &self.cache(), &self.repo.id)
        }

        fn item_ids(&self) -> Vec<String> {
            let listed = self.store.lock().list_repositories_with_items().unwrap();
            listed[0].items.iter().map(|i| i.item_id.clone()).collect()
        }
    }

    /// A `file://` URL git accepts on every platform.
    pub(crate) fn file_url(path: &Path) -> String {
        let path = path.to_string_lossy().replace('\\', "/");
        format!("file:///{}", path.trim_start_matches('/'))
    }

    pub(crate) fn write_skill(repo: &Path, name: &str) {
        let dir = repo.join("skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let content = format!("---\nname: {name}\ndescription: demo\n---\nv1\n");
        std::fs::write(dir.join("SKILL.md"), content).unwrap();
    }

    pub(crate) fn commit_all(repo: &Path, init: bool) {
        let run = |args: &[&str]| {
            let ok = crate::command("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .current_dir(repo)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?} failed");
        };
        if init {
            run(&["init", "-q"]);
        }
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "change"]);
    }

    #[test]
    fn the_spellings_of_one_repository_share_a_key() {
        let key = repo_key("https://github.com/Acme/Skills");
        for spelling in [
            "https://github.com/acme/skills.git",
            "https://github.com/acme/skills/",
            "git@github.com:acme/skills.git",
            "ssh://git@github.com/acme/skills",
            "https://github.com/acme/skills/tree/main/skills/pdf",
            "acme/skills",
        ] {
            assert_eq!(repo_key(spelling), key, "{spelling}");
        }
        assert_ne!(repo_key("https://gitlab.com/acme/skills"), key);
        assert_ne!(
            repo_key("https://gitlab.com/group/sub/repo"),
            repo_key("https://gitlab.com/group/sub")
        );
    }

    #[test]
    fn adding_syncs_the_repository_and_rejects_it_a_second_time() {
        let fx = Fixture::new();
        assert_eq!(fx.item_ids(), ["alpha", "beta"]);
        assert_eq!(fx.repo.last_commit_hash.as_ref().map(String::len), Some(40));
        assert_eq!(fx.repo.last_commit_message.as_deref(), Some("change"));

        let again = add_repository(&fx.store, &fx.cache(), &fx.repo.url);
        assert!(matches!(again, Err(HkError::Conflict(_))));
    }

    #[test]
    fn adding_keeps_nothing_when_the_first_sync_fails() {
        let home = TempDir::new().unwrap();
        let store = Mutex::new(Store::open(&home.path().join("test.db")).unwrap());
        let cache = home.path().join("repos");
        let missing = file_url(&home.path().join("missing"));

        assert!(add_repository(&store, &cache, &missing).is_err());

        assert!(store.lock().list_repositories().unwrap().is_empty());
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn a_new_repository_takes_shorthand_and_refuses_urls_git_should_not_get() {
        let repo = new_repository("acme/skills").unwrap();
        assert_eq!(repo.url, "https://github.com/acme/skills");
        assert_eq!(repo.name, "acme/skills");
        let inside = new_repository("https://github.com/acme/skills/tree/main/skills/pdf");
        assert_eq!(inside.unwrap().url, repo.url);
        let subgroup = new_repository("https://gitlab.com/group/sub/repo.git").unwrap();
        assert_eq!(subgroup.name, "group/sub/repo");

        for bad in [
            "--upload-pack=x",
            "/tmp/repo",
            "https://user:secret@host/a/b",
            "ssh://git:secret@host/a/b",
        ] {
            let result = new_repository(bad);
            assert!(matches!(result, Err(HkError::Validation(_))), "{bad}");
        }
    }

    #[test]
    fn a_repository_that_is_one_skill_lists_and_installs_it() {
        let fx = Fixture::new();
        let solo = fx.home.path().join("solo");
        std::fs::create_dir_all(&solo).unwrap();
        std::fs::write(
            solo.join("SKILL.md"),
            "---\nname: solo\ndescription: demo\n---\nv1\n",
        )
        .unwrap();
        commit_all(&solo, true);
        let repo_id = add_repository(&fx.store, &fx.cache(), &file_url(&solo)).unwrap();

        let req = InstallRepositoryItemRequest {
            repo_id: repo_id.clone(),
            item_id: String::new(),
            target_agents: vec!["claude".into()],
            target_scope: crate::models::ConfigScope::Global,
            hermes_category: None,
        };
        install_item(&fx.store, &fx.adapters, &fx.cache(), &req).unwrap();

        let listed = fx.store.lock().list_repositories_with_items().unwrap();
        let solo = listed.iter().find(|r| r.repository.id == repo_id).unwrap();
        assert_eq!(solo.items[0].installed.len(), 1);
    }

    #[test]
    fn tracking_a_clone_takes_it_over_as_the_checkout() {
        let fx = Fixture::new();
        let other = fx.home.path().join("other");
        write_skill(&other, "gamma");
        commit_all(&other, true);
        let url = file_url(&other);
        let clone = fx.home.path().join("clone");
        let dir = clone.to_string_lossy();
        git::run(None, &["clone", "--depth", "1", "--", &url, &dir]).unwrap();

        track_clone(&fx.store, &fx.cache(), &url, &clone);
        track_clone(&fx.store, &fx.cache(), &url, &clone);

        assert!(!clone.exists());
        let listed = fx.store.lock().list_repositories_with_items().unwrap();
        assert_eq!(listed.len(), 2);
        let tracked = listed.iter().find(|r| r.repository.id != fx.repo.id);
        assert_eq!(tracked.unwrap().items[0].item_id, "gamma");
    }

    #[test]
    fn a_clone_that_cannot_be_moved_is_copied() {
        let fx = Fixture::new();
        let url = file_url(&fx.upstream());
        let clone = fx.home.path().join("clone");
        let dir = clone.to_string_lossy();
        git::run(None, &["clone", "--depth", "1", "--", &url, &dir]).unwrap();
        // A rename into a directory that does not exist fails, as one across
        // filesystems does.
        let checkout = Checkout {
            dir: fx.home.path().join("missing").join("checkout"),
            _lock: tempfile::tempfile().unwrap(),
        };

        checkout.adopt(&clone).unwrap();
        write_skill(&fx.upstream(), "gamma");
        commit_all(&fx.upstream(), false);
        checkout.refresh(&url).unwrap();

        assert!(checkout.dir.join("skills/gamma/SKILL.md").exists());
    }

    #[test]
    fn a_broken_checkout_is_cloned_again_instead_of_touching_a_repository_around_it() {
        let fx = Fixture::new();
        commit_all(fx.home.path(), true);
        let checkout = fx.cache().join(&fx.repo.id);
        std::fs::remove_file(checkout.join(".git/HEAD")).unwrap();
        std::fs::remove_dir_all(checkout.join("skills")).unwrap();

        fx.sync().unwrap();

        assert!(checkout.join("skills/alpha/SKILL.md").exists());
        assert!(!fx.home.path().join("skills").exists());
    }

    #[test]
    fn a_second_sync_follows_the_upstream_repository() {
        let fx = Fixture::new();

        std::fs::remove_dir_all(fx.upstream().join("skills/beta")).unwrap();
        write_skill(&fx.upstream(), "gamma");
        commit_all(&fx.upstream(), false);
        fx.sync().unwrap();

        assert_eq!(fx.item_ids(), ["alpha", "gamma"]);
    }

    #[test]
    fn a_failed_sync_is_recorded_until_one_works_and_keeps_the_synced_items() {
        let fx = Fixture::new();
        let away = fx.home.path().join("away");
        let error = || {
            let repo = fx.store.lock().get_repository(&fx.repo.id).unwrap();
            repo.unwrap().last_sync_error
        };

        std::fs::rename(fx.upstream(), &away).unwrap();
        assert!(fx.sync().is_err());
        assert!(error().unwrap().contains("git fetch failed"));
        assert_eq!(fx.item_ids(), ["alpha", "beta"]);

        std::fs::rename(&away, fx.upstream()).unwrap();
        fx.sync().unwrap();
        assert_eq!(error(), None);
    }

    #[test]
    fn installing_an_item_copies_the_skill_and_marks_the_item_installed() {
        let fx = Fixture::new();
        fx.store
            .lock()
            .set_repository_item_ignored(&fx.repo.id, "alpha", true)
            .unwrap();

        let req = InstallRepositoryItemRequest {
            repo_id: fx.repo.id.clone(),
            item_id: "alpha".into(),
            target_agents: vec!["claude".into()],
            target_scope: crate::models::ConfigScope::Global,
            hermes_category: None,
        };
        install_item(&fx.store, &fx.adapters, &fx.cache(), &req).unwrap();

        assert!(
            fx.home
                .path()
                .join(".claude/skills/alpha/SKILL.md")
                .exists()
        );
        let listed = fx.store.lock().list_repositories_with_items().unwrap();
        let installed = |id: &str| -> Vec<String> {
            let item = listed[0].items.iter().find(|i| i.item_id == id).unwrap();
            item.installed.iter().map(|c| c.agent.clone()).collect()
        };
        assert_eq!(installed("alpha"), ["claude"]);
        assert!(installed("beta").is_empty());
        assert!(listed[0].items.iter().all(|i| !i.ignored));
    }

    #[test]
    fn an_item_can_be_read_before_it_is_installed() {
        let fx = Fixture::new();
        let read = |item: &str| item_content(&fx.store, &fx.cache(), &fx.repo.id, item);

        assert!(read("alpha").unwrap().contains("name: alpha"));
        assert!(matches!(read("missing"), Err(HkError::NotFound(_))));
    }

    #[test]
    fn deleting_removes_the_checkout() {
        let fx = Fixture::new();
        assert!(fx.cache().join(&fx.repo.id).exists());

        delete_repository(&fx.store, &fx.cache(), &fx.repo.id).unwrap();
        assert_eq!(std::fs::read_dir(fx.cache()).unwrap().count(), 0);
        assert!(fx.store.lock().list_repositories().unwrap().is_empty());
    }
}
