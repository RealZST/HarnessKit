//! Native title bar colors on Windows.
//!
//! With "Transparency effects" on, Windows 11 paints every window's caption
//! with Mica, a tint derived from the desktop wallpaper, which clashes with
//! the app's own palette. An explicit caption color opts the window out of
//! that tint (DWM applies it regardless of the user's transparency setting).
//! The frontend sends the sidebar colors whenever the theme changes so the
//! title bar reads as part of the sidebar below it.

use tauri::command;

/// Parse `#rrggbb` into a Win32 `COLORREF` (`0x00BBGGRR`).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn colorref(hex: &str) -> Result<u32, String> {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    if digits.len() != 6 {
        return Err(format!("expected #rrggbb, got {hex:?}"));
    }
    let rgb = u32::from_str_radix(digits, 16).map_err(|e| format!("bad color {hex:?}: {e}"))?;
    let (r, g, b) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
    Ok((b << 16) | (g << 8) | r)
}

#[cfg(target_os = "windows")]
#[command]
pub fn set_title_bar_color(
    window: tauri::WebviewWindow,
    background: String,
    text: String,
) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    };

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
    for (attribute, hex) in [(DWMWA_CAPTION_COLOR, &background), (DWMWA_TEXT_COLOR, &text)] {
        let color = colorref(hex)?;
        // SAFETY: `hwnd` is this window's live handle, and the attribute takes
        // a COLORREF, which `color` is, passed with its size.
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute as u32,
                &color as *const u32 as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        // The result is deliberately ignored: Windows 10 rejects these
        // attributes (it has no Mica to opt out of either), and a failure
        // here is cosmetic.
    }
    Ok(())
}

/// Only Windows paints a native title bar whose color the app can set.
#[cfg(not(target_os = "windows"))]
#[command]
pub fn set_title_bar_color(_background: String, _text: String) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::colorref;

    #[test]
    fn colorref_swaps_red_and_blue_bytes() {
        assert_eq!(colorref("#112233").unwrap(), 0x0033_2211);
        assert_eq!(colorref("ffffff").unwrap(), 0x00ff_ffff);
    }

    #[test]
    fn colorref_rejects_anything_but_six_hex_digits() {
        assert!(colorref("#fff").is_err());
        assert!(colorref("#gg0000").is_err());
        assert!(colorref("rgb(1, 2, 3)").is_err());
    }
}
