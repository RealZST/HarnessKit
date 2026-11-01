use crate::adapter::all_adapters;
use crate::kits::project_summary::{ProjectExtensionCounts, count_project_extensions};
use std::fs;
use tempfile::tempdir;

#[test]
fn empty_project_returns_all_zeros() {
    let dir = tempdir().unwrap();
    let adapters = all_adapters();
    let counts = count_project_extensions(dir.path(), &adapters);
    assert_eq!(
        counts,
        ProjectExtensionCounts {
            skills: 0,
            mcp: 0,
            cli: 0,
            hook: 0
        }
    );
}

#[test]
fn claude_project_with_two_skills_and_one_mcp() {
    let dir = tempdir().unwrap();
    let proj = dir.path();
    fs::create_dir_all(proj.join(".claude/skills/foo")).unwrap();
    fs::write(
        proj.join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\n---\n",
    )
    .unwrap();
    fs::create_dir_all(proj.join(".claude/skills/bar")).unwrap();
    fs::write(
        proj.join(".claude/skills/bar/SKILL.md"),
        "---\nname: bar\n---\n",
    )
    .unwrap();
    fs::write(
        proj.join(".mcp.json"),
        r#"{"mcpServers":{"prettier":{"command":"npx","args":["-y","prettier-mcp"]}}}"#,
    )
    .unwrap();

    let adapters = all_adapters();
    let counts = count_project_extensions(proj, &adapters);
    assert_eq!(counts.skills, 2);
    assert_eq!(counts.mcp, 1);
    assert_eq!(counts.cli, 0);
    assert_eq!(counts.hook, 0);
}

#[test]
fn opencode_jsonc_only_project_counts_mcp_servers() {
    let dir = tempdir().unwrap();
    let proj = dir.path();
    fs::write(
        proj.join("opencode.jsonc"),
        r#"{
  // project-local servers
  "mcp": {
    "docs": {"type": "local", "command": ["npx", "docs-mcp"]},
    "api": {"type": "remote", "url": "https://example.com/mcp"},
  },
}"#,
    )
    .unwrap();

    let adapters = all_adapters();
    let counts = count_project_extensions(proj, &adapters);
    assert_eq!(counts.mcp, 2);
}
