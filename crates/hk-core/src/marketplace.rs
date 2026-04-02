use anyhow::{Context, Result};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const SMITHERY_API: &str = "https://api.smithery.ai";
const SKILLS_SH_API: &str = "https://skills.sh/api";
const AUDIT_API: &str = "https://add-skill.vercel.sh";

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client")
}

// --- Unified marketplace item ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceItem {
    pub id: String,
    pub name: String,
    pub description: String,
    /// For skills: GitHub "owner/repo" path. For MCP: Smithery qualified name.
    pub source: String,
    /// Skill identifier within a repo (for subdirectory lookup)
    pub skill_id: String,
    pub kind: String,
    pub installs: u64,
    pub icon_url: Option<String>,
    pub verified: bool,
    pub categories: Vec<String>,
    /// GitHub stars count (CLI items only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    /// Direct URL to the GitHub repo (CLI items only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
}

// --- skills.sh types ---

#[derive(Debug, Deserialize)]
struct SkillsShSearchResponse {
    skills: Vec<SkillsShSkill>,
}

#[derive(Debug, Deserialize)]
struct SkillsShSkill {
    id: String,
    #[serde(rename = "skillId")]
    skill_id: String,
    name: String,
    installs: u64,
    source: String,
}

// --- Smithery types ---

#[derive(Debug, Deserialize)]
struct SmitherySkillsResponse {
    skills: Vec<SmitherySkill>,
}

#[derive(Debug, Deserialize)]
struct SmitherySkill {
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    slug: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "gitUrl")]
    git_url: Option<String>,
    #[serde(rename = "totalActivations", default)]
    total_activations: u64,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SmitheryServersResponse {
    servers: Vec<SmitheryServer>,
}

#[derive(Debug, Deserialize)]
struct SmitheryServer {
    #[serde(rename = "qualifiedName", default)]
    qualified_name: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "iconUrl")]
    icon_url: Option<String>,
    #[serde(default)]
    verified: bool,
    #[serde(rename = "useCount", default)]
    use_count: u64,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let end = s.char_indices().take(max).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(max);
    format!("{}...", &s[..end])
}

/// Extract "owner/repo" from a GitHub URL
fn github_repo_from_url(url: &str) -> Option<String> {
    url.strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .map(|s| s.trim_end_matches('/').trim_end_matches(".git").to_string())
}

// --- Public API: Skills (via skills.sh) ---

/// Search skills via skills.sh. Returns items with source in GitHub "owner/repo" format.
pub fn search_skills(query: &str, limit: usize) -> Result<Vec<MarketplaceItem>> {
    if query.len() < 2 { return Ok(vec![]); }
    let url = format!("{SKILLS_SH_API}/search?q={}&limit={}", urlencoded(query), limit);
    let resp: SkillsShSearchResponse = client()?.get(&url).send()
        .context("Failed to reach skills.sh")?
        .json()
        .context("Failed to parse skills.sh response")?;
    Ok(resp.skills.into_iter().map(|s| MarketplaceItem {
        id: s.id.clone(),
        name: s.name,
        description: String::new(), // skills.sh doesn't return descriptions
        source: s.source,
        skill_id: s.skill_id,
        kind: "skill".into(),
        installs: s.installs,
        icon_url: None,
        verified: false,
        categories: vec![],
        stars: None,
        repo_url: None,
    }).collect())
}

// --- Public API: MCP Servers (via Smithery) ---

pub fn search_servers(query: &str, limit: usize) -> Result<Vec<MarketplaceItem>> {
    if query.len() < 2 { return Ok(vec![]); }
    let url = format!("{SMITHERY_API}/servers?q={}&pageSize={}", urlencoded(query), limit);
    let resp: SmitheryServersResponse = client()?.get(&url).send()
        .context("Failed to reach Smithery")?
        .json()
        .context("Failed to parse Smithery response")?;
    Ok(resp.servers.into_iter().map(|s| MarketplaceItem {
        id: s.qualified_name.clone(),
        name: s.display_name,
        description: truncate(&s.description, 120),
        source: s.qualified_name,
        skill_id: String::new(),
        kind: "mcp".into(),
        installs: s.use_count,
        icon_url: s.icon_url,
        verified: s.verified,
        categories: vec![],
        stars: None,
        repo_url: None,
    }).collect())
}

// --- Public API: Trending (via Smithery, mapped to skills.sh format for skills) ---

pub fn trending_skills(limit: usize) -> Result<Vec<MarketplaceItem>> {
    // Rotate through pages 1-5 based on day of year
    let page = (chrono::Utc::now().ordinal() % 5) + 1;
    let url = format!("{SMITHERY_API}/skills?pageSize={}&page={}", limit, page);
    let resp: SmitherySkillsResponse = client()?.get(&url).send()
        .context("Failed to reach Smithery")?
        .json()
        .context("Failed to parse Smithery response")?;
    Ok(resp.skills.into_iter().filter(|s| {
        // Filter out Smithery self-promotion
        s.namespace != "smithery-ai"
    }).map(|s| {
        // Derive GitHub "owner/repo" from git_url for content/audit fetching
        let github_source = s.git_url.as_deref()
            .and_then(github_repo_from_url)
            .unwrap_or_else(|| format!("{}/{}", s.namespace, s.slug));
        MarketplaceItem {
            id: format!("{}/{}", s.namespace, s.slug),
            name: s.display_name,
            description: truncate(&s.description, 120),
            source: github_source,
            // Leave skill_id empty for trending — Smithery slug != skills.sh skill_id.
            // Frontend will re-search via skills.sh to get correct source/skill_id.
            skill_id: String::new(),
            kind: "skill".into(),
            installs: s.total_activations,
            icon_url: None,
            verified: s.verified,
            categories: s.categories,
            stars: None,
            repo_url: None,
        }
    }).collect())
}

pub fn trending_servers(limit: usize) -> Result<Vec<MarketplaceItem>> {
    // Rotate through pages 1-5 based on day of year
    let page = (chrono::Utc::now().ordinal() % 5) + 1;
    let url = format!("{SMITHERY_API}/servers?pageSize={}&page={}", limit, page);
    let resp: SmitheryServersResponse = client()?.get(&url).send()
        .context("Failed to reach Smithery")?
        .json()
        .context("Failed to parse Smithery response")?;
    Ok(resp.servers.into_iter().map(|s| MarketplaceItem {
        id: s.qualified_name.clone(),
        name: s.display_name,
        description: truncate(&s.description, 120),
        source: s.qualified_name,
        skill_id: String::new(),
        kind: "mcp".into(),
        installs: s.use_count,
        icon_url: s.icon_url,
        verified: s.verified,
        categories: vec![],
        stars: None,
        repo_url: None,
    }).collect())
}

// --- Content & Audit fetching (uses GitHub source format) ---

/// Fetch SKILL.md from GitHub. source = "owner/repo", skill_id = "skill-name"
pub fn fetch_skill_content(source: &str, skill_id: &str) -> Result<String> {
    let c = client()?;
    for branch in &["main", "master"] {
        let paths = [
            format!("skills/{skill_id}/SKILL.md"),
            format!("{skill_id}/SKILL.md"),
            "SKILL.md".to_string(),
        ];
        for path in &paths {
            let url = format!("https://raw.githubusercontent.com/{source}/{branch}/{path}");
            if let Ok(resp) = c.get(&url).send() {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text() {
                        if !text.is_empty() { return Ok(text); }
                    }
                }
            }
        }
    }
    anyhow::bail!("Could not find SKILL.md for {source}/{skill_id}")
}

// --- Audit ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAuditInfo {
    pub ath: Option<AuditPartner>,
    pub socket: Option<AuditPartner>,
    pub snyk: Option<AuditPartner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPartner {
    pub risk: Option<String>,
    pub score: Option<u32>,
    pub alerts: Option<u32>,
    #[serde(rename = "analyzedAt")]
    pub analyzed_at: Option<String>,
}

/// Fetch audit info from skills.sh audit service. source = "owner/repo", skill_id = "skill-name"
pub fn fetch_audit_info(source: &str, skill_id: &str) -> Result<Option<SkillAuditInfo>> {
    let url = format!("{AUDIT_API}/audit?source={}&skills={}", urlencoded(source), urlencoded(skill_id));
    let resp = client()?.get(&url).send().context("Failed to reach audit service")?;
    if !resp.status().is_success() { return Ok(None); }
    let data: HashMap<String, SkillAuditInfo> = resp.json().unwrap_or_default();
    Ok(data.into_values().next())
}

pub fn git_url_for_source(source: &str) -> String {
    format!("https://github.com/{source}.git")
}

fn urlencoded(s: &str) -> String {
    s.replace(' ', "+").replace('&', "%26").replace('?', "%3F").replace('#', "%23")
}

// --- CLI Marketplace Registry ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliRegistryEntry {
    pub binary_name: String,
    pub display_name: String,
    pub description: String,
    pub install_command: String,
    pub skills_repo: String,
    pub skills_install_command: Option<String>,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub verified: bool,
    pub api_domains: Vec<String>,
    pub credentials_path: Option<String>,
}

struct CliRegistryCache {
    entries: Vec<CliRegistryEntry>,
    fetched_at: Instant,
}

static CLI_REGISTRY_CACHE: LazyLock<Mutex<Option<CliRegistryCache>>> =
    LazyLock::new(|| Mutex::new(None));

const CLI_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/RealZST/harnesskit-resources/main/cli-registry/registry.json";
const CLI_REGISTRY_TTL: Duration = Duration::from_secs(300); // 5 minutes

fn fetch_remote_cli_registry() -> Result<Vec<CliRegistryEntry>> {
    let resp = client()?
        .get(CLI_REGISTRY_URL)
        .send()
        .context("Failed to fetch remote CLI registry")?
        .error_for_status()
        .context("Remote CLI registry returned error status")?;
    let entries: Vec<CliRegistryEntry> = resp
        .json()
        .context("Failed to parse remote CLI registry JSON")?;
    Ok(entries)
}

/// Embedded fallback registry — used when remote fetch fails
static CLI_REGISTRY: LazyLock<Vec<CliRegistryEntry>> = LazyLock::new(|| {
    vec![
        CliRegistryEntry {
            binary_name: "wecom-cli".into(),
            display_name: "WeChat Work CLI".into(),
            description: "CLI for WeChat Work (WeCom) — team messaging, approvals, and bots".into(),
            install_command: "npm install -g @wecom/cli".into(),
            skills_repo: "WecomTeam/wecom-cli".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["collaboration".into(), "messaging".into()],
            verified: true,
            api_domains: vec!["qyapi.weixin.qq.com".into()],
            credentials_path: Some("~/.wecom/credentials.json".into()),
        },
        CliRegistryEntry {
            binary_name: "lark-cli".into(),
            display_name: "Lark / Feishu CLI".into(),
            description: "CLI for Lark (Feishu) — docs, messages, calendar, and approvals".into(),
            install_command: "npm install -g @larksuite/cli".into(),
            skills_repo: "larksuite/cli".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["collaboration".into(), "productivity".into()],
            verified: true,
            api_domains: vec!["open.feishu.cn".into(), "open.larksuite.com".into()],
            credentials_path: Some("~/.lark/credentials.json".into()),
        },
        CliRegistryEntry {
            binary_name: "dws".into(),
            display_name: "DingTalk Workspace CLI".into(),
            description: "CLI for DingTalk — workspace management, bots, and messaging".into(),
            install_command: "curl -fsSL https://raw.githubusercontent.com/DingTalk-Real-AI/dingtalk-workspace-cli/main/install.sh | sh".into(),
            skills_repo: "DingTalk-Real-AI/dingtalk-workspace-cli".into(),
            skills_install_command: Some("curl -fsSL https://raw.githubusercontent.com/DingTalk-Real-AI/dingtalk-workspace-cli/main/install-skills.sh | sh".into()),
            icon_url: None,
            categories: vec!["collaboration".into(), "messaging".into()],
            verified: true,
            api_domains: vec!["oapi.dingtalk.com".into()],
            credentials_path: Some("~/.dingtalk/credentials.json".into()),
        },
        CliRegistryEntry {
            binary_name: "meitu".into(),
            display_name: "Meitu CLI".into(),
            description: "CLI for Meitu — AI-powered image editing, filters, and batch processing".into(),
            install_command: "npm install -g meitu-cli".into(),
            skills_repo: "meitu/meitu-skills".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["image".into(), "ai".into()],
            verified: true,
            api_domains: vec!["api.meitu.com".into()],
            credentials_path: Some("~/.meitu/credentials.json".into()),
        },
        CliRegistryEntry {
            binary_name: "officecli".into(),
            display_name: "OfficeCLI".into(),
            description: "CLI for office document management — create, convert, and automate documents".into(),
            // iOfficeAI is a third-party team, not an established vendor
            install_command: "curl -fsSL https://raw.githubusercontent.com/iOfficeAI/OfficeCLI/main/install.sh | sh".into(),
            skills_repo: "iOfficeAI/OfficeCLI".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["office".into(), "documents".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
        },
        CliRegistryEntry {
            binary_name: "notion-cli".into(),
            display_name: "Notion CLI".into(),
            description: "CLI for Notion — search, view, create, and edit pages, databases, and comments via remote MCP".into(),
            install_command: "go install github.com/lox/notion-cli@latest".into(),
            skills_repo: "lox/notion-cli".into(),
            skills_install_command: Some("npx skills add lox/notion-cli".into()),
            icon_url: None,
            categories: vec!["productivity".into(), "notion".into()],
            verified: false,
            api_domains: vec!["mcp.notion.com".into()],
            credentials_path: Some("~/.config/notion-cli/token.json".into()),
        },
        CliRegistryEntry {
            binary_name: "opencli".into(),
            display_name: "OpenCLI".into(),
            description: "Universal CLI hub — turns any website, Electron app, or local tool into a command-line interface with AI-native discovery".into(),
            install_command: "npm install -g @jackwener/opencli".into(),
            skills_repo: "jackwener/opencli".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["browser-automation".into(), "cli-hub".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
        },
        CliRegistryEntry {
            binary_name: "cli-anything".into(),
            display_name: "CLI-Anything".into(),
            description: "Framework that makes any GUI software agent-native — generates CLI harnesses for GIMP, Blender, Audacity, LibreOffice, and more".into(),
            install_command: "pip install git+https://github.com/HKUDS/CLI-Anything.git".into(),
            skills_repo: "HKUDS/CLI-Anything".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["development".into(), "agent-framework".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
        },
        CliRegistryEntry {
            binary_name: "agent-browser".into(),
            display_name: "Agent Browser".into(),
            description: "Browser automation CLI for AI agents — CDP snapshots, screenshots, form interaction, and session management".into(),
            install_command: "npm install -g agent-browser".into(),
            skills_repo: "vercel-labs/agent-browser".into(),
            skills_install_command: Some("npx skills add vercel-labs/agent-browser".into()),
            icon_url: None,
            categories: vec!["browser-automation".into(), "web-testing".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: Some("~/.agent-browser/sessions/".into()),
        },
        CliRegistryEntry {
            binary_name: "rtk".into(),
            display_name: "RTK".into(),
            description: "CLI proxy that filters and compresses command output — reduces LLM token consumption by 60-90% on common dev commands".into(),
            install_command: "brew install rtk".into(),
            skills_repo: "rtk-ai/rtk".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["token-optimization".into(), "developer-tools".into(), "cli-proxy".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
        },
        CliRegistryEntry {
            binary_name: "open-pencil".into(),
            display_name: "OpenPencil".into(),
            description: "Open-source design editor CLI — inspect, lint, export, and script .fig and .pen design files with JSON output".into(),
            install_command: "bun add -g @open-pencil/cli".into(),
            skills_repo: "open-pencil/open-pencil".into(),
            skills_install_command: Some("npx skills add open-pencil/skills@open-pencil".into()),
            icon_url: None,
            categories: vec!["design-tools".into(), "figma".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
        },
    ]
});

/// Fetch GitHub stargazers_count for a repo. Cached in-process.
fn fetch_github_stars(owner_repo: &str) -> Option<u64> {
    static CACHE: LazyLock<std::sync::Mutex<HashMap<String, (std::time::Instant, Option<u64>)>>> =
        LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

    let ttl = Duration::from_secs(3600); // 1-hour cache
    if let Ok(cache) = CACHE.lock() {
        if let Some((ts, stars)) = cache.get(owner_repo) {
            if ts.elapsed() < ttl {
                return *stars;
            }
        }
    }

    let url = format!("https://api.github.com/repos/{}", owner_repo);
    let stars = client().ok()
        .and_then(|c| c.get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "HarnessKit")
            .send().ok())
        .filter(|r| r.status().is_success())
        .and_then(|r| r.json::<serde_json::Value>().ok())
        .and_then(|v| v.get("stargazers_count")?.as_u64());

    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(owner_repo.to_string(), (std::time::Instant::now(), stars));
    }
    stars
}

pub fn list_cli_registry() -> Vec<MarketplaceItem> {
    // Resolve the registry entries: try cache, then remote, then embedded fallback
    let registry = resolve_cli_registry();

    // Fetch stars for all repos in parallel to avoid serial latency
    let star_results: Vec<Option<u64>> = std::thread::scope(|s| {
        let handles: Vec<_> = registry.iter()
            .map(|entry| s.spawn(|| fetch_github_stars(&entry.skills_repo)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    });

    let mut items: Vec<MarketplaceItem> = registry.iter().zip(star_results).map(|(entry, stars)| {
        let repo_url = Some(format!("https://github.com/{}", entry.skills_repo));
        MarketplaceItem {
            id: format!("cli:{}", entry.binary_name),
            name: entry.display_name.clone(),
            description: entry.description.clone(),
            source: entry.skills_repo.clone(),
            skill_id: String::new(),
            kind: "cli".into(),
            installs: 0,
            icon_url: entry.icon_url.clone(),
            verified: entry.verified,
            categories: entry.categories.clone(),
            stars,
            repo_url,
        }
    }).collect();
    // Sort by stars descending (most popular first)
    items.sort_by(|a, b| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)));
    items
}

/// Resolve CLI registry entries: check cache, try remote fetch, fall back to embedded.
fn resolve_cli_registry() -> Vec<CliRegistryEntry> {
    // Check cache first
    if let Ok(guard) = CLI_REGISTRY_CACHE.lock() {
        if let Some(ref cache) = *guard {
            if cache.fetched_at.elapsed() < CLI_REGISTRY_TTL {
                return cache.entries.clone();
            }
        }
    }

    // Try remote fetch
    match fetch_remote_cli_registry() {
        Ok(entries) => {
            if let Ok(mut guard) = CLI_REGISTRY_CACHE.lock() {
                *guard = Some(CliRegistryCache {
                    entries: entries.clone(),
                    fetched_at: Instant::now(),
                });
            }
            entries
        }
        Err(e) => {
            eprintln!("[cli-registry] remote fetch failed, using embedded fallback: {e}");
            CLI_REGISTRY.clone()
        }
    }
}

pub fn get_cli_registry_entry(binary_name: &str) -> Option<CliRegistryEntry> {
    resolve_cli_registry()
        .into_iter()
        .find(|e| e.binary_name == binary_name)
}

/// Look up a CLI entry from the EMBEDDED registry only (not remote).
/// Used by install_cli to prevent RCE via compromised remote registry.
pub fn get_embedded_cli_entry(binary_name: &str) -> Option<CliRegistryEntry> {
    CLI_REGISTRY.iter().find(|e| e.binary_name == binary_name).cloned()
}
