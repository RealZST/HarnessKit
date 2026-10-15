//! Subprocess helpers.
//!
//! On Windows every spawned process is created with `CREATE_NO_WINDOW` so the
//! Tauri desktop build (a GUI process with no console) never flashes a console
//! window when probing binaries (`where`, `git`, `npm`, …).

use std::process::Command;

/// Create a `Command` that never pops a console window on Windows.
/// No-op on non-Windows platforms.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
