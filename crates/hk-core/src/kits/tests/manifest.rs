use crate::kits::manifest::{
    sha256_of_bytes, KitManifest, ManifestConfigFile, ManifestExtension, KIT_FORMAT_VERSION,
};
use crate::models::{ConfigCategory, ExtensionKind};
use chrono::Utc;

#[test]
fn round_trip_manifest_preserves_fields() {
    let manifest = KitManifest {
        kit_format_version: KIT_FORMAT_VERSION,
        kit_id: "kit-abc".into(),
        name: "Frontend Setup".into(),
        description: "React work".into(),
        created_at: Utc::now(),
        exported_from: "HarnessKit 1.5.0".into(),
        extensions: vec![ManifestExtension {
            name: "react-helper".into(),
            kind: ExtensionKind::Skill,
            source_extension_id: "ext-1".into(),
            source_url: Some("https://github.com/x/y".into()),
            content_hash: "sha256:deadbeef".into(),
            asset_path: "assets/ext-1/".into(),
            position: 0,
            source_revision: None,
            source_branch: None,
        }],
        config_files: vec![ManifestConfigFile {
            agent: "claude".into(),
            category: ConfigCategory::Rules,
            filename: "CLAUDE.md".into(),
            asset_path: "assets/config-claude-rules/CLAUDE.md".into(),
            position: 0,
        }],
        secrets_stripped: vec!["ext-mcp-figma".into()],
    };
    let json = serde_json::to_string(&manifest).unwrap();
    let parsed: KitManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.kit_format_version, KIT_FORMAT_VERSION);
    assert_eq!(parsed.extensions.len(), 1);
    assert_eq!(parsed.extensions[0].kind, ExtensionKind::Skill);
    assert_eq!(parsed.config_files[0].category, ConfigCategory::Rules);
    assert_eq!(parsed.secrets_stripped, vec!["ext-mcp-figma".to_string()]);
}

#[test]
fn sha256_of_bytes_is_deterministic_and_prefixed() {
    let h1 = sha256_of_bytes(b"hello");
    let h2 = sha256_of_bytes(b"hello");
    let h3 = sha256_of_bytes(b"world");
    assert!(h1.starts_with("sha256:"));
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
    assert_eq!(h1.len(), "sha256:".len() + 64);
}

#[test]
fn sha256_of_dir_skips_dot_git() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("README.md"), b"hello").unwrap();
    // Without .git
    let hash_clean = crate::kits::manifest::sha256_of_dir(dir.path()).unwrap();
    // Add a .git/ folder with content
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git").join("HEAD"), b"ref: refs/heads/main").unwrap();
    let hash_with_git = crate::kits::manifest::sha256_of_dir(dir.path()).unwrap();
    assert_eq!(hash_clean, hash_with_git, "hash should not change when .git is added");
}

#[cfg(unix)]
#[test]
fn sha256_of_dir_skips_symlinks() {
    use std::fs;
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("real.txt"), b"hello").unwrap();
    let hash_before = crate::kits::manifest::sha256_of_dir(dir.path()).unwrap();
    // Add a symlink to an outside path
    symlink("/etc/hosts", dir.path().join("link.txt")).unwrap();
    let hash_after = crate::kits::manifest::sha256_of_dir(dir.path()).unwrap();
    assert_eq!(hash_before, hash_after, "hash should not change when symlink is added");
}
