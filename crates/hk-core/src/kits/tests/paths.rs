use crate::kits::paths::{kits_dir, zip_path_for};

#[test]
fn zip_path_for_uses_hk_kit_zip_suffix() {
    let p = zip_path_for("abc-123").unwrap();
    assert!(p.to_string_lossy().ends_with("abc-123.hk-kit.zip"));
}

#[test]
fn kits_dir_is_under_home() {
    let dir = kits_dir().unwrap();
    let home = dirs::home_dir().unwrap();
    assert!(dir.starts_with(home));
    assert!(dir.ends_with(".harnesskit/kits") || dir.ends_with(r".harnesskit\kits"));
}
