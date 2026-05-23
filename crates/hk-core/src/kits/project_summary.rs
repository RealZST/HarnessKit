//! Lightweight per-project extension counts for the v2 Kits page's
//! Project cards summary line ("3 skills · 2 MCP servers · 1 CLI tool").
//!
//! Walks each adapter's project-level locations and counts top-level
//! entries. Does NOT read DB, does NOT trigger a full scope scan.

use crate::adapter::AgentAdapter;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectExtensionCounts {
    pub skills: u32,
    pub mcp: u32,
    pub cli: u32,
    pub hook: u32,
}

/// Walk each adapter's project-level locations and count top-level entries.
///
/// - Skills: top-level subdirectories under each `project_skill_dirs()` entry.
/// - MCP servers: entries returned by `read_mcp_servers_from()` for the
///   adapter's `project_mcp_config_relpath()`.
/// - CLI: not yet supported — the adapter trait has no `project_cli_dirs()`
///   method today, so this field is always 0. Reserved for future adapter
///   capability.
/// - Hooks: entries returned by `read_hooks_from()` for the adapter's
///   `project_hook_config_relpath()`.
pub fn count_project_extensions(
    project_path: &Path,
    adapters: &[Box<dyn AgentAdapter>],
) -> ProjectExtensionCounts {
    let mut counts = ProjectExtensionCounts { skills: 0, mcp: 0, cli: 0, hook: 0 };

    for adapter in adapters {
        // Skills — top-level subdirectories of each declared project skill dir.
        for rel in adapter.project_skill_dirs() {
            let dir = project_path.join(&rel);
            if let Ok(read) = std::fs::read_dir(&dir) {
                counts.skills += read
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .count() as u32;
            }
        }
        // MCP servers — parse each adapter's project MCP config file.
        if let Some(rel) = adapter.project_mcp_config_relpath() {
            let path = project_path.join(&rel);
            if path.is_file() {
                counts.mcp += adapter.read_mcp_servers_from(&path).len() as u32;
            }
        }
        // Hooks — parse each adapter's project hook config file.
        if let Some(rel) = adapter.project_hook_config_relpath() {
            let path = project_path.join(&rel);
            if path.is_file() {
                counts.hook += adapter.read_hooks_from(&path).len() as u32;
            }
        }
    }

    counts
}
