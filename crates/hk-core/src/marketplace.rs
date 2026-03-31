use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

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
    let url = format!("{SMITHERY_API}/skills?pageSize={}", limit);
    let resp: SmitherySkillsResponse = client()?.get(&url).send()
        .context("Failed to reach Smithery")?
        .json()
        .context("Failed to parse Smithery response")?;
    Ok(resp.skills.into_iter().filter(|s| {
        // Filter out Smithery self-promotion
        !(s.namespace == "smithery-ai" && s.slug == "cli")
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
    let url = format!("{SMITHERY_API}/servers?pageSize={}", limit);
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
            install_command: "curl -fsSL https://raw.githubusercontent.com/iOfficeAI/OfficeCLI/main/install.sh | sh".into(),
            skills_repo: "iOfficeAI/OfficeCLI".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["office".into(), "documents".into()],
            verified: true,
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
    CLI_REGISTRY.iter().map(|entry| {
        let stars = fetch_github_stars(&entry.skills_repo);
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
    }).collect()
}

pub fn get_cli_registry_entry(binary_name: &str) -> Option<&'static CliRegistryEntry> {
    CLI_REGISTRY.iter().find(|e| e.binary_name == binary_name)
}
