use crate::{
    HkError,
    adapter::AgentAdapter,
    auditor::{AuditInput, Auditor},
    deployer,
    models::*,
    scanner,
    store::Store,
};
use parking_lot::Mutex;

/// Compare two filesystem paths, resolving symlinks where possible so that
/// e.g. `~/.gemini/antigravity/skills` (symlink) and `~/.claude/skills`
/// (target) are recognized as the same install location. Falls back to
/// textual equality when either path can't be canonicalized (doesn't yet
/// exist, permission error, etc.) so non-existent paths still compare
/// correctly against each other.
fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// Whether `adapter` scans the directory `dir` for skills under `scope`.
/// Used to identify cross-vendor sibling adapters that share an install
/// target (notably the `~/.agents/skills/` and `<repo>/.agents/skills`
/// paths declared by multiple adapters, plus user-created symlinks across
/// agent skill dirs).
fn adapter_scans_dir(
    adapter: &dyn AgentAdapter,
    dir: &std::path::Path,
    scope: &ConfigScope,
) -> bool {
    match scope {
        ConfigScope::Global => adapter.skill_dirs().iter().any(|d| paths_equal(d, dir)),
        ConfigScope::Project { path, .. } => {
            let project_path = std::path::Path::new(path);
            adapter
                .project_skill_dirs()
                .iter()
                .any(|rel| paths_equal(&project_path.join(rel), dir))
        }
    }
}

/// Common post-install flow: scan affected agents, sync to store, set install meta,
/// update pack, audit the installed extension(s), and persist audit results.
///
/// This extracts the duplicated 30-50 line pattern found in install_from_local,
/// install_from_git, install_from_marketplace, scan_git_repo, and install_scanned_skills.
pub fn post_install_sync(
    store: &Store,
    adapters: &[Box<dyn AgentAdapter>],
    agent_names: &[String],
    skill_name: &str,
    install_meta: Option<InstallMeta>,
    pack: Option<&str>,
    target_scope: &ConfigScope,
) -> Result<Vec<Extension>, HkError> {
    // 1. Scan and sync affected agents — scope-aware.
    let mut extensions = Vec::new();
    for a in adapters {
        if !agent_names.contains(&a.name().to_string()) {
            continue;
        }
        match target_scope {
            ConfigScope::Global => {
                // Existing path: scan_adapter covers global skill_dirs / mcp /
                // hooks / plugins, and sync_extensions_for_agent's stale-removal
                // is correct here (we DO want stale global rows for this agent
                // to be cleaned up).
                let exts = scanner::scan_adapter(a.as_ref());
                store.sync_extensions_for_agent(a.name(), &exts)?;
                extensions.extend(exts);
            }
            ConfigScope::Project { name, path } => {
                // Project path: scan_project_extensions returns Project-scoped
                // rows with scope-aware stable_ids. We deliberately use
                // insert_extension (upsert-only, no stale removal) instead of
                // sync_extensions_for_agent — the latter would treat every
                // global row for this agent as stale (since they're absent
                // from the project scan) and delete the unprotected ones.
                let exts =
                    scanner::scan_project_extensions(a.as_ref(), name, std::path::Path::new(path));
                for ext in &exts {
                    store.insert_extension(ext)?;
                }
                extensions.extend(exts);
            }
        }
    }

    // 2. Set install meta and pack for each agent — scope-aware id so the
    // right row gets updated.
    if let Some(ref meta) = install_meta {
        for agent_name in agent_names {
            let ext_id =
                scanner::stable_id_with_scope_for(skill_name, "skill", agent_name, target_scope);
            let _ = store.set_install_meta(&ext_id, meta);
            if let Some(p) = pack {
                let _ = store.update_pack(&ext_id, Some(p));
            }
        }
    }

    // 2b. Cross-vendor sibling propagation: when the install target_dir is a
    // shared path (e.g. `~/.agents/skills/` is scanned by Codex / Gemini CLI /
    // Cursor / Copilot / OpenCode at user-global scope; `<repo>/.agents/skills`
    // similarly at project scope), every other detected adapter that scans the
    // same directory will produce its own row on the next scan_all but lack
    // install_meta. Without this propagation, the Marketplace's URL-based
    // "installed?" check matches only the explicit target and falsely shows
    // the cross-vendor siblings as not installed.
    if let Some(ref meta) = install_meta {
        let Some(primary_agent) = agent_names.first() else {
            return Ok(extensions);
        };
        let Some(target_adapter) = adapters.iter().find(|a| a.name() == primary_agent.as_str())
        else {
            return Ok(extensions);
        };
        let Some(target_dir) = target_adapter.skill_dir_for(target_scope) else {
            return Ok(extensions);
        };

        for sibling in adapters.iter() {
            let sibling_name = sibling.name().to_string();
            if agent_names.contains(&sibling_name) {
                continue;
            }
            if !sibling.detect() {
                continue;
            }
            if !adapter_scans_dir(sibling.as_ref(), &target_dir, target_scope) {
                continue;
            }
            // Sibling shares target_dir — scan it so its row exists in the
            // store, then propagate install_meta and pack onto that row.
            match target_scope {
                ConfigScope::Global => {
                    let exts = scanner::scan_adapter(sibling.as_ref());
                    store.sync_extensions_for_agent(&sibling_name, &exts)?;
                    extensions.extend(exts);
                }
                ConfigScope::Project { name, path } => {
                    let exts = scanner::scan_project_extensions(
                        sibling.as_ref(),
                        name,
                        std::path::Path::new(path),
                    );
                    for ext in &exts {
                        store.insert_extension(ext)?;
                    }
                    extensions.extend(exts);
                }
            }
            let sibling_id = scanner::stable_id_with_scope_for(
                skill_name,
                "skill",
                &sibling_name,
                target_scope,
            );
            let _ = store.set_install_meta(&sibling_id, meta);
            if let Some(p) = pack {
                let _ = store.update_pack(&sibling_id, Some(p));
            }
        }
    }

    // 3. Audit the installed extensions
    let audit_results = audit_extension_by_name(skill_name, &extensions, adapters);
    for r in &audit_results {
        let _ = store.insert_audit_result(r);
    }

    Ok(extensions)
}

/// Whether an extension is eligible for HK's update flow.
///
/// Skills are the only kind that supports update via git clone + redeploy.
/// User-managed project skills (no install_meta) are excluded so the
/// marketplace name-match auto-linker doesn't bind them to a marketplace
/// skill that just happens to share a name. Project skills installed by HK
/// itself (which always carry install_meta) ARE eligible.
pub fn is_update_eligible(ext: &Extension) -> bool {
    if ext.kind != ExtensionKind::Skill {
        return false;
    }
    matches!(ext.scope, ConfigScope::Global) || ext.install_meta.is_some()
}

// --- Manual source binding ---------------------------------------------------
//
// Users with locally-scanned skills (no install_meta) can type a GitHub
// `owner/repo` into the detail panel's pack field to opt the row into Check
// Updates. We synthesize an install_meta with `install_type = "manual"` so the
// existing update pipeline picks it up, and so we can distinguish user-bound
// rows from real git/marketplace installs when the user later edits or
// unbinds.

/// Marker `install_type` value for install_meta rows that were synthesized
/// from a user-edited pack field. Distinct from "git" / "marketplace" so we
/// know which rows are safe to mutate when the user changes or clears pack.
/// Kept module-private — handlers compare against the literal `"manual"` to
/// stay symmetric with how they compare against `"git"` / `"marketplace"`.
const INSTALL_TYPE_MANUAL: &str = "manual";

/// Whether `pack` matches the `owner/repo` shape we synthesize URLs from.
/// Owner half rejects `.` so non-github paste forms like `gitlab.com/foo`
/// don't pass and get synthesized into a wrong `https://github.com/...` URL.
/// Repo half still allows `.` since GitHub permits it in repo names.
/// Apply `normalize_pack` first if the input might be a URL.
pub fn is_valid_pack_format(pack: &str) -> bool {
    let parts: Vec<&str> = pack.split('/').collect();
    if parts.len() != 2 {
        return false;
    }
    let owner_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_');
    let repo_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts[0].chars().all(owner_char)
        && parts[1].chars().all(repo_char)
}

/// Reduce common GitHub source identifiers to the canonical `owner/repo`
/// form so users can paste raw repo URLs into the detail panel's source
/// field. Returns the input trimmed-and-unchanged when no recognized
/// pattern matches; `is_valid_pack_format` then decides whether to accept.
///
/// Recognized inputs:
/// * `owner/repo` (already canonical)
/// * `https://github.com/owner/repo` (with/without scheme, `.git`, trailing `/`)
/// * `github.com/owner/repo/tree/main` (extra path segments ignored)
/// * `git@github.com:owner/repo.git` (SSH clone URL)
///
/// Delegates URL parsing to `scanner::extract_pack_from_url` and adds two
/// constraints the install-flow parser doesn't enforce: (1) host must be
/// github.com — otherwise we'd synthesize a wrong `https://github.com/{...}`
/// URL from e.g. a gitlab paste; (2) schemeless `github.com/owner/repo`
/// must be accepted (the short-format branch rejects host-shaped first
/// segments, so we re-attempt with an https prefix).
pub fn normalize_pack(input: &str) -> String {
    let trimmed = input.trim();
    // Reject non-github paste forms up front — we can't honor them anyway
    // since our synthesized URL is always github.com/{pack}.git. Bare
    // `owner/repo` has no scheme, so we treat the absence of `://` as
    // permissive (it'll be re-validated as canonical pack below).
    let looks_like_github = trimmed.starts_with("git@github.com:")
        || trimmed.starts_with("https://github.com/")
        || trimmed.starts_with("http://github.com/")
        || trimmed.starts_with("github.com/")
        || !trimmed.contains("://");
    if !looks_like_github {
        return trimmed.to_string();
    }
    // extract_pack_from_url's short-format branch refuses host-shaped first
    // segments (`!parts[0].contains('.')`), so schemeless `github.com/...`
    // needs an https prefix to hit the HTTPS branch instead.
    let to_parse: String = if trimmed.starts_with("github.com/") {
        format!("https://{}", trimmed)
    } else {
        trimmed.to_string()
    };
    scanner::extract_pack_from_url(&to_parse).unwrap_or_else(|| trimmed.to_string())
}

fn synthesize_manual_install_meta(pack: &str) -> InstallMeta {
    InstallMeta {
        install_type: INSTALL_TYPE_MANUAL.into(),
        url: Some(format!("https://github.com/{}.git", pack)),
        url_resolved: None,
        branch: None,
        subpath: None,
        revision: None,
        remote_revision: None,
        checked_at: None,
        check_error: None,
    }
}

/// Persist a pack value across a group of extension rows and maintain the
/// matching manual-bound install_meta. Single source of truth for the
/// "user types in the detail panel's source field" flow — both the desktop
/// and web settings handlers funnel through here.
///
/// Rules per row (only Skill kind participates; other kinds get pack updated
/// but no install_meta side-effect):
///
/// * `(valid pack, no install_meta)` → synthesize a `"manual"` install_meta.
/// * `(valid pack, existing "manual" meta)` → refresh url to follow pack.
/// * `(valid pack, real "git" / "marketplace" meta)` → leave meta untouched
///   (pack is a grouping hint, not authority over real installs).
/// * `(cleared pack, existing "manual" meta)` → clear install_meta so the row
///   drops out of Check Updates again.
/// * `(cleared pack, real meta)` → leave meta untouched.
/// * `(invalid pack format, anything)` → pack column still updates so the
///   user sees what they typed, but install_meta is left alone; the UI is
///   expected to warn before reaching this branch.
pub fn bind_pack(store: &Store, ids: &[String], pack: Option<&str>) -> Result<(), HkError> {
    // Normalize first so URLs/SSH paths get reduced to owner/repo before we
    // touch the DB. Frontend also normalizes; this is defense-in-depth for
    // any non-UI caller (CLI, future API client).
    let trimmed: Option<String> = pack
        .map(normalize_pack)
        .filter(|s| !s.is_empty());

    // Phase 1: always persist the pack column update so the user's typing
    // is preserved even when we can't synthesize meta (invalid format).
    store.batch_update_pack(ids, trimmed.as_deref())?;

    // Phase 2: maintain install_meta per row based on (current state, new pack).
    for id in ids {
        let Some(ext) = store.get_extension(id)? else {
            continue;
        };
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        let is_manual = ext
            .install_meta
            .as_ref()
            .is_some_and(|m| m.install_type == INSTALL_TYPE_MANUAL);
        // Real (git / marketplace) install_meta is sacred — pack is just a
        // grouping hint when a real install exists. Skip both synth and clear.
        if ext.install_meta.is_some() && !is_manual {
            continue;
        }
        match trimmed.as_deref() {
            Some(p) if is_valid_pack_format(p) => {
                store.set_install_meta(id, &synthesize_manual_install_meta(p))?;
            }
            None if is_manual => {
                store.clear_install_meta(id)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Best-effort one-shot migration: any Skill row with a valid `pack` value
/// but no install_meta gets a `"manual"` install_meta synthesized. Lets users
/// who typed pack into the detail panel **before** this feature shipped pick
/// up Check Updates on their next click without having to re-touch the field.
///
/// Idempotent: rows that already have install_meta (including manual ones)
/// are skipped. Safe to call at the top of every Check Updates invocation.
pub fn migrate_pack_to_manual_meta(store: &Store) -> Result<usize, HkError> {
    let extensions = store.list_extensions(None, None)?;
    let mut migrated = 0;
    for ext in extensions {
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        if ext.install_meta.is_some() {
            continue;
        }
        let Some(raw_pack) = ext.pack.as_deref() else {
            continue;
        };
        // Pre-feature DB rows may hold raw GitHub URLs the user pasted —
        // normalize before validation so they migrate the same as plain
        // owner/repo entries.
        let pack = normalize_pack(raw_pack);
        if !is_valid_pack_format(&pack) {
            continue;
        }
        // If normalization changed the value, persist the canonical form so
        // subsequent reads see the same shape the UI now writes.
        if pack != raw_pack {
            store.update_pack(&ext.id, Some(&pack))?;
        }
        store.set_install_meta(&ext.id, &synthesize_manual_install_meta(&pack))?;
        migrated += 1;
    }
    Ok(migrated)
}

/// Whether two extensions share the same scope. Used by update-apply flows
/// to scope sibling refreshes — a Global update should only refresh Global
/// copies (not clobber a user's project copy of the same name) and a
/// project update should only refresh that project's own copies.
pub fn same_scope(a: &ConfigScope, b: &ConfigScope) -> bool {
    match (a, b) {
        (ConfigScope::Global, ConfigScope::Global) => true,
        (ConfigScope::Project { path: pa, .. }, ConfigScope::Project { path: pb, .. }) => pa == pb,
        _ => false,
    }
}

/// Full audit of all extensions — scans skill content, MCP server info, hooks, plugins,
/// and CLIs, then runs the auditor's rule engine and persists results.
///
/// This is the service-layer equivalent of the desktop `run_audit` command
/// and the CLI `cmd_audit` logic.
pub fn run_full_audit(
    store: &Store,
    adapters: &[Box<dyn AgentAdapter>],
) -> Result<Vec<AuditResult>, HkError> {
    let extensions = store.list_extensions(None, None)?;
    let results = audit_extensions(&extensions, adapters);

    for result in &results {
        let _ = store.insert_audit_result(result);
    }

    Ok(results)
}

/// Run audit on a pre-fetched list of extensions without needing a store reference.
/// Useful when callers need to control lock scope separately for reads and writes.
pub fn audit_extensions(
    extensions: &[Extension],
    adapters: &[Box<dyn AgentAdapter>],
) -> Vec<AuditResult> {
    let auditor = Auditor::new();
    let mut inputs = Vec::new();

    for ext in extensions {
        let (content, mcp_command, mcp_args, mcp_env, file_path) = match ext.kind {
            ExtensionKind::Skill => {
                let (skill_content, skill_path) =
                    find_skill_content(adapters, &ext.id, &ext.agents);
                (
                    skill_content,
                    None,
                    vec![],
                    Default::default(),
                    skill_path.unwrap_or_else(|| ext.name.clone()),
                )
            }
            ExtensionKind::Mcp => {
                let mut cmd = None;
                let mut args = vec![];
                let mut env = std::collections::HashMap::new();
                for a in adapters {
                    if !ext.agents.contains(&a.name().to_string()) {
                        continue;
                    }
                    for server in a.read_mcp_servers() {
                        if scanner::stable_id_for(&server.name, "mcp", a.name()) == ext.id {
                            cmd = Some(server.command);
                            args = server.args;
                            env = server.env;
                            break;
                        }
                    }
                }
                (String::new(), cmd, args, env, ext.name.clone())
            }
            ExtensionKind::Hook => {
                let raw_command = ext
                    .name
                    .splitn(3, ':')
                    .nth(2)
                    .unwrap_or(&ext.name)
                    .to_string();
                (
                    raw_command,
                    None,
                    vec![],
                    Default::default(),
                    ext.name.clone(),
                )
            }
            ExtensionKind::Plugin => {
                let plugin_dir = ext.source_path.as_deref().unwrap_or(&ext.name);
                let content = read_plugin_content(plugin_dir);
                let file_path = ext.source_path.clone().unwrap_or_else(|| ext.name.clone());
                (content, None, vec![], Default::default(), file_path)
            }
            ExtensionKind::Cli => (
                String::new(),
                None,
                vec![],
                Default::default(),
                ext.name.clone(),
            ),
        };

        let input = AuditInput {
            extension_id: ext.id.clone(),
            kind: ext.kind,
            name: ext.name.clone(),
            content,
            source: ext.source.clone(),
            file_path,
            mcp_command,
            mcp_args,
            mcp_env,
            installed_at: ext.installed_at,
            updated_at: ext.updated_at,
            permissions: ext.permissions.clone(),
            cli_parent_id: ext.cli_parent_id.clone(),
            cli_meta: ext.cli_meta.clone(),
            child_permissions: vec![],
            pack: ext.pack.clone(),
        };
        inputs.push(input);
    }

    auditor.audit_batch(&inputs)
}

/// Audit a single extension by name (best-effort, skills only).
/// Returns audit results to be stored by the caller.
fn audit_extension_by_name(
    name: &str,
    extensions: &[Extension],
    adapters: &[Box<dyn AgentAdapter>],
) -> Vec<AuditResult> {
    let auditor = Auditor::new();
    let mut results = Vec::new();
    for ext in extensions {
        if ext.name != name {
            continue;
        }
        let input = match ext.kind {
            ExtensionKind::Skill => {
                let (content, file_path) = find_skill_content(adapters, &ext.id, &ext.agents);
                AuditInput {
                    extension_id: ext.id.clone(),
                    kind: ext.kind,
                    name: ext.name.clone(),
                    content,
                    source: ext.source.clone(),
                    file_path: file_path.unwrap_or_else(|| ext.name.clone()),
                    mcp_command: None,
                    mcp_args: vec![],
                    mcp_env: Default::default(),
                    installed_at: ext.installed_at,
                    updated_at: ext.updated_at,
                    permissions: ext.permissions.clone(),
                    cli_parent_id: ext.cli_parent_id.clone(),
                    cli_meta: ext.cli_meta.clone(),
                    child_permissions: vec![],
                    pack: ext.pack.clone(),
                }
            }
            _ => continue,
        };
        results.push(auditor.audit(&input));
    }
    results
}

/// Read source files from a plugin directory for audit analysis.
/// Returns concatenated content with file markers.
/// Reads .js, .ts, .py, .sh files up to a total of 512 KB.
/// NOTE: .json files are excluded — package.json is handled separately by
/// `infer_plugin_permissions` and `plugin-lifecycle-scripts` rule, and
/// package-lock.json would consume the entire read budget with URLs.
fn plugin_file_extension(path: &std::path::Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy();
    let base = file_name.strip_suffix(".disabled").unwrap_or(&file_name);
    std::path::Path::new(base)
        .extension()
        .map(|ext| ext.to_string_lossy().to_string())
}

fn read_plugin_content(plugin_path: &str) -> String {
    use std::path::Path;

    let allowed_extensions = ["js", "ts", "py", "sh", "mjs", "cjs"];
    let max_total_bytes: usize = 512 * 1024;
    let mut total_bytes = 0usize;
    let mut parts = Vec::new();

    let path = Path::new(plugin_path);
    let mut files = Vec::new();
    if path.is_file() {
        if let Some(ext) = plugin_file_extension(path)
            && allowed_extensions.contains(&ext.as_str())
        {
            files.push(path.to_path_buf());
        }
    } else if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let file = entry.path();
            if !file.is_file() {
                continue;
            }
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
            if allowed_extensions.contains(&ext) {
                files.push(file);
            }
        }
    } else {
        return String::new();
    }

    for file in files {
        if let Ok(content) = std::fs::read_to_string(&file) {
            let bytes_to_add = content.len();
            if total_bytes + bytes_to_add > max_total_bytes {
                break;
            }
            parts.push(format!(
                "// === {} ===\n{}",
                file.file_name().unwrap_or_default().to_string_lossy(),
                content
            ));
            total_bytes += bytes_to_add;
        }
    }

    parts.join("\n")
}

fn remove_path(path: &std::path::Path) -> Result<(), HkError> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else if path.is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn read_plugin_detail(path: &std::path::Path, fallback: &str) -> String {
    if path.is_file() {
        return std::fs::read_to_string(path).unwrap_or_else(|_| fallback.to_string());
    }

    for candidate in [path.join("README.md"), path.join("readme.md")] {
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            return text;
        }
    }

    let mut dir = path.to_path_buf();
    while dir.pop() {
        if dir.join(".git").exists() {
            for name in ["README.md", "readme.md"] {
                if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
                    return text;
                }
            }
            break;
        }
    }

    fallback.to_string()
}

/// Find skill content and file path by scanning adapters for the matching extension.
fn find_skill_content(
    adapters: &[Box<dyn AgentAdapter>],
    ext_id: &str,
    agent_filter: &[String],
) -> (String, Option<String>) {
    for a in adapters {
        if !agent_filter.contains(&a.name().to_string()) {
            continue;
        }
        for skill_dir in a.skill_dirs() {
            let Ok(entries) = std::fs::read_dir(&skill_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let skill_file = if path.is_dir() {
                    let md = path.join("SKILL.md");
                    if md.exists() {
                        md
                    } else {
                        path.join("SKILL.md.disabled")
                    }
                } else if path
                    .extension()
                    .is_some_and(|e| e == "md" || e == "disabled")
                {
                    path.clone()
                } else {
                    continue;
                };
                if !skill_file.exists() {
                    continue;
                }
                let name = scanner::parse_skill_name(&skill_file).unwrap_or_else(|| {
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });
                if scanner::stable_id_for(&name, "skill", a.name()) == ext_id {
                    let content = std::fs::read_to_string(&skill_file).unwrap_or_default();
                    return (content, Some(skill_file.to_string_lossy().to_string()));
                }
            }
        }
    }
    (String::new(), None)
}

// --- Extension command flows shared by hk-desktop and hk-web -------------

/// Rich detail returned by `get_extension_content`. Surfaces the on-disk
/// representation (file/dir path + readable text) so the UI's detail panel
/// can show it. `symlink_target` is only set for skills whose entry or
/// containing dir is a symlink — useful for development setups where the
/// user keeps the canonical copy elsewhere.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtensionContent {
    pub content: String,
    pub path: Option<String>,
    pub symlink_target: Option<String>,
}

/// Remove an extension from disk/config (per-kind) and then from the DB.
///
/// Disk and DB are mutated in two separately-locked phases so I/O does not
/// hold the store mutex. The DB delete happens last; if disk removal fails
/// the row stays so the next scan can recover.
/// Find other skill rows whose source_path resolves (via symlinks) to the
/// same physical file as `target`. Used by `delete_extension` to cascade
/// the DB cleanup: deleting `~/.gemini/antigravity/skills/X/SKILL.md`
/// (which is a symlink into `~/.claude/skills/X/SKILL.md`) physically
/// removes both, so Claude's row must come along too. Canonicalization
/// is done before Phase 2 of delete_extension so the target file still
/// exists on disk; once it's gone canonicalize would fail.
fn find_canonical_skill_siblings(
    store: &Store,
    target: &Extension,
) -> Result<Vec<String>, HkError> {
    let Some(target_path) = target.source_path.as_deref() else {
        return Ok(Vec::new());
    };
    let Ok(target_canon) = std::fs::canonicalize(target_path) else {
        return Ok(Vec::new());
    };
    let candidates = store.find_ids_by_name_and_kind(&target.name, "skill")?;
    let mut siblings = Vec::new();
    for cand_id in candidates {
        if cand_id == target.id {
            continue;
        }
        let Ok(Some(cand)) = store.get_extension(&cand_id) else {
            continue;
        };
        let Some(cand_path) = cand.source_path.as_deref() else {
            continue;
        };
        if std::fs::canonicalize(cand_path).ok().as_ref() == Some(&target_canon) {
            siblings.push(cand_id);
        }
    }
    Ok(siblings)
}

pub fn delete_extension(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    id: &str,
) -> Result<(), HkError> {
    // Phase 1: read metadata + find canonical siblings under the lock, then
    // drop it before any I/O. An empty Option from get_extension is treated
    // as success so the delete contract is idempotent — a parallel batch
    // delete can ask to remove a row that an earlier cascade already cleared.
    let (ext, projects, sibling_ids) = {
        let store = store.lock();
        let Some(ext) = store.get_extension(id)? else {
            return Ok(());
        };
        let projects = store.list_project_tuples();
        let sibling_ids = if matches!(ext.kind, ExtensionKind::Skill) {
            find_canonical_skill_siblings(&store, &ext)?
        } else {
            Vec::new()
        };
        (ext, projects, sibling_ids)
    };

    // Phase 2: filesystem / config-file mutation. No DB access here.
    match ext.kind {
        ExtensionKind::Skill => {
            if let Some(loc) = scanner::find_skill_by_id(adapters, id, &ext.agents, &projects) {
                if loc.entry_path.is_dir() {
                    std::fs::remove_dir_all(&loc.entry_path)?;
                } else {
                    std::fs::remove_file(&loc.entry_path)?;
                }
            }
        }
        ExtensionKind::Mcp => {
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.mcp_config_path_for(&ext.scope) else {
                    continue;
                };
                for server in adapter.read_mcp_servers_from(&config_path) {
                    let candidate = scanner::stable_id_with_scope_for(
                        &server.name,
                        "mcp",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        deployer::remove_mcp_server(
                            &config_path,
                            &server.name,
                            adapter.mcp_format(),
                        )?;
                    }
                }
            }
        }
        ExtensionKind::Hook => {
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.hook_config_path_for(&ext.scope) else {
                    continue;
                };
                for hook in adapter.read_hooks_from(&config_path) {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    let candidate = scanner::stable_id_with_scope_for(
                        &hook_name,
                        "hook",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        deployer::remove_hook(
                            &config_path,
                            &hook.event,
                            hook.matcher.as_deref(),
                            &hook.command,
                            adapter.hook_format(),
                        )?;
                    }
                }
            }
        }
        ExtensionKind::Cli => {
            // Child skills/MCPs are deleted separately by their own IDs.
            // This branch only runs for full CLI uninstall (parent record cleanup).
        }
        ExtensionKind::Plugin => {
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(
                        &format!("{}:{}", plugin.name, plugin.source),
                        "plugin",
                        adapter.name(),
                    ) != id
                    {
                        continue;
                    }
                    let plugin_key = if plugin.source.is_empty() {
                        plugin.name.clone()
                    } else {
                        format!("{}@{}", plugin.name, plugin.source)
                    };
                    if adapter.name() == "claude" {
                        let config_path = adapter.plugin_config_path();
                        deployer::remove_plugin_entry(&config_path, &plugin_key)?;
                    } else if adapter.name() == "codex" {
                        // Remove folder + config.toml entry
                        if let Some(ref path) = plugin.path {
                            let target = if let Some(parent) = path.parent() {
                                if parent
                                    .file_name()
                                    .map(|n| n != "cache" && n != "plugins")
                                    .unwrap_or(false)
                                {
                                    parent
                                } else {
                                    path.as_path()
                                }
                            } else {
                                path.as_path()
                            };
                            if target.is_dir() {
                                std::fs::remove_dir_all(target)?;
                            } else if target.is_file() {
                                std::fs::remove_file(target)?;
                            }
                        }
                        deployer::remove_codex_plugin_entry(
                            &adapter.mcp_config_path(),
                            &plugin_key,
                        )?;
                    } else if adapter.name() == "gemini" {
                        if let Some(ref path) = plugin.path {
                            remove_path(path)?;
                        }
                        deployer::remove_gemini_extension_entry(
                            &adapter.base_dir().join("extensions"),
                            &plugin.name,
                        )?;
                    } else if adapter.name() == "copilot" {
                        if let Some(ref path) = plugin.path {
                            remove_path(path)?;
                        }
                        if let (Some(uri), Some(vscode_dir)) =
                            (&plugin.uri, adapter.vscode_user_dir())
                        {
                            // Best-effort: VS Code may hold a lock on state.vscdb
                            if let Err(e) = deployer::remove_vscode_plugin_entry(&vscode_dir, uri) {
                                eprintln!("Warning: failed to clean up VS Code plugin entry: {e}");
                            }
                        }
                    } else if let Some(ref path) = plugin.path {
                        remove_path(path)?;
                    }
                }
            }
        }
    }

    // Phase 3: DB delete, only after disk side succeeded. Cascade to any
    // canonical-path siblings — their on-disk files were the same inode
    // (the user reached them through a different agent's symlinked
    // skill_dir) so they're already gone after Phase 2.
    {
        let store = store.lock();
        store.delete_extension(id)?;
        for sib_id in &sibling_ids {
            let _ = store.delete_extension(sib_id);
        }
    }
    Ok(())
}

/// Read the rich on-disk content for an extension (skill text, MCP server
/// config summary, hook detail, plugin README, …). Pure read-only — locks
/// the store only to fetch metadata, then releases before any I/O.
pub fn get_extension_content(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    id: &str,
) -> Result<ExtensionContent, HkError> {
    let (ext, projects) = {
        let store = store.lock();
        let ext = store
            .get_extension(id)?
            .ok_or_else(|| HkError::NotFound("Extension not found".into()))?;
        let projects = store.list_project_tuples();
        (ext, projects)
    };

    match ext.kind {
        ExtensionKind::Skill => {
            if let Some(loc) = scanner::find_skill_by_id(adapters, id, &ext.agents, &projects) {
                let dir = if loc.entry_path.is_dir() {
                    loc.entry_path.to_string_lossy().to_string()
                } else {
                    loc.skill_file
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                };
                // Detect symlink: check entry itself, then parent skill_dir
                let dir_symlink_target = if loc
                    .skill_dir
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    std::fs::read_link(&loc.skill_dir).ok()
                } else {
                    None
                };
                let symlink_target = if loc
                    .entry_path
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    std::fs::read_link(&loc.entry_path)
                        .ok()
                        .map(|t| t.to_string_lossy().to_string())
                } else if let Some(ref resolved_dir) = dir_symlink_target {
                    let entry_name = loc.entry_path.file_name().unwrap_or_default();
                    Some(resolved_dir.join(entry_name).to_string_lossy().to_string())
                } else {
                    None
                };
                let content = std::fs::read_to_string(&loc.skill_file)?;
                Ok(ExtensionContent {
                    content,
                    path: Some(dir),
                    symlink_target,
                })
            } else {
                Err(HkError::NotFound("Skill file not found".into()))
            }
        }
        ExtensionKind::Mcp => {
            // The trait helper resolves the right file per scope; the
            // scanner's `source_path` is the canonical config path for project
            // entries — we prefer it when set.
            let mut fallback_config_path = ext.source_path.clone();
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.mcp_config_path_for(&ext.scope) else {
                    continue;
                };
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for server in adapter.read_mcp_servers_from(&config_path) {
                    let candidate = scanner::stable_id_with_scope_for(
                        &server.name,
                        "mcp",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        let mut lines = vec![format!("Command: {}", server.command)];
                        if !server.args.is_empty() {
                            lines.push(format!("Args: {}", server.args.join(" ")));
                        }
                        if !server.env.is_empty() {
                            lines.push("Environment:".into());
                            for k in server.env.keys() {
                                lines.push(format!("  {} = ****", k));
                            }
                        }
                        return Ok(ExtensionContent {
                            content: lines.join("\n"),
                            path: Some(config_path.to_string_lossy().to_string()),
                            symlink_target: None,
                        });
                    }
                }
            }
            // Disabled MCP: still surface the config path where it lived.
            Ok(ExtensionContent {
                content: ext.description,
                path: fallback_config_path,
                symlink_target: None,
            })
        }
        ExtensionKind::Hook => {
            let mut fallback_config_path = ext.source_path.clone();
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(config_path) = adapter.hook_config_path_for(&ext.scope) else {
                    continue;
                };
                if fallback_config_path.is_none() {
                    fallback_config_path = Some(config_path.to_string_lossy().to_string());
                }
                for hook in adapter.read_hooks_from(&config_path) {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    let candidate = scanner::stable_id_with_scope_for(
                        &hook_name,
                        "hook",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == id {
                        let mut lines = vec![format!("Event: {}", hook.event)];
                        if let Some(m) = &hook.matcher {
                            lines.push(format!("Matcher: {}", m));
                        }
                        lines.push(format!("Command: {}", hook.command));
                        return Ok(ExtensionContent {
                            content: lines.join("\n"),
                            path: Some(config_path.to_string_lossy().to_string()),
                            symlink_target: None,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: fallback_config_path,
                symlink_target: None,
            })
        }
        ExtensionKind::Plugin => {
            for adapter in adapters {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                for plugin in adapter.read_plugins() {
                    if scanner::stable_id_for(
                        &format!("{}:{}", plugin.name, plugin.source),
                        "plugin",
                        adapter.name(),
                    ) == id
                    {
                        let path_str = plugin
                            .path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        // Try README.md from plugin dir first, then walk up to
                        // find the repo root (for git-cloned plugins where
                        // README sits one or more levels above the manifest).
                        let content = plugin
                            .path
                            .as_ref()
                            .map(|path| read_plugin_detail(path, &ext.description))
                            .unwrap_or_else(|| ext.description.clone());
                        return Ok(ExtensionContent {
                            content,
                            path: path_str,
                            symlink_target: None,
                        });
                    }
                }
            }
            Ok(ExtensionContent {
                content: ext.description,
                path: None,
                symlink_target: None,
            })
        }
        ExtensionKind::Cli => Ok(ExtensionContent {
            content: ext.description,
            path: None,
            symlink_target: None,
        }),
    }
}

/// Cross-agent deploy: copy a Skill / MCP / Hook / CLI from its source agent
/// into `target_agent`. Returns a human-readable identifier of what was
/// deployed (skill name, MCP server name, or `event:command` for hooks) so
/// the UI can show the result. The wrapper is responsible for any post-deploy
/// rescan/sync (web does this; desktop does not, matching prior behavior).
pub fn install_to_agent(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    extension_id: &str,
    target_agent: &str,
    hermes_category: Option<&str>,
) -> Result<String, HkError> {
    let (ext, projects) = {
        let store = store.lock();
        let ext = store
            .get_extension(extension_id)?
            .ok_or_else(|| HkError::NotFound("Extension not found".into()))?;
        let projects = store.list_project_tuples();
        (ext, projects)
    };

    let target_adapter = adapters
        .iter()
        .find(|a| a.name() == target_agent)
        .ok_or_else(|| HkError::NotFound(format!("Agent '{}' not found", target_agent)))?;

    match ext.kind {
        ExtensionKind::Skill => {
            let source_path =
                scanner::find_skill_by_id(adapters, extension_id, &ext.agents, &projects)
                    .map(|loc| loc.entry_path)
                    .ok_or_else(|| HkError::Internal("Could not find source skill files".into()))?;
            // Cross-agent install always lands at the target's global scope.
            // `skill_dir_for_category` returns None for flat-layout agents, so
            // non-Hermes targets fall through to their default skill dir.
            let target_dir = hermes_category
                .and_then(|cat| {
                    target_adapter.skill_dir_for_category(&ConfigScope::Global, cat)
                })
                .or_else(|| target_adapter.skill_dirs().into_iter().next())
                .ok_or_else(|| {
                    HkError::Internal(format!("No skill directory for agent '{}'", target_agent))
                })?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir)?;

            // Propagate install_meta from source to the new target row so
            // cross-agent deploys produce consistent provenance — and let
            // post_install_sync fan that out to cross-vendor siblings (when
            // target_dir is a shared path like `~/.agents/skills/`).
            // Hand-managed sources (no install_meta) skip; the target file
            // is on disk and a future scan_all will pick it up unlinked,
            // matching the source's lack of provenance.
            if let Some(meta) = ext.install_meta.clone() {
                let store_guard = store.lock();
                post_install_sync(
                    &store_guard,
                    adapters,
                    &[target_agent.to_string()],
                    &deployed_name,
                    Some(meta),
                    ext.pack.as_deref(),
                    &ConfigScope::Global,
                )?;
            }
            Ok(deployed_name)
        }
        ExtensionKind::Mcp => {
            let mut source_entry = None;
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(source_path) = adapter.mcp_config_path_for(&ext.scope) else {
                    continue;
                };
                for server in adapter.read_mcp_servers_from(&source_path) {
                    let candidate = scanner::stable_id_with_scope_for(
                        &server.name,
                        "mcp",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == extension_id {
                        source_entry = Some(server);
                        break;
                    }
                }
                if source_entry.is_some() {
                    break;
                }
            }
            let mut entry = source_entry.ok_or_else(|| {
                HkError::Internal("Could not find source MCP server config".into())
            })?;
            if target_adapter.needs_path_injection() {
                deployer::ensure_path_injection(&mut entry);
            }
            let config_path = target_adapter.mcp_config_path();
            deployer::deploy_mcp_server(&config_path, &entry, target_adapter.mcp_format())?;
            Ok(entry.name)
        }
        ExtensionKind::Hook => {
            let mut source_entry = None;
            for adapter in adapters.iter() {
                if !ext.agents.contains(&adapter.name().to_string()) {
                    continue;
                }
                let Some(source_path) = adapter.hook_config_path_for(&ext.scope) else {
                    continue;
                };
                for hook in adapter.read_hooks_from(&source_path) {
                    let hook_name = format!(
                        "{}:{}:{}",
                        hook.event,
                        hook.matcher.as_deref().unwrap_or("*"),
                        hook.command
                    );
                    let candidate = scanner::stable_id_with_scope_for(
                        &hook_name,
                        "hook",
                        adapter.name(),
                        &ext.scope,
                    );
                    if candidate == extension_id {
                        source_entry = Some(hook);
                        break;
                    }
                }
                if source_entry.is_some() {
                    break;
                }
            }
            let mut entry = source_entry
                .ok_or_else(|| HkError::Internal("Could not find source hook config".into()))?;

            // Translate event name to the target agent's convention. Agents
            // disagree on hook event names (Claude `PreToolUse` vs Codex
            // `pre_tool_use`, etc.) so a missing translation is a hard error.
            let translated_event = target_adapter
                .translate_hook_event(&entry.event)
                .ok_or_else(|| {
                    HkError::Internal(format!(
                        "Hook event '{}' is not supported by {}",
                        entry.event, target_agent
                    ))
                })?;
            entry.event = translated_event;

            let config_path = target_adapter.hook_config_path();
            deployer::deploy_hook(&config_path, &entry, target_adapter.hook_format())?;

            // Codex requires hooks feature enabled in config.toml
            if target_adapter.name() == "codex"
                && let Err(e) = deployer::ensure_codex_hooks_enabled(&target_adapter.base_dir())
            {
                eprintln!("[hk] warning: {e}");
            }

            Ok(format!("{}:{}", entry.event, entry.command))
        }
        ExtensionKind::Cli => {
            // Deploy the CLI's associated skill to the target agent.
            let binary_name = ext
                .cli_meta
                .as_ref()
                .map(|m| m.binary_name.clone())
                .unwrap_or_else(|| ext.name.to_lowercase());
            // CLI source skills are global-only today, but search every scope
            // so a future project-scoped CLI skill can still seed install_to_agent.
            let locations = scanner::skill_locations(&binary_name, adapters, &projects, None);
            let source_path = locations
                .into_iter()
                .next()
                .map(|(_, path)| path)
                .ok_or_else(|| {
                    HkError::Internal("Could not find source skill files for CLI".into())
                })?;
            let target_dir = target_adapter
                .skill_dirs()
                .into_iter()
                .next()
                .ok_or_else(|| {
                    HkError::Internal(format!("No skill directory for agent '{}'", target_agent))
                })?;
            let deployed_name = deployer::deploy_skill(&source_path, &target_dir)?;
            Ok(deployed_name)
        }
        other => Err(HkError::Internal(format!(
            "Cross-agent deploy not supported for '{}' extensions",
            other.as_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use tempfile::TempDir;

    fn test_store() -> (Store, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        (store, dir)
    }

    fn make_skill(scope: ConfigScope, install_meta: Option<InstallMeta>) -> Extension {
        Extension {
            id: "test-id".into(),
            kind: ExtensionKind::Skill,
            name: "test".into(),
            description: String::new(),
            source: Source {
                origin: SourceOrigin::Git,
                url: None,
                version: None,
                commit_hash: None,
            },
            agents: vec!["claude".into()],
            tags: vec![],
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            scope,
            install_meta,
            pack: None,
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
        }
    }

    fn meta() -> InstallMeta {
        InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://github.com/x/y".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        }
    }

    #[test]
    fn test_is_update_eligible_global_skill() {
        // Global skill, no install_meta — eligible (auto-link via name match).
        assert!(is_update_eligible(&make_skill(ConfigScope::Global, None)));
        // Global skill, has install_meta — eligible.
        assert!(is_update_eligible(&make_skill(
            ConfigScope::Global,
            Some(meta()),
        )));
    }

    #[test]
    fn test_is_update_eligible_project_skill() {
        let proj = ConfigScope::Project {
            name: "demo".into(),
            path: "/p/demo".into(),
        };
        // Project skill, no install_meta — NOT eligible (user-managed).
        assert!(!is_update_eligible(&make_skill(proj.clone(), None)));
        // Project skill, has install_meta — eligible (HK-installed).
        assert!(is_update_eligible(&make_skill(proj, Some(meta()))));
    }

    #[test]
    fn test_is_update_eligible_non_skill_kinds_skipped() {
        let mut mcp = make_skill(ConfigScope::Global, Some(meta()));
        mcp.kind = ExtensionKind::Mcp;
        assert!(!is_update_eligible(&mcp));
    }

    #[test]
    fn test_same_scope() {
        let g = ConfigScope::Global;
        let p1 = ConfigScope::Project {
            name: "a".into(),
            path: "/a".into(),
        };
        let p2 = ConfigScope::Project {
            name: "b".into(),
            path: "/b".into(),
        };
        // Project name is irrelevant — same path is the contract.
        let p1_alias = ConfigScope::Project {
            name: "renamed".into(),
            path: "/a".into(),
        };

        assert!(same_scope(&g, &g));
        assert!(same_scope(&p1, &p1_alias));
        assert!(!same_scope(&g, &p1));
        assert!(!same_scope(&p1, &p2));
    }

    #[test]
    fn paths_equal_textual_match_for_nonexistent_paths() {
        // Falls back to textual eq when canonicalize fails (paths don't exist).
        let p = std::path::Path::new("/nonexistent/path/x");
        assert!(paths_equal(p, p));
    }

    #[test]
    fn paths_equal_different_nonexistent_paths_not_equal() {
        // Different non-existent paths return false (canonicalize fails, no fallback match).
        let a = std::path::Path::new("/nonexistent/a");
        let b = std::path::Path::new("/nonexistent/b");
        assert!(!paths_equal(a, b));
    }

    #[test]
    #[cfg(unix)]
    fn paths_equal_resolves_symlink_to_same_target() {
        // The fix at issue: a symlink dir and its target should compare equal.
        // Mirrors the user-visible bug where Antigravity's skill_dir is a
        // symlink into Claude's skill_dir and cross-vendor propagation missed
        // the connection due to textual-only comparison.
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(paths_equal(&target, &link));
        assert!(paths_equal(&link, &target));
    }

    #[test]
    #[cfg(unix)]
    fn paths_equal_distinct_real_dirs_not_equal() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        assert!(!paths_equal(&a, &b));
    }

    #[test]
    #[cfg(unix)]
    fn canonical_siblings_finds_symlinked_row() {
        // Mirrors the user-visible bug: Antigravity's skill_dir is a symlink
        // pointing into Claude's skill_dir. Deleting Claude's row must
        // cascade to Antigravity's row because the physical file is shared.
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join("claude/skills/code-review");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let claude_md = claude_dir.join("SKILL.md");
        std::fs::write(&claude_md, "shared").unwrap();

        let antigravity_parent = tmp.path().join("gemini/antigravity");
        std::fs::create_dir_all(&antigravity_parent).unwrap();
        let antigravity_skills = antigravity_parent.join("skills");
        std::os::unix::fs::symlink(tmp.path().join("claude/skills"), &antigravity_skills)
            .unwrap();
        let antigravity_md = antigravity_skills.join("code-review/SKILL.md");
        // Symlinked path should resolve to the same inode as claude_md.
        assert_eq!(
            std::fs::canonicalize(&antigravity_md).unwrap(),
            std::fs::canonicalize(&claude_md).unwrap()
        );

        let (store, _dir) = test_store();
        let mut claude_row = make_skill(ConfigScope::Global, Some(meta()));
        claude_row.id = "claude-row".into();
        claude_row.name = "code-review".into();
        claude_row.source_path = Some(claude_md.to_string_lossy().to_string());
        store.insert_extension(&claude_row).unwrap();

        let mut antigrav_row = claude_row.clone();
        antigrav_row.id = "antigrav-row".into();
        antigrav_row.agents = vec!["antigravity".into()];
        antigrav_row.source_path = Some(antigravity_md.to_string_lossy().to_string());
        store.insert_extension(&antigrav_row).unwrap();

        let siblings = find_canonical_skill_siblings(&store, &claude_row).unwrap();
        assert_eq!(siblings, vec!["antigrav-row".to_string()]);
    }

    #[test]
    fn canonical_siblings_skips_unrelated_rows() {
        // Same name + kind but distinct physical paths must NOT cascade.
        let tmp = TempDir::new().unwrap();
        let a_dir = tmp.path().join("a/code-review");
        let b_dir = tmp.path().join("b/code-review");
        std::fs::create_dir_all(&a_dir).unwrap();
        std::fs::create_dir_all(&b_dir).unwrap();
        let a_md = a_dir.join("SKILL.md");
        let b_md = b_dir.join("SKILL.md");
        std::fs::write(&a_md, "a").unwrap();
        std::fs::write(&b_md, "b").unwrap();

        let (store, _dir) = test_store();
        let mut a_row = make_skill(ConfigScope::Global, Some(meta()));
        a_row.id = "a-row".into();
        a_row.name = "code-review".into();
        a_row.source_path = Some(a_md.to_string_lossy().to_string());
        store.insert_extension(&a_row).unwrap();

        let mut b_row = a_row.clone();
        b_row.id = "b-row".into();
        b_row.agents = vec!["codex".into()];
        b_row.source_path = Some(b_md.to_string_lossy().to_string());
        store.insert_extension(&b_row).unwrap();

        let siblings = find_canonical_skill_siblings(&store, &a_row).unwrap();
        assert!(siblings.is_empty());
    }

    #[test]
    fn canonical_siblings_returns_empty_when_no_source_path() {
        let (store, _dir) = test_store();
        let target = make_skill(ConfigScope::Global, None);
        // source_path = None → can't canonicalize → empty.
        let siblings = find_canonical_skill_siblings(&store, &target).unwrap();
        assert!(siblings.is_empty());
    }

    #[test]
    fn test_post_install_sync_empty_agents() {
        let (store, _dir) = test_store();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![];
        let result = post_install_sync(
            &store,
            &adapters,
            &[],
            "test-skill",
            None,
            None,
            &ConfigScope::Global,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    /// Project-scope post_install_sync must scan the project directory, upsert
    /// the project row, and write install_meta to the project-scoped row id —
    /// not the unscoped (global) one.
    #[test]
    fn test_post_install_sync_writes_install_meta_to_project_scoped_row() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let proj_dir = TempDir::new().unwrap();
        let home = dir.path();
        let store = Store::open(&home.join("test.db")).unwrap();

        // Project-scope skill on disk (matches Claude's project_skill_dirs())
        let proj_path = proj_dir.path().to_string_lossy().to_string();
        let skills_dir = proj_dir.path().join(".claude").join("skills").join("foo");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("SKILL.md"), "---\nname: foo\n---\n").unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![Box::new(
            adapter::claude::ClaudeAdapter::with_home(home.to_path_buf()),
        )];

        let target_scope = ConfigScope::Project {
            name: "demo".into(),
            path: proj_path.clone(),
        };
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/foo/bar".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };

        post_install_sync(
            &store,
            &adapters,
            &["claude".into()],
            "foo",
            Some(meta.clone()),
            None,
            &target_scope,
        )
        .unwrap();

        // Assert: install_meta lands on the project-scoped row
        let project_id = scanner::stable_id_with_scope_for("foo", "skill", "claude", &target_scope);
        let ext = store
            .get_extension(&project_id)
            .unwrap()
            .expect("project-scoped row should exist after sync");
        assert_eq!(
            ext.install_meta
                .as_ref()
                .expect("install_meta should be set")
                .url,
            meta.url,
        );

        // And: no global row got bogus meta
        let global_id = scanner::stable_id_for("foo", "skill", "claude");
        let global = store.get_extension(&global_id).unwrap();
        assert!(
            global.is_none() || global.unwrap().install_meta.is_none(),
            "global row should not exist or should not have install_meta",
        );
    }

    #[test]
    fn test_run_full_audit_empty_store() {
        let (store, _dir) = test_store();
        let adapters: Vec<Box<dyn AgentAdapter>> = vec![];
        let result = run_full_audit(&store, &adapters);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_read_plugin_content_reads_js_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.js"), "eval(user_input)").unwrap();
        std::fs::write(tmp.path().join("readme.md"), "# Hello").unwrap(); // should be skipped
        let content = read_plugin_content(&tmp.path().to_string_lossy());
        assert!(content.contains("eval(user_input)"));
        assert!(!content.contains("# Hello"));
    }

    #[test]
    fn test_read_plugin_content_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let content = read_plugin_content(&tmp.path().to_string_lossy());
        assert!(content.is_empty());
    }

    #[test]
    fn test_read_plugin_content_reads_single_file_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin = tmp.path().join("plugin.ts");
        std::fs::write(&plugin, "export const value = 1;").unwrap();
        let content = read_plugin_content(&plugin.to_string_lossy());
        assert!(content.contains("plugin.ts"));
        assert!(content.contains("export const value = 1"));
    }

    /// Cross-agent skill deploy must propagate the source's install_meta to
    /// the new target row. Otherwise dedup later splits a logically-single
    /// marketplace skill across agents that have inconsistent install_meta.
    #[test]
    fn test_install_to_agent_propagates_install_meta() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let store_raw = Store::open(&home.join("test.db")).unwrap();
        let store = Mutex::new(store_raw);

        // Source: a Claude global skill installed from a marketplace.
        std::fs::create_dir_all(home.join(".claude").join("skills").join("foo")).unwrap();
        std::fs::write(
            home.join(".claude")
                .join("skills")
                .join("foo")
                .join("SKILL.md"),
            "---\nname: foo\n---\n",
        )
        .unwrap();

        // Codex must detect (`<home>/.codex/` exists) so scan_adapter picks
        // up the deployed copy.
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::claude::ClaudeAdapter::with_home(
                home.to_path_buf(),
            )),
            Box::new(adapter::codex::CodexAdapter::with_home(home.to_path_buf())),
        ];

        let source_id = scanner::stable_id_for("foo", "skill", "claude");
        let install_meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://github.com/foo/bar/foo".into()),
            url_resolved: Some("https://github.com/foo/bar.git".into()),
            branch: None,
            subpath: Some("foo".into()),
            revision: Some("abc123".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        let source_ext = Extension {
            id: source_id.clone(),
            kind: ExtensionKind::Skill,
            name: "foo".into(),
            description: String::new(),
            source: Source {
                origin: SourceOrigin::Agent,
                url: None,
                version: None,
                commit_hash: None,
            },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source_path: Some(
                home.join(".claude")
                    .join("skills")
                    .join("foo")
                    .join("SKILL.md")
                    .to_string_lossy()
                    .to_string(),
            ),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: Some(install_meta.clone()),
            scope: ConfigScope::Global,
        };
        store.lock().insert_extension(&source_ext).unwrap();

        // Cross-agent deploy: claude/foo → codex.
        install_to_agent(&store, &adapters, &source_id, "codex", None).unwrap();

        // File deployed to codex's canonical skill dir (~/.agents/skills),
        // which is now first in skill_dirs() per Codex's current docs;
        // ~/.codex/skills is kept as a deprecated fallback.
        let target_skill_md = home
            .join(".agents")
            .join("skills")
            .join("foo")
            .join("SKILL.md");
        assert!(
            target_skill_md.exists(),
            "deploy_skill should write target SKILL.md"
        );

        // Target row carries the same install_meta as source — the whole
        // point of this test.
        let target_id = scanner::stable_id_for("foo", "skill", "codex");
        let target = store.lock().get_extension(&target_id).unwrap().unwrap();
        let target_meta = target
            .install_meta
            .expect("target row should have install_meta propagated from source");
        assert_eq!(target_meta.install_type, install_meta.install_type);
        assert_eq!(target_meta.url, install_meta.url);
        assert_eq!(target_meta.url_resolved, install_meta.url_resolved);
        assert_eq!(target_meta.subpath, install_meta.subpath);
        assert_eq!(target_meta.revision, install_meta.revision);
    }

    /// When the source skill has no install_meta (hand-managed), deploying
    /// to another agent must NOT fabricate one — target stays unlinked,
    /// matching the source's provenance.
    #[test]
    fn test_install_to_agent_skips_when_source_has_no_install_meta() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let store_raw = Store::open(&home.join("test.db")).unwrap();
        let store = Mutex::new(store_raw);

        std::fs::create_dir_all(home.join(".claude").join("skills").join("bar")).unwrap();
        std::fs::write(
            home.join(".claude")
                .join("skills")
                .join("bar")
                .join("SKILL.md"),
            "---\nname: bar\n---\n",
        )
        .unwrap();
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::claude::ClaudeAdapter::with_home(
                home.to_path_buf(),
            )),
            Box::new(adapter::codex::CodexAdapter::with_home(home.to_path_buf())),
        ];

        let source_id = scanner::stable_id_for("bar", "skill", "claude");
        let source_ext = Extension {
            id: source_id.clone(),
            kind: ExtensionKind::Skill,
            name: "bar".into(),
            description: String::new(),
            source: Source {
                origin: SourceOrigin::Agent,
                url: None,
                version: None,
                commit_hash: None,
            },
            agents: vec!["claude".into()],
            tags: vec![],
            pack: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source_path: Some(
                home.join(".claude")
                    .join("skills")
                    .join("bar")
                    .join("SKILL.md")
                    .to_string_lossy()
                    .to_string(),
            ),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
            scope: ConfigScope::Global,
        };
        store.lock().insert_extension(&source_ext).unwrap();

        install_to_agent(&store, &adapters, &source_id, "codex", None).unwrap();

        // No install_meta to propagate — target row may not even exist in
        // the DB yet (we only sync target when there's meta to write). The
        // file is on disk in codex's canonical skill dir; that's enough.
        let target_skill_md = home
            .join(".agents")
            .join("skills")
            .join("bar")
            .join("SKILL.md");
        assert!(target_skill_md.exists());

        // If a row happens to be there from a previous flow, it must NOT
        // have install_meta fabricated.
        let target_id = scanner::stable_id_for("bar", "skill", "codex");
        if let Some(row) = store.lock().get_extension(&target_id).unwrap() {
            assert!(
                row.install_meta.is_none(),
                "must not synthesize install_meta when source had none"
            );
        }
    }

    /// Cross-vendor sibling propagation: when install lands in a directory
    /// that multiple detected adapters scan (e.g. ~/.agents/skills/ shared by
    /// Codex / Gemini CLI / Cursor / Copilot / OpenCode at user-global
    /// scope), every sibling's row must also receive install_meta. Without
    /// this, the Marketplace's URL-based "installed?" check matches only
    /// the explicit target and falsely shows the siblings as not installed.
    #[test]
    fn test_install_to_agent_propagates_install_meta_to_cross_vendor_siblings() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let store_raw = Store::open(&home.join("test.db")).unwrap();
        let store = Mutex::new(store_raw);

        // Source: Claude global skill (its skill_dirs() does not include
        // ~/.agents/skills/, so Claude is intentionally NOT a sibling).
        std::fs::create_dir_all(home.join(".claude").join("skills").join("baz")).unwrap();
        std::fs::write(
            home.join(".claude")
                .join("skills")
                .join("baz")
                .join("SKILL.md"),
            "---\nname: baz\n---\n",
        )
        .unwrap();

        // Detect Codex (target) AND Gemini (sibling — also scans
        // ~/.agents/skills/). Cursor/Copilot/OpenCode would also qualify but
        // two siblings is enough to prove the loop logic.
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::create_dir_all(home.join(".gemini")).unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::claude::ClaudeAdapter::with_home(
                home.to_path_buf(),
            )),
            Box::new(adapter::codex::CodexAdapter::with_home(home.to_path_buf())),
            Box::new(adapter::gemini::GeminiAdapter::with_home(
                home.to_path_buf(),
            )),
        ];

        let source_id = scanner::stable_id_for("baz", "skill", "claude");
        let install_meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://github.com/foo/bar/baz".into()),
            url_resolved: Some("https://github.com/foo/bar.git".into()),
            branch: None,
            subpath: Some("baz".into()),
            revision: Some("def456".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        store
            .lock()
            .insert_extension(&Extension {
                id: source_id.clone(),
                kind: ExtensionKind::Skill,
                name: "baz".into(),
                description: String::new(),
                source: Source {
                    origin: SourceOrigin::Agent,
                    url: None,
                    version: None,
                    commit_hash: None,
                },
                agents: vec!["claude".into()],
                tags: vec![],
                pack: None,
                permissions: vec![],
                enabled: true,
                trust_score: None,
                installed_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                source_path: Some(
                    home.join(".claude")
                        .join("skills")
                        .join("baz")
                        .join("SKILL.md")
                        .to_string_lossy()
                        .to_string(),
                ),
                cli_parent_id: None,
                cli_meta: None,
                install_meta: Some(install_meta.clone()),
                scope: ConfigScope::Global,
            })
            .unwrap();

        install_to_agent(&store, &adapters, &source_id, "codex", None).unwrap();

        let target_id = scanner::stable_id_for("baz", "skill", "codex");
        let sibling_id = scanner::stable_id_for("baz", "skill", "gemini");

        let codex_row = store.lock().get_extension(&target_id).unwrap().unwrap();
        let gemini_row = store.lock().get_extension(&sibling_id).unwrap().unwrap();

        let codex_meta = codex_row
            .install_meta
            .expect("codex row should carry install_meta");
        let gemini_meta = gemini_row
            .install_meta
            .expect("gemini sibling must also carry propagated install_meta");
        assert_eq!(codex_meta.url, install_meta.url);
        assert_eq!(gemini_meta.url, install_meta.url);
        assert_eq!(gemini_meta.revision, install_meta.revision);
    }

    /// Same propagation guarantee, but exercised through `post_install_sync`
    /// directly (the path Marketplace install actually runs through). The
    /// previous test covers `install_to_agent` (cross-agent deploy button);
    /// this one covers the marketplace install flow that drove the bug
    /// the user reported during manual testing.
    #[test]
    fn test_post_install_sync_propagates_install_meta_to_cross_vendor_siblings() {
        use crate::adapter;

        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let store_raw = Store::open(&home.join("test.db")).unwrap();
        let store = Mutex::new(store_raw);

        // Marketplace flow already wrote the file; we just simulate the
        // post-deploy bookkeeping. Drop a SKILL.md into the shared
        // ~/.agents/skills/qux/ where Codex and Gemini will both find it.
        std::fs::create_dir_all(home.join(".agents").join("skills").join("qux")).unwrap();
        std::fs::write(
            home.join(".agents")
                .join("skills")
                .join("qux")
                .join("SKILL.md"),
            "---\nname: qux\n---\n",
        )
        .unwrap();

        // Detect Codex (target) + Gemini (sibling — also scans
        // ~/.agents/skills/). Claude is intentionally not registered as an
        // adapter here — its presence wouldn't affect the test, but keeping
        // the adapter list small clarifies intent.
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::create_dir_all(home.join(".gemini")).unwrap();

        let adapters: Vec<Box<dyn adapter::AgentAdapter>> = vec![
            Box::new(adapter::codex::CodexAdapter::with_home(home.to_path_buf())),
            Box::new(adapter::gemini::GeminiAdapter::with_home(home.to_path_buf())),
        ];

        let install_meta = InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://github.com/foo/bar/qux".into()),
            url_resolved: Some("https://github.com/foo/bar.git".into()),
            branch: None,
            subpath: Some("qux".into()),
            revision: Some("789xyz".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };

        {
            let store_guard = store.lock();
            post_install_sync(
                &store_guard,
                &adapters,
                &["codex".to_string()],
                "qux",
                Some(install_meta.clone()),
                None,
                &ConfigScope::Global,
            )
            .unwrap();
        }

        let codex_id = scanner::stable_id_for("qux", "skill", "codex");
        let gemini_id = scanner::stable_id_for("qux", "skill", "gemini");
        let codex_meta = store
            .lock()
            .get_extension(&codex_id)
            .unwrap()
            .unwrap()
            .install_meta
            .expect("codex row should carry install_meta from explicit target");
        let gemini_meta = store
            .lock()
            .get_extension(&gemini_id)
            .unwrap()
            .unwrap()
            .install_meta
            .expect("gemini sibling row must carry propagated install_meta");
        assert_eq!(codex_meta.url, install_meta.url);
        assert_eq!(gemini_meta.url, install_meta.url);
        assert_eq!(gemini_meta.revision, install_meta.revision);
    }

    // --- bind_pack / migrate_pack_to_manual_meta ----------------------------

    fn insert_skill_with_pack(
        store: &Store,
        id: &str,
        pack: Option<&str>,
        install_meta: Option<InstallMeta>,
    ) -> Extension {
        let mut ext = make_skill(ConfigScope::Global, install_meta);
        ext.id = id.into();
        ext.name = id.into();
        store.insert_extension(&ext).unwrap();
        if let Some(p) = pack {
            store.update_pack(id, Some(p)).unwrap();
        }
        ext
    }

    #[test]
    fn normalize_pack_preserves_canonical_form() {
        assert_eq!(normalize_pack("anthropics/skills"), "anthropics/skills");
        assert_eq!(normalize_pack("  baoyu/foo  "), "baoyu/foo");
    }

    #[test]
    fn normalize_pack_strips_github_url_scheme_and_suffix() {
        assert_eq!(
            normalize_pack("https://github.com/anthropics/skills"),
            "anthropics/skills"
        );
        assert_eq!(
            normalize_pack("http://github.com/anthropics/skills.git"),
            "anthropics/skills"
        );
        assert_eq!(
            normalize_pack("github.com/anthropics/skills/"),
            "anthropics/skills"
        );
    }

    #[test]
    fn normalize_pack_handles_url_with_trailing_path() {
        // Browser address bar URLs often carry /tree/main, /blob/..., etc.
        assert_eq!(
            normalize_pack("https://github.com/anthropics/skills/tree/main"),
            "anthropics/skills"
        );
        assert_eq!(
            normalize_pack("https://github.com/anthropics/skills/issues/42"),
            "anthropics/skills"
        );
    }

    #[test]
    fn normalize_pack_handles_ssh_clone_url() {
        assert_eq!(
            normalize_pack("git@github.com:anthropics/skills.git"),
            "anthropics/skills"
        );
        assert_eq!(
            normalize_pack("git@github.com:anthropics/skills"),
            "anthropics/skills"
        );
    }

    #[test]
    fn normalize_pack_returns_input_when_no_pattern_matches() {
        // Validator will reject these — normalize just passes them through
        // (trimmed) so the validator's error path stays the source of truth.
        assert_eq!(normalize_pack("not-a-pack"), "not-a-pack");
        assert_eq!(normalize_pack("https://example.com/foo/bar"), "https://example.com/foo/bar");
    }

    #[test]
    fn bind_pack_accepts_url_input() {
        let (store, _dir) = test_store();
        insert_skill_with_pack(&store, "row1", None, None);
        bind_pack(
            &store,
            &["row1".into()],
            Some("https://github.com/anthropics/skills.git"),
        )
        .unwrap();

        let row = store.get_extension("row1").unwrap().unwrap();
        // pack column gets the canonical form, not the raw URL.
        assert_eq!(row.pack.as_deref(), Some("anthropics/skills"));
        let meta = row.install_meta.expect("URL input must synthesize meta");
        assert_eq!(meta.install_type, INSTALL_TYPE_MANUAL);
        assert_eq!(
            meta.url.as_deref(),
            Some("https://github.com/anthropics/skills.git")
        );
    }

    #[test]
    fn is_valid_pack_format_accepts_owner_repo() {
        assert!(is_valid_pack_format("anthropics/skills"));
        assert!(is_valid_pack_format("user-name/repo.name"));
        assert!(is_valid_pack_format("a_b/c.d-e"));
    }

    #[test]
    fn is_valid_pack_format_rejects_malformed() {
        assert!(!is_valid_pack_format(""));
        assert!(!is_valid_pack_format("noslash"));
        assert!(!is_valid_pack_format("a/b/c"));
        assert!(!is_valid_pack_format("/repo"));
        assert!(!is_valid_pack_format("owner/"));
        assert!(!is_valid_pack_format("https://github.com/a/b"));
        assert!(!is_valid_pack_format("a b/c"));
    }

    #[test]
    fn is_valid_pack_format_rejects_host_shaped_owner() {
        // Owner with a '.' looks like a hostname (gitlab.com/foo). Accepting
        // it would let bind_pack synthesize a wrong github URL, so reject.
        assert!(!is_valid_pack_format("gitlab.com/foo"));
        assert!(!is_valid_pack_format("google.com/foo"));
        // Repo half still allows '.' (legitimate GitHub repo name pattern).
        assert!(is_valid_pack_format("user/repo.name"));
    }

    #[test]
    fn bind_pack_synthesizes_meta_when_none() {
        let (store, _dir) = test_store();
        insert_skill_with_pack(&store, "row1", None, None);
        bind_pack(&store, &["row1".into()], Some("baoyu/skills")).unwrap();

        let row = store.get_extension("row1").unwrap().unwrap();
        let meta = row.install_meta.expect("manual meta should be synthesized");
        assert_eq!(meta.install_type, INSTALL_TYPE_MANUAL);
        assert_eq!(
            meta.url.as_deref(),
            Some("https://github.com/baoyu/skills.git")
        );
        assert!(meta.revision.is_none());
        assert_eq!(row.pack.as_deref(), Some("baoyu/skills"));
    }

    #[test]
    fn bind_pack_refreshes_manual_meta_url() {
        let (store, _dir) = test_store();
        insert_skill_with_pack(&store, "row1", Some("old/repo"), None);
        bind_pack(&store, &["row1".into()], Some("old/repo")).unwrap();
        // Now rename to new/repo — meta url should follow.
        bind_pack(&store, &["row1".into()], Some("new/repo")).unwrap();

        let meta = store
            .get_extension("row1")
            .unwrap()
            .unwrap()
            .install_meta
            .unwrap();
        assert_eq!(meta.install_type, INSTALL_TYPE_MANUAL);
        assert_eq!(
            meta.url.as_deref(),
            Some("https://github.com/new/repo.git")
        );
    }

    #[test]
    fn bind_pack_clears_manual_meta_on_empty_pack() {
        let (store, _dir) = test_store();
        insert_skill_with_pack(&store, "row1", None, None);
        bind_pack(&store, &["row1".into()], Some("foo/bar")).unwrap();
        assert!(
            store
                .get_extension("row1")
                .unwrap()
                .unwrap()
                .install_meta
                .is_some()
        );
        bind_pack(&store, &["row1".into()], None).unwrap();

        let row = store.get_extension("row1").unwrap().unwrap();
        assert!(row.install_meta.is_none());
        assert!(row.pack.is_none());
    }

    #[test]
    fn bind_pack_preserves_real_install_meta() {
        let (store, _dir) = test_store();
        let real_meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/actual/source.git".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("abc1234".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        insert_skill_with_pack(&store, "row1", None, Some(real_meta.clone()));
        bind_pack(&store, &["row1".into()], Some("user/typed")).unwrap();

        let row = store.get_extension("row1").unwrap().unwrap();
        let meta = row.install_meta.expect("real meta must remain");
        assert_eq!(meta.install_type, "git");
        assert_eq!(meta.url, real_meta.url);
        assert_eq!(meta.revision, real_meta.revision);
        // Pack column still updates so the user's input is honored.
        assert_eq!(row.pack.as_deref(), Some("user/typed"));
    }

    #[test]
    fn bind_pack_skips_synthesis_for_invalid_pack() {
        let (store, _dir) = test_store();
        insert_skill_with_pack(&store, "row1", None, None);
        bind_pack(&store, &["row1".into()], Some("not-a-pack")).unwrap();

        let row = store.get_extension("row1").unwrap().unwrap();
        assert!(row.install_meta.is_none(), "no meta for invalid pack");
        assert_eq!(
            row.pack.as_deref(),
            Some("not-a-pack"),
            "pack column still records what user typed"
        );
    }

    #[test]
    fn bind_pack_skips_non_skill_kinds() {
        let (store, _dir) = test_store();
        let mut mcp = make_skill(ConfigScope::Global, None);
        mcp.id = "mcp-row".into();
        mcp.name = "mcp-row".into();
        mcp.kind = ExtensionKind::Mcp;
        store.insert_extension(&mcp).unwrap();

        bind_pack(&store, &["mcp-row".into()], Some("foo/bar")).unwrap();

        let row = store.get_extension("mcp-row").unwrap().unwrap();
        assert!(
            row.install_meta.is_none(),
            "MCP / hook / plugin / cli rows must not get a manual install_meta",
        );
        // Pack column update is still honored for non-skill rows.
        assert_eq!(row.pack.as_deref(), Some("foo/bar"));
    }

    #[test]
    fn bind_pack_batches_across_group_instances() {
        let (store, _dir) = test_store();
        insert_skill_with_pack(&store, "a", None, None);
        insert_skill_with_pack(&store, "b", None, None);
        insert_skill_with_pack(&store, "c", None, None);
        bind_pack(
            &store,
            &["a".into(), "b".into(), "c".into()],
            Some("group/key"),
        )
        .unwrap();

        for id in ["a", "b", "c"] {
            let meta = store
                .get_extension(id)
                .unwrap()
                .unwrap()
                .install_meta
                .unwrap_or_else(|| panic!("row {id} missing meta"));
            assert_eq!(meta.install_type, INSTALL_TYPE_MANUAL);
            assert_eq!(
                meta.url.as_deref(),
                Some("https://github.com/group/key.git")
            );
        }
    }

    #[test]
    fn migrate_pack_to_manual_meta_fills_legacy_rows() {
        let (store, _dir) = test_store();
        // Legacy row: pack present, no install_meta — should get migrated.
        insert_skill_with_pack(&store, "legacy", Some("baoyu/skills"), None);
        // Already-bound row: pack + manual meta — should be left alone (no double-write).
        insert_skill_with_pack(&store, "bound", None, None);
        bind_pack(&store, &["bound".into()], Some("foo/bar")).unwrap();
        // Real install row: real meta present — must not be clobbered.
        let real_meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://example.com/real.git".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("real-rev".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        insert_skill_with_pack(&store, "real", Some("any/pack"), Some(real_meta.clone()));
        // Invalid pack: scanner-mangled or user-typed garbage — should be skipped.
        insert_skill_with_pack(&store, "invalid", Some("not-a-pack"), None);

        let migrated = migrate_pack_to_manual_meta(&store).unwrap();
        assert_eq!(migrated, 1, "only the legacy row should migrate");

        let legacy = store
            .get_extension("legacy")
            .unwrap()
            .unwrap()
            .install_meta
            .unwrap();
        assert_eq!(legacy.install_type, INSTALL_TYPE_MANUAL);
        assert_eq!(
            legacy.url.as_deref(),
            Some("https://github.com/baoyu/skills.git")
        );

        let real = store
            .get_extension("real")
            .unwrap()
            .unwrap()
            .install_meta
            .unwrap();
        assert_eq!(real.url, real_meta.url, "real install meta untouched");

        assert!(
            store
                .get_extension("invalid")
                .unwrap()
                .unwrap()
                .install_meta
                .is_none()
        );
    }
}
