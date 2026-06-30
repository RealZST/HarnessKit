use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use hk_core::{
    adapter,
    manager,
    models::*,
    scanner,
    service,
    store::Store,
};
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
        /// List subcommand (e.g., "agents")
        sub: Option<String>,
    },
    /// Show extension details
    Info {
        /// Extension name
        name: String,
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

    if let Commands::Serve { port, host, token, no_token, name } = cli.command {
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
            sub,
        } => {
            if sub.as_deref() == Some("agents") {
                cmd_list_agents(&adapters)
            } else {
                let kind_filter = kind.as_deref().and_then(|k| k.parse().ok());
                cmd_list(
                    &store,
                    kind_filter,
                    agent.as_deref(),
                    pack.as_deref(),
                    &extensions,
                )
            }
        }
        Commands::Info { name } => cmd_info(&extensions, &name),
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
        if parts.len() >= 3 { parts[2].to_string() } else { ext.name.clone() }
    } else {
        ext.name.clone()
    };
    let developer = ext.source.url.as_deref()
        .and_then(|u| {
            // Extract "owner/repo" from URL
            let u = u.trim_end_matches('/').trim_end_matches(".git");
            let parts: Vec<&str> = u.rsplitn(3, '/').collect();
            if parts.len() >= 2 { Some(format!("{}/{}", parts[1], parts[0])) } else { None }
        })
        .unwrap_or_default();
    format!("{}\0{}\0{}\0{}", ext.kind.as_str(), name, ext.source.origin.as_str(), developer)
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
) -> Result<()> {
    let mut seen_groups = std::collections::HashSet::new();
    let filtered: Vec<&Extension> = extensions
        .iter()
        .filter(|e| seen_groups.insert(group_key(e)))
        .filter(|e| kind.is_none() || Some(e.kind) == kind)
        .filter(|e| agent.is_none() || e.agents.iter().any(|a| a == agent.unwrap()))
        .filter(|e| pack.is_none() || e.pack.as_deref() == pack)
        .collect();

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Name", "Kind", "Agent", "Source", "Score", "Status"]);

    for ext in &filtered {
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
    println!("\n  {} {}", filtered.len().to_string().bold(), "results".dimmed());
    println!("{table}");
    Ok(())
}

fn cmd_list_agents(adapters: &[Box<dyn adapter::AgentAdapter>]) -> Result<()> {
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

fn cmd_info(extensions: &[Extension], name: &str) -> Result<()> {
    let ext = extensions
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("Extension not found: {name}"))?;
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
    let low_risk = sorted.iter().filter(|g| g.trust_score >= 60 && g.trust_score < 80).count();
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
