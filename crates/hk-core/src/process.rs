//! Subprocess helpers.
//!
//! Every subprocess HarnessKit spawns (`git`, `where`, `npm`, agent
//! uninstallers, …) goes through [`command`]. On Windows a GUI process such as
//! the Tauri desktop app has no console of its own, so each console child would
//! otherwise flash a conhost window; the desktop opts into
//! [`hide_console_windows`] at startup to suppress that.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static HIDE_CONSOLE_WINDOWS: AtomicBool = AtomicBool::new(false);

/// Spawn every subsequent [`command`] with `CREATE_NO_WINDOW` on Windows.
///
/// Only a process without a console should call this: the desktop app,
/// which is console-less in release builds (`windows_subsystem = "windows"`). Console programs (`hk`,
/// `hk serve`) must keep the default: `CREATE_NO_WINDOW` gives the child no
/// console at all, so a child that needs the terminal — `git` asking for
/// credentials, `ssh` confirming a host key — cannot prompt and fails.
/// No-op on non-Windows platforms.
pub fn hide_console_windows() {
    HIDE_CONSOLE_WINDOWS.store(true, Ordering::Relaxed);
}

/// Create a `Command`, hiding the console window on Windows when
/// [`hide_console_windows`] was called. No-op on non-Windows platforms.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    if HIDE_CONSOLE_WINDOWS.load(Ordering::Relaxed) {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
