use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use hk_core::{adapter, manager, models::*, scanner, service, store::Store};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "hk",
    about = "HarnessKit — manage your AI agent extensions",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show status overview
    Status,
    /// List extensions
    List {
        /// Filter by kind: skill, mcp, plugin, hook
        #[arg(long)]
        kind: Option<String>,
        /// Filter by agent name
        #[arg(long)]
        agent: Option<String>,
        /// Filter by source pack (owner/repo)
        #[arg(long)]
        pack: Option<String>,
        /// Print extension inventory as JSON
        #[arg(long)]
        json: bool,
        /// List subcommand (e.g., "agents")
        sub: Option<String>,
    },
    /// Show extension details
    Info {
        /// Extension name
        name: String,
        /// Print extension details as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run security audit
    Audit {
        /// Audit a specific extension by name
        name: Option<String>,
        /// Filter by kind
        #[arg(long)]
        kind: Option<String>,
        /// Filter by minimum severity
        #[arg(long)]
        severity: Option<String>,
        /// Skip the initial scan and sync (use existing DB state)
        #[arg(long)]
        no_scan: bool,
    },
    /// Enable an extension (or all in a pack)
    Enable {
        /// Extension name
        name: Option<String>,
        /// Enable all extensions in a pack (owner/repo)
        #[arg(long)]
        pack: Option<String>,
    },
    /// Disable an extension (or all in a pack)
    Disable {
        /// Extension name
        name: Option<String>,
        /// Disable all extensions in a pack (owner/repo)
        #[arg(long)]
        pack: Option<String>,
    },
    /// Start the web UI server
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "7070")]
        port: u16,

        /// Bind address (127.0.0.1 = local only, 0.0.0.0 = all interfaces)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Access token. If omitted, a persistent token is auto-generated and
        /// stored at ~/.harnesskit/web-token (mode 0600). Prefer this over
        /// passing --token on shared hosts: command-line args are visible to
        /// other users via `ps`/`/proc/<pid>/cmdline`.
        #[arg(long)]
        token: Option<String>,

        /// Disable authentication entirely (no token). Only safe on a trusted
        /// single-user machine — on a shared host (e.g. an HPC login node)
        /// loopback is not isolated per-user, so any local user could connect.
        #[arg(long)]
        no_token: bool,

        /// Node label shown in the web UI (defaults to the machine hostname).
        /// Useful when opening multiple tabs against different remote nodes.
        #[arg(long)]
        name: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Serve {
        port,
        host,
        token,
        no_token,
        name,
    } = cli.command
    {
        let effective_token = resolve_serve_token(token, no_token);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(hk_web::serve(hk_web::ServeOptions {
            port,
            host,
            token: effective_token,
            name,
        }))?;
        return Ok(());
    }

    let data_dir = hk_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let store = Store::open(&data_dir.join("metadata.db"))?;
    let adapters = adapter::all_adapters();

    // Sync: scan all agents and upsert into store
    let projects = store.list_project_tuples();
    let scanned = scanner::scan_all(&adapters, &projects);
    store.sync_extensions(&scanned)?;
    // Read back from DB so we get backfilled fields (e.g. pack from install_url)
    let extensions = store.list_extensions(None, None)?;

    match cli.command {
        Commands::Status => cmd_status(&store, &adapters, &extensions),
        Commands::List {
            kind,
            agent,
            pack,
            json,
            sub,
        } => {
            if sub.as_deref() == Some("agents") {
                cmd_list_agents(&adapters, json)
            } else {
                let kind_filter = kind.as_deref().and_then(|k| k.parse().ok());
                cmd_list(
                    &store,
                    kind_filter,
                    agent.as_deref(),
                    pack.as_deref(),
                    &extensions,
                    json,
                )
            }
        }
        Commands::Info { name, json } => cmd_info(&extensions, &name, json),
        Commands::Audit {
            name,
            kind,
            severity,
            no_scan,
        } => cmd_audit(
            &store,
            &adapters,
            name.as_deref(),
            kind.as_deref(),
            severity.as_deref(),
            no_scan,
        ),
        Commands::Enable { name, pack } => {
            cmd_toggle(&store, &extensions, name.as_deref(), pack.as_deref(), true)
        }
        Commands::Disable { name, pack } => {
            cmd_toggle(&store, &extensions, name.as_deref(), pack.as_deref(), false)
        }
        Commands::Serve { .. } => unreachable!("handled above"),
    }
}

fn hk_data_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".harnesskit")
}

/// Resolve the web access token for `hk serve`.
///
/// Secure-by-default: with neither `--token` nor `--no-token`, a persistent
/// token is loaded (or generated) from `~/.harnesskit/web-token` and enforced
/// even for `127.0.0.1` binds. On a shared host (HPC login node) the loopback
/// interface is not isolated per-user, so binding `127.0.0.1` alone does not
/// keep other local users out — only the token does.
fn resolve_serve_token(explicit: Option<String>, no_token: bool) -> Option<String> {
    if no_token {
        return None;
    }
    if let Some(token) = explicit {
        return Some(token);
    }
    Some(load_or_create_token())
}

/// Generate a 128-bit random token rendered as 32 hex chars.
fn generate_token() -> String {
    use rand::Rng;
    let token_value: u128 = rand::rng().random();
    format!("{token_value:032x}")
}

/// Load the persisted token from `~/.harnesskit/web-token`, or create one.
///
/// The file is created/rewritten with mode `0600` (owner-only). An existing
/// file is reused only if it is non-empty and owner-only — if its permissions
/// are looser than `0600` (e.g. left world-readable by an older version or a
/// hostile pre-creation), the token is treated as compromised and regenerated.
/// Falls back to an in-memory token if the file cannot be persisted.
fn load_or_create_token() -> String {
    let path = hk_data_dir().join("web-token");

    if let Some(token) = read_token_if_secure(&path) {
        return token;
    }

    let token = generate_token();
    if let Err(err) = write_token_0600(&path, &token) {
        eprintln!(
            "warning: could not persist web token to {}: {err}. Using a one-time token for this run.",
            path.display()
        );
    }
    token
}

/// Return the stored token only if the file exists, is non-empty, and (on Unix)
/// is owner-only (`0600`). Returns `None` otherwise so the caller regenerates.
fn read_token_if_secure(path: &std::path::Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Reject anything readable/writable by group or others.
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return None;
        }
    }
    let token = std::fs::read_to_string(path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

/// Write the token with owner-only permissions, creating the data dir first.
fn write_token_0600(path: &std::path::Path, token: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;

    // `OpenOptions::mode` is ignored when the file already exists, so set the
    // permissions explicitly to harden a pre-existing, looser-permissioned file.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    file.write_all(token.as_bytes())?;
    Ok(())
}

/// Build a grouping key matching the desktop's `extensionGroupKey`:
/// `kind \0 name \0 origin \0 developer`
/// For hooks, strip event/matcher prefix and keep only the command part.
fn group_key(ext: &Extension) -> String {
    let name = if ext.kind == ExtensionKind::Hook {
        // Hook name format: "event:matcher:command" — extract just the command
        let parts: Vec<&str> = ext.name.splitn(3, ':').collect();
        if parts.len() >= 3 {
            parts[2].to_string()
        } else {
            ext.name.clone()
        }
    } else {
        ext.name.clone()
    };
    let developer = ext
        .source
        .url
        .as_deref()
        .and_then(|u| {
            // Extract "owner/repo" from URL
            let u = u.trim_end_matches('/').trim_end_matches(".git");
            let parts: Vec<&str> = u.rsplitn(3, '/').collect();
            if parts.len() >= 2 {
                Some(format!("{}/{}", parts[1], parts[0]))
            } else {
                None
            }
        })
        .unwrap_or_default();
    format!(
        "{}\0{}\0{}\0{}",
        ext.kind.as_str(),
        name,
        ext.source.origin.as_str(),
        developer
    )
}

#[derive(Serialize)]
struct ListJsonOutput<'a> {
    schema_version: u8,
    command: &'static str,
    semantics: &'static str,
    filters: ListJsonFilters<'a>,
    count: usize,
    rows: Vec<ListJsonRow<'a>>,
}

#[derive(Serialize)]
struct ListJsonFilters<'a> {
    kind: Option<&'static str>,
    agent: Option<&'a str>,
    pack: Option<&'a str>,
}

#[derive(Serialize)]
struct ListJsonRow<'a> {
    name: &'a str,
    kind: &'static str,
    agents: &'a [String],
    pack: Option<&'a str>,
    trust_score: Option<u8>,
    enabled: bool,
    status: &'static str,
}

#[derive(Serialize)]
struct InfoJsonOutput<'a> {
    schema_version: u8,
    command: &'static str,
    match_semantics: &'static str,
    extension: InfoJsonExtension<'a>,
}

#[derive(Serialize)]
struct InfoJsonExtension<'a> {
    id: &'a str,
    name: &'a str,
    kind: &'static str,
    agents: &'a [String],
    enabled: bool,
    pack: Option<&'a str>,
    source: InfoJsonSource<'a>,
    source_path: Option<&'a str>,
    install_meta: Option<InfoJsonInstallMeta<'a>>,
    trust_score: Option<u8>,
    update_eligible: bool,
    update_reason: &'static str,
}

#[derive(Serialize)]
struct InfoJsonSource<'a> {
    origin: &'static str,
    url: Option<&'a str>,
    version: Option<&'a str>,
    commit_hash: Option<&'a str>,
    from_manifest: bool,
}

#[derive(Serialize)]
struct InfoJsonInstallMeta<'a> {
    install_type: &'a str,
    url: Option<&'a str>,
    url_resolved: Option<&'a str>,
    branch: Option<&'a str>,
    subpath: Option<&'a str>,
    revision: Option<&'a str>,
}

fn cmd_status(
    _store: &Store,
    adapters: &[Box<dyn adapter::AgentAdapter>],
    extensions: &[Extension],
) -> Result<()> {
    // Group extensions the same way the desktop does, skipping CLI children
    let mut groups = std::collections::HashSet::new();
    let mut skills = 0u32;
    let mut mcps = 0u32;
    let mut plugins = 0u32;
    let mut hooks = 0u32;
    let mut clis = 0u32;

    for ext in extensions {
        let key = group_key(ext);
        if !groups.insert(key) {
            continue;
        }
        match ext.kind {
            ExtensionKind::Skill => skills += 1,
            ExtensionKind::Mcp => mcps += 1,
            ExtensionKind::Plugin => plugins += 1,
            ExtensionKind::Hook => hooks += 1,
            ExtensionKind::Cli => clis += 1,
        }
    }
    let total = groups.len();

    let detected: Vec<&str> = adapters
        .iter()
        .filter(|a| a.detect())
        .map(|a| a.name())
        .collect();

    println!();
    println!(
        "  {}        {} detected ({})",
        "Agents".dimmed(),
        detected.len(),
        detected.join(" · ")
    );
    println!(
        "  {}    {} total ({} skills · {} mcp · {} plugins · {} hooks · {} clis)",
        "Extensions".dimmed(),
        total,
        skills,
        mcps,
        plugins,
        hooks,
        clis
    );
    println!();
    Ok(())
}

fn cmd_list(
    _store: &Store,
    kind: Option<ExtensionKind>,
    agent: Option<&str>,
    pack: Option<&str>,
    extensions: &[Extension],
    json: bool,
) -> Result<()> {
    let filtered = select_grouped_list_rows(extensions, kind, agent, pack);

    if json {
        let output = build_list_json_output(kind, agent, pack, &filtered);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    print_list_table(&filtered);
    Ok(())
}

fn select_grouped_list_rows<'a>(
    extensions: &'a [Extension],
    kind: Option<ExtensionKind>,
    agent: Option<&str>,
    pack: Option<&str>,
) -> Vec<&'a Extension> {
    let mut seen_groups = std::collections::HashSet::new();
    extensions
        .iter()
        .filter(|e| seen_groups.insert(group_key(e)))
        .filter(|e| kind.is_none() || Some(e.kind) == kind)
        .filter(|e| agent.is_none() || e.agents.iter().any(|a| a == agent.unwrap()))
        .filter(|e| pack.is_none() || e.pack.as_deref() == pack)
        .collect()
}

fn build_list_json_output<'a>(
    kind: Option<ExtensionKind>,
    agent: Option<&'a str>,
    pack: Option<&'a str>,
    rows: &[&'a Extension],
) -> ListJsonOutput<'a> {
    let rows: Vec<ListJsonRow<'a>> = rows
        .iter()
        .map(|ext| ListJsonRow {
            name: &ext.name,
            kind: ext.kind.as_str(),
            agents: &ext.agents,
            pack: ext.pack.as_deref(),
            trust_score: ext.trust_score,
            enabled: ext.enabled,
            status: if ext.enabled { "enabled" } else { "disabled" },
        })
        .collect();

    ListJsonOutput {
        schema_version: 1,
        command: "list",
        semantics: "grouped",
        filters: ListJsonFilters {
            kind: kind.map(|kind| kind.as_str()),
            agent,
            pack,
        },
        count: rows.len(),
        rows,
    }
}

fn print_list_table(filtered: &[&Extension]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Name", "Kind", "Agent", "Source", "Score", "Status"]);

    for ext in filtered {
        let score_str = ext
            .trust_score
            .map(format_score)
            .unwrap_or_else(|| "—".dimmed().to_string());
        let status = if ext.enabled {
            "enabled".green().to_string()
        } else {
            "disabled".red().to_string()
        };
        let source = ext.pack.as_deref().unwrap_or("—");
        table.add_row(vec![
            &ext.name,
            ext.kind.as_str(),
            &ext.agents.join(", "),
            source,
            &score_str,
            &status,
        ]);
    }
    println!(
        "\n  {} {}",
        filtered.len().to_string().bold(),
        "results".dimmed()
    );
    println!("{table}");
}

fn cmd_list_agents(adapters: &[Box<dyn adapter::AgentAdapter>], json: bool) -> Result<()> {
    if json {
        return Err(anyhow::anyhow!(
            "--json is only supported for extension list output, not list agents"
        ));
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["Agent", "Detected"]);
    for adapter in adapters {
        let status = if adapter.detect() {
            "yes".green().to_string()
        } else {
            "no".red().to_string()
        };
        table.add_row(vec![adapter.name(), &status]);
    }
    println!("{table}");
    Ok(())
}

fn cmd_info(extensions: &[Extension], name: &str, json: bool) -> Result<()> {
    let ext = select_info_extension(extensions, name)?;

    if json {
        let output = build_info_json_output(ext);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!();
    println!("  {} {}", "Name:".dimmed(), ext.name.bold());
    println!("  {} {}", "Kind:".dimmed(), ext.kind.as_str());
    println!("  {} {}", "Agents:".dimmed(), ext.agents.join(", "));
    println!("  {} {}", "Enabled:".dimmed(), ext.enabled);
    println!("  {} {}", "Source:".dimmed(), ext.source.origin.as_str());
    if let Some(url) = &ext.source.url {
        println!("  {} {}", "URL:".dimmed(), url);
    }
    println!(
        "  {} {}",
        "Installed:".dimmed(),
        ext.installed_at.format("%Y-%m-%d %H:%M")
    );
    if let Some(score) = ext.trust_score {
        println!("  {} {}", "Trust Score:".dimmed(), format_score(score));
    }
    println!();
    Ok(())
}

fn select_info_extension<'a>(extensions: &'a [Extension], name: &str) -> Result<&'a Extension> {
    extensions
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("Extension not found: {name}"))
}

fn build_info_json_output(ext: &Extension) -> InfoJsonOutput<'_> {
    let install_meta = ext.install_meta.as_ref().map(|meta| InfoJsonInstallMeta {
        install_type: &meta.install_type,
        url: meta.url.as_deref(),
        url_resolved: meta.url_resolved.as_deref(),
        branch: meta.branch.as_deref(),
        subpath: meta.subpath.as_deref(),
        revision: meta.revision.as_deref(),
    });

    InfoJsonOutput {
        schema_version: 1,
        command: "info",
        match_semantics: "first_name_match",
        extension: InfoJsonExtension {
            id: &ext.id,
            name: &ext.name,
            kind: ext.kind.as_str(),
            agents: &ext.agents,
            enabled: ext.enabled,
            pack: ext.pack.as_deref(),
            source: InfoJsonSource {
                origin: ext.source.origin.as_str(),
                url: ext.source.url.as_deref(),
                version: ext.source.version.as_deref(),
                commit_hash: ext.source.commit_hash.as_deref(),
                from_manifest: ext.source.from_manifest,
            },
            source_path: ext.source_path.as_deref(),
            install_meta,
            trust_score: ext.trust_score,
            update_eligible: service::is_update_eligible(ext),
            update_reason: update_reason(ext),
        },
    }
}

fn update_reason(ext: &Extension) -> &'static str {
    if ext.kind != ExtensionKind::Skill {
        return "not_skill";
    }

    match &ext.scope {
        ConfigScope::Global => "global_skill",
        ConfigScope::Project { .. } if ext.install_meta.is_some() => {
            "project_skill_with_install_meta"
        }
        ConfigScope::Project { .. } => "project_skill_without_install_meta",
    }
}

fn cmd_audit(
    store: &Store,
    adapters: &[Box<dyn adapter::AgentAdapter>],
    name: Option<&str>,
    _kind: Option<&str>,
    _severity: Option<&str>,
    no_scan: bool,
) -> Result<()> {
    // When --no-scan is set, skip the scan_and_sync that main() already did
    if !no_scan {
        let projects = store.list_project_tuples();
        let scanned = scanner::scan_all(adapters, &projects);
        store.sync_extensions(&scanned)?;
    }

    let results = service::run_full_audit(store, adapters)?;
    let extensions = store.list_extensions(None, None)?;

    // Build a map from extension_id -> extension for display
    let ext_map: std::collections::HashMap<&str, &Extension> =
        extensions.iter().map(|e| (e.id.as_str(), e)).collect();

    // Group audit results by extension group key (same logic as desktop)
    struct GroupedAudit {
        name: String,
        trust_score: u8,
        findings: Vec<AuditFinding>,
    }
    let mut groups: std::collections::HashMap<String, GroupedAudit> =
        std::collections::HashMap::new();

    for result in &results {
        let ext = match ext_map.get(result.extension_id.as_str()) {
            Some(e) => e,
            None => continue,
        };
        let key = group_key(ext);
        let group = groups.entry(key).or_insert_with(|| GroupedAudit {
            name: ext.name.clone(),
            trust_score: result.trust_score,
            findings: Vec::new(),
        });
        // Use the minimum trust score across agents
        group.trust_score = group.trust_score.min(result.trust_score);
        // Merge findings, dedup by message+location
        let mut seen: std::collections::HashSet<String> = group
            .findings
            .iter()
            .map(|f| format!("{}\0{}", f.message, f.location))
            .collect();
        for finding in &result.findings {
            let key = format!("{}\0{}", finding.message, finding.location);
            if seen.insert(key) {
                group.findings.push(finding.clone());
            }
        }
    }

    // Sort by trust score ascending (worst first)
    let mut sorted: Vec<_> = groups.into_values().collect();
    sorted.sort_by(|a, b| a.trust_score.cmp(&b.trust_score));

    // Filter by name if specified
    if let Some(n) = name {
        sorted.retain(|g| g.name == n);
    }

    // Summary
    let total = sorted.len();
    let safe = sorted.iter().filter(|g| g.trust_score >= 80).count();
    let low_risk = sorted
        .iter()
        .filter(|g| g.trust_score >= 60 && g.trust_score < 80)
        .count();
    let needs_review = sorted.iter().filter(|g| g.trust_score < 60).count();
    println!();
    println!(
        "  {} {} ({} {} · {} {} · {} {})",
        total.to_string().bold(),
        "extensions scanned".dimmed(),
        safe.to_string().green(),
        "safe".green(),
        low_risk.to_string().yellow(),
        "low risk".yellow(),
        needs_review.to_string().red(),
        "needs review".red(),
    );

    for group in &sorted {
        println!();
        println!(
            "  {} Trust Score: {}",
            group.name.bold(),
            format_score(group.trust_score)
        );
        if group.findings.is_empty() {
            println!("  {}", "No issues found".green());
        }
        for finding in &group.findings {
            let sev_str = match finding.severity {
                Severity::Critical => "CRITICAL".red().bold().to_string(),
                Severity::High => "HIGH".yellow().bold().to_string(),
                Severity::Medium => "MEDIUM".yellow().to_string(),
                Severity::Low => "LOW".dimmed().to_string(),
            };
            println!("  {} {}", sev_str, finding.message);
            if !finding.location.is_empty() {
                println!("       {} {}", "└─".dimmed(), finding.location.dimmed());
            }
        }
    }
    println!();
    Ok(())
}

fn cmd_toggle(
    store: &Store,
    extensions: &[Extension],
    name: Option<&str>,
    pack: Option<&str>,
    enabled: bool,
) -> Result<()> {
    if let Some(pack_name) = pack {
        let targets: Vec<&Extension> = extensions
            .iter()
            .filter(|e| e.pack.as_deref() == Some(pack_name))
            .collect();
        if targets.is_empty() {
            return Err(anyhow::anyhow!(
                "No extensions found with source: {pack_name}"
            ));
        }
        for ext in &targets {
            manager::toggle_extension(store, &ext.id, enabled)?;
        }
        let action = if enabled { "Enabled" } else { "Disabled" };
        println!(
            "{} {} extensions from source '{}'",
            action.green(),
            targets.len(),
            pack_name
        );
        return Ok(());
    }

    let name = name.ok_or_else(|| anyhow::anyhow!("Either --pack or a name is required"))?;
    let ext = extensions
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("Extension not found: {name}"))?;
    manager::toggle_extension(store, &ext.id, enabled)?;
    let action = if enabled { "Enabled" } else { "Disabled" };
    println!("{} {}", action.green(), name);
    Ok(())
}

fn format_score(score: u8) -> String {
    let tier = TrustTier::from_score(score);
    match tier {
        TrustTier::Safe => format!("{score}").green().to_string(),
        TrustTier::LowRisk => format!("{score}").yellow().to_string(),
        TrustTier::NeedsReview => format!("{score}").truecolor(255, 165, 0).to_string(),
    }
}

#[cfg(test)]
mod cli_json_tests {
    use super::*;
    use clap::Parser;
    use serde_json::{Value, json};

    fn extension(id: &str, name: &str) -> Extension {
        serde_json::from_value(json!({
            "id": id,
            "kind": "skill",
            "name": name,
            "description": "test extension",
            "source": {
                "origin": "git",
                "url": "https://github.com/acme/tools.git",
                "version": "1.2.3",
                "commit_hash": "abc123",
                "from_manifest": true
            },
            "agents": ["codex"],
            "tags": [],
            "pack": "acme/tools",
            "permissions": [],
            "enabled": true,
            "trust_score": 91,
            "installed_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "source_path": "/tmp/acme-tools",
            "cli_parent_id": null,
            "cli_meta": null,
            "install_meta": null,
            "scope": { "type": "global" }
        }))
        .unwrap()
    }

    fn install_meta() -> InstallMeta {
        serde_json::from_value(json!({
            "install_type": "git",
            "url": "https://github.com/acme/tools.git",
            "url_resolved": "https://github.com/acme/tools.git",
            "branch": "main",
            "subpath": "skills/demo",
            "revision": "abc123",
            "remote_revision": "def456",
            "checked_at": "2026-02-03T04:05:06Z",
            "check_error": "network timeout"
        }))
        .unwrap()
    }

    fn as_value<T: Serialize>(value: &T) -> Value {
        serde_json::to_value(value).unwrap()
    }

    #[test]
    fn parse_list_json_flag() {
        let cli = Cli::try_parse_from(["hk", "list", "--json"]).unwrap();

        match cli.command {
            Commands::List { json, .. } => assert!(json),
            _ => panic!("expected list command"),
        }
    }

    #[test]
    fn parse_list_agents_json_flag() {
        let cli = Cli::try_parse_from(["hk", "list", "agents", "--json"]).unwrap();

        match cli.command {
            Commands::List { sub, json, .. } => {
                assert_eq!(sub.as_deref(), Some("agents"));
                assert!(json);
            }
            _ => panic!("expected list command"),
        }
    }

    #[test]
    fn parse_info_json_flag() {
        let cli = Cli::try_parse_from(["hk", "info", "demo", "--json"]).unwrap();

        match cli.command {
            Commands::Info { json, .. } => assert!(json),
            _ => panic!("expected info command"),
        }
    }

    #[test]
    fn list_json_uses_grouped_table_semantics() {
        let first = extension("first", "demo");
        let mut duplicate = extension("duplicate", "demo");
        duplicate.agents = vec!["claude".into()];
        let second = extension("second", "other");
        let extensions = vec![first, duplicate, second];

        let rows = select_grouped_list_rows(&extensions, None, None, None);
        let output = build_list_json_output(None, None, None, &rows);
        let value = as_value(&output);

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["command"], "list");
        assert_eq!(value["semantics"], "grouped");
        assert_eq!(value["count"], 2);
        assert_eq!(value["rows"][0]["name"], "demo");
        assert_eq!(value["rows"][0]["agents"], json!(["codex"]));
        assert_eq!(value["rows"][1]["name"], "other");
    }

    #[test]
    fn list_json_preserves_dedupe_before_filter_order() {
        let mut first = extension("first", "demo");
        first.agents = vec!["claude".into()];
        let mut second = extension("second", "demo");
        second.agents = vec!["codex".into()];
        let extensions = vec![first, second];

        let rows = select_grouped_list_rows(&extensions, None, Some("codex"), None);

        assert!(rows.is_empty());
    }

    #[test]
    fn list_json_omits_instance_level_fields() {
        let ext = extension("first", "demo");
        let rows = vec![&ext];
        let output = build_list_json_output(None, None, None, &rows);
        let value = as_value(&output);
        let row = value["rows"][0].as_object().unwrap();

        for forbidden in [
            "id",
            "source_path",
            "source",
            "install_meta",
            "update_eligible",
            "update_reason",
            "remote_revision",
            "checked_at",
            "check_error",
            "instances",
        ] {
            assert!(!row.contains_key(forbidden), "row leaked {forbidden}");
        }
    }

    #[test]
    fn list_agents_json_returns_unsupported_error() {
        let err = cmd_list_agents(&[], true).unwrap_err();

        assert_eq!(
            err.to_string(),
            "--json is only supported for extension list output, not list agents"
        );
    }

    #[test]
    fn info_json_uses_first_name_match() {
        let first = extension("first", "demo");
        let second = extension("second", "demo");
        let extensions = vec![first, second];
        let selected = select_info_extension(&extensions, "demo").unwrap();
        let output = build_info_json_output(selected);
        let value = as_value(&output);

        assert_eq!(value["match_semantics"], "first_name_match");
        assert_eq!(value["extension"]["id"], "first");
    }

    #[test]
    fn info_json_includes_provenance_without_update_status() {
        let mut ext = extension("first", "demo");
        ext.install_meta = Some(install_meta());
        let output = build_info_json_output(&ext);
        let value = as_value(&output);
        let extension = value["extension"].as_object().unwrap();
        let source = value["extension"]["source"].as_object().unwrap();
        let install_meta = value["extension"]["install_meta"].as_object().unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["command"], "info");
        assert_eq!(source["origin"], "git");
        assert_eq!(source["url"], "https://github.com/acme/tools.git");
        assert_eq!(source["version"], "1.2.3");
        assert_eq!(source["commit_hash"], "abc123");
        assert_eq!(source["from_manifest"], true);
        assert_eq!(install_meta["install_type"], "git");
        assert_eq!(install_meta["url"], "https://github.com/acme/tools.git");
        assert_eq!(
            install_meta["url_resolved"],
            "https://github.com/acme/tools.git"
        );
        assert_eq!(install_meta["branch"], "main");
        assert_eq!(install_meta["subpath"], "skills/demo");
        assert_eq!(install_meta["revision"], "abc123");
        assert_eq!(extension["update_eligible"], true);
        assert_eq!(extension["update_reason"], "global_skill");

        for forbidden in [
            "remote_revision",
            "checked_at",
            "check_error",
            "status",
            "installed_at",
            "updated_at",
        ] {
            assert!(
                !extension.contains_key(forbidden),
                "extension leaked {forbidden}"
            );
            assert!(
                !install_meta.contains_key(forbidden),
                "install_meta leaked {forbidden}"
            );
        }
    }

    #[test]
    fn info_json_marks_non_skill_as_not_update_eligible() {
        let mut ext = extension("first", "demo");
        ext.kind = ExtensionKind::Mcp;

        let output = build_info_json_output(&ext);
        let value = as_value(&output);

        assert_eq!(value["extension"]["update_eligible"], false);
        assert_eq!(value["extension"]["update_reason"], "not_skill");
    }

    #[test]
    fn update_reason_global_skill() {
        let ext = extension("first", "demo");

        assert_eq!(update_reason(&ext), "global_skill");
    }

    #[test]
    fn update_reason_project_skill_with_install_meta() {
        let mut ext = extension("first", "demo");
        ext.scope = ConfigScope::Project {
            name: "Repo".into(),
            path: "/tmp/repo".into(),
        };
        ext.install_meta = Some(install_meta());

        assert_eq!(update_reason(&ext), "project_skill_with_install_meta");
    }

    #[test]
    fn update_reason_project_skill_without_install_meta() {
        let mut ext = extension("first", "demo");
        ext.scope = ConfigScope::Project {
            name: "Repo".into(),
            path: "/tmp/repo".into(),
        };

        assert_eq!(update_reason(&ext), "project_skill_without_install_meta");
    }

    #[test]
    fn update_reason_not_skill() {
        let mut ext = extension("first", "demo");
        ext.kind = ExtensionKind::Mcp;

        assert_eq!(update_reason(&ext), "not_skill");
    }
}

#[cfg(test)]
mod token_persistence_tests {
    use super::{read_token_if_secure, write_token_0600};

    /// Write persists the token and read reads it back; on Unix the file is
    /// owner-only (0600).
    #[test]
    fn write_persists_token_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("web-token");

        write_token_0600(&path, "deadbeef").unwrap();

        assert_eq!(read_token_if_secure(&path).as_deref(), Some("deadbeef"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "token file must be owner-only");
        }
    }

    /// A token file readable by group or others is refused, so the caller
    /// regenerates rather than trusting a token other users could have read.
    #[cfg(unix)]
    #[test]
    fn read_rejects_group_or_world_readable_token() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("web-token");
        std::fs::write(&path, "deadbeef").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert_eq!(read_token_if_secure(&path), None);
    }

    /// Overwriting an existing token file resets its permissions to owner-only,
    /// even if it was previously group/world-readable.
    #[cfg(unix)]
    #[test]
    fn write_rehardens_preexisting_loose_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("web-token");
        std::fs::write(&path, "old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_token_0600(&path, "new").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(read_token_if_secure(&path).as_deref(), Some("new"));
    }
}
