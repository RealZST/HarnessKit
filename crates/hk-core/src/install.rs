//! Installing skills from a folder, a Git repository or the Marketplace: the
//! skill is copied into each target agent's skill directory, and where it
//! came from is recorded on the installed rows.

use crate::{
    HkError,
    adapter::AgentAdapter,
    deployer,
    manager::{self, DiscoveredSkill, InstallResult},
    marketplace,
    models::*,
    repositories, scanner, service,
    store::Store,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a scanned repository waits for the user to choose its skills.
const PENDING_CLONE_TTL: Duration = Duration::from_secs(600);

/// Who an install is for.
pub struct Target<'a> {
    pub agents: &'a [String],
    pub scope: &'a ConfigScope,
    /// Only category-aware agents (Hermes) use it.
    pub hermes_category: Option<&'a str>,
}

impl Target<'_> {
    /// The skill directory of every target agent.
    fn skill_dirs(&self, adapters: &[Box<dyn AgentAdapter>]) -> Result<Vec<PathBuf>, HkError> {
        if self.agents.is_empty() {
            return Err(HkError::Validation("No agent to install for".into()));
        }
        self.agents
            .iter()
            .map(|agent| {
                let adapter = adapters
                    .iter()
                    .find(|a| a.name() == agent)
                    .ok_or_else(|| HkError::NotFound(format!("Agent '{agent}' not found")))?;
                service::skill_target_dir(adapter.as_ref(), self.scope, self.hermes_category)
            })
            .collect()
    }
}

/// Where a cloned skill comes from, as recorded on the installed rows.
pub(crate) struct Origin<'a> {
    install_type: &'static str,
    url: &'a str,
    /// The Git URL that was cloned, when `url` is not one.
    url_resolved: Option<&'a str>,
}

impl<'a> Origin<'a> {
    pub(crate) fn git(url: &'a str) -> Self {
        Self {
            install_type: "git",
            url,
            url_resolved: None,
        }
    }

    fn marketplace(source: &'a str, git_url: &'a str) -> Self {
        Self {
            install_type: "marketplace",
            url: source,
            url_resolved: Some(git_url),
        }
    }
}

/// Copy one skill of a cloned repository into every target agent's skill
/// directory and record its origin. An empty `skill_id` is the repository's
/// only skill.
pub(crate) fn install_from_clone(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    clone_dir: &Path,
    origin: &Origin,
    skill_id: &str,
    target: &Target,
) -> Result<InstallResult, HkError> {
    let subpath = Some(skill_id).filter(|id| !id.is_empty());
    let clone_url = origin.url_resolved.unwrap_or(origin.url);
    let mut installed = InstallResult::default();
    for dir in target.skill_dirs(adapters)? {
        installed = manager::install_from_clone(clone_dir, &dir, subpath, clone_url)?;
    }
    let meta = InstallMeta {
        install_type: origin.install_type.into(),
        url: Some(origin.url.into()),
        url_resolved: origin.url_resolved.map(Into::into),
        branch: None,
        subpath: subpath.map(Into::into),
        revision: installed.revision.clone(),
        remote_revision: None,
        checked_at: None,
        check_error: None,
    };
    let pack = scanner::extract_pack_from_url(origin.url)
        .or_else(|| origin.url_resolved.and_then(scanner::extract_pack_from_url));
    service::post_install_sync(
        &store.lock(),
        adapters,
        target.agents,
        &installed.name,
        Some(meta),
        pack.as_deref(),
        target.scope,
    )?;
    Ok(installed)
}

/// Install the skill in the folder `path`.
pub fn install_from_local(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    path: &str,
    target: &Target,
) -> Result<InstallResult, HkError> {
    let source = Path::new(path);
    if !source.is_dir() {
        return Err(HkError::Validation(
            "Selected path is not a directory".into(),
        ));
    }
    let skill_md = source.join("SKILL.md");
    if !skill_md.exists() {
        return Err(HkError::Validation(
            "Selected directory does not contain a SKILL.md file".into(),
        ));
    }
    let name = scanner::parse_skill_name(&skill_md).unwrap_or_else(|| {
        source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    for dir in target.skill_dirs(adapters)? {
        deployer::deploy_skill(source, &dir)?;
    }

    // A folder inside a Git checkout is recorded with that repository.
    let git_source = scanner::detect_source_for(source);
    let meta = InstallMeta {
        install_type: "local".into(),
        url: git_source.url.clone().or_else(|| Some(path.into())),
        url_resolved: None,
        branch: None,
        subpath: None,
        revision: git_source.commit_hash,
        remote_revision: None,
        checked_at: None,
        check_error: None,
    };
    let pack = git_source
        .url
        .as_deref()
        .and_then(scanner::extract_pack_from_url);
    service::post_install_sync(
        &store.lock(),
        adapters,
        target.agents,
        &name,
        Some(meta),
        pack.as_deref(),
        target.scope,
    )?;
    Ok(InstallResult {
        name,
        ..Default::default()
    })
}

/// Install a Marketplace skill, named by its `source` and `skill_id` there.
pub fn install_from_marketplace(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    source: &str,
    skill_id: &str,
    target: &Target,
) -> Result<InstallResult, HkError> {
    target.skill_dirs(adapters)?;
    let git_url = marketplace::git_url_for_source(source);
    let temp = tempfile::tempdir()?;
    let clone_dir = temp.path().join("repo");
    manager::shallow_clone(&git_url, &clone_dir)?;
    let origin = Origin::marketplace(source, &git_url);
    install_from_clone(store, adapters, &clone_dir, &origin, skill_id, target)
}

/// A repository `scan_git_repo` cloned and found several skills in.
struct PendingClone {
    temp: tempfile::TempDir,
    url: String,
    created_at: Instant,
}

impl PendingClone {
    fn dir(&self) -> PathBuf {
        self.temp.path().join("repo")
    }
}

/// The clones waiting, by id, for `install_scanned_skills`.
#[derive(Default)]
pub struct PendingClones(Mutex<HashMap<String, PendingClone>>);

#[derive(serde::Serialize)]
#[serde(tag = "type")]
pub enum ScanResult {
    Installed {
        result: InstallResult,
    },
    MultipleSkills {
        clone_id: String,
        skills: Vec<DiscoveredSkill>,
        trackable: bool,
    },
    NoSkills,
}

/// Clone a repository and install its skill if it holds exactly one. With
/// several, the clone is kept for `install_scanned_skills` and they are listed.
pub fn scan_git_repo(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    pending: &PendingClones,
    url: &str,
    target: &Target,
) -> Result<ScanResult, HkError> {
    pending
        .0
        .lock()
        .retain(|_, clone| clone.created_at.elapsed() < PENDING_CLONE_TTL);
    target.skill_dirs(adapters)?;

    let clone = PendingClone {
        temp: tempfile::tempdir()?,
        url: url.into(),
        created_at: Instant::now(),
    };
    manager::shallow_clone(url, &clone.dir())?;
    let skills = manager::scan_repo_skills(&clone.dir());
    match skills.as_slice() {
        [] => Ok(ScanResult::NoSkills),
        [skill] => Ok(ScanResult::Installed {
            result: install_from_clone(
                store,
                adapters,
                &clone.dir(),
                &Origin::git(url),
                &skill.skill_id,
                target,
            )?,
        }),
        _ => {
            let clone_id = uuid::Uuid::new_v4().to_string();
            pending.0.lock().insert(clone_id.clone(), clone);
            Ok(ScanResult::MultipleSkills {
                clone_id,
                skills,
                trackable: repositories::can_track(url),
            })
        }
    }
}

/// Install the chosen skills of a repository `scan_git_repo` listed. With
/// `track_into`, the cache root, the repository is then followed, with the
/// clone as its checkout.
pub fn install_scanned_skills(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    pending: &PendingClones,
    clone_id: &str,
    skill_ids: &[String],
    target: &Target,
    track_into: Option<&Path>,
) -> Result<Vec<InstallResult>, HkError> {
    target.skill_dirs(adapters)?;
    let clone = pending
        .0
        .lock()
        .remove(clone_id)
        .ok_or_else(|| HkError::Internal("Clone session expired. Please try again.".into()))?;
    let results = skill_ids
        .iter()
        .map(|skill_id| {
            install_from_clone(
                store,
                adapters,
                &clone.dir(),
                &Origin::git(&clone.url),
                skill_id,
                target,
            )
        })
        .collect::<Result<_, _>>()?;
    if let Some(cache_root) = track_into {
        repositories::track_clone(store, cache_root, &clone.url, &clone.dir());
    }
    Ok(results)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::adapter::claude::ClaudeAdapter;
    use crate::repositories::tests::{commit_all, file_url, write_skill};
    use tempfile::TempDir;

    /// A home with a Claude adapter and store, and an upstream repository
    /// holding the named skills, if any.
    pub(crate) struct Fixture {
        pub home: TempDir,
        pub store: Mutex<Store>,
        pub adapters: Vec<Box<dyn AgentAdapter>>,
        pending: PendingClones,
        agents: Vec<String>,
    }

    impl Fixture {
        pub(crate) fn new(skills: &[&str]) -> Self {
            let home = TempDir::new().unwrap();
            for skill in skills {
                write_skill(&home.path().join("upstream"), skill);
            }
            if !skills.is_empty() {
                commit_all(&home.path().join("upstream"), true);
            }
            Self {
                store: Mutex::new(Store::open(&home.path().join("test.db")).unwrap()),
                adapters: vec![Box::new(ClaudeAdapter::with_home(
                    home.path().to_path_buf(),
                ))],
                pending: PendingClones::default(),
                agents: vec!["claude".into()],
                home,
            }
        }

        pub(crate) fn url(&self) -> String {
            file_url(&self.home.path().join("upstream"))
        }

        fn target(&self) -> Target<'_> {
            Target {
                agents: &self.agents,
                scope: &ConfigScope::Global,
                hermes_category: None,
            }
        }

        pub(crate) fn scan(&self) -> ScanResult {
            scan_git_repo(
                &self.store,
                &self.adapters,
                &self.pending,
                &self.url(),
                &self.target(),
            )
            .unwrap()
        }

        /// Where Claude holds an installed skill.
        pub(crate) fn installed(&self, skill: &str) -> PathBuf {
            self.home.path().join(".claude/skills").join(skill)
        }

        /// The install type and URL recorded on an installed skill.
        fn origin_of(&self, skill: &str) -> (String, Option<String>) {
            let skills = self
                .store
                .lock()
                .list_extensions(Some(ExtensionKind::Skill), None)
                .unwrap();
            let meta = skills.into_iter().find(|e| e.name == skill).unwrap();
            let meta = meta.install_meta.unwrap();
            (meta.install_type, meta.url)
        }
    }

    #[test]
    fn scanning_a_repository_with_one_skill_installs_it() {
        let fx = Fixture::new(&["alpha"]);

        let ScanResult::Installed { result } = fx.scan() else {
            panic!("expected the only skill to be installed");
        };

        assert_eq!(result.name, "alpha");
        assert!(fx.installed("alpha").exists());
        assert_eq!(fx.origin_of("alpha"), ("git".into(), Some(fx.url())));
    }

    #[test]
    fn the_chosen_skills_of_a_scanned_repository_are_installed_and_it_is_followed() {
        let fx = Fixture::new(&["alpha", "beta"]);
        let ScanResult::MultipleSkills {
            clone_id, skills, ..
        } = fx.scan()
        else {
            panic!("expected the skills to be listed");
        };
        assert_eq!(skills.len(), 2);
        assert!(!fx.installed("alpha").exists() && !fx.installed("beta").exists());

        let cache = fx.home.path().join("repos");
        let install = || {
            install_scanned_skills(
                &fx.store,
                &fx.adapters,
                &fx.pending,
                &clone_id,
                &["beta".into()],
                &fx.target(),
                Some(&cache),
            )
        };
        let installed = install().unwrap();

        assert_eq!(installed.len(), 1);
        assert!(fx.installed("beta").exists() && !fx.installed("alpha").exists());
        assert_eq!(fx.origin_of("beta"), ("git".into(), Some(fx.url())));
        let followed = fx.store.lock().list_repositories().unwrap();
        assert_eq!(followed.len(), 1);
        assert!(cache.join(&followed[0].id).join(".git").exists());
        assert!(install().is_err(), "a clone is used once");
    }

    #[test]
    fn a_local_folder_is_installed_and_remembers_where_it_came_from() {
        let fx = Fixture::new(&[]);
        write_skill(&fx.home.path().join("drafts"), "gamma");
        let folder = fx.home.path().join("drafts/skills/gamma");
        let path = folder.to_string_lossy().to_string();

        install_from_local(&fx.store, &fx.adapters, &path, &fx.target()).unwrap();

        assert!(fx.installed("gamma").exists());
        assert_eq!(fx.origin_of("gamma"), ("local".into(), Some(path)));
    }

    #[test]
    fn installing_for_no_agent_or_an_unknown_one_is_refused() {
        let fx = Fixture::new(&[]);
        write_skill(&fx.home.path().join("drafts"), "gamma");
        let folder = fx.home.path().join("drafts/skills/gamma");
        let install = |agents: &[String]| {
            let target = Target {
                agents,
                ..fx.target()
            };
            install_from_local(&fx.store, &fx.adapters, &folder.to_string_lossy(), &target)
        };

        assert!(matches!(install(&[]), Err(HkError::Validation(_))));
        assert!(matches!(
            install(&["nobody".into()]),
            Err(HkError::NotFound(_))
        ));
    }
}
