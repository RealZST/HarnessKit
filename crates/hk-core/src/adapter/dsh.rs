// DeepSeek Harness (dsh) config references — verified against
// github.com/deepseek-ai/deepseek-harness @ v0.1.0-rc (2026-08-14):
// - Home resolution: packages/util/home-paths (`$DSH_HOME` else `~/.dsh`;
//   the harness keeps all user data under one root).
// - Skills:  docs/subsystems/skills.md — roots (rank order) `<project>/.dsh/skills`,
//   `<project>/.agents/skills`, `<dshHome>/skills` (skips `.system` child),
//   `<agentsHome>/skills` where agentsHome = `$DSH_AGENTS_HOME` else `~/.agents`
//   (packages/skill/skill-filesystem/src/index.ts). Bundle `<name>/SKILL.md` or
//   flat `<name>.md`, one level deep — matches scan_skill_dir exactly, and the
//   `.system` dir is naturally invisible to a one-level scan (its skills nest
//   one level deeper).
// - MCP: packages/mcp/mcp-client — servers are `@deepseek-ai/dsh-mcp-client`
//   plugin rows in cordis patch files. dsh runs one profile at a time
//   (profile patch, then home patch); HK reads the HOME layer only —
//   `<dshHome>/cordis.patch.yml`, the one user layer every profile applies.
//   No project-level MCP config exists.
// - Hooks: packages/hooks — dsh has no own hook format; bridge plugins replay
//   Claude Code / Codex hooks.json. HookFormat::None.
// - Rules: packages/context/agent-instructions — `$DSH_HOME/AGENTS.md` global,
//   project chain reads AGENTS.md / CLAUDE.md + AGENTS.local.md / CLAUDE.local.md.

use super::{
    AgentAdapter, HookEntry, HookFormat, McpFormat, McpServerEntry, McpTransport, ProjectMarker,
};
use std::path::{Path, PathBuf};

pub struct DshAdapter {
    /// `$DSH_HOME` else `~/.dsh`.
    dsh_home: PathBuf,
    /// `$DSH_AGENTS_HOME` else `~/.agents` (cross-vendor shared skills root).
    agents_home: PathBuf,
}

impl Default for DshAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve `(dsh_home, agents_home)` from the `DSH_HOME` / `DSH_AGENTS_HOME`
/// overrides, falling back to `<home>/.dsh` / `<home>/.agents`. Pure so it can
/// be tested with explicit inputs (mutating process env in tests is racy
/// under parallel execution).
fn resolve_homes(
    dsh_home: Option<std::ffi::OsString>,
    agents_home: Option<std::ffi::OsString>,
    home: &Path,
) -> (PathBuf, PathBuf) {
    (
        dsh_home
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".dsh")),
        agents_home
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".agents")),
    )
}

impl DshAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let (dsh_home, agents_home) = resolve_homes(
            std::env::var_os("DSH_HOME"),
            std::env::var_os("DSH_AGENTS_HOME"),
            &home,
        );
        Self {
            dsh_home,
            agents_home,
        }
    }

    /// Test/deployer constructor rooting both homes under `home`; production
    /// uses `new()`.
    pub fn with_home(home: PathBuf) -> Self {
        Self {
            dsh_home: home.join(".dsh"),
            agents_home: home.join(".agents"),
        }
    }

    /// Existing per-profile patch files (settings listing only — MCP reading
    /// is home-layer-only by design; see module header).
    fn profile_patch_files(&self) -> Vec<PathBuf> {
        Self::profile_dirs_in(&self.dsh_home)
            .into_iter()
            .map(|d| d.join("cordis.patch.yml"))
            .filter(|p| p.is_file())
            .collect()
    }

    /// Sorted profile directories under `<dsh_home>/profiles/`. Skips the
    /// `node_modules` entry — that is dsh's in-box-bundle symlink farm
    /// (healed on every launch), not a profile.
    fn profile_dirs_in(dsh_home: &Path) -> Vec<PathBuf> {
        let profiles = dsh_home.join("profiles");
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&profiles)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "node_modules"))
            .collect();
        dirs.sort();
        dirs
    }

    fn profile_dirs(&self) -> Vec<PathBuf> {
        Self::profile_dirs_in(&self.dsh_home)
    }

    /// Patch texts of every profile layer under `dsh_home`, in sorted-dir
    /// order — the layers dsh applies BEFORE the home patch. Associated fn
    /// (not `&self`) so `deployer::set_dsh_plugin_enabled` can call it for
    /// the exact home it is editing.
    pub fn profile_patch_texts(dsh_home: &Path) -> Vec<String> {
        Self::profile_dirs_in(dsh_home)
            .into_iter()
            .filter_map(|d| std::fs::read_to_string(d.join("cordis.patch.yml")).ok())
            .collect()
    }

    /// On-disk directory of an npm package visible to `profile_dir`.
    ///
    /// dsh keeps a maintained symlink farm at
    /// `<dsh_home>/profiles/node_modules/<pkg>` (healed on every launch)
    /// holding the in-box bundles AND their transitive deps — which is where
    /// the packages named by BUNDLE rows live. A profile's own
    /// `node_modules` holds what the user installed into that profile. The
    /// likelier location for this package is tried first and the other used
    /// as a fallback; `None` when it is in neither (fresh install, dsh never
    /// booted) — never an error, just an unknown path.
    fn package_dir(&self, profile_dir: &Path, pkg: &str) -> Option<PathBuf> {
        let farm = self.dsh_home.join("profiles/node_modules").join(pkg);
        let local = profile_dir.join("node_modules").join(pkg);
        let (first, second) = if IN_BOX_BUNDLES.contains(&pkg) {
            (farm, local)
        } else {
            (local, farm)
        };
        if first.is_dir() {
            Some(first)
        } else {
            second.is_dir().then_some(second)
        }
    }

    /// A mounted bundle's OWN patch layer — an ordinary cordis patch file at
    /// the package-relative path its `package.json` declares under
    /// `dsh.bundle.patch` (verified against the installed
    /// `@deepseek-ai/dsh-base` / `dsh-web-app` 0.1.0-rc.6:
    /// `"patch": "./cordis.patch.yml"`). This is the file that actually
    /// carries a bundle's plugin ROWS; the bundle itself is a layer, not a
    /// plugin. `None` when the package is not on disk, declares no patch, or
    /// the declared file is missing.
    fn bundle_patch_path(&self, profile_dir: &Path, bundle: &str) -> Option<PathBuf> {
        let dir = self.package_dir(profile_dir, bundle)?;
        let manifest_text = std::fs::read_to_string(dir.join("package.json")).ok()?;
        let manifest: BundleManifest = serde_json::from_str(&manifest_text).ok()?;
        let rel = manifest.dsh.bundle.patch?;
        let path = dir.join(rel.trim_start_matches("./"));
        path.is_file().then_some(path)
    }
}

/// dsh's `@deepseek-ai/dsh-mcp-client` plugin name — the marker for MCP rows.
pub(crate) const MCP_CLIENT_PLUGIN: &str = "@deepseek-ai/dsh-mcp-client";

/// Suffix of a plugin entry's `source` string for insert rows that carry no
/// `id:`. A shared producer/consumer contract: the adapter renders it and
/// `manager` matches on it.
/// Rendered source strings feed `scanner::stable_id_for`, so this value is
/// part of extension identity and must never change.
pub(crate) const ANON_ROW_SOURCE_SUFFIX: &str = "anonymous row";

/// One entry parsed from a patch file. `from_insert` distinguishes row
/// DEFINITIONS (inside `insert:`) from id-targeted overrides — upstream, an
/// override can never create a row.
struct CordisRow {
    id: Option<String>,
    name: Option<String>,
    /// Literal booleans, plus `null` ≡ `false` (static upstream rule,
    /// vendor/loader/src/config/entry.ts:104-107). Only absent keys and
    /// `!!js` expressions (which arrive as tag-stripped strings) read as
    /// None — upstream evaluates js `disabled` at runtime; HK can't, so it
    /// shows the base state.
    disabled: Option<bool>,
    config: serde_yaml::Value,
    from_insert: bool,
}

/// Parse one patch file into ordered entries. `{id, insert}` (group append)
/// is out of scope and skipped. A parse failure returns empty WITH a stderr
/// diagnostic — silence here would read as "dsh has no MCP".
fn parse_patch_rows(text: &str, origin: &Path) -> Vec<CordisRow> {
    // Absent/empty file is normal, not malformed — callers feed "" for a
    // missing patch file. (dsh's empty-file-must-be-`[]` boot rule applies
    // only to files that EXIST; "" here means the file was absent or empty.)
    if text.trim().is_empty() {
        return vec![];
    }
    let doc: serde_yaml::Value = match serde_yaml::from_str(text) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("[hk] warning: cannot parse {}: {err}", origin.display());
            return vec![];
        }
    };
    let Some(items) = doc.as_sequence() else {
        eprintln!(
            "[hk] warning: {} is not a YAML list (cordis patch files are top-level arrays)",
            origin.display()
        );
        return vec![];
    };
    let mut rows = Vec::new();
    for item in items {
        let Some(map) = item.as_mapping() else { continue };
        let has_id = map.get("id").is_some();
        let insert = map.get("insert").and_then(|v| v.as_sequence());
        match (has_id, insert) {
            (false, Some(inserted)) => {
                for row in inserted {
                    let Some(rm) = row.as_mapping() else { continue };
                    rows.push(CordisRow {
                        id: yaml_str(rm, "id"),
                        name: yaml_str(rm, "name"),
                        disabled: yaml_disabled(rm.get("disabled")),
                        config: rm.get("config").cloned().unwrap_or(serde_yaml::Value::Null),
                        from_insert: true,
                    });
                }
            }
            (true, None) => rows.push(CordisRow {
                id: yaml_str(map, "id"),
                name: yaml_str(map, "name"),
                disabled: yaml_disabled(map.get("disabled")),
                config: map.get("config").cloned().unwrap_or(serde_yaml::Value::Null),
                from_insert: false,
            }),
            // {id, insert} = group append; bare junk = neither. Both skipped.
            _ => {}
        }
    }
    rows
}

fn yaml_str(map: &serde_yaml::Mapping, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn yaml_disabled(v: Option<&serde_yaml::Value>) -> Option<bool> {
    match v {
        Some(serde_yaml::Value::Null) => Some(false), // upstream: null ≡ false
        Some(serde_yaml::Value::Bool(b)) => Some(*b),
        _ => None, // absent, or !!js expression (arrives as String — HK can't evaluate)
    }
}

fn yaml_config_str(config: &serde_yaml::Value, key: &str) -> Option<String> {
    config.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// In-box bundles resolve from dsh's maintained symlink farm at
/// `<dsh_home>/profiles/node_modules/<pkg>` (healed on every dsh launch),
/// NOT from any profile's own node_modules.
const IN_BOX_BUNDLES: [&str; 3] = [
    "@deepseek-ai/dsh-base",
    "@deepseek-ai/dsh-web-app",
    "@deepseek-ai/dsh-headless",
];

/// The plugin-relevant slice of a profile `package.json`:
/// `{ dsh: { profile: { bundles: [...] } } }`
/// (upstream: packages/boot/app-boot/src/profile.ts). `dependencies` is
/// deliberately NOT modeled: a dependency no layer mounts is never loaded by
/// dsh and never shown in its UI, so HK does not list it either.
#[derive(serde::Deserialize, Default)]
struct ProfileManifest {
    #[serde(default)]
    dsh: ProfileDshSection,
}

#[derive(serde::Deserialize, Default)]
struct ProfileDshSection {
    #[serde(default)]
    profile: ProfileSection,
}

#[derive(serde::Deserialize, Default)]
struct ProfileSection {
    #[serde(default)]
    bundles: Vec<String>,
}

/// The patch-relevant slice of a BUNDLE package's `package.json`:
/// `{ dsh: { bundle: { patch: "./cordis.patch.yml" } } }`. Verified against
/// the installed `@deepseek-ai/dsh-base` / `@deepseek-ai/dsh-web-app`
/// 0.1.0-rc.6 — `patch` is a package-relative path string, and its presence
/// is also what makes `dsh plugin add` auto-mount a package as a bundle.
#[derive(serde::Deserialize, Default)]
struct BundleManifest {
    #[serde(default)]
    dsh: BundleDshSection,
}

#[derive(serde::Deserialize, Default)]
struct BundleDshSection {
    #[serde(default)]
    bundle: BundleSection,
}

#[derive(serde::Deserialize, Default)]
struct BundleSection {
    #[serde(default)]
    patch: Option<String>,
}

/// One patch text folded by dsh's apply rule: a single ordered pass in which
/// an `insert:` row DEFINES an entry and a later id-targeted row OVERRIDES it
/// (mirrors upstream applyEntryPatches — an override can never create a row).
struct FoldedText {
    /// Rows DEFINED in this text, in definition order (anonymous ones last),
    /// each carrying the merged effect of every later override in the SAME
    /// text. Only definitions the caller's `is_def` predicate selected.
    defined: Vec<CordisRow>,
    /// Last literal `disabled:` value each row id received in this text,
    /// including ids this text only OVERRIDES — their definition lives in
    /// another layer, and dsh applies layers in order, so such an override is
    /// still live. Definitions contribute their own value (absent ≡ `false`, so
    /// a definition whose `disabled` is an unevaluable `!!js` expression reads
    /// as enabled — the P0 "show the base state" rule); an OVERRIDE carrying
    /// `!!js` contributes nothing, since HK cannot evaluate it.
    disabled_by_id: std::collections::HashMap<String, bool>,
}

/// The one fold used by every dsh patch reader. `is_def` selects which
/// definitions this caller cares about — mcp-client rows for the MCP reader,
/// every other named row for the plugin reader, any insert row for the
/// per-id state lookup. Overrides are merged uniformly (`disabled` AND
/// `config`); callers that model no config simply drop it.
fn fold_rows_in_text(
    text: &str,
    origin: &Path,
    is_def: impl Fn(&CordisRow) -> bool,
) -> FoldedText {
    let mut order: Vec<String> = Vec::new();
    let mut by_id: std::collections::HashMap<String, CordisRow> =
        std::collections::HashMap::new();
    let mut anon: Vec<CordisRow> = Vec::new();
    let mut disabled_by_id: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();

    for row in parse_patch_rows(text, origin) {
        let is_definition = is_def(&row);
        let Some(id) = row.id.clone() else {
            if is_definition {
                anon.push(row);
            }
            continue;
        };
        if is_definition {
            disabled_by_id.insert(id.clone(), row.disabled.unwrap_or(false));
            order.push(id.clone());
            by_id.insert(id, row);
        } else if !row.from_insert {
            // Override: mutates an existing row (upstream: an unknown id is
            // warn+skip, never a definition) — but its literal state still
            // counts for `disabled_by_id`, since the definition may live in
            // an earlier layer that this text never sees.
            if let Some(d) = row.disabled {
                disabled_by_id.insert(id.clone(), d);
            }
            if let Some(existing) = by_id.get_mut(&id) {
                if let Some(d) = row.disabled {
                    existing.disabled = Some(d);
                }
                if !row.config.is_null() {
                    existing.config = row.config;
                }
            }
        }
        // else: from-insert definition of a kind this caller ignores — never
        // an override; skip (even on a malformed id collision).
    }
    let mut defined: Vec<CordisRow> =
        order.into_iter().filter_map(|id| by_id.remove(&id)).collect();
    defined.extend(anon);
    FoldedText { defined, disabled_by_id }
}

/// Folded final state of one third-party plugin row within one patch file.
/// Excludes mcp-client rows (modeled as MCP servers) and override-only rows
/// (an override can never create a row upstream).
struct PluginRowState {
    id: Option<String>,
    name: String,
    disabled: bool,
}

/// Returns the rows this text DEFINES plus the `disabled` state it
/// establishes for every id it touches (definitions and overrides alike).
/// Callers compose the second value across an ordered layer chain — later
/// layers win — which is why it is returned instead of recomputed per row:
/// a per-row lookup would re-parse each ~450-line bundle patch once for
/// every one of its ~80 rows.
fn fold_plugin_rows_in_text(
    text: &str,
    origin: &Path,
) -> (Vec<PluginRowState>, std::collections::HashMap<String, bool>) {
    let folded = fold_rows_in_text(text, origin, |row| {
        row.from_insert && row.name.as_deref().is_some_and(|n| n != MCP_CLIENT_PLUGIN)
    });
    let rows = folded
        .defined
        .into_iter()
        .map(|row| PluginRowState {
            id: row.id,
            name: row.name.expect("a plugin definition implies a name"),
            disabled: row.disabled.unwrap_or(false),
        })
        .collect();
    (rows, folded.disabled_by_id)
}

/// Display name, identity-bearing `source`, and toggle `uri` of one composed
/// plugin row that the layer `where_` owns ("profile web, bundle <pkg>",
/// "profile web", "home layer").
///
/// **The display name is the cordis patch ROW ID, not the npm package name.**
/// dsh's own Settings → Plugins list labels a row by its id (`hmr`, `timer`,
/// `llm`, `api-gateway`, …) — verified against dsh rc.6's UI — and HK shows
/// what dsh shows.
///
/// The package name (`CordisRow.name`, what the row instantiates) is real,
/// useful information, so it moves into the `source` string, which
/// `scanner::scan_plugins` renders verbatim as the extension's description
/// ("Plugin from <source>") in the detail panel. It is the only field that
/// suits it: `path` is absent for home-layer rows (they apply to whichever
/// profile is booted, so no single `<profile>/node_modules/<pkg>` exists) and
/// `source_url` means "upstream URL from the agent's own manifest", which a
/// package name is not. The package slot REPLACES the old `row <id>` suffix,
/// which the name now carries.
///
/// Identity: `scanner::plugin_extension_id` hashes `"<name>:<source>"`, so
/// `(id, where_)` must be unique. It is — a row id is defined at most once per
/// layer chain (`fold_rows_in_text` keeps the first definition per file,
/// `read_plugins` the first per profile via `seen_ids`), and `where_` names
/// the profile (or the home layer), which is what keeps two profiles'
/// instances of the same row apart: they compose different layer chains and
/// can disagree on the enabled state.
///
/// An ANONYMOUS row (no `id:`) has no id to be named by, so it keeps the
/// package name and the `ANON_ROW_SOURCE_SUFFIX` marker — unchanged, and never
/// equal to an id-bearing row's source, which always ends in `package <pkg>`.
fn plugin_row_identity(row: &PluginRowState, where_: &str) -> (String, String, Option<String>) {
    match &row.id {
        Some(id) => (
            id.clone(),
            format!("{where_}, package {}", row.name),
            Some(id.clone()),
        ),
        None => (
            row.name.clone(),
            format!("{where_}, {ANON_ROW_SOURCE_SUFFIX}"),
            None,
        ),
    }
}

/// Folded final state of one MCP row within one patch file.
struct McpRowState {
    id: Option<String>,
    disabled: bool,
    config: serde_yaml::Value,
}

impl DshAdapter {
    fn fold_mcp_rows_in_text(text: &str, origin: &Path) -> Vec<McpRowState> {
        fold_rows_in_text(text, origin, |row| {
            row.from_insert && row.name.as_deref() == Some(MCP_CLIENT_PLUGIN)
        })
        .defined
        .into_iter()
        .map(|row| McpRowState {
            id: row.id,
            disabled: row.disabled.unwrap_or(false),
            config: row.config,
        })
        .collect()
    }

    fn mcp_entries_in_text(text: &str, origin: &Path) -> Vec<McpServerEntry> {
        Self::fold_mcp_rows_in_text(text, origin)
            .into_iter()
            .filter_map(|row| {
                let config = &row.config;
                let server_name = yaml_config_str(config, "serverName")?;
                // Remote MCP: {url, headers?} — stdio MCP: {command, args, env}.
                // `url` decides remote-vs-stdio FIRST (as in hermes.rs): dsh
                // ships only stdio and streamable-http, so a url-bearing row is
                // Streamable HTTP even when `transport` is omitted or carries
                // some other value. Deciding on the transport string instead
                // would emit a contradictory stdio entry with an empty command.
                let url = yaml_config_str(config, "url");
                let (transport, command) = match &url {
                    Some(_) => (McpTransport::Http, String::new()),
                    // Command may be absent on a malformed row; keep the entry
                    // visible (empty command) rather than hiding it.
                    None => (
                        McpTransport::Stdio,
                        yaml_config_str(config, "command").unwrap_or_default(),
                    ),
                };
                Some(McpServerEntry {
                    name: server_name,
                    command,
                    args: config
                        .get("args")
                        .and_then(|v| v.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    env: super::yaml_string_map(config, "env"),
                    transport,
                    url,
                    headers: super::yaml_string_map(config, "headers"),
                    enabled: !row.disabled,
                })
            })
            .collect()
    }

    /// Row id for a serverName (unique across live instances upstream).
    /// Text-based so the deployer can evaluate the file it already read.
    pub fn mcp_row_id_in_text(text: &str, server_name: &str) -> Option<String> {
        Self::fold_mcp_rows_in_text(text, Path::new("cordis.patch.yml"))
            .into_iter()
            .find(|r| yaml_config_str(&r.config, "serverName").as_deref() == Some(server_name))
            .and_then(|r| r.id)
    }

    /// Per-text state of one plugin row id: `(defined, disabled)` where
    /// `defined` is true when the text contains an insert DEFINITION of the
    /// id, and `disabled` is the last value the text establishes for it —
    /// definition default `false`, later literal overrides win, `!!js`/absent
    /// overrides change nothing. Callers fold across the layer texts dsh
    /// composes for ONE profile (that profile's patch, then home) — never
    /// across sibling profiles, which are never loaded together.
    ///
    /// A plain lookup over the shared fold: `is_def` is "any insert row",
    /// because this per-id question is name-agnostic (an id-targeted
    /// override does not know what kind of plugin it targets).
    pub fn plugin_row_state_in_text(text: &str, row_id: &str) -> (bool, Option<bool>) {
        let folded = fold_rows_in_text(text, Path::new("cordis.patch.yml"), |row| row.from_insert);
        let defined = folded
            .defined
            .iter()
            .any(|row| row.id.as_deref() == Some(row_id));
        (defined, folded.disabled_by_id.get(row_id).copied())
    }

    /// Every row id appearing in a patch text (definitions AND overrides) —
    /// the collision domain for HK-generated insert-row ids.
    pub fn row_ids_in_text(text: &str) -> std::collections::HashSet<String> {
        parse_patch_rows(text, Path::new("cordis.patch.yml"))
            .into_iter()
            .filter_map(|r| r.id)
            .collect()
    }

    /// serverName → enabled for the given home-layer text (deployer uses this
    /// to compute base state with HK's managed block stripped).
    pub fn mcp_enabled_in_text(text: &str) -> std::collections::HashMap<String, bool> {
        Self::fold_mcp_rows_in_text(text, Path::new("cordis.patch.yml"))
            .into_iter()
            .filter_map(|r| Some((yaml_config_str(&r.config, "serverName")?, !r.disabled)))
            .collect()
    }
}

impl AgentAdapter for DshAdapter {
    fn name(&self) -> &str {
        "dsh"
    }

    fn base_dir(&self) -> PathBuf {
        self.dsh_home.clone()
    }

    fn detect(&self) -> bool {
        self.dsh_home.exists()
    }

    fn skill_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.dsh_home.join("skills"),
            self.agents_home.join("skills"),
        ]
    }

    /// Home-level user patch — the highest always-applied user layer and the
    /// canonical write target for HK's managed toggle block. NEVER point this
    /// at `<profileDir>/cordis.yml`: dsh overwrites that file on every boot.
    fn mcp_config_path(&self) -> PathBuf {
        self.dsh_home.join("cordis.patch.yml")
    }

    fn hook_config_path(&self) -> PathBuf {
        // dsh has no own hook config; return the settings doc so the default
        // plugin_config_path() has a sane anchor. Never read for hooks
        // (hook_format is None).
        self.dsh_home.join("settings.yaml")
    }

    fn plugin_dirs(&self) -> Vec<PathBuf> {
        vec![]
    }

    /// dsh plugin discovery. dsh's own vocabulary is the composed ROW: its
    /// Settings → Plugins list shows one Enabled/Disabled entry per row
    /// (`timer`, `hmr`, `llm`, …), and a BUNDLE never appears there at all —
    /// a bundle is a patch LAYER that inserts rows, not a plugin. HK mirrors
    /// that, so two sources per profile:
    ///
    /// 1. Rows DEFINED by each mounted bundle's own patch file
    ///    (`dsh.bundle.patch` in the bundle's package.json — an ordinary
    ///    patch file, parsed by the same `parse_patch_rows`). This is the
    ///    bulk of the list (~130 rows on a stock install) and the only place
    ///    most toggleable rows live.
    /// 2. Rows DEFINED by the user's own patch files (home layer first, then
    ///    each profile's `cordis.patch.yml`).
    ///
    /// A package that is a profile `dependency` but which no layer mounts is
    /// deliberately NOT listed: dsh never loads it and its own UI never shows
    /// it, so neither does HK ("if dsh doesn't show it, HK doesn't show it").
    ///
    /// mcp-client rows are excluded throughout (modeled as MCP servers), and
    /// bundles themselves are NOT emitted as entries.
    ///
    /// Each entry is named by its patch ROW ID, exactly as dsh's own
    /// Settings → Plugins list labels it; the package the row instantiates
    /// lives in the `source` string. See `plugin_row_identity`.
    ///
    /// Ordering and identity within a profile follow dsh's own composition:
    /// bundle patches in `bundles` order, then the profile patch, then the
    /// home patch. The EARLIEST layer defining a row id owns the entry (a
    /// later layer can only override it — upstream, an override never
    /// creates a row), and the `disabled` state folds across the whole chain,
    /// which is why `hmr` reads as disabled: `@deepseek-ai/dsh-base` defines
    /// it and `@deepseek-ai/dsh-web-app` disables it two layers later.
    ///
    /// Known parser limitation (accepted): `{id, insert}` group-appends are
    /// skipped, so plugins inserted into a group are invisible.
    fn read_plugins(&self) -> Vec<super::PluginEntry> {
        use super::PluginEntry;
        let mut entries: Vec<PluginEntry> = Vec::new();

        // --- Home layer rows (source 2, home) ---
        let home_patch = self.mcp_config_path();
        let home_text = std::fs::read_to_string(&home_patch).unwrap_or_default();
        let (home_rows, home_disabled) = fold_plugin_rows_in_text(&home_text, &home_patch);
        for row in home_rows {
            let (name, source, uri) = plugin_row_identity(&row, "home layer");
            entries.push(PluginEntry {
                name,
                source,
                // The home layer is applied LAST, so nothing overrides a
                // home-defined row but the home text itself (already folded).
                enabled: !row.disabled,
                // No path: a home row applies to EVERY profile, so its
                // package resolves under whichever profile is booted — there
                // is no single `<profile>/node_modules/<name>` to probe.
                path: None,
                source_url: None,
                uri,
                installed_at: None,
                updated_at: None,
                // Its own layer; the writer recognises the home patch and
                // folds the (block-stripped) user text instead of re-reading.
                base_layers: vec![home_patch.clone()],
            });
        }

        // --- Per profile: bundle rows (1), profile rows (2) ---
        for profile_dir in self.profile_dirs() {
            let profile = profile_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let manifest_path = profile_dir.join("package.json");
            let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
                continue; // no package.json — not a plugin-bearing profile
            };
            let manifest: ProfileManifest = match serde_json::from_str(&manifest_text) {
                Ok(m) => m,
                Err(err) => {
                    // Skip THIS profile with a diagnostic; never abort the scan.
                    eprintln!(
                        "[hk] warning: cannot parse {}: {err}",
                        manifest_path.display()
                    );
                    continue;
                }
            };
            let bundles = manifest.dsh.profile.bundles;

            // The layers dsh composes for this profile BELOW the home patch,
            // in application order: each mounted bundle's own patch file,
            // then the profile's own. A bundle whose patch cannot be
            // resolved (package absent, no `dsh.bundle.patch`, file missing)
            // simply contributes no layer — it is not an error here; dsh
            // itself fails loud on a bundle it cannot load.
            let profile_patch = profile_dir.join("cordis.patch.yml");
            let mut layers: Vec<(Option<String>, PathBuf)> = bundles
                .iter()
                .filter_map(|b| {
                    self.bundle_patch_path(&profile_dir, b)
                        .map(|p| (Some(b.clone()), p))
                })
                .collect();
            layers.push((None, profile_patch));
            let base_layers: Vec<PathBuf> = layers.iter().map(|(_, p)| p.clone()).collect();

            // One ordered pass: collect each layer's row DEFINITIONS
            // (earliest layer wins per id) while folding the `disabled` state
            // every layer establishes. Computed once per profile — asking
            // per row would re-parse each ~450-line bundle patch ~80 times.
            let mut defined: Vec<(Option<String>, PluginRowState)> = Vec::new();
            let mut seen_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut composed: std::collections::HashMap<String, bool> =
                std::collections::HashMap::new();
            for (bundle, path) in &layers {
                let text = std::fs::read_to_string(path).unwrap_or_default();
                let (rows, disabled_by_id) = fold_plugin_rows_in_text(&text, path);
                composed.extend(disabled_by_id);
                for row in rows {
                    // `insert` returns false when the id was already seen —
                    // an earlier layer defines it, and upstream a later
                    // restatement can only override, never redefine.
                    if row
                        .id
                        .as_ref()
                        .is_some_and(|id| !seen_ids.insert(id.clone()))
                    {
                        continue;
                    }
                    defined.push((bundle.clone(), row));
                }
            }
            // The home patch applies after every profile layer, so its
            // overrides win — user text and HK's managed block alike (the
            // block is plain YAML within the file).
            composed.extend(home_disabled.clone());

            // (1) + (2) one entry per composed row.
            for (bundle, row) in defined {
                let enabled = match &row.id {
                    Some(id) => !composed.get(id).copied().unwrap_or(row.disabled),
                    None => !row.disabled,
                };
                // Identity is id-load-bearing (scanner::stable_id over
                // "<name>:<source>"), so the source names the profile and the
                // bundle that provided the row when one did. The profile
                // prefix is what keeps two profiles' instances of the same
                // bundle row apart — they can differ in enabled state and
                // compose different layer chains.
                let where_ = match &bundle {
                    Some(pkg) => format!("profile {profile}, bundle {pkg}"),
                    None => format!("profile {profile}"),
                };
                let (name, source, uri) = plugin_row_identity(&row, &where_);
                entries.push(PluginEntry {
                    name,
                    source,
                    enabled,
                    path: self.package_dir(&profile_dir, &row.name),
                    source_url: None,
                    uri,
                    installed_at: None,
                    updated_at: None,
                    base_layers: base_layers.clone(),
                });
            }
        }
        entries
    }

    fn hook_format(&self) -> HookFormat {
        HookFormat::None
    }

    fn mcp_format(&self) -> McpFormat {
        McpFormat::DshCordis
    }

    fn supports_native_mcp_toggle(&self) -> bool {
        // Toggle appends id-targeted patch rows via a managed block (deployer::set_dsh_mcp_enabled); never rewrites user YAML.
        true
    }

    fn read_mcp_servers(&self) -> Vec<McpServerEntry> {
        self.read_mcp_servers_from(&self.mcp_config_path())
    }

    fn read_mcp_servers_from(&self, path: &Path) -> Vec<McpServerEntry> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return vec![];
        };
        Self::mcp_entries_in_text(&text, path)
    }

    fn read_hooks(&self) -> Vec<HookEntry> {
        vec![]
    }

    fn global_rules_files(&self) -> Vec<PathBuf> {
        vec![self.dsh_home.join("AGENTS.md")]
    }

    fn global_settings_files(&self) -> Vec<PathBuf> {
        let mut files = vec![
            self.dsh_home.join("settings.yaml"),
            self.dsh_home.join("cordis.patch.yml"),
        ];
        files.extend(self.profile_patch_files());
        files
    }

    fn project_rules_patterns(&self) -> Vec<String> {
        vec![
            "AGENTS.md".into(),
            "CLAUDE.md".into(),
            "AGENTS.local.md".into(),
            "CLAUDE.local.md".into(),
        ]
    }

    fn project_markers(&self) -> Vec<ProjectMarker> {
        vec![ProjectMarker::Dir(".dsh")]
    }

    fn project_skill_dirs(&self) -> Vec<String> {
        vec![".dsh/skills".into()]
    }

    fn project_skill_read_dirs(&self) -> Vec<String> {
        vec![".agents/skills".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::AgentAdapter;
    use super::*;

    #[test]
    fn resolve_homes_env_overrides_and_fallbacks() {
        let home = Path::new("/home/u");

        // Both env vars set → both override.
        let (dsh, agents) = resolve_homes(
            Some("/custom/dsh".into()),
            Some("/custom/agents".into()),
            home,
        );
        assert_eq!(dsh, PathBuf::from("/custom/dsh"));
        assert_eq!(agents, PathBuf::from("/custom/agents"));

        // Both unset → ~/.dsh and ~/.agents fallbacks.
        let (dsh, agents) = resolve_homes(None, None, home);
        assert_eq!(dsh, home.join(".dsh"));
        assert_eq!(agents, home.join(".agents"));

        // DSH_HOME set, DSH_AGENTS_HOME unset → mixed.
        let (dsh, agents) = resolve_homes(Some("/custom/dsh".into()), None, home);
        assert_eq!(dsh, PathBuf::from("/custom/dsh"));
        assert_eq!(agents, home.join(".agents"));
    }

    #[test]
    fn detect_requires_dsh_home() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        assert!(!adapter.detect());
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        assert!(adapter.detect());
    }

    #[test]
    fn skill_dirs_cover_dsh_and_agents_homes() {
        let tmp = tempfile::tempdir().unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        assert_eq!(
            adapter.skill_dirs(),
            vec![
                tmp.path().join(".dsh/skills"),
                tmp.path().join(".agents/skills"),
            ]
        );
        // Canonical install target is the dsh-owned dir (skill_dir_for uses first).
        assert_eq!(adapter.project_skill_dirs(), vec![".dsh/skills".to_string()]);
        assert_eq!(
            adapter.project_skill_read_dirs(),
            vec![".agents/skills".to_string()]
        );
    }

    #[test]
    fn config_discovery_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let dsh_home = tmp.path().join(".dsh");
        // settings files include existing per-profile patch files
        std::fs::create_dir_all(dsh_home.join("profiles/web")).unwrap();
        std::fs::write(dsh_home.join("profiles/web/cordis.patch.yml"), "[]\n").unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());

        assert_eq!(adapter.global_rules_files(), vec![dsh_home.join("AGENTS.md")]);
        let settings = adapter.global_settings_files();
        assert!(settings.contains(&dsh_home.join("settings.yaml")));
        assert!(settings.contains(&dsh_home.join("cordis.patch.yml")));
        assert!(settings.contains(&dsh_home.join("profiles/web/cordis.patch.yml")));
        assert_eq!(
            adapter.project_rules_patterns(),
            vec!["AGENTS.md", "CLAUDE.md", "AGENTS.local.md", "CLAUDE.local.md"]
        );
        // HK-side project-discovery marker (dsh's only project-level dir).
        // dsh itself finds project roots by walking to the nearest `.git`.
        assert_eq!(adapter.project_markers(), vec![super::super::ProjectMarker::Dir(".dsh")]);
        // No project-level MCP/hook config exists upstream.
        assert_eq!(adapter.project_mcp_config_relpath(), None);
        assert_eq!(adapter.project_hook_config_relpath(), None);
    }

    /// Pins the load-bearing dependency behavior: serde_yaml parses `!!js`
    /// scalars (plain / quoted / block forms) WITHOUT error and silently
    /// strips the tag, yielding the expression as a plain String. Every
    /// reader below builds on this. If this test ever fails, serde_yaml's
    /// tag handling changed — re-verify the readers before touching them.
    #[test]
    fn serde_yaml_strips_double_bang_tags_to_plain_strings() {
        for (text, expected) in [
            ("k: !!js process.cwd()", "process.cwd()"),
            ("k: !!js '`Bearer ${process.env.T}`'", "`Bearer ${process.env.T}`"),
            (
                "k: !!js >-\n  process.env.X?.trim() ||\n  fallback()",
                "process.env.X?.trim() || fallback()",
            ),
        ] {
            let v: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
            assert_eq!(v.get("k").and_then(|v| v.as_str()), Some(expected), "input: {text}");
        }
    }

    /// Home-layer fixture. The `env` block-scalar entry is copied from dsh's
    /// own examples/mcp-memory/mcp-reference-memory.cordis.yml — its README
    /// tells users to merge exactly this into the patch files HK reads.
    const HOME_PATCH: &str = r#"# user layer — keep this comment
- insert:
    - id: mcp-github
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: github
        transport: stdio
        command: npx
        args: ['-y', '@modelcontextprotocol/server-github']
        cwd: !!js process.cwd()
        env:
          GITHUB_TOKEN: !!js process.env.GITHUB_TOKEN
          MEMORY_FILE_PATH: !!js >-
            process.env.MEMORY_FILE_PATH?.trim() ||
            process.getBuiltinModule('node:path').join(process.cwd(), 'memory.json')
    - id: mcp-web
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: web
        transport: streamable-http
        url: http://localhost:3000/mcp
        headers:
          Authorization: !!js '`Bearer ${process.env.MCP_TOKEN}`'
- id: mcp-github
  disabled: true
"#;

    fn write_home_patch(home: &Path, text: &str) {
        std::fs::create_dir_all(home.join(".dsh")).unwrap();
        std::fs::write(home.join(".dsh/cordis.patch.yml"), text).unwrap();
    }

    #[test]
    fn read_mcp_servers_parses_home_layer_with_js_tags() {
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(tmp.path(), HOME_PATCH);
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 2);

        let gh = servers.iter().find(|s| s.name == "github").unwrap();
        assert!(!gh.enabled, "later same-file override disabled mcp-github");
        assert_eq!(gh.command, "npx");
        assert_eq!(gh.args, vec!["-y", "@modelcontextprotocol/server-github"]);
        // !!js values arrive tag-stripped as bare expression text — shown
        // as-is, never evaluated (see the probe test above).
        assert_eq!(gh.env["GITHUB_TOKEN"], "process.env.GITHUB_TOKEN");
        assert!(gh.env["MEMORY_FILE_PATH"].starts_with("process.env.MEMORY_FILE_PATH?.trim()"));

        let web = servers.iter().find(|s| s.name == "web").unwrap();
        assert_eq!(web.transport, McpTransport::Http);
        assert_eq!(web.url.as_deref(), Some("http://localhost:3000/mcp"));
        assert_eq!(web.headers["Authorization"], "`Bearer ${process.env.MCP_TOKEN}`");
        assert!(web.enabled);
    }

    #[test]
    fn disabled_false_later_in_file_reenables() {
        let tmp = tempfile::tempdir().unwrap();
        let text = format!("{HOME_PATCH}- id: mcp-github\n  disabled: false\n");
        write_home_patch(tmp.path(), &text);
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let gh = adapter
            .read_mcp_servers()
            .into_iter()
            .find(|s| s.name == "github")
            .unwrap();
        assert!(gh.enabled, "later entry wins (single ordered apply upstream)");
    }

    #[test]
    fn bare_override_never_creates_a_row() {
        // Upstream: a patch targeting an unknown id is warn+skip, never a
        // definition (vendor/include/src/index.ts:107-112).
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(
            tmp.path(),
            "- id: ghost\n  name: '@deepseek-ai/dsh-mcp-client'\n  config:\n    serverName: ghost\n",
        );
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        assert!(adapter.read_mcp_servers().is_empty());
    }

    #[test]
    fn absent_patch_file_text_parses_to_no_rows_without_warning() {
        // Callers feed "" for a MISSING cordis.patch.yml (read_to_string
        // .unwrap_or_default()); empty/whitespace text is the absent-file
        // case, not a malformed list — no rows, and no stderr warning.
        assert!(parse_patch_rows("", Path::new("cordis.patch.yml")).is_empty());
        assert!(parse_patch_rows(" \n\t\n", Path::new("cordis.patch.yml")).is_empty());
    }

    #[test]
    fn mcp_row_id_lookup_by_server_name() {
        assert_eq!(
            DshAdapter::mcp_row_id_in_text(HOME_PATCH, "github").as_deref(),
            Some("mcp-github")
        );
        assert_eq!(DshAdapter::mcp_row_id_in_text(HOME_PATCH, "nope"), None);
    }

    #[test]
    fn mcp_enabled_map_reflects_folded_state() {
        let map = DshAdapter::mcp_enabled_in_text(HOME_PATCH);
        assert_eq!(map.len(), 2);
        assert_eq!(map["github"], false);
        assert_eq!(map["web"], true);

        // `disabled: null` ≡ false upstream (static rule, entry.ts:104-107) —
        // a null override re-enables a previously disabled row.
        let text = format!("{HOME_PATCH}- id: mcp-github\n  disabled: null\n");
        let map = DshAdapter::mcp_enabled_in_text(&text);
        assert_eq!(map["github"], true, "disabled: null re-enables");
    }

    #[test]
    fn url_decides_remote_even_without_a_transport_key() {
        // A url-bearing row with `transport` omitted (or set to anything other
        // than streamable-http) is still remote — never a stdio entry with an
        // empty command.
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(
            tmp.path(),
            r#"- insert:
    - id: mcp-a
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: a
        url: https://a.example/mcp
    - id: mcp-b
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: b
        transport: sse
        url: https://b.example/mcp
"#,
        );
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 2);
        for s in &servers {
            assert_eq!(s.transport, McpTransport::Http, "{} should be remote", s.name);
            assert!(s.command.is_empty(), "{} should carry no command", s.name);
            assert!(s.url.is_some(), "{} should keep its url", s.name);
        }
    }

    fn write_profile(home: &Path, profile: &str, package_json: &str, patch: Option<&str>) {
        let dir = home.join(".dsh/profiles").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), package_json).unwrap();
        if let Some(p) = patch {
            std::fs::write(dir.join("cordis.patch.yml"), p).unwrap();
        }
    }

    /// Install a mounted BUNDLE package into dsh's symlink farm the way a
    /// real one ships: a `package.json` declaring `dsh.bundle.patch` plus the
    /// patch file it points at (verified shape: `"patch": "./cordis.patch.yml"`).
    fn write_bundle(home: &Path, pkg: &str, patch: &str) {
        let dir = home.join(".dsh/profiles/node_modules").join(pkg);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            format!(
                r#"{{"name": "{pkg}", "dsh": {{"bundle": {{"patch": "./cordis.patch.yml"}}}}}}"#
            ),
        )
        .unwrap();
        std::fs::write(dir.join("cordis.patch.yml"), patch).unwrap();
    }

    /// Shape copied from the installed `@deepseek-ai/dsh-base` rc.6: ONE
    /// `insert:` group holding every base row.
    const BASE_BUNDLE_PATCH: &str = "\
- insert:
    - id: timer
      name: '@deepseek-ai/cordis-plugin-timer'
    - id: hmr
      name: '@deepseek-ai/cordis-plugin-hmr'
      config:
        root: ['.']
    - id: llm
      name: '@deepseek-ai/dsh-llm'
";

    /// Shape copied from the installed `@deepseek-ai/dsh-web-app` rc.6: id
    /// overrides of base rows (including the real `hmr` disable) plus its own
    /// insert group.
    const WEB_APP_BUNDLE_PATCH: &str = "\
- id: hmr
  disabled: true
- insert:
    - id: web-server
      name: '@deepseek-ai/dsh-host-webserver'
";

    const WEB_MANIFEST: &str = r#"{
  "name": "dsh-profile-web",
  "dependencies": {
    "@deepseek-ai/dsh-base": "0.1.0",
    "@deepseek-ai/dsh-mcp-client": "0.1.0",
    "dsh-plugin-tool": "1.0.0",
    "left-pad": "1.3.0"
  },
  "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base"] } }
}"#;

    const WEB_PATCH: &str = "- insert:\n    - id: tool-policy\n      name: dsh-plugin-tool\n      config:\n        mode: strict\n";

    /// `web` mounting both in-box bundles, with a user row — the real
    /// machine's shape in miniature.
    fn two_bundle_profile(tmp: &Path) {
        write_bundle(tmp, "@deepseek-ai/dsh-base", BASE_BUNDLE_PATCH);
        write_bundle(tmp, "@deepseek-ai/dsh-web-app", WEB_APP_BUNDLE_PATCH);
        write_profile(
            tmp,
            "web",
            r#"{
  "dependencies": {"dsh-plugin-tool": "1.0.0", "left-pad": "1.3.0"},
  "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] } }
}"#,
            Some(WEB_PATCH),
        );
    }

    #[test]
    fn read_plugins_lists_bundle_rows_and_user_rows() {
        let tmp = tempfile::tempdir().unwrap();
        two_bundle_profile(tmp.path());
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();

        // Source 1: a row from a mounted bundle's own patch file — listed
        // under its ROW ID (what dsh's own plugin list shows), with the
        // bundle and the package it instantiates in the identity-bearing
        // source string.
        let timer = plugins.iter().find(|p| p.name == "timer").unwrap();
        assert_eq!(
            timer.source,
            "profile web, bundle @deepseek-ai/dsh-base, package @deepseek-ai/cordis-plugin-timer"
        );
        assert_eq!(timer.uri.as_deref(), Some("timer"));
        assert!(timer.enabled);
        // Toggle input: the profile's whole chain, bundles first.
        assert_eq!(
            timer.base_layers,
            vec![
                tmp.path().join(".dsh/profiles/node_modules/@deepseek-ai/dsh-base/cordis.patch.yml"),
                tmp.path()
                    .join(".dsh/profiles/node_modules/@deepseek-ai/dsh-web-app/cordis.patch.yml"),
                tmp.path().join(".dsh/profiles/web/cordis.patch.yml"),
            ]
        );

        // A row a LATER bundle inserts is owned by that bundle.
        let web_server = plugins.iter().find(|p| p.name == "web-server").unwrap();
        assert_eq!(
            web_server.source,
            "profile web, bundle @deepseek-ai/dsh-web-app, package @deepseek-ai/dsh-host-webserver"
        );

        // Source 2: the user's own profile patch row.
        let row = plugins.iter().find(|p| p.name == "tool-policy").unwrap();
        assert_eq!(row.source, "profile web, package dsh-plugin-tool");
        assert_eq!(row.uri.as_deref(), Some("tool-policy"));
        assert!(row.enabled);

        // A dependency no layer mounts is NOT listed: dsh never loads it and
        // never shows it, so neither does HK. `left-pad` is such a dep of the
        // `web` profile below.
        assert!(plugins
            .iter()
            .all(|p| p.name != "left-pad" && !p.source.ends_with("package left-pad")));

        // The profiles/node_modules symlink farm is not a profile.
        assert!(plugins.iter().all(|p| !p.source.starts_with("profile node_modules")));
    }

    #[test]
    fn bundles_themselves_are_not_plugin_entries() {
        // A bundle is a LAYER. dsh's own Settings → Plugins list has no such
        // entry — searching it for "@deepseek-ai/dsh-base" finds nothing —
        // so neither does HK; only the rows the layer inserts are listed.
        let tmp = tempfile::tempdir().unwrap();
        two_bundle_profile(tmp.path());
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        for bundle in ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] {
            assert!(
                plugins
                    .iter()
                    .all(|p| p.name != bundle && !p.source.ends_with(&format!("package {bundle}"))),
                "{bundle} must not be listed as a plugin"
            );
        }
        // ...not even when a profile also lists the bundle as a dependency.
        write_profile(
            tmp.path(),
            "web2",
            r#"{"dependencies": {"@deepseek-ai/dsh-base": "0.1.0"}, "dsh": {"profile": {"bundles": ["@deepseek-ai/dsh-base"]}}}"#,
            None,
        );
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        assert!(plugins
            .iter()
            .all(|p| !p.source.ends_with("package @deepseek-ai/dsh-base")));
    }

    #[test]
    fn a_later_bundle_layer_disables_an_earlier_bundles_row() {
        // dsh's real `hmr` case: defined by dsh-base, disabled by dsh-web-app
        // two layers on. The entry belongs to the DEFINING bundle and reads
        // as disabled — the composed state, not the definition's.
        let tmp = tempfile::tempdir().unwrap();
        two_bundle_profile(tmp.path());
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        let hmr = plugins.iter().find(|p| p.name == "hmr").unwrap();
        assert_eq!(
            hmr.source,
            "profile web, bundle @deepseek-ai/dsh-base, package @deepseek-ai/cordis-plugin-hmr",
            "the earliest layer defining the id owns the entry"
        );
        assert!(!hmr.enabled, "a later bundle layer's disable wins");
    }

    #[test]
    fn home_layer_override_disables_a_bundle_row() {
        // The home patch applies after every profile layer, so an HK managed
        // block disable (plain YAML in that file) turns a bundle row off —
        // exactly the mechanism the plugin toggle writes.
        let tmp = tempfile::tempdir().unwrap();
        two_bundle_profile(tmp.path());
        write_home_patch(tmp.path(), "- id: timer\n  disabled: true\n");
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        let timer = plugins.iter().find(|p| p.name == "timer").unwrap();
        assert!(!timer.enabled, "home-layer disable of a bundle row wins");

        // ...and re-enabling a bundle-disabled row works the same way.
        write_home_patch(tmp.path(), "- id: hmr\n  disabled: false\n");
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        let hmr = plugins.iter().find(|p| p.name == "hmr").unwrap();
        assert!(hmr.enabled, "home layer wins over the web-app bundle disable");
    }

    #[test]
    fn mcp_client_rows_in_a_bundle_patch_are_not_plugins() {
        // mcp-client rows are modeled as MCP servers wherever they appear.
        let tmp = tempfile::tempdir().unwrap();
        write_bundle(
            tmp.path(),
            "dsh-mcp-bundle",
            "- insert:\n    - id: mcp-x\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        serverName: x\n    - id: plain\n      name: dsh-plugin-plain\n",
        );
        write_profile(
            tmp.path(),
            "web",
            r#"{"dependencies": {}, "dsh": {"profile": {"bundles": ["dsh-mcp-bundle"]}}}"#,
            None,
        );
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        assert!(plugins
            .iter()
            .all(|p| p.uri.as_deref() != Some("mcp-x")
                && !p.source.ends_with(&format!("package {MCP_CLIENT_PLUGIN}"))));
        assert!(plugins.iter().any(|p| p.name == "plain"));
    }

    #[test]
    fn earliest_layer_defining_a_row_id_owns_the_entry() {
        // Upstream, only the first `insert` of an id creates the row; a later
        // layer restating it can merely override. One entry, owned by the
        // bundle — not two.
        let tmp = tempfile::tempdir().unwrap();
        write_bundle(
            tmp.path(),
            "dsh-extra-bundle",
            "- insert:\n    - id: extra-row\n      name: dsh-plugin-extra\n",
        );
        write_profile(
            tmp.path(),
            "web",
            r#"{"dependencies": {}, "dsh": {"profile": {"bundles": ["dsh-extra-bundle"]}}}"#,
            Some("- insert:\n    - id: extra-row\n      name: dsh-plugin-extra\n"),
        );
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        let entries: Vec<_> = plugins.iter().filter(|p| p.uri.as_deref() == Some("extra-row")).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].source,
            "profile web, bundle dsh-extra-bundle, package dsh-plugin-extra"
        );
    }

    #[test]
    fn a_bundle_without_a_resolvable_patch_contributes_no_rows_and_no_error() {
        // Fresh install / never-booted dsh: the symlink farm may be absent.
        // The scan must degrade to "no rows from that layer", not blow up.
        let tmp = tempfile::tempdir().unwrap();
        write_profile(tmp.path(), "web", WEB_MANIFEST, Some(WEB_PATCH));
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        // Only the user's own row survives.
        assert!(plugins.iter().any(|p| p.name == "tool-policy"));
        assert!(plugins.iter().all(|p| !p.source.contains("bundle ")));
        // mcp-client is mounted (its rows are MCP servers), never a plugin.
        assert!(plugins
            .iter()
            .all(|p| !p.source.ends_with(&format!("package {MCP_CLIENT_PLUGIN}"))));
    }

    #[test]
    fn bundle_rows_resolve_their_package_from_the_symlink_farm() {
        // A bundle row's package is a transitive dep of the bundle, hoisted
        // into `<dsh_home>/profiles/node_modules` — not into the profile's
        // own node_modules.
        let tmp = tempfile::tempdir().unwrap();
        two_bundle_profile(tmp.path());
        let farm = tmp
            .path()
            .join(".dsh/profiles/node_modules/@deepseek-ai/dsh-llm");
        std::fs::create_dir_all(&farm).unwrap();
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        let llm = plugins.iter().find(|p| p.name == "llm").unwrap();
        assert_eq!(
            llm.source,
            "profile web, bundle @deepseek-ai/dsh-base, package @deepseek-ai/dsh-llm"
        );
        assert_eq!(llm.path.as_deref(), Some(farm.as_path()));
    }

    #[test]
    fn home_layer_rows_and_home_overrides_of_profile_rows() {
        let tmp = tempfile::tempdir().unwrap();
        write_profile(
            tmp.path(),
            "web",
            r#"{"dependencies": {"dsh-plugin-tool": "1.0.0"}, "dsh": {"profile": {"bundles": []}}}"#,
            Some(WEB_PATCH),
        );
        std::fs::write(
            tmp.path().join(".dsh/cordis.patch.yml"),
            "- insert:\n    - id: theme-row\n      name: dsh-plugin-theme\n- id: tool-policy\n  disabled: true\n",
        )
        .unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();

        let theme = plugins.iter().find(|p| p.name == "theme-row").unwrap();
        assert_eq!(theme.source, "home layer, package dsh-plugin-theme");
        assert_eq!(theme.uri.as_deref(), Some("theme-row"));
        assert!(theme.enabled);

        // The home layer applies after every profile layer, so a home
        // `disabled: true` override (user- OR HK-block-authored — the block
        // is plain YAML within the file) wins over the profile definition.
        let tool = plugins.iter().find(|p| p.name == "tool-policy").unwrap();
        assert!(!tool.enabled, "home-layer disable override wins");
    }

    #[test]
    fn unparseable_profile_manifest_skips_that_profile_only() {
        let tmp = tempfile::tempdir().unwrap();
        write_profile(tmp.path(), "bad", "{ not json", None);
        write_profile(
            tmp.path(),
            "good",
            r#"{"dependencies": {"dsh-plugin-tool": "1.0.0"}, "dsh": {"profile": {"bundles": []}}}"#,
            Some(WEB_PATCH),
        );
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        assert_eq!(plugins.len(), 1, "bad profile skipped, scan not aborted");
        assert_eq!(plugins[0].name, "tool-policy");
        assert_eq!(plugins[0].source, "profile good, package dsh-plugin-tool");
    }

    #[test]
    fn in_box_bundle_patch_resolves_from_the_symlink_farm() {
        // The three in-box bundles resolve from dsh's maintained farm, NOT
        // from a profile's own node_modules — a stale copy there must not
        // shadow the farm's rows.
        let tmp = tempfile::tempdir().unwrap();
        write_bundle(tmp.path(), "@deepseek-ai/dsh-base", BASE_BUNDLE_PATCH);
        write_profile(
            tmp.path(),
            "web",
            r#"{"dependencies": {}, "dsh": {"profile": {"bundles": ["@deepseek-ai/dsh-base"]}}}"#,
            None,
        );
        let stale = tmp
            .path()
            .join(".dsh/profiles/web/node_modules/@deepseek-ai/dsh-base");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(
            stale.join("package.json"),
            r#"{"dsh": {"bundle": {"patch": "./cordis.patch.yml"}}}"#,
        )
        .unwrap();
        std::fs::write(
            stale.join("cordis.patch.yml"),
            "- insert:\n    - id: stale\n      name: dsh-plugin-stale\n",
        )
        .unwrap();

        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();
        assert!(
            plugins.iter().any(|p| p.uri.as_deref() == Some("timer")),
            "farm patch is the one that is read"
        );
        assert!(plugins.iter().all(|p| p.uri.as_deref() != Some("stale")));
    }

    #[test]
    fn two_rows_of_same_package_have_distinct_identities() {
        // Cordis is instance-based: one package can carry two insert rows
        // with distinct ids (the mcp-client pattern). Each row is NAMED by
        // its id, so the two are distinct extensions — and the package they
        // share is still visible, in the source string.
        let tmp = tempfile::tempdir().unwrap();
        write_profile(
            tmp.path(),
            "web",
            r#"{"dependencies": {}, "dsh": {"profile": {"bundles": []}}}"#,
            Some("- insert:\n    - id: a-row\n      name: dsh-plugin-multi\n    - id: b-row\n      name: dsh-plugin-multi\n"),
        );
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let plugins = adapter.read_plugins();
        let rows: Vec<_> = plugins
            .iter()
            .filter(|p| p.source == "profile web, package dsh-plugin-multi")
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "a-row");
        assert_eq!(rows[1].name, "b-row");
        let ids: Vec<String> = rows
            .iter()
            .map(|p| crate::scanner::plugin_extension_id(&p.name, &p.source, "dsh"))
            .collect();
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn one_row_id_in_several_layers_keeps_distinct_identities() {
        // Naming a row by its id makes the SOURCE the only discriminator
        // left, so it has to carry the owning layer: the same id can be
        // defined once per profile and once in the home layer, and those are
        // different rows with different composed states. An anonymous row
        // (no id) keeps the package name and the anon marker, which can never
        // equal an id row's `…, package <pkg>` source.
        let tmp = tempfile::tempdir().unwrap();
        write_profile(
            tmp.path(),
            "web",
            r#"{"dependencies": {}, "dsh": {"profile": {"bundles": []}}}"#,
            Some("- insert:\n    - id: shared\n      name: dsh-plugin-a\n    - name: dsh-plugin-anon\n"),
        );
        write_profile(
            tmp.path(),
            "cli",
            r#"{"dependencies": {}, "dsh": {"profile": {"bundles": []}}}"#,
            Some("- insert:\n    - id: shared\n      name: dsh-plugin-b\n"),
        );
        write_home_patch(
            tmp.path(),
            "- insert:\n    - id: shared\n      name: dsh-plugin-c\n",
        );
        let plugins = DshAdapter::with_home(tmp.path().to_path_buf()).read_plugins();

        let mut sources: Vec<&str> = plugins
            .iter()
            .filter(|p| p.name == "shared")
            .map(|p| p.source.as_str())
            .collect();
        sources.sort();
        assert_eq!(
            sources,
            vec![
                "home layer, package dsh-plugin-c",
                "profile cli, package dsh-plugin-b",
                "profile web, package dsh-plugin-a",
            ]
        );

        // The anonymous row is named by its package and is untoggleable.
        let anon = plugins
            .iter()
            .find(|p| p.name == "dsh-plugin-anon")
            .unwrap();
        assert_eq!(anon.source, "profile web, anonymous row");
        assert!(anon.uri.is_none());

        // Every entry in the scan is a distinct extension.
        let mut ids: Vec<String> = plugins
            .iter()
            .map(|p| crate::scanner::plugin_extension_id(&p.name, &p.source, "dsh"))
            .collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "no two rows may collide on (name, source)");
    }

    #[test]
    fn scan_plugins_carries_identity_to_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        write_profile(tmp.path(), "web", WEB_MANIFEST, Some(WEB_PATCH));
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let exts = crate::scanner::scan_plugins(&adapter);
        let row = exts.iter().find(|e| e.name == "tool-policy").unwrap();
        // The package the row instantiates rides in the description — this is
        // where the detail panel surfaces it now that the name is the row id.
        assert_eq!(
            row.description,
            "Plugin from profile web, package dsh-plugin-tool"
        );
        assert!(row.enabled);
        // Global scope only (existing scan_plugins behavior; dsh has no
        // project-level plugins). ConfigScope has no PartialEq — use matches!.
        assert!(matches!(row.scope, crate::models::ConfigScope::Global));
    }

    #[test]
    fn read_mcp_servers_from_reads_the_given_file() {
        // The service delete path locates entries via read_mcp_servers_from;
        // returning them (instead of the trait's empty default) turns dsh MCP
        // deletion into a loud DshCordis error instead of a silent no-op.
        let tmp = tempfile::tempdir().unwrap();
        write_home_patch(tmp.path(), HOME_PATCH);
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let servers = adapter.read_mcp_servers_from(&tmp.path().join(".dsh/cordis.patch.yml"));
        assert_eq!(servers.len(), 2);
    }
}
