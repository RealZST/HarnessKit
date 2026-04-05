use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use hk_core::{
    adapter,
    auditor::{AuditInput, Auditor},
    manager,
    models::*,
    scanner,
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = hk_data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let store = Store::open(&data_dir.join("metadata.db"))?;
    let adapters = adapter::all_adapters();

    // Sync: scan all agents and upsert into store
    let scanned = scanner::scan_all(&adapters);
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
        } => cmd_audit(
            &extensions,
            name.as_deref(),
            kind.as_deref(),
            severity.as_deref(),
        ),
        Commands::Enable { name, pack } => {
            cmd_toggle(&store, &extensions, name.as_deref(), pack.as_deref(), true)
        }
        Commands::Disable { name, pack } => {
            cmd_toggle(&store, &extensions, name.as_deref(), pack.as_deref(), false)
        }
    }
}

fn hk_data_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".harnesskit")
}

fn cmd_status(
    _store: &Store,
    adapters: &[Box<dyn adapter::AgentAdapter>],
    extensions: &[Extension],
) -> Result<()> {
    let skills = extensions
        .iter()
        .filter(|e| e.kind == ExtensionKind::Skill)
        .count();
    let mcps = extensions
        .iter()
        .filter(|e| e.kind == ExtensionKind::Mcp)
        .count();
    let plugins = extensions
        .iter()
        .filter(|e| e.kind == ExtensionKind::Plugin)
        .count();
    let hooks = extensions
        .iter()
        .filter(|e| e.kind == ExtensionKind::Hook)
        .count();
    let detected: Vec<&str> = adapters
        .iter()
        .filter(|a| a.detect())
        .map(|a| a.name())
        .collect();

    println!();
    println!("  {} v0.1.0", "HarnessKit".bold());
    println!();
    println!(
        "  {}    {} total ({} skills · {} mcp · {} plugins · {} hooks)",
        "Extensions".dimmed(),
        extensions.len(),
        skills,
        mcps,
        plugins,
        hooks
    );
    println!(
        "  {}        {} detected ({})",
        "Agents".dimmed(),
        detected.len(),
        detected.join(" · ")
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
    let filtered: Vec<&Extension> = extensions
        .iter()
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
    extensions: &[Extension],
    name: Option<&str>,
    _kind: Option<&str>,
    _severity: Option<&str>,
) -> Result<()> {
    let auditor = Auditor::new();
    let targets: Vec<&Extension> = if let Some(n) = name {
        extensions.iter().filter(|e| e.name == n).collect()
    } else {
        extensions.iter().collect()
    };

    for ext in targets {
        let input = AuditInput {
            extension_id: ext.id.clone(),
            kind: ext.kind,
            name: ext.name.clone(),
            content: String::new(),
            source: ext.source.clone(),
            file_path: ext.name.clone(),
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
        };
        let result = auditor.audit(&input);
        println!();
        println!(
            "  {} Trust Score: {}",
            ext.name.bold(),
            format_score(result.trust_score)
        );
        if result.findings.is_empty() {
            println!("  {}", "No issues found".green());
        }
        for finding in &result.findings {
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
