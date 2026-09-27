//! Install-source metadata maintenance: symlink healing, stale git meta
//! refresh, and pack backfill. Invoked from `sync_extensions*` (see `super::sync`).

use super::*;

impl Store {

    /// Clear `git`-typed install_meta (and the pack derived from it) that the
    /// git-source backfill wrongly stamped onto a symlinked skill. Such a skill
    /// is reached through a link sitting inside an agent-home dotfiles repo
    /// (e.g. `~/.claude/skills/X` -> `~/.agents/skills/X`); its real content
    /// lives elsewhere with its own (e.g. marketplace) source. Now that the
    /// scanner resolves symlinks before walking up for `.git`, these rows scan
    /// as non-git, so a leftover `git` install_meta is stale pollution that
    /// forks the skill into a bogus dotfiles-repo group. Real git installs are
    /// plain files (never symlinks), so they are never matched.
    pub(super) fn heal_symlinked_git_install_meta(
        conn: &rusqlite::Connection,
        extensions: &[Extension],
    ) -> Result<(), HkError> {
        for ext in extensions {
            if ext.kind != ExtensionKind::Skill || ext.source.origin == SourceOrigin::Git {
                continue;
            }
            let Some(source_path) = ext.source_path.as_deref() else {
                continue;
            };
            let is_symlink = std::fs::symlink_metadata(skill_entry_path(source_path))
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if is_symlink {
                conn.execute(
                    "UPDATE extensions
                     SET install_type = NULL, install_url = NULL, install_url_resolved = NULL,
                         install_branch = NULL, install_subpath = NULL, install_revision = NULL,
                         remote_revision = NULL, checked_at = NULL, check_error = NULL, pack = NULL
                     WHERE id = ?1 AND install_type = 'git'",
                    params![ext.id],
                )?;
            }
        }
        Ok(())
    }

    /// Refresh `git` install_meta that the source backfill stamped from a now-
    /// corrected source. The backfill (above) only fires on `install_type IS
    /// NULL`, so a row stamped in an earlier sync keeps its old `install_url`
    /// even after the scanner learns the real source (e.g. a plugin first seen
    /// as the enclosing dotfiles repo, now resolved to its marketplace repo via
    /// the install manifest). `deriveExtensionUrl` prefers `install_url`, so the
    /// stale value would keep the extension in the wrong group.
    ///
    /// We compare by **pack** (`owner/repo`), not by raw URL string: a genuine
    /// git install records its URL verbatim (`…/repo`) while the scanner reports
    /// the `.git/config` remote (`…/repo.git`), so a string compare would fire
    /// every sync and wipe a legitimate install's pinned revision/check state.
    /// Only a real owner/repo change realigns `install_url`/revision and clears
    /// the now-stale branch/subpath + pack (re-derived by `backfill_packs`).
    /// Limited to skills/plugins; marketplace/manual/cli installs are untouched.
    pub(super) fn refresh_stale_git_install_meta(conn: &rusqlite::Connection) -> Result<(), HkError> {
        let mut stmt = conn.prepare(
            "SELECT id, install_url, json_extract(source_json, '$.url'),
                    json_extract(source_json, '$.commit_hash')
             FROM extensions
             WHERE install_type = 'git'
               AND kind IN ('skill', 'plugin')
               AND json_extract(source_json, '$.url') IS NOT NULL
               -- Only an authoritative manifest source may correct an install
               -- record. A `.git`-inferred source (e.g. an HK-git-installed
               -- skill that merely sits under a dotfiles repo) must NOT overwrite
               -- the real install_url it was recorded with.
               AND json_extract(source_json, '$.from_manifest') = 1",
        )?;
        let rows: Vec<(String, Option<String>, String, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect();

        // GitHub owner/repo is case-insensitive, so compare lowercased to avoid
        // churning a row whose stored URL only differs in case.
        let norm = |url: &str| crate::scanner::extract_pack_from_url(url).map(|p| p.to_lowercase());
        for (id, install_url, source_url, source_commit) in &rows {
            let new_pack = norm(source_url);
            let old_pack = install_url.as_deref().and_then(norm);
            // Same repo (or unparseable new source) → leave the install record alone.
            if new_pack.is_none() || new_pack == old_pack {
                continue;
            }
            conn.execute(
                "UPDATE extensions
                 SET install_url = ?1, install_url_resolved = NULL,
                     install_branch = NULL, install_subpath = NULL,
                     install_revision = ?2, remote_revision = NULL,
                     checked_at = NULL, check_error = NULL, pack = NULL
                 WHERE id = ?3",
                params![source_url, source_commit, id],
            )?;
        }
        Ok(())
    }

    /// Backfill `pack` from install_url, source_json URL, or child extensions.
    /// Deployed skills lose their git context after being copied to agent directories,
    /// but install_url retains the repo URL. CLI parent extensions inherit pack from children.
    pub(super) fn backfill_packs(conn: &rusqlite::Connection) -> Result<(), HkError> {
        // 1. Backfill from own install_url or source_json URL
        let mut stmt = conn.prepare(
            "SELECT id, install_url, json_extract(source_json, '$.url')
             FROM extensions
             WHERE pack IS NULL
               AND (install_url IS NOT NULL OR json_extract(source_json, '$.url') IS NOT NULL)",
        )?;
        let rows: Vec<(String, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect();

        for (id, install_url, source_url) in &rows {
            let url = install_url.as_deref().or(source_url.as_deref());
            if let Some(pack) = url.and_then(crate::scanner::extract_pack_from_url) {
                conn.execute(
                    "UPDATE extensions SET pack = ?1 WHERE id = ?2",
                    params![pack, id],
                )?;
            }
        }

        // 2. CLI parents inherit pack from their children
        conn.execute_batch(
            "UPDATE extensions SET pack = (
                SELECT c.pack FROM extensions c
                WHERE c.cli_parent_id = extensions.id AND c.pack IS NOT NULL
                LIMIT 1
             )
             WHERE pack IS NULL
               AND kind = 'cli'
               AND EXISTS (
                SELECT 1 FROM extensions c
                WHERE c.cli_parent_id = extensions.id AND c.pack IS NOT NULL
               )",
        )?;

        // 3. CLI children inherit pack from their parent
        conn.execute_batch(
            "UPDATE extensions SET pack = (
                SELECT p.pack FROM extensions p
                WHERE p.id = extensions.cli_parent_id AND p.pack IS NOT NULL
             )
             WHERE pack IS NULL
               AND cli_parent_id IS NOT NULL
               AND EXISTS (
                SELECT 1 FROM extensions p
                WHERE p.id = extensions.cli_parent_id AND p.pack IS NOT NULL
               )",
        )?;

        Ok(())
    }

    /// Public wrapper so callers can re-run pack backfill after setting install_meta.
    pub fn run_backfill_packs(&self) -> Result<(), HkError> {
        Self::backfill_packs(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;

#[test]
    fn test_sync_preserves_install_meta() {
        let (store, _dir) = test_store();

        // Insert extension with install meta
        let mut ext = sample_extension();
        ext.id = "git-skill".into();
        ext.name = "git-skill".into();
        ext.install_meta = Some(InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/user/repo".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("abc123".into()),
            remote_revision: Some("def456".into()),
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        // Verify install meta was stored
        let fetched = store.get_extension("git-skill").unwrap().unwrap();
        assert!(fetched.install_meta.is_some());
        assert_eq!(
            fetched.install_meta.as_ref().unwrap().revision.as_deref(),
            Some("abc123")
        );

        // Sync with the same extension (scanner doesn't know about install meta)
        let mut synced = ext.clone();
        synced.install_meta = None;
        store.sync_extensions(&[synced]).unwrap();

        // Install meta should survive the sync
        let fetched = store.get_extension("git-skill").unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "git");
        assert_eq!(im.revision.as_deref(), Some("abc123"));
        assert_eq!(im.remote_revision.as_deref(), Some("def456"));
    }

#[cfg(unix)]
    #[test]
    fn test_sync_heals_symlinked_git_install_meta() {
        // Regression: the git-source backfill stamped `install_type=git` (+ a
        // pack) onto a skill symlinked in from `~/.agents/skills` while
        // `~/.claude` sat inside a dotfiles repo, forking it into a bogus
        // dotfiles-repo group. Such symlinked, non-git skills must be healed;
        // real file-backed git installs must be left intact.
        use std::os::unix::fs::symlink;
        let (store, dir) = test_store();

        // Symlinked skill: real content elsewhere, link under .claude/skills.
        let real = dir.path().join(".agents").join("skills").join("tdd");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("SKILL.md"), "---\nname: tdd\n---\n").unwrap();
        let claude_skills = dir.path().join(".claude").join("skills");
        std::fs::create_dir_all(&claude_skills).unwrap();
        let link = claude_skills.join("tdd");
        symlink(&real, &link).unwrap();

        let mut linked = sample_extension();
        linked.id = "linked-tdd".into();
        linked.name = "tdd".into();
        linked.source.origin = SourceOrigin::Agent;
        linked.source.url = None;
        linked.source_path = Some(link.join("SKILL.md").to_string_lossy().into());
        store.insert_extension(&linked).unwrap();
        store
            .set_install_meta(
                "linked-tdd",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/octo/dotfiles".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: Some("03dc45c".into()),
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        store.update_pack("linked-tdd", Some("octo/dotfiles")).unwrap();

        // Real (non-symlink) git install: must survive untouched.
        let realdir = dir.path().join(".codex").join("skills").join("foo");
        std::fs::create_dir_all(&realdir).unwrap();
        std::fs::write(realdir.join("SKILL.md"), "---\nname: foo\n---\n").unwrap();
        let mut plain = sample_extension();
        plain.id = "plain-foo".into();
        plain.name = "foo".into();
        plain.source.origin = SourceOrigin::Agent;
        plain.source_path = Some(realdir.join("SKILL.md").to_string_lossy().into());
        store.insert_extension(&plain).unwrap();
        store
            .set_install_meta(
                "plain-foo",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/real/repo".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: None,
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();

        // Disabled symlinked skill: on disk the file is `SKILL.md.disabled`, but
        // the scanner records source_path as `<dir>/SKILL.md` (see scanner
        // scan_skill_dir), and the entry dir is still a symlink, so it must heal
        // too.
        let real_dis = dir.path().join(".agents").join("skills").join("ddd");
        std::fs::create_dir_all(&real_dis).unwrap();
        std::fs::write(real_dis.join("SKILL.md.disabled"), "---\nname: ddd\n---\n").unwrap();
        let link_dis = claude_skills.join("ddd");
        symlink(&real_dis, &link_dis).unwrap();
        let mut disabled = sample_extension();
        disabled.id = "linked-ddd".into();
        disabled.name = "ddd".into();
        disabled.enabled = false;
        disabled.source.origin = SourceOrigin::Agent;
        disabled.source.url = None;
        disabled.source_path = Some(link_dis.join("SKILL.md").to_string_lossy().into());
        store.insert_extension(&disabled).unwrap();
        store
            .set_install_meta(
                "linked-ddd",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/octo/dotfiles".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: None,
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        store.update_pack("linked-ddd", Some("octo/dotfiles")).unwrap();

        // Symlinked skill whose *real content* is itself a git repo: the scanner
        // reports origin=Git, so heal must skip it (symlink alone is not enough)
        // and preserve its install_meta + pack.
        let real_git = dir.path().join(".agents").join("skills").join("ggg");
        std::fs::create_dir_all(&real_git).unwrap();
        std::fs::write(real_git.join("SKILL.md"), "---\nname: ggg\n---\n").unwrap();
        let link_git = claude_skills.join("ggg");
        symlink(&real_git, &link_git).unwrap();
        let mut gitlink = sample_extension();
        gitlink.id = "linked-ggg".into();
        gitlink.name = "ggg".into();
        gitlink.source.origin = SourceOrigin::Git;
        gitlink.source.url = Some("https://github.com/team/shared".into());
        gitlink.source_path = Some(link_git.join("SKILL.md").to_string_lossy().into());
        store.insert_extension(&gitlink).unwrap();
        store
            .set_install_meta(
                "linked-ggg",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/team/shared".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: None,
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        store.update_pack("linked-ggg", Some("team/shared")).unwrap();

        // Re-sync as the scanner now reports them (no install_meta; the
        // git-backed symlink keeps origin=Git).
        let mut s_linked = linked.clone();
        s_linked.install_meta = None;
        s_linked.pack = None;
        let mut s_plain = plain.clone();
        s_plain.install_meta = None;
        let mut s_disabled = disabled.clone();
        s_disabled.install_meta = None;
        s_disabled.pack = None;
        let mut s_gitlink = gitlink.clone();
        s_gitlink.install_meta = None;
        store
            .sync_extensions(&[s_linked, s_plain, s_disabled, s_gitlink])
            .unwrap();

        let healed = store.get_extension("linked-tdd").unwrap().unwrap();
        assert!(
            healed.install_meta.is_none(),
            "symlinked skill's bogus git install_meta should be cleared"
        );
        assert!(
            healed.pack.is_none(),
            "symlinked skill's dotfiles-repo pack should be cleared"
        );

        let kept = store.get_extension("plain-foo").unwrap().unwrap();
        assert_eq!(
            kept.install_meta.expect("real git install preserved").install_type,
            "git"
        );

        let healed_dis = store.get_extension("linked-ddd").unwrap().unwrap();
        assert!(
            healed_dis.install_meta.is_none(),
            "disabled symlinked skill (SKILL.md.disabled) must heal too"
        );
        assert!(healed_dis.pack.is_none(), "disabled symlinked skill's pack must clear");

        let kept_git = store.get_extension("linked-ggg").unwrap().unwrap();
        assert_eq!(
            kept_git.install_meta.expect("git-backed symlink preserved").install_type,
            "git"
        );
        assert_eq!(
            kept_git.pack.as_deref(),
            Some("team/shared"),
            "git-backed symlink's pack must survive (heal skips origin=Git)"
        );
    }

#[test]
    fn test_sync_refreshes_stale_git_install_meta() {
        // Regression: a plugin first scanned as the enclosing dotfiles repo got
        // install_type='git' + install_url/pack of that repo. After the scanner
        // learns the real marketplace source, the backfill (install_type IS
        // NULL only) can't update it, so the stale install_url kept the plugin
        // in the dotfiles group. Sync must realign install_url + pack to the
        // corrected source; a git row already in agreement stays untouched.
        let (store, _dir) = test_store();

        // Polluted plugin: stale dotfiles install_meta, but the scan now reports
        // the real marketplace source.
        let mut polluted = sample_extension();
        polluted.id = "plugin-cr".into();
        polluted.kind = ExtensionKind::Plugin;
        polluted.name = "code-review".into();
        polluted.source.origin = SourceOrigin::Git;
        polluted.source.url = Some("https://github.com/anthropics/claude-plugins-official".into());
        store.insert_extension(&polluted).unwrap();
        store
            .set_install_meta(
                "plugin-cr",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/octo/dotfiles".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: None,
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        store.update_pack("plugin-cr", Some("octo/dotfiles")).unwrap();

        // Consistent git install: install_url already matches its source.
        let mut consistent = sample_extension();
        consistent.id = "plugin-ok".into();
        consistent.kind = ExtensionKind::Plugin;
        consistent.name = "ok".into();
        consistent.source.origin = SourceOrigin::Git;
        consistent.source.url = Some("https://github.com/real/repo".into());
        store.insert_extension(&consistent).unwrap();
        store
            .set_install_meta(
                "plugin-ok",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/real/repo".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: None,
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        store.update_pack("plugin-ok", Some("real/repo")).unwrap();

        // Same repo, different URL string form: a genuine git install records
        // `.../repo` while the scanner reports the `.git/config` remote
        // `.../repo.git`. Same pack → must NOT be touched (pinned revision and
        // check state preserved), or every sync would churn legitimate installs.
        let mut variant = sample_extension();
        variant.id = "plugin-variant".into();
        variant.kind = ExtensionKind::Plugin;
        variant.name = "variant".into();
        variant.source.origin = SourceOrigin::Git;
        variant.source.url = Some("https://github.com/owner/repo.git".into());
        store.insert_extension(&variant).unwrap();
        store
            .set_install_meta(
                "plugin-variant",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/owner/repo".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: Some("pinned123".into()),
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();

        // Same repo, case-only difference: GitHub owner/repo is case-insensitive,
        // so a stored `Owner/Repo` vs scanned `owner/repo` must NOT churn.
        let mut casevar = sample_extension();
        casevar.id = "plugin-case".into();
        casevar.kind = ExtensionKind::Plugin;
        casevar.name = "case".into();
        casevar.source.origin = SourceOrigin::Git;
        casevar.source.url = Some("https://github.com/owner/repo".into());
        store.insert_extension(&casevar).unwrap();
        store
            .set_install_meta(
                "plugin-case",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/Owner/Repo".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: Some("casepin99".into()),
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();

        // Re-sync as the scanner now reports them (install_meta carried in DB).
        // The corrected plugin source is manifest-derived (known_marketplaces.json),
        // which is what licenses refresh to realign the stale install_url.
        let mut s_polluted = polluted.clone();
        s_polluted.install_meta = None;
        s_polluted.pack = None;
        s_polluted.source.from_manifest = true;
        let mut s_consistent = consistent.clone();
        s_consistent.install_meta = None;
        s_consistent.pack = None;
        let mut s_variant = variant.clone();
        s_variant.install_meta = None;
        let mut s_casevar = casevar.clone();
        s_casevar.install_meta = None;
        store
            .sync_extensions(&[s_polluted, s_consistent, s_variant, s_casevar])
            .unwrap();

        let fixed = store.get_extension("plugin-cr").unwrap().unwrap();
        assert_eq!(
            fixed.install_meta.expect("install_meta kept").url.as_deref(),
            Some("https://github.com/anthropics/claude-plugins-official"),
            "stale install_url must be realigned to the corrected source"
        );
        assert_eq!(
            fixed.pack.as_deref(),
            Some("anthropics/claude-plugins-official"),
            "pack must re-derive from the corrected source"
        );

        let kept = store.get_extension("plugin-ok").unwrap().unwrap();
        assert_eq!(kept.pack.as_deref(), Some("real/repo"), "consistent git row untouched");

        // Same-repo string variant: install record (incl. pinned revision) intact.
        let variant_kept = store.get_extension("plugin-variant").unwrap().unwrap();
        let vm = variant_kept.install_meta.expect("variant install_meta kept");
        assert_eq!(
            vm.url.as_deref(),
            Some("https://github.com/owner/repo"),
            "same-repo URL string variant must not be rewritten"
        );
        assert_eq!(
            vm.revision.as_deref(),
            Some("pinned123"),
            "same-repo variant's pinned revision must survive"
        );

        // Same-repo case-only variant: install record (incl. pinned revision) intact.
        let case_kept = store.get_extension("plugin-case").unwrap().unwrap();
        let cm = case_kept.install_meta.expect("case variant install_meta kept");
        assert_eq!(
            cm.revision.as_deref(),
            Some("casepin99"),
            "case-only repo variant must not be churned"
        );
    }

#[test]
    fn test_refresh_preserves_authoritative_install_url_for_inferred_source() {
        // Regression: an HK-git-installed skill records the real upstream in
        // install_meta. If the user keeps ~/.claude under a dotfiles git repo,
        // the scanner (no .skill-lock.json entry for an HK install) infers the
        // enclosing dotfiles repo as the source. refresh must NOT trust that
        // inferred source over the authoritative install_url (which would
        // re-attribute the skill to the dotfiles repo and wipe its pinned
        // revision). Only manifest-derived sources may realign.
        let (store, _dir) = test_store();
        let mut skill = sample_extension();
        skill.id = "hk-skill".into();
        skill.kind = ExtensionKind::Skill;
        skill.name = "my-skill".into();
        skill.source.origin = SourceOrigin::Git;
        skill.source.url = Some("https://github.com/octo/dotfiles".into());
        skill.source.from_manifest = false; // inferred from the enclosing .git
        store.insert_extension(&skill).unwrap();
        store
            .set_install_meta(
                "hk-skill",
                &InstallMeta {
                    install_type: "git".into(),
                    url: Some("https://github.com/real/my-skill".into()),
                    url_resolved: None,
                    branch: None,
                    subpath: None,
                    revision: Some("pinnedabc123".into()),
                    remote_revision: None,
                    checked_at: None,
                    check_error: None,
                },
            )
            .unwrap();
        store.update_pack("hk-skill", Some("real/my-skill")).unwrap();

        let mut scanned = skill.clone();
        scanned.install_meta = None;
        store.sync_extensions(&[scanned]).unwrap();

        let got = store.get_extension("hk-skill").unwrap().unwrap();
        let im = got.install_meta.expect("install_meta preserved");
        assert_eq!(
            im.url.as_deref(),
            Some("https://github.com/real/my-skill"),
            "authoritative install_url must survive an inferred (non-manifest) source"
        );
        assert_eq!(
            im.revision.as_deref(),
            Some("pinnedabc123"),
            "pinned revision must not be wiped by an inferred source"
        );
    }

#[test]
    fn test_sync_backfills_install_meta_from_git_source() {
        let (store, _dir) = test_store();

        // Create an extension with git source but no install_meta
        // (simulates a skill that existed before harnesskit was installed)
        let mut ext = sample_extension();
        ext.id = "pre-existing".into();
        ext.name = "pre-existing".into();
        ext.source = Source {
            origin: SourceOrigin::Git,
            url: Some("https://github.com/user/old-skill".into()),
            version: None,
            commit_hash: Some("aaa111".into()),
            from_manifest: false,
        };
        ext.install_meta = None;

        // Sync (as if scanner discovered it for the first time)
        store.sync_extensions(&[ext.clone()]).unwrap();

        // install_meta should be backfilled from source_json
        let fetched = store.get_extension("pre-existing").unwrap().unwrap();
        let im = fetched
            .install_meta
            .expect("install_meta should be backfilled");
        assert_eq!(im.install_type, "git");
        assert_eq!(im.url.as_deref(), Some("https://github.com/user/old-skill"));
        assert_eq!(im.revision.as_deref(), Some("aaa111"));
        // Fields not derivable from Source should remain None
        assert!(im.branch.is_none());
        assert!(im.subpath.is_none());
    }

#[test]
    fn test_sync_backfill_does_not_overwrite_existing_install_meta() {
        let (store, _dir) = test_store();

        // Extension with explicit install_meta (installed through our UI)
        let mut ext = sample_extension();
        ext.id = "our-install".into();
        ext.name = "our-install".into();
        ext.source = Source {
            origin: SourceOrigin::Git,
            url: Some("https://github.com/user/skill".into()),
            version: None,
            commit_hash: Some("new-scan-hash".into()),
            from_manifest: false,
        };
        ext.install_meta = Some(InstallMeta {
            install_type: "marketplace".into(),
            url: Some("marketplace-source".into()),
            url_resolved: Some("https://github.com/user/skill".into()),
            branch: None,
            subpath: Some("my-skill".into()),
            revision: Some("original-hash".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        // Sync with scanner data (install_meta = None from scanner)
        ext.install_meta = None;
        store.sync_extensions(&[ext]).unwrap();

        // Backfill should NOT overwrite — install_type is already set
        let fetched = store.get_extension("our-install").unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "marketplace"); // NOT overwritten to "git"
        assert_eq!(im.url.as_deref(), Some("marketplace-source")); // preserved
        assert_eq!(im.revision.as_deref(), Some("original-hash")); // NOT overwritten
    }

#[test]
    fn test_sync_backfill_skips_non_git_sources() {
        let (store, _dir) = test_store();

        // Extension with agent source (no .git detected)
        let mut ext = sample_extension();
        ext.id = "agent-skill".into();
        ext.name = "agent-skill".into();
        ext.source = Source {
            origin: SourceOrigin::Agent,
            url: None,
            version: None,
            commit_hash: None,
            from_manifest: false,
        };
        ext.install_meta = None;

        store.sync_extensions(&[ext]).unwrap();

        // Should NOT be backfilled
        let fetched = store.get_extension("agent-skill").unwrap().unwrap();
        assert!(fetched.install_meta.is_none());
    }
}
