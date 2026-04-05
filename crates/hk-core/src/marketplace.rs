use anyhow::{Context, Result};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const SMITHERY_API: &str = "https://api.smithery.ai";
const SKILLS_SH_API: &str = "https://skills.sh/api";
const AUDIT_API: &str = "https://add-skill.vercel.sh";

// --- Caches for skill content & audit ---
type TimedCache<T> = LazyLock<Mutex<HashMap<String, (Instant, Option<T>)>>>;

static SKILL_CONTENT_CACHE: TimedCache<String> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static AUDIT_INFO_CACHE: TimedCache<SkillAuditInfo> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
const DETAIL_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const MAX_CACHE_ENTRIES: usize = 200;

/// Insert into a timed cache with size limit. Evicts the oldest entry if at capacity.
fn cache_insert<T>(cache: &Mutex<HashMap<String, (Instant, Option<T>)>>, key: String, value: Option<T>) {
    let Ok(mut map) = cache.lock() else { return };
    if map.len() >= MAX_CACHE_ENTRIES {
        if let Some(oldest_key) = map.iter()
            .min_by_key(|(_, (ts, _))| *ts)
            .map(|(k, _)| k.clone())
        {
            map.remove(&oldest_key);
        }
    }
    map.insert(key, (Instant::now(), value));
}

fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client")
}

fn async_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build async HTTP client")
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

/// Extract "owner/repo" from a GitHub URL (strips tree/branch/path suffixes).
fn github_repo_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    // "owner/repo/tree/main/..." → take only first two segments
    let parts: Vec<&str> = rest.splitn(3, '/').collect();
    if parts.len() >= 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Parse a full GitHub tree URL into (owner/repo, branch, subdir_path).
/// e.g. "https://github.com/anthropics/claude-code/tree/main/plugins/skills/foo"
///   → Some(("anthropics/claude-code", "main", "plugins/skills/foo"))
fn parse_github_tree_url(url: &str) -> Option<(String, String, String)> {
    let rest = url.strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    // Expected: "owner/repo/tree/branch/path..."
    let parts: Vec<&str> = rest.splitn(5, '/').collect();
    // parts: [owner, repo, "tree", branch, path]
    if parts.len() == 5 && parts[2] == "tree" {
        Some((
            format!("{}/{}", parts[0], parts[1]),
            parts[3].to_string(),
            parts[4].trim_end_matches('/').to_string(),
        ))
    } else {
        None
    }
}

// --- Public API: Skills (via skills.sh) ---

/// Search skills via skills.sh. Returns items with source in GitHub "owner/repo" format.
pub fn search_skills(query: &str, limit: usize) -> Result<Vec<MarketplaceItem>> {
    if query.len() < 2 { return Ok(vec![]); }
    let resp: SkillsShSearchResponse = client()?
        .get(format!("{SKILLS_SH_API}/search"))
        .query(&[("q", query), ("limit", &limit.to_string())])
        .send()
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

/// Async version of [`search_skills`] for use in Tauri commands.
pub async fn search_skills_async(query: &str, limit: usize) -> Result<Vec<MarketplaceItem>> {
    if query.len() < 2 { return Ok(vec![]); }
    let resp: SkillsShSearchResponse = async_client()?
        .get(format!("{SKILLS_SH_API}/search"))
        .query(&[("q", query), ("limit", &limit.to_string())])
        .send().await
        .context("Failed to reach skills.sh")?
        .json().await
        .context("Failed to parse skills.sh response")?;
    Ok(resp.skills.into_iter().map(|s| MarketplaceItem {
        id: s.id.clone(),
        name: s.name,
        description: String::new(),
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
    let resp: SmitheryServersResponse = client()?
        .get(format!("{SMITHERY_API}/servers"))
        .query(&[("q", query), ("pageSize", &limit.to_string())])
        .send()
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

/// Async version of [`search_servers`] for use in Tauri commands.
pub async fn search_servers_async(query: &str, limit: usize) -> Result<Vec<MarketplaceItem>> {
    if query.len() < 2 { return Ok(vec![]); }
    let resp: SmitheryServersResponse = async_client()?
        .get(format!("{SMITHERY_API}/servers"))
        .query(&[("q", query), ("pageSize", &limit.to_string())])
        .send().await
        .context("Failed to reach Smithery")?
        .json().await
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
        // Preserve full git_url as repo_url for direct SKILL.md fetching
        let repo_url = s.git_url.clone();
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
            repo_url,
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

/// Async version of [`trending_skills`] for use in Tauri commands.
pub async fn trending_skills_async(limit: usize) -> Result<Vec<MarketplaceItem>> {
    let page = (chrono::Utc::now().ordinal() % 5) + 1;
    let url = format!("{SMITHERY_API}/skills?pageSize={}&page={}", limit, page);
    let resp: SmitherySkillsResponse = async_client()?.get(&url).send().await
        .context("Failed to reach Smithery")?
        .json().await
        .context("Failed to parse Smithery response")?;
    Ok(resp.skills.into_iter().filter(|s| {
        s.namespace != "smithery-ai"
    }).map(|s| {
        let github_source = s.git_url.as_deref()
            .and_then(github_repo_from_url)
            .unwrap_or_else(|| format!("{}/{}", s.namespace, s.slug));
        let repo_url = s.git_url.clone();
        MarketplaceItem {
            id: format!("{}/{}", s.namespace, s.slug),
            name: s.display_name,
            description: truncate(&s.description, 120),
            source: github_source,
            skill_id: String::new(),
            kind: "skill".into(),
            installs: s.total_activations,
            icon_url: None,
            verified: s.verified,
            categories: s.categories,
            stars: None,
            repo_url,
        }
    }).collect())
}

/// Async version of [`trending_servers`] for use in Tauri commands.
pub async fn trending_servers_async(limit: usize) -> Result<Vec<MarketplaceItem>> {
    let page = (chrono::Utc::now().ordinal() % 5) + 1;
    let url = format!("{SMITHERY_API}/servers?pageSize={}&page={}", limit, page);
    let resp: SmitheryServersResponse = async_client()?.get(&url).send().await
        .context("Failed to reach Smithery")?
        .json().await
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

/// Build the list of candidate paths for SKILL.md.
/// When `skill_id` is empty, only tries root-level SKILL.md.
fn skill_md_paths(skill_id: &str) -> Vec<String> {
    if skill_id.is_empty() {
        vec!["SKILL.md".to_string(), "skill.md".to_string()]
    } else {
        vec![
            format!("skills/{skill_id}/SKILL.md"),
            format!("skills/{skill_id}/skill.md"),
            format!("{skill_id}/SKILL.md"),
            format!("{skill_id}/skill.md"),
            "SKILL.md".to_string(),
            "skill.md".to_string(),
        ]
    }
}

/// Fetch SKILL.md from GitHub.
/// - `source`: "owner/repo"
/// - `skill_id`: skill name for path probing
/// - `git_url`: optional full GitHub tree URL (e.g. from Smithery gitUrl) for direct fetch
pub fn fetch_skill_content(source: &str, skill_id: &str, git_url: Option<&str>) -> Result<String> {
    let cache_key = format!("{source}/{skill_id}");
    if let Ok(cache) = SKILL_CONTENT_CACHE.lock()
        && let Some((ts, cached)) = cache.get(&cache_key)
            && ts.elapsed() < DETAIL_CACHE_TTL {
                return match cached {
                    Some(content) => Ok(content.clone()),
                    None => anyhow::bail!("Could not find SKILL.md for {source}/{skill_id} (cached)"),
                };
            }
    let c = client()?;
    // Phase 1: if git_url provides exact path, try it directly
    if let Some(url) = git_url
        && let Some((repo, branch, subdir)) = parse_github_tree_url(url) {
            for filename in &["SKILL.md", "skill.md"] {
                let raw = format!("https://raw.githubusercontent.com/{repo}/{branch}/{subdir}/{filename}");
                if let Ok(resp) = c.get(&raw).send()
                    && resp.status().is_success()
                        && let Ok(text) = resp.text()
                            && !text.is_empty() {
                                cache_insert(&SKILL_CONTENT_CACHE, cache_key.clone(), Some(text.clone()));
                                return Ok(text);
                            }
            }
        }
    // Phase 2: try well-known paths
    let paths = skill_md_paths(skill_id);
    for branch in &["main", "master"] {
        for path in &paths {
            let url = format!("https://raw.githubusercontent.com/{source}/{branch}/{path}");
            if let Ok(resp) = c.get(&url).send()
                && resp.status().is_success()
                    && let Ok(text) = resp.text()
                        && !text.is_empty() {
                            cache_insert(&SKILL_CONTENT_CACHE, cache_key.clone(), Some(text.clone()));
                            return Ok(text);
                        }
        }
    }
    cache_insert(&SKILL_CONTENT_CACHE, cache_key, None::<String>);
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
    let cache_key = format!("{source}/{skill_id}");
    if let Ok(cache) = AUDIT_INFO_CACHE.lock()
        && let Some((ts, cached)) = cache.get(&cache_key)
            && ts.elapsed() < DETAIL_CACHE_TTL {
                return Ok(cached.clone());
            }
    let resp = client()?
        .get(format!("{AUDIT_API}/audit"))
        .query(&[("source", source), ("skills", skill_id)])
        .send()
        .context("Failed to reach audit service")?;
    let result = if resp.status().is_success() {
        let data: HashMap<String, SkillAuditInfo> = resp.json().unwrap_or_default();
        data.into_values().next()
    } else {
        None
    };
    cache_insert(&AUDIT_INFO_CACHE, cache_key, result.clone());
    Ok(result)
}

/// Async version of [`fetch_skill_content`] for use in Tauri commands.
pub async fn fetch_skill_content_async(source: &str, skill_id: &str, git_url: Option<&str>) -> Result<String> {
    let cache_key = format!("{source}/{skill_id}");
    if let Ok(cache) = SKILL_CONTENT_CACHE.lock()
        && let Some((ts, cached)) = cache.get(&cache_key)
            && ts.elapsed() < DETAIL_CACHE_TTL {
                return match cached {
                    Some(content) => Ok(content.clone()),
                    None => anyhow::bail!("Could not find SKILL.md for {source}/{skill_id} (cached)"),
                };
            }
    let c = async_client()?;
    // Phase 1: if git_url provides exact path, try it directly
    if let Some(url) = git_url
        && let Some((repo, branch, subdir)) = parse_github_tree_url(url) {
            for filename in &["SKILL.md", "skill.md"] {
                let raw = format!("https://raw.githubusercontent.com/{repo}/{branch}/{subdir}/{filename}");
                if let Ok(resp) = c.get(&raw).send().await
                    && resp.status().is_success()
                        && let Ok(text) = resp.text().await
                            && !text.is_empty() {
                                cache_insert(&SKILL_CONTENT_CACHE, cache_key.clone(), Some(text.clone()));
                                return Ok(text);
                            }
            }
        }
    // Phase 2: try well-known paths
    let paths = skill_md_paths(skill_id);
    for branch in &["main", "master"] {
        for path in &paths {
            let url = format!("https://raw.githubusercontent.com/{source}/{branch}/{path}");
            if let Ok(resp) = c.get(&url).send().await
                && resp.status().is_success()
                    && let Ok(text) = resp.text().await
                        && !text.is_empty() {
                            cache_insert(&SKILL_CONTENT_CACHE, cache_key.clone(), Some(text.clone()));
                            return Ok(text);
                        }
        }
    }
    cache_insert(&SKILL_CONTENT_CACHE, cache_key, None::<String>);
    anyhow::bail!("Could not find SKILL.md for {source}/{skill_id}")
}

/// Async version of [`fetch_audit_info`] for use in Tauri commands.
pub async fn fetch_audit_info_async(source: &str, skill_id: &str) -> Result<Option<SkillAuditInfo>> {
    let cache_key = format!("{source}/{skill_id}");
    if let Ok(cache) = AUDIT_INFO_CACHE.lock()
        && let Some((ts, cached)) = cache.get(&cache_key)
            && ts.elapsed() < DETAIL_CACHE_TTL {
                return Ok(cached.clone());
            }
    let resp = async_client()?
        .get(format!("{AUDIT_API}/audit"))
        .query(&[("source", source), ("skills", skill_id)])
        .send().await
        .context("Failed to reach audit service")?;
    let result = if resp.status().is_success() {
        let data: HashMap<String, SkillAuditInfo> = resp.json().await.unwrap_or_default();
        data.into_values().next()
    } else {
        None
    };
    cache_insert(&AUDIT_INFO_CACHE, cache_key, result.clone());
    Ok(result)
}

pub fn git_url_for_source(source: &str) -> String {
    format!("https://github.com/{source}.git")
}

// --- CLI README fetching ---

static CLI_README_CACHE: TimedCache<String> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fetch README.md from a GitHub repo. `source` = "owner/repo".
pub async fn fetch_cli_readme_async(source: &str) -> Result<String> {
    if let Ok(cache) = CLI_README_CACHE.lock()
        && let Some((ts, cached)) = cache.get(source)
            && ts.elapsed() < DETAIL_CACHE_TTL {
                return match cached {
                    Some(content) => Ok(content.clone()),
                    None => anyhow::bail!("No README found for {source} (cached)"),
                };
            }
    let c = async_client()?;
    for branch in &["main", "master"] {
        for filename in &["README.md", "readme.md", "Readme.md"] {
            let url = format!("https://raw.githubusercontent.com/{source}/{branch}/{filename}");
            if let Ok(resp) = c.get(&url).send().await
                && resp.status().is_success()
                    && let Ok(text) = resp.text().await
                        && !text.is_empty() {
                            cache_insert(&CLI_README_CACHE, source.to_string(), Some(text.clone()));
                            return Ok(text);
                        }
        }
    }
    cache_insert(&CLI_README_CACHE, source.to_string(), None::<String>);
    anyhow::bail!("No README found for {source}")
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
    /// Structured install: the program to execute (e.g. "npm", "go", "brew").
    /// When present, `install_args` must also be set. Used to avoid `sh -c`.
    #[serde(default)]
    pub install_program: Option<String>,
    /// Structured install: arguments to pass to `install_program`.
    #[serde(default)]
    pub install_args: Option<Vec<String>>,
}

/// Shell interpreters that must never be used as `install_program`.
/// If one of these is set, `resolved_command()` returns `None` so the
/// caller falls back to the `sh -c install_command` path — which is the
/// only path audited for shell execution.
const BLOCKED_INSTALL_PROGRAMS: &[&str] = &["sh", "bash", "zsh", "dash", "fish", "cmd", "cmd.exe", "powershell", "powershell.exe", "pwsh"];

impl CliRegistryEntry {
    /// Returns the resolved command to execute for installation.
    /// If structured fields (`install_program` + `install_args`) are present,
    /// returns `Some((program, args))` for use with `Command::new(program).args(args)`.
    /// Falls back to `None`, meaning the caller must use `sh -c install_command`.
    ///
    /// Returns `None` (forcing `sh -c` fallback) when `install_program` is a
    /// shell interpreter, since that would defeat the purpose of structured execution.
    pub fn resolved_command(&self) -> Option<(&str, &[String])> {
        match (&self.install_program, &self.install_args) {
            (Some(prog), Some(args)) => {
                if BLOCKED_INSTALL_PROGRAMS.iter().any(|s| s.eq_ignore_ascii_case(prog)) {
                    None
                } else {
                    Some((prog.as_str(), args.as_slice()))
                }
            }
            _ => None,
        }
    }
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
            install_program: Some("npm".into()),
            install_args: Some(vec!["install".into(), "-g".into(), "@wecom/cli".into()]),
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
            install_program: Some("npm".into()),
            install_args: Some(vec!["install".into(), "-g".into(), "@larksuite/cli".into()]),
        },
        CliRegistryEntry {
            binary_name: "dws".into(),
            display_name: "DingTalk Workspace CLI".into(),
            description: "CLI for DingTalk — workspace management, bots, and messaging".into(),
            // Piped command — cannot be structured; must use sh -c
            install_command: "curl -fsSL https://raw.githubusercontent.com/DingTalk-Real-AI/dingtalk-workspace-cli/main/install.sh | sh".into(),
            skills_repo: "DingTalk-Real-AI/dingtalk-workspace-cli".into(),
            skills_install_command: Some("curl -fsSL https://raw.githubusercontent.com/DingTalk-Real-AI/dingtalk-workspace-cli/main/install-skills.sh | sh".into()),
            icon_url: None,
            categories: vec!["collaboration".into(), "messaging".into()],
            verified: true,
            api_domains: vec!["oapi.dingtalk.com".into()],
            credentials_path: Some("~/.dingtalk/credentials.json".into()),
            install_program: None,
            install_args: None,
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
            install_program: Some("npm".into()),
            install_args: Some(vec!["install".into(), "-g".into(), "meitu-cli".into()]),
        },
        CliRegistryEntry {
            binary_name: "officecli".into(),
            display_name: "OfficeCLI".into(),
            description: "CLI for office document management — create, convert, and automate documents".into(),
            // Piped command — cannot be structured; must use sh -c
            install_command: "curl -fsSL https://raw.githubusercontent.com/iOfficeAI/OfficeCLI/main/install.sh | sh".into(),
            skills_repo: "iOfficeAI/OfficeCLI".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec!["office".into(), "documents".into()],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
            install_program: None,
            install_args: None,
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
            install_program: Some("go".into()),
            install_args: Some(vec!["install".into(), "github.com/lox/notion-cli@latest".into()]),
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
            install_program: Some("npm".into()),
            install_args: Some(vec!["install".into(), "-g".into(), "@jackwener/opencli".into()]),
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
            install_program: Some("pip".into()),
            install_args: Some(vec!["install".into(), "git+https://github.com/HKUDS/CLI-Anything.git".into()]),
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
            install_program: Some("npm".into()),
            install_args: Some(vec!["install".into(), "-g".into(), "agent-browser".into()]),
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
            install_program: Some("brew".into()),
            install_args: Some(vec!["install".into(), "rtk".into()]),
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
            install_program: Some("bun".into()),
            install_args: Some(vec!["add".into(), "-g".into(), "@open-pencil/cli".into()]),
        },
    ]
});

/// Fetch GitHub stargazers_count for a repo. Cached in-process.
fn fetch_github_stars(owner_repo: &str) -> Option<u64> {
    static CACHE: TimedCache<u64> =
        LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

    let ttl = Duration::from_secs(3600); // 1-hour cache
    if let Ok(cache) = CACHE.lock()
        && let Some((ts, stars)) = cache.get(owner_repo)
            && ts.elapsed() < ttl {
                return *stars;
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

    cache_insert(&CACHE, owner_repo.to_string(), stars);
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
    if let Ok(guard) = CLI_REGISTRY_CACHE.lock()
        && let Some(ref cache) = *guard
            && cache.fetched_at.elapsed() < CLI_REGISTRY_TTL {
                return cache.entries.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolved_command_structured() {
        let entry = CliRegistryEntry {
            binary_name: "test".into(),
            display_name: "Test".into(),
            description: "".into(),
            install_command: "npm install -g test".into(),
            skills_repo: "test/test".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec![],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
            install_program: Some("npm".into()),
            install_args: Some(vec!["install".into(), "-g".into(), "test".into()]),
        };
        let (prog, args) = entry.resolved_command().unwrap();
        assert_eq!(prog, "npm");
        assert_eq!(args, &["install", "-g", "test"]);
    }

    #[test]
    fn test_resolved_command_fallback_piped() {
        let entry = CliRegistryEntry {
            binary_name: "dws".into(),
            display_name: "DWS".into(),
            description: "".into(),
            install_command: "curl -fsSL https://example.com/install.sh | sh".into(),
            skills_repo: "test/test".into(),
            skills_install_command: None,
            icon_url: None,
            categories: vec![],
            verified: false,
            api_domains: vec![],
            credentials_path: None,
            install_program: None,
            install_args: None,
        };
        assert!(entry.resolved_command().is_none());
    }

    #[test]
    fn test_resolved_command_rejects_shell_interpreters() {
        let shells = &["sh", "bash", "zsh", "cmd", "powershell", "Bash", "SH", "cmd.exe", "pwsh"];
        for shell in shells {
            let entry = CliRegistryEntry {
                binary_name: "test".into(),
                display_name: "Test".into(),
                description: "".into(),
                install_command: format!("{} -c 'echo hello'", shell),
                skills_repo: "test/test".into(),
                skills_install_command: None,
                icon_url: None,
                categories: vec![],
                verified: false,
                api_domains: vec![],
                credentials_path: None,
                install_program: Some(shell.to_string()),
                install_args: Some(vec!["-c".into(), "echo hello".into()]),
            };
            assert!(
                entry.resolved_command().is_none(),
                "resolved_command() should return None for shell '{}', forcing sh -c fallback",
                shell,
            );
        }
    }

    #[test]
    fn test_embedded_registry_structured_entries() {
        // Verify that all entries with install_program also have install_args
        for entry in CLI_REGISTRY.iter() {
            match (&entry.install_program, &entry.install_args) {
                (Some(_), None) => panic!("{}: has install_program but no install_args", entry.binary_name),
                (None, Some(_)) => panic!("{}: has install_args but no install_program", entry.binary_name),
                _ => {} // Both Some or both None is valid
            }
        }
        // Verify piped commands (dws, officecli) do NOT have structured fields
        let dws = CLI_REGISTRY.iter().find(|e| e.binary_name == "dws").unwrap();
        assert!(dws.resolved_command().is_none());
        let officecli = CLI_REGISTRY.iter().find(|e| e.binary_name == "officecli").unwrap();
        assert!(officecli.resolved_command().is_none());
        // Verify structured commands return correctly
        let wecom = CLI_REGISTRY.iter().find(|e| e.binary_name == "wecom-cli").unwrap();
        assert!(wecom.resolved_command().is_some());
        let rtk = CLI_REGISTRY.iter().find(|e| e.binary_name == "rtk").unwrap();
        let (prog, _args) = rtk.resolved_command().unwrap();
        assert_eq!(prog, "brew");
    }

    #[test]
    fn test_cli_registry_entry_serde_default() {
        // Verify backward compatibility: deserializing without structured fields works
        let json = r#"{
            "binary_name": "test",
            "display_name": "Test",
            "description": "",
            "install_command": "npm install -g test",
            "skills_repo": "test/test",
            "categories": [],
            "verified": false,
            "api_domains": []
        }"#;
        let entry: CliRegistryEntry = serde_json::from_str(json).unwrap();
        assert!(entry.install_program.is_none());
        assert!(entry.install_args.is_none());
        assert!(entry.resolved_command().is_none());
    }

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        let result = truncate("hello world", 8);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 11); // up to 8 chars + "..."
    }

    #[test]
    fn test_truncate_empty() {
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_truncate_one_char() {
        assert_eq!(truncate("a", 1), "a");
    }

    #[test]
    fn test_truncate_zero_max() {
        let result = truncate("hello", 0);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_unicode() {
        // Test with multi-byte UTF-8 characters
        let result = truncate("hello🚀world", 8);
        assert!(result.ends_with("..."));
        // Should not panic or produce invalid UTF-8
        // If we got a string back, UTF-8 is valid by definition
        assert!(!result.is_empty());
    }

    #[test]
    fn test_github_repo_from_url_https() {
        let url = "https://github.com/owner/repo";
        assert_eq!(github_repo_from_url(url), Some("owner/repo".to_string()));
    }

    #[test]
    fn test_github_repo_from_url_http() {
        let url = "http://github.com/owner/repo";
        assert_eq!(github_repo_from_url(url), Some("owner/repo".to_string()));
    }

    #[test]
    fn test_github_repo_from_url_with_git_suffix() {
        let url = "https://github.com/owner/repo.git";
        assert_eq!(github_repo_from_url(url), Some("owner/repo".to_string()));
    }

    #[test]
    fn test_github_repo_from_url_with_trailing_slash() {
        let url = "https://github.com/owner/repo/";
        assert_eq!(github_repo_from_url(url), Some("owner/repo".to_string()));
    }

    #[test]
    fn test_github_repo_from_url_with_tree_and_branch() {
        let url = "https://github.com/owner/repo/tree/main";
        assert_eq!(github_repo_from_url(url), Some("owner/repo".to_string()));
    }

    #[test]
    fn test_github_repo_from_url_with_full_path() {
        let url = "https://github.com/owner/repo/tree/main/some/path/file.txt";
        assert_eq!(github_repo_from_url(url), Some("owner/repo".to_string()));
    }

    #[test]
    fn test_github_repo_from_url_invalid_prefix() {
        let url = "https://gitlab.com/owner/repo";
        assert_eq!(github_repo_from_url(url), None);
    }

    #[test]
    fn test_github_repo_from_url_no_repo() {
        let url = "https://github.com/owner";
        assert_eq!(github_repo_from_url(url), None);
    }

    #[test]
    fn test_parse_github_tree_url_valid() {
        let url = "https://github.com/anthropics/claude-code/tree/main/plugins/skills/foo";
        let result = parse_github_tree_url(url);
        assert_eq!(
            result,
            Some((
                "anthropics/claude-code".to_string(),
                "main".to_string(),
                "plugins/skills/foo".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_github_tree_url_no_subdir() {
        let url = "https://github.com/owner/repo/tree/main";
        let result = parse_github_tree_url(url);
        // Should not match because there's no path after branch
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_github_tree_url_without_tree_keyword() {
        let url = "https://github.com/owner/repo/main/path";
        let result = parse_github_tree_url(url);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_github_tree_url_http() {
        let url = "http://github.com/owner/repo/tree/develop/src";
        let result = parse_github_tree_url(url);
        assert_eq!(
            result,
            Some((
                "owner/repo".to_string(),
                "develop".to_string(),
                "src".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_github_tree_url_with_trailing_slash() {
        let url = "https://github.com/owner/repo/tree/main/path/";
        let result = parse_github_tree_url(url);
        assert_eq!(
            result,
            Some((
                "owner/repo".to_string(),
                "main".to_string(),
                "path".to_string()
            ))
        );
    }

    #[test]
    fn test_git_url_for_source() {
        assert_eq!(
            git_url_for_source("owner/repo"),
            "https://github.com/owner/repo.git"
        );
    }

    #[test]
    fn test_git_url_for_source_with_hyphens() {
        assert_eq!(
            git_url_for_source("my-org/my-repo"),
            "https://github.com/my-org/my-repo.git"
        );
    }

    #[test]
    fn test_get_embedded_cli_entry_wecom_cli() {
        let entry = get_embedded_cli_entry("wecom-cli");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.binary_name, "wecom-cli");
        assert_eq!(entry.display_name, "WeChat Work CLI");
        assert!(!entry.install_command.is_empty());
        assert!(entry.verified);
    }

    #[test]
    fn test_get_embedded_cli_entry_lark_cli() {
        let entry = get_embedded_cli_entry("lark-cli");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.binary_name, "lark-cli");
        assert_eq!(entry.display_name, "Lark / Feishu CLI");
    }

    #[test]
    fn test_get_embedded_cli_entry_notion_cli() {
        let entry = get_embedded_cli_entry("notion-cli");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.binary_name, "notion-cli");
        assert!(!entry.verified); // notion-cli is not verified
    }

    #[test]
    fn test_get_embedded_cli_entry_unknown() {
        assert!(get_embedded_cli_entry("nonexistent-cli-xyz").is_none());
    }

    #[test]
    fn test_get_embedded_cli_entry_empty_string() {
        assert!(get_embedded_cli_entry("").is_none());
    }

    #[test]
    fn test_get_embedded_cli_entry_case_sensitive() {
        // Should be case-sensitive
        assert!(get_embedded_cli_entry("Wecom-CLI").is_none());
    }

    // NOTE: Do NOT test get_cli_registry_entry() — it may make network calls
    // NOTE: Do NOT test search_skills(), search_servers(), trending_* — they make HTTP calls
    // NOTE: Do NOT test fetch_skill_content(), fetch_audit_info() — they make HTTP calls
}

