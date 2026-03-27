use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
