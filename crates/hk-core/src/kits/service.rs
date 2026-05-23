use crate::adapter::{AgentAdapter, McpFormat, McpServerEntry};
use crate::kits::agent_overlap::shared_skill_dir_agents;
use crate::kits::manifest::{
    sha256_of_bytes, sha256_of_dir, KitManifest, ManifestConfigFile,
    ManifestExtension, KIT_FORMAT_VERSION,
};
use crate::kits::install_plan;
use crate::kits::paths::ensure_kits_dir;
use crate::kits::source_resolver::{
    find_extension_source_by_id, strip_secrets_from_env, ExtensionLocation,
};
use crate::kits::types::{
    ConflictReason, CreateKitRequest, KindCounts, KitAssetCandidates, KitConfigConflict,
    KitConfigFileRef, KitConflictPreview, KitDetails, KitExtensionConflict, KitExtensionRef,
    KitSummary, KitSyncResult, KitSyncTarget, PreviewKitConflictsRequest,
    SyncKitRequest, UnsyncKitRequest, UpdateKitRequest,
};
use crate::kits::zip_io::{extract_entry_to_path, extract_prefix_to_dir, pack_kit, read_manifest_from_zip, PackEntry};
use crate::models::{AgentConfigFile, ConfigCategory, ExtensionKind};
use crate::store::{KitAssetRow, KitConfigFileRow, KitRow, Store, SyncRecordRow};
use crate::HkError;
use chrono::Utc;
use parking_lot::Mutex;
use uuid::Uuid;

const EXPORTED_FROM_STRING: &str = concat!("HarnessKit ", env!("CARGO_PKG_VERSION"));

const HOOK_V1_NOT_KITABLE: &str =
    "Hook extensions are not Kit-able (sync deferred)";

/// List all Kits as summaries (counts + corrupt flag).
pub fn list_kits(store: &Mutex<Store>) -> Result<Vec<KitSummary>, HkError> {
    let store = store.lock();
    let rows = store.list_kit_rows()?;
    // extension_id -> kind lookup avoids N+1 fetch per asset.
    let all_exts = store.list_extensions(None, None)?;
    let kind_by_ext_id: std::collections::HashMap<String, ExtensionKind> = all_exts
        .into_iter()
        .map(|e| (e.id, e.kind))
        .collect();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let assets = store.list_kit_assets(&row.id)?;
        let cfgs = store.list_kit_config_files(&row.id)?;
        let syncs = store.list_sync_records_for_kit(&row.id)?;
        let mut kind_counts = KindCounts::default();
        for a in &assets {
            match kind_by_ext_id.get(&a.extension_id).copied() {
                Some(ExtensionKind::Skill) => kind_counts.skill += 1,
                Some(ExtensionKind::Mcp) => kind_counts.mcp += 1,
                Some(ExtensionKind::Plugin) => kind_counts.plugin += 1,
                Some(ExtensionKind::Hook) => kind_counts.hook += 1,
                Some(ExtensionKind::Cli) => kind_counts.cli += 1,
                // Unresolvable extension: skip kind tally; extension_count still reflects total.
                None => {}
            }
        }
        out.push(KitSummary {
            id: row.id.clone(),
            name: row.name,
            description: row.description,
            extension_count: assets.len(),
            config_file_count: cfgs.len(),
            sync_count: syncs.len(),
            kind_counts,
            created_at: row.created_at,
            updated_at: row.updated_at,
            corrupt: !std::path::Path::new(&row.zip_path).exists(),
        });
    }
    Ok(out)
}

pub fn create_kit(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    req: CreateKitRequest,
) -> Result<KitSummary, HkError> {
    validate_kit_input(&req.name)?;
    check_kit_name_available(store, &req.name, None)?;
    let kit_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    pack_and_persist_kit(
        store,
        adapters,
        &kit_id,
        &req.name,
        &req.description,
        &req.extension_ids,
        &req.config_files,
        now,
        now,
        true,
    )?;
    list_kits(store)?
        .into_iter()
        .find(|k| k.id == kit_id)
        .ok_or_else(|| HkError::Internal("created Kit not found in list".into()))
}

pub fn update_kit(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    req: UpdateKitRequest,
) -> Result<KitSummary, HkError> {
    validate_kit_input(&req.name)?;
    check_kit_name_available(store, &req.name, Some(&req.id))?;
    let original = {
        let store = store.lock();
        store.get_kit_row(&req.id)?.ok_or_else(|| HkError::NotFound("Kit not found".into()))?
    };
    let now = Utc::now();
    pack_and_persist_kit(
        store,
        adapters,
        &req.id,
        &req.name,
        &req.description,
        &req.extension_ids,
        &req.config_files,
        now,
        original.created_at,
        false,
    )?;
    list_kits(store)?
        .into_iter()
        .find(|k| k.id == req.id)
        .ok_or_else(|| HkError::Internal("updated Kit not found in list".into()))
}

fn validate_kit_input(name: &str) -> Result<(), HkError> {
    if name.trim().is_empty() {
        return Err(HkError::Validation("Kit name must not be empty".into()));
    }
    Ok(())
}

/// Surface a taggable `kit-name-exists:` error (case-insensitive, NOCASE index).
/// `self_id` excludes the row being renamed during `update_kit`.
fn check_kit_name_available(
    store: &Mutex<Store>,
    name: &str,
    self_id: Option<&str>,
) -> Result<(), HkError> {
    let trimmed = name.trim();
    let st = store.lock();
    let exists = st
        .list_kit_rows()?
        .into_iter()
        .any(|r| r.name.eq_ignore_ascii_case(trimmed) && Some(r.id.as_str()) != self_id);
    drop(st);
    if exists {
        return Err(HkError::Conflict(format!("kit-name-exists:{trimmed}")));
    }
    Ok(())
}

/// Resolve sources, build manifest, write zip, upsert DB rows. Shared by create/update.
#[allow(clippy::too_many_arguments)]
fn pack_and_persist_kit(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    kit_id: &str,
    name: &str,
    description: &str,
    extension_ids: &[String],
    config_files: &[KitConfigFileRef],
    updated_at: chrono::DateTime<chrono::Utc>,
    created_at: chrono::DateTime<chrono::Utc>,
    is_create: bool,
) -> Result<(), HkError> {
    let zip_dir = ensure_kits_dir()?;
    let zip_path = zip_dir.join(format!("{kit_id}.hk-kit.zip"));

    let (extensions_meta, ext_entries, secrets_stripped) =
        resolve_and_pack_extensions(store, adapters, extension_ids)?;
    // Multiple sources sharing (agent, category) are merged at install time, not at pack time.
    let (config_meta, cfg_entries) = resolve_and_pack_configs(config_files)?;

    let manifest = KitManifest {
        kit_format_version: KIT_FORMAT_VERSION,
        kit_id: kit_id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        created_at,
        exported_from: EXPORTED_FROM_STRING.to_string(),
        extensions: extensions_meta.clone(),
        config_files: config_meta.clone(),
        secrets_stripped,
    };
    let entries: Vec<PackEntry> = ext_entries.into_iter().chain(cfg_entries).collect();

    pack_kit(&zip_path, &manifest, &entries)?;

    let store = store.lock();
    if is_create {
        store.insert_kit(&KitRow {
            id: kit_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            zip_path: zip_path.to_string_lossy().into(),
            created_at,
            updated_at,
        })?;
    } else {
        store.update_kit_meta(kit_id, name, description, updated_at)?;
    }
    let asset_rows: Vec<KitAssetRow> = extensions_meta
        .iter()
        .enumerate()
        .map(|(i, e)| KitAssetRow {
            kit_id: kit_id.to_string(),
            extension_id: e.source_extension_id.clone(),
            asset_name: e.name.clone(),
            position: i as i64,
        })
        .collect();
    store.replace_kit_assets(kit_id, &asset_rows)?;

    let cfg_rows: Vec<KitConfigFileRow> = config_files
        .iter()
        .enumerate()
        .map(|(i, c)| KitConfigFileRow {
            kit_id: kit_id.to_string(),
            agent: c.agent.clone(),
            category: c.category,
            source_path: c.source_path.clone().unwrap_or_default(),
            source_file_name: c.source_file_name.clone(),
            position: i as i64,
        })
        .collect();
    store.replace_kit_config_files(kit_id, &cfg_rows)?;
    Ok(())
}

fn resolve_and_pack_extensions(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    extension_ids: &[String],
) -> Result<(Vec<ManifestExtension>, Vec<PackEntry>, Vec<String>), HkError> {
    let mut meta = Vec::with_capacity(extension_ids.len());
    let mut entries = Vec::new();
    let mut secrets = Vec::new();

    let (extensions, projects) = {
        let store = store.lock();
        let exts = store.list_extensions(None, None)?;
        let projects = store.list_project_tuples();
        (exts, projects)
    };

    for (position, id) in extension_ids.iter().enumerate() {
        let ext = extensions
            .iter()
            .find(|e| &e.id == id)
            .cloned()
            .ok_or_else(|| HkError::NotFound(format!("Extension '{id}' not found in store")))?;
        let location =
            find_extension_source_by_id(adapters, id, ext.kind, &ext.agents, &ext.scope, &projects)
                .ok_or_else(|| HkError::NotFound(format!(
                    "{kind} '{name}' (id {id}) source could not be located on disk",
                    kind = match ext.kind {
                        ExtensionKind::Skill => "Skill",
                        ExtensionKind::Mcp => "MCP server",
                        ExtensionKind::Hook => "Hook",
                        ExtensionKind::Cli => "CLI",
                        ExtensionKind::Plugin => "Plugin",
                    },
                    name = ext.name,
                )))?;
        let asset_prefix = format!("assets/ext-{id}/");
        let (asset_path, content_hash, bytes_entries, secret_stripped) =
            embed_extension(&location, ext.kind, &asset_prefix, &ext.name, adapters)?;
        if secret_stripped {
            secrets.push(id.clone());
        }
        for be in bytes_entries {
            entries.push(be);
        }
        meta.push(ManifestExtension {
            name: ext.name,
            kind: ext.kind,
            source_extension_id: id.clone(),
            source_url: ext.source.url,
            content_hash,
            asset_path,
            position: position as i64,
        });
    }
    Ok((meta, entries, secrets))
}

fn embed_extension(
    loc: &ExtensionLocation,
    kind: ExtensionKind,
    asset_prefix: &str,
    ext_name: &str,
    adapters: &[Box<dyn AgentAdapter>],
) -> Result<(String, String, Vec<PackEntry>, bool), HkError> {
    match kind {
        ExtensionKind::Skill | ExtensionKind::Cli => {
            let base = &loc.entry_path;
            // Standalone `<name>.md` skills are promoted to `<name>/SKILL.md` folder
            // layout so the install side produces a discoverable folder skill.
            if base.is_file() && matches!(kind, ExtensionKind::Skill) {
                let bytes = std::fs::read(base).map_err(|e| {
                    HkError::Internal(format!(
                        "pack standalone skill {}: {e}",
                        base.display()
                    ))
                })?;
                let zip_path = format!("{asset_prefix}SKILL.md");
                let hash = sha256_of_bytes(&bytes);
                return Ok((
                    asset_prefix.to_string(),
                    hash,
                    vec![PackEntry { zip_path, bytes }],
                    false,
                ));
            }
            // Folder: walk recursively, pack each file at <prefix><relpath>.
            let mut entries = Vec::new();
            let prefix = asset_prefix.to_string();
            crate::kits::util::walk_dir(base, &mut |path, rel| {
                let zip_path = format!("{prefix}{rel}");
                let bytes = std::fs::read(path)?;
                entries.push(PackEntry { zip_path, bytes });
                Ok(())
            })
            .map_err(|e| HkError::Internal(format!("pack skill dir {}: {e}", base.display())))?;
            let hash = sha256_of_dir(base)
                .map_err(|e| HkError::Internal(format!("hash skill dir: {e}")))?;
            Ok((asset_prefix.to_string(), hash, entries, false))
        }
        ExtensionKind::Hook => Err(HkError::Validation(HOOK_V1_NOT_KITABLE.into())),
        ExtensionKind::Mcp => {
            // Read via the source adapter's format-aware parser; normalize to the
            // canonical `{command, args, env}` blob so install-side stays format-agnostic.
            let agent_name = loc.agent.as_deref().ok_or_else(|| {
                HkError::Internal("MCP source location missing adapter binding".into())
            })?;
            let adapter = adapter_for_agent(adapters, agent_name).ok_or_else(|| {
                HkError::NotFound(format!("Adapter '{agent_name}' not registered"))
            })?;
            let mut entry = adapter
                .read_mcp_servers_from(&loc.entry_path)
                .into_iter()
                .find(|e| e.name == ext_name)
                .ok_or_else(|| {
                    HkError::NotFound(format!(
                        "MCP server '{ext_name}' not found in {}",
                        loc.entry_path.display()
                    ))
                })?;
            // URL/SSE-based MCP servers leave `command` empty; only stdio is supported.
            if entry.command.is_empty() {
                return Err(HkError::Validation(format!(
                    "MCP server '{ext_name}' has no 'command' (URL/SSE-based?). \
                     Only stdio servers can be embedded in Kits."
                )));
            }
            let stripped = strip_secrets_from_env(&mut entry.env);
            let bytes = serde_json::to_vec_pretty(&entry)
                .map_err(|e| HkError::Internal(format!("serialize mcp entry: {e}")))?;
            let asset_path = format!("{asset_prefix}mcp.json");
            let hash = sha256_of_bytes(&bytes);
            Ok((asset_path.clone(), hash, vec![PackEntry { zip_path: asset_path, bytes }], stripped))
        }
        ExtensionKind::Plugin => Err(HkError::Internal("Plugin kind is not Kit-able".into())),
    }
}

fn resolve_and_pack_configs(
    config_files: &[KitConfigFileRef],
) -> Result<(Vec<ManifestConfigFile>, Vec<PackEntry>), HkError> {
    let mut meta = Vec::with_capacity(config_files.len());
    let mut entries = Vec::new();
    for (position, c) in config_files.iter().enumerate() {
        if !matches!(c.category, ConfigCategory::Rules | ConfigCategory::Memory) {
            return Err(HkError::Validation(format!(
                "Kit only supports rules/memory; got {:?}",
                c.category
            )));
        }
        let source_path = c.source_path.as_deref().ok_or_else(|| {
            HkError::Validation(format!(
                "Config file '{}' has no source_path (imported config files cannot be repacked)",
                c.source_file_name
            ))
        })?;
        let path = std::path::PathBuf::from(source_path);
        if !path.exists() {
            return Err(HkError::NotFound(format!(
                "Config file source '{source_path}' does not exist"
            )));
        }
        let bytes = std::fs::read(&path)?;
        // Position-prefix avoids zip-namespace collision when multiple sources share
        // (agent, category); install-side merges them into one target file.
        let asset_path = format!(
            "assets/config-{}-{}/{:03}-{}",
            c.agent,
            c.category.as_str(),
            position,
            c.source_file_name
        );
        meta.push(ManifestConfigFile {
            agent: c.agent.clone(),
            category: c.category,
            filename: c.source_file_name.clone(),
            asset_path: asset_path.clone(),
            position: position as i64,
        });
        entries.push(PackEntry {
            zip_path: asset_path,
            bytes,
        });
    }
    Ok((meta, entries))
}

pub fn list_kit_asset_candidates(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
) -> Result<KitAssetCandidates, HkError> {
    let store = store.lock();
    let all_exts = store.list_extensions(None, None)?;
    let project_tuples = store.list_project_tuples();
    let kit_able = all_exts.into_iter().filter(|e| match e.kind {
        // URL-only MCPs surface here; the pack-time error explains why they fail.
        ExtensionKind::Skill | ExtensionKind::Cli | ExtensionKind::Mcp => true,
        ExtensionKind::Hook | ExtensionKind::Plugin => false,
    });

    // Probe each row through the resolver so candidates exclude stale marketplace
    // rows whose on-disk source has disappeared (which would fail at pack time).
    let resolvable: Vec<crate::models::Extension> = kit_able
        .filter(|ext| {
            find_extension_source_by_id(
                adapters,
                &ext.id,
                ext.kind,
                &ext.agents,
                &ext.scope,
                &project_tuples,
            )
            .is_some()
        })
        .collect();

    let mut extensions = dedupe_kit_candidates(resolvable);
    // Stable, case-insensitive name order for the Kit editor.
    extensions.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut config_files: Vec<AgentConfigFile> = Vec::new();
    for adapter in adapters.iter() {
        if !adapter.detect() {
            continue;
        }
        for c in crate::scanner::scan_agent_configs(adapter.as_ref(), &project_tuples) {
            if matches!(c.category, ConfigCategory::Rules | ConfigCategory::Memory) && c.exists {
                config_files.push(c);
            }
        }
    }
    // Stable Files-tab order: category (rules before memory) → agent → file_name.
    config_files.sort_by(|a, b| {
        let rank = |c: &ConfigCategory| match c {
            ConfigCategory::Rules => 0,
            ConfigCategory::Memory => 1,
            _ => 99,
        };
        rank(&a.category)
            .cmp(&rank(&b.category))
            .then_with(|| a.agent.cmp(&b.agent))
            .then_with(|| {
                a.file_name
                    .to_lowercase()
                    .cmp(&b.file_name.to_lowercase())
            })
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(KitAssetCandidates {
        extensions,
        config_files,
    })
}

/// Group extension rows by `(kind, name, source.origin, url-or-scope)` and
/// aggregate sibling agents into the winner's `agents` field. Mirrors the
/// frontend's `extensionGroupKey` policy.
pub(crate) fn dedupe_kit_candidates(
    rows: Vec<crate::models::Extension>,
) -> Vec<crate::models::Extension> {
    let mut groups: std::collections::HashMap<
        (ExtensionKind, String, &'static str, String),
        Vec<crate::models::Extension>,
    > = std::collections::HashMap::new();
    for ext in rows {
        let url_or_scope = ext
            .source
            .url
            .clone()
            .unwrap_or_else(|| ext.scope.scope_key());
        let key = (
            ext.kind,
            ext.name.clone(),
            ext.source.origin.as_str(),
            url_or_scope,
        );
        groups.entry(key).or_default().push(ext);
    }

    groups
        .into_values()
        .map(|mut group| {
            group.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            let mut winner = group.remove(0);
            for other in group {
                for a in other.agents {
                    if !winner.agents.contains(&a) {
                        winner.agents.push(a);
                    }
                }
            }
            winner
        })
        .collect()
}

pub fn get_kit_details(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    id: &str,
) -> Result<KitDetails, HkError> {
    let summary = list_kits(store)?
        .into_iter()
        .find(|k| k.id == id)
        .ok_or_else(|| HkError::NotFound("Kit not found".into()))?;

    let store_guard = store.lock();
    let row = store_guard
        .get_kit_row(id)?
        .ok_or_else(|| HkError::NotFound("Kit not found".into()))?;
    let assets = store_guard.list_kit_assets(id)?;
    let configs = store_guard.list_kit_config_files(id)?;
    let syncs = store_guard.list_sync_records_for_kit(id)?;
    drop(store_guard);

    let manifest = if std::path::Path::new(&row.zip_path).exists() {
        Some(read_manifest_from_zip(std::path::Path::new(&row.zip_path))?)
    } else {
        None
    };

    let mut ext_refs = Vec::with_capacity(assets.len());
    for asset in &assets {
        let manifest_entry = manifest
            .as_ref()
            .and_then(|m| m.extensions.iter().find(|e| e.source_extension_id == asset.extension_id));
        let kind = manifest_entry.map(|m| m.kind).unwrap_or(ExtensionKind::Skill);
        let content_hash = manifest_entry
            .map(|m| m.content_hash.clone())
            .unwrap_or_default();
        let secrets_stripped = manifest
            .as_ref()
            .map(|m| m.secrets_stripped.iter().any(|s| s == &asset.extension_id))
            .unwrap_or(false);
        ext_refs.push(KitExtensionRef {
            extension_id: asset.extension_id.clone(),
            asset_name: asset.asset_name.clone(),
            kind,
            content_hash,
            secrets_stripped,
        });
    }

    let cfg_refs: Vec<KitConfigFileRef> = configs
        .into_iter()
        .map(|c| KitConfigFileRef {
            agent: c.agent,
            category: c.category,
            source_path: if c.source_path.is_empty() {
                None
            } else {
                Some(c.source_path)
            },
            source_file_name: c.source_file_name,
        })
        .collect();

    let sync_targets = syncs
        .into_iter()
        .map(|s| KitSyncTarget {
            shared_with: shared_skill_dir_agents(
                adapters,
                &s.agent_name,
                &s.project_path,
            ),
            project_path: s.project_path,
            agent_name: s.agent_name,
            synced_at: s.synced_at,
            file_count: s.written_paths.len(),
        })
        .collect();

    Ok(KitDetails {
        summary,
        extensions: ext_refs,
        config_files: cfg_refs,
        sync_targets,
    })
}


pub fn preview_kit_project_conflicts(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    req: PreviewKitConflictsRequest,
) -> Result<KitConflictPreview, HkError> {
    let zip_path = {
        let store = store.lock();
        store
            .get_kit_row(&req.kit_id)?
            .ok_or_else(|| HkError::NotFound("Kit not found".into()))?
            .zip_path
    };
    let plan = install_plan::compute_kit_install_plan(
        adapters,
        std::path::Path::new(&zip_path),
        &req.project_path,
        &req.agent_name,
    )?;

    let mut extension_conflicts = Vec::new();
    let mut config_conflicts = Vec::new();
    for item in &plan {
        if !item.conflicts_with_existing {
            continue;
        }
        match &item.asset_kind {
            install_plan::PlanItemKind::Extension { source_extension_id, asset_name, .. } => {
                let reason = if item.target_path.is_dir() {
                    ConflictReason::DirExists
                } else {
                    ConflictReason::FileExists
                };
                extension_conflicts.push(KitExtensionConflict {
                    extension_id: source_extension_id.clone(),
                    asset_name: asset_name.clone(),
                    target_path: item.target_path.to_string_lossy().into(),
                    conflict_reason: reason,
                });
            }
            install_plan::PlanItemKind::Config { agent, category } => {
                config_conflicts.push(KitConfigConflict {
                    agent: agent.clone(),
                    category: *category,
                    target_path: item.target_path.to_string_lossy().into(),
                });
            }
        }
    }
    Ok(KitConflictPreview {
        extension_conflicts,
        config_conflicts,
    })
}

pub fn delete_kit(store: &Mutex<Store>, id: &str) -> Result<(), HkError> {
    // Capture zip path and delete the row under a single lock.
    let zip_path = {
        let store = store.lock();
        let zp = store.get_kit_row(id)?.map(|r| r.zip_path);
        store.delete_kit(id)?;
        zp
    };
    if let Some(p) = zip_path {
        // Best-effort cleanup; missing zip is fine.
        let _ = std::fs::remove_file(&p);
    }
    Ok(())
}

pub fn sync_kit_to_project(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    req: SyncKitRequest,
) -> Result<KitSyncResult, HkError> {
    let zip_path = {
        let store = store.lock();
        store
            .get_kit_row(&req.kit_id)?
            .ok_or_else(|| HkError::NotFound("Kit not found".into()))?
            .zip_path
    };
    let zip_pb = std::path::PathBuf::from(&zip_path);
    let plan = install_plan::compute_kit_install_plan(
        adapters,
        &zip_pb,
        &req.project_path,
        &req.agent_name,
    )?;
    let config_groups = build_config_merge_groups(&plan);

    let mut written_paths = Vec::new();
    let mut skipped_paths = Vec::new();
    let mut installed_count = 0;
    let mut skipped_conflict_count = 0;
    let mut applied_config_targets: std::collections::HashSet<std::path::PathBuf> =
        std::collections::HashSet::new();

    for item in &plan {
        let force = item_is_forced(
            item,
            &req.force_overwrite_extension_ids,
            &req.force_overwrite_config_keys,
        );
        if item.conflicts_with_existing && !force {
            skipped_conflict_count += 1;
            skipped_paths.push(item.target_path.to_string_lossy().into());
            continue;
        }
        if let Some(parent) = item.target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match &item.asset_kind {
            install_plan::PlanItemKind::Extension { .. } => {
                if let Some(record) =
                    apply_extension_item(item, force, adapters, &req.agent_name, &zip_pb)?
                {
                    written_paths.push(record);
                    installed_count += 1;
                }
            }
            install_plan::PlanItemKind::Config { .. } => {
                if applied_config_targets.contains(&item.target_path) {
                    continue;
                }
                let group = config_groups
                    .get(&item.target_path)
                    .expect("config target must be in group map");
                apply_config_item(&item.target_path, group, &zip_pb)?;
                applied_config_targets.insert(item.target_path.clone());
                written_paths.push(item.target_path.to_string_lossy().into());
                installed_count += 1;
            }
        }
    }

    {
        let store = store.lock();
        store.register_project_by_path(&req.project_path);
        store.upsert_sync_record(&SyncRecordRow {
            id: Uuid::new_v4().to_string(),
            kit_id: req.kit_id,
            project_path: req.project_path,
            agent_name: req.agent_name,
            written_paths: written_paths.clone(),
            synced_at: Utc::now(),
        })?;
    }

    Ok(KitSyncResult {
        installed_count,
        skipped_conflict_count,
        skipped_paths,
        written_paths,
    })
}

/// Group Config plan items by target_path so the main loop can look up
/// "every source landing here" in O(1) instead of an inner O(n) filter.
fn build_config_merge_groups(
    plan: &[install_plan::PlanItem],
) -> std::collections::HashMap<std::path::PathBuf, Vec<&install_plan::PlanItem>> {
    let mut groups: std::collections::HashMap<
        std::path::PathBuf,
        Vec<&install_plan::PlanItem>,
    > = std::collections::HashMap::new();
    for item in plan {
        if matches!(item.asset_kind, install_plan::PlanItemKind::Config { .. }) {
            groups
                .entry(item.target_path.clone())
                .or_default()
                .push(item);
        }
    }
    groups
}

/// Write one Extension plan item to disk. Returns the written_paths record
/// entry on success, or None for Plugin (skipped — not Kit-able).
fn apply_extension_item(
    item: &install_plan::PlanItem,
    force: bool,
    adapters: &[Box<dyn AgentAdapter>],
    agent_name: &str,
    zip_pb: &std::path::Path,
) -> Result<Option<String>, HkError> {
    let (kind, asset_name) = match &item.asset_kind {
        install_plan::PlanItemKind::Extension {
            kind, asset_name, ..
        } => (*kind, asset_name.clone()),
        _ => unreachable!(),
    };
    match kind {
        ExtensionKind::Skill | ExtensionKind::Cli => {
            // Force-overwrite wipes the target first to avoid stale-file merges.
            if item.target_path.exists() && force {
                std::fs::remove_dir_all(&item.target_path)?;
            }
            extract_prefix_to_dir(zip_pb, &item.zip_entry_path, &item.target_path)?;
            Ok(Some(item.target_path.to_string_lossy().into()))
        }
        ExtensionKind::Mcp => {
            let entry_bytes = read_zip_entry_bytes(zip_pb, &item.zip_entry_path)?;
            let mut entry_struct: McpServerEntry = serde_json::from_slice(&entry_bytes)
                .map_err(|e| HkError::ConfigCorrupted(format!("mcp entry json: {e}")))?;
            entry_struct.name = asset_name.clone();
            let mcp_format = adapter_for_agent(adapters, agent_name)
                .ok_or_else(|| {
                    HkError::NotFound(format!("Agent '{}' not found", agent_name))
                })?
                .mcp_format();
            crate::deployer::deploy_mcp_server(&item.target_path, &entry_struct, mcp_format)?;
            // "mcp:<config_path>:<server_name>" lets unsync target only this entry.
            Ok(Some(format!(
                "mcp:{}:{}",
                item.target_path.to_string_lossy(),
                asset_name
            )))
        }
        ExtensionKind::Hook => Err(HkError::Validation(HOOK_V1_NOT_KITABLE.into())),
        ExtensionKind::Plugin => Ok(None),
    }
}

/// Write one Config target. `group` is every plan item landing at this path;
/// single → straight extract, multiple → concat with a blank-line separator.
fn apply_config_item(
    target_path: &std::path::Path,
    group: &[&install_plan::PlanItem],
    zip_pb: &std::path::Path,
) -> Result<(), HkError> {
    if group.len() == 1 {
        extract_entry_to_path(zip_pb, &group[0].zip_entry_path, target_path)?;
        return Ok(());
    }
    let mut merged: Vec<u8> = Vec::new();
    for (i, p) in group.iter().enumerate() {
        let bytes = read_zip_entry_bytes(zip_pb, &p.zip_entry_path)?;
        if i > 0 {
            if !merged.ends_with(b"\n") {
                merged.push(b'\n');
            }
            merged.push(b'\n');
        }
        merged.extend_from_slice(&bytes);
    }
    std::fs::write(target_path, &merged)?;
    Ok(())
}

fn item_is_forced(
    item: &install_plan::PlanItem,
    force_ext_ids: &[String],
    force_cfg_keys: &[String],
) -> bool {
    match &item.asset_kind {
        install_plan::PlanItemKind::Extension { source_extension_id, .. } => {
            force_ext_ids.iter().any(|id| id == source_extension_id)
        }
        install_plan::PlanItemKind::Config { agent, category } => {
            let key = format!("{}:{}", agent, category.as_str());
            force_cfg_keys.iter().any(|k| k == &key)
        }
    }
}

pub fn unsync_kit_from_project(
    store: &Mutex<Store>,
    adapters: &[Box<dyn AgentAdapter>],
    req: UnsyncKitRequest,
) -> Result<(), HkError> {
    let record = {
        let store = store.lock();
        store
            .get_sync_record(&req.kit_id, &req.project_path, &req.agent_name)?
            .ok_or_else(|| HkError::NotFound("Sync record not found".into()))?
    };
    // written_paths entries: "mcp:<config_path>:<server_name>" for MCP (merge-aware
    // removal), bare directory for skill/CLI assets, bare file for config files.
    for entry in &record.written_paths {
        if let Some(rest) = entry.strip_prefix("mcp:") {
            // rsplit_once tolerates ':' inside server names (path is everything before).
            if let Some((path_str, name)) = rest.rsplit_once(':') {
                let config_path = std::path::Path::new(path_str);
                let mcp_format = adapter_for_agent(adapters, &record.agent_name)
                    .map(|a| a.mcp_format())
                    .unwrap_or(McpFormat::McpServers);
                let _ = crate::deployer::remove_mcp_server(config_path, name, mcp_format);
            } else {
                eprintln!("[kits] unsync: malformed mcp entry, skipping: {entry}");
            }
        } else {
            let path = std::path::Path::new(entry.as_str());
            if !path.exists() {
                continue;
            }
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let store = store.lock();
    store.delete_sync_record(&req.kit_id, &req.project_path, &req.agent_name)?;
    Ok(())
}

pub fn export_kit(
    store: &Mutex<Store>,
    kit_id: &str,
    target_path: &str,
) -> Result<(), HkError> {
    let row = {
        let store = store.lock();
        store
            .get_kit_row(kit_id)?
            .ok_or_else(|| HkError::NotFound("Kit not found".into()))?
    };
    if !std::path::Path::new(&row.zip_path).exists() {
        return Err(HkError::Internal("Kit zip is missing on disk".into()));
    }
    std::fs::copy(&row.zip_path, target_path)?;
    Ok(())
}

pub fn import_kit(
    store: &Mutex<Store>,
    source_zip_path: &str,
) -> Result<KitSummary, HkError> {
    let src = std::path::Path::new(source_zip_path);
    if !src.exists() {
        return Err(HkError::NotFound("Source zip not found".into()));
    }
    let manifest = read_manifest_from_zip(src)?;
    if manifest.kit_format_version > KIT_FORMAT_VERSION {
        return Err(HkError::Validation(format!(
            "Kit format v{} not supported (max v{KIT_FORMAT_VERSION})",
            manifest.kit_format_version
        )));
    }
    validate_kit_input(&manifest.name)?;

    // Validate every entry path first to catch path traversal.
    {
        let f = std::fs::File::open(src)?;
        let mut a = zip::ZipArchive::new(f).map_err(|e| HkError::Internal(format!("zip: {e}")))?;
        for i in 0..a.len() {
            let entry = a.by_index(i).map_err(|e| HkError::Internal(format!("zip: {e}")))?;
            crate::kits::zip_io::validate_entry_path(entry.name())?;
        }
    }

    let new_id = Uuid::new_v4().to_string();
    let dest = crate::kits::paths::zip_path_for(&new_id)?;
    crate::kits::paths::ensure_kits_dir()?;
    std::fs::copy(src, &dest)?;

    // Delete the copied zip if anything below fails.
    let mut zip_guard = crate::kits::util::RemovePathGuard::new(&dest);

    let now = Utc::now();

    // Fetch + dedupe + insert under one lock to avoid TOCTOU on duplicate names.
    let name = {
        let store = store.lock();
        let existing = store.list_kit_rows()?;
        let name = dedupe_name(
            &manifest.name,
            &existing.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        )?;
        store.insert_kit(&KitRow {
            id: new_id.clone(),
            name: name.clone(),
            description: manifest.description.clone(),
            zip_path: dest.to_string_lossy().into(),
            created_at: now,
            updated_at: now,
        })?;
        let asset_rows: Vec<KitAssetRow> = manifest
            .extensions
            .iter()
            .map(|e| KitAssetRow {
                kit_id: new_id.clone(),
                extension_id: e.source_extension_id.clone(),
                asset_name: e.name.clone(),
                position: e.position,
            })
            .collect();
        store.replace_kit_assets(&new_id, &asset_rows)?;
        let cfg_rows: Vec<KitConfigFileRow> = manifest
            .config_files
            .iter()
            .map(|c| KitConfigFileRow {
                kit_id: new_id.clone(),
                agent: c.agent.clone(),
                category: c.category,
                source_path: String::new(),
                source_file_name: c.filename.clone(),
                position: c.position,
            })
            .collect();
        store.replace_kit_config_files(&new_id, &cfg_rows)?;
        name
    };

    zip_guard.disarm();
    // Manifest is authoritative on import; the local `extensions` table may not be populated yet.
    let mut kind_counts = KindCounts::default();
    for e in &manifest.extensions {
        match e.kind {
            ExtensionKind::Skill => kind_counts.skill += 1,
            ExtensionKind::Mcp => kind_counts.mcp += 1,
            ExtensionKind::Plugin => kind_counts.plugin += 1,
            ExtensionKind::Hook => kind_counts.hook += 1,
            ExtensionKind::Cli => kind_counts.cli += 1,
        }
    }
    let summary = KitSummary {
        id: new_id.clone(),
        name: name.clone(),
        description: manifest.description.clone(),
        extension_count: manifest.extensions.len(),
        config_file_count: manifest.config_files.len(),
        sync_count: 0,
        kind_counts,
        created_at: now,
        updated_at: now,
        corrupt: false,
    };
    Ok(summary)
}

/// Find the adapter for a given agent name.
fn adapter_for_agent<'a>(
    adapters: &'a [Box<dyn AgentAdapter>],
    name: &str,
) -> Option<&'a dyn AgentAdapter> {
    adapters.iter().find(|a| a.name() == name).map(|a| a.as_ref())
}


/// Read all bytes from a single named entry inside a zip file.
fn read_zip_entry_bytes(zip_path: &std::path::Path, entry_name: &str) -> Result<Vec<u8>, HkError> {
    let f = std::fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(f).map_err(|e| HkError::Internal(format!("zip open: {e}")))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| HkError::NotFound(format!("zip entry missing: {entry_name}")))?;
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut buf)?;
    Ok(buf)
}

fn dedupe_name(desired: &str, existing: &[&str]) -> Result<String, HkError> {
    if !existing.contains(&desired) {
        return Ok(desired.to_string());
    }
    for idx in 2..=1000u32 {
        let cand = format!("{desired} ({idx})");
        if !existing.iter().any(|n| *n == cand) {
            return Ok(cand);
        }
    }
    Err(HkError::Validation(format!(
        "Could not find a unique name for imported kit '{desired}' after 1000 attempts"
    )))
}

