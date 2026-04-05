use tauri::command;

#[cfg(target_os = "macos")]
#[command]
pub fn set_app_icon(app: tauri::AppHandle, name: String) -> Result<(), String> {
    use objc2::AnyThread;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    use tauri::Manager;

    let resource_name = match name.as_str() {
        "icon-1" => "app-icon-1.png",
        "icon-2" => "app-icon-2.png",
        _ => return Err(format!("Unknown icon: {name}")),
    };

    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e: tauri::Error| format!("resource_dir error: {e}"))?
        .join("icons")
        .join(resource_name);

    // Fallback for dev mode: CARGO_MANIFEST_DIR points to crates/hk-desktop/
    let resource_path = if resource_path.exists() {
        resource_path
    } else {
        let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("icons")
            .join(resource_name);
        if dev_path.exists() {
            dev_path
        } else {
            return Err(format!("Icon not found. Tried:\n  1. {}\n  2. {}", resource_path.display(), dev_path.display()));
        }
    };

    let png_data = std::fs::read(&resource_path)
        .map_err(|e| format!("Failed to read icon {}: {e}", resource_path.display()))?;

    unsafe {
        let data = NSData::with_bytes(&png_data);
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .ok_or("Failed to create NSImage from PNG data")?;
        let mtm = MainThreadMarker::new()
            .ok_or("set_app_icon must be called from the main thread")?;
        let app_instance = NSApplication::sharedApplication(mtm);
        app_instance.setApplicationIconImage(Some(&image));
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[command]
pub fn set_app_icon(_name: String) -> Result<(), String> {
    Ok(())
}
