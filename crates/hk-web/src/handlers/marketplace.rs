use axum::Json;
use hk_core::marketplace::{self, MarketplaceItem, SkillAuditInfo};
use serde::Deserialize;

use crate::router::ApiError;

type Result<T> = std::result::Result<Json<T>, ApiError>;

#[derive(Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub kind: String,
    pub limit: Option<usize>,
}

pub async fn search_marketplace(
    Json(params): Json<SearchParams>,
) -> Result<Vec<MarketplaceItem>> {
    let limit = params.limit.unwrap_or(20);
    let results = match params.kind.as_str() {
        "mcp" => marketplace::search_servers_async(&params.query, limit).await?,
        _ => marketplace::search_skills_async(&params.query, limit).await?,
    };
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct TrendingParams {
    pub kind: String,
    pub limit: Option<usize>,
}

pub async fn trending_marketplace(
    Json(params): Json<TrendingParams>,
) -> Result<Vec<MarketplaceItem>> {
    let limit = params.limit.unwrap_or(10);
    let results = match params.kind.as_str() {
        "mcp" => marketplace::trending_servers_async(limit).await?,
        _ => marketplace::trending_skills_async(limit).await?,
    };
    Ok(Json(results))
}

pub async fn list_cli_marketplace() -> Result<Vec<MarketplaceItem>> {
    // list_cli_registry uses reqwest::blocking internally (via fetch_github_stars),
    // which panics inside a tokio runtime. Must run in a blocking thread.
    let items = tokio::task::spawn_blocking(marketplace::list_cli_registry)
        .await
        .map_err(|e| ApiError::from(hk_core::HkError::Internal(e.to_string())))?;
    Ok(Json(items))
}

#[derive(Deserialize)]
pub struct FetchPreviewParams {
    pub source: String,
    pub skill_id: String,
    pub git_url: Option<String>,
}

pub async fn fetch_skill_preview(
    Json(params): Json<FetchPreviewParams>,
) -> Result<String> {
    let content = marketplace::fetch_skill_content_async(
        &params.source,
        &params.skill_id,
        params.git_url.as_deref(),
    )
    .await?;
    Ok(Json(content))
}

#[derive(Deserialize)]
pub struct FetchCliReadmeParams {
    pub source: String,
}

pub async fn fetch_cli_readme(
    Json(params): Json<FetchCliReadmeParams>,
) -> Result<String> {
    let content = marketplace::fetch_cli_readme_async(&params.source).await?;
    Ok(Json(content))
}

#[derive(Deserialize)]
pub struct FetchAuditParams {
    pub source: String,
    pub skill_id: String,
}

pub async fn fetch_skill_audit(
    Json(params): Json<FetchAuditParams>,
) -> Result<Option<SkillAuditInfo>> {
    let info = marketplace::fetch_audit_info_async(&params.source, &params.skill_id).await?;
    Ok(Json(info))
}
