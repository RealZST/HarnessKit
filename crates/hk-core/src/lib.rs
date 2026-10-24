pub mod adapter;
pub mod auditor;
pub mod config;
pub mod deployer;
pub mod error;
pub mod kits;
pub mod manager;
pub mod marketplace;
pub mod models;
pub mod process;
pub mod sanitize;
pub mod scanner;
pub mod service;
pub mod skills_cli;
pub mod store;

pub use error::HkError;
pub use process::command;

/// Process-wide home directory used by every adapter and path helper.
///
/// `HARNESSKIT_HOME` wins when set (test isolation on Windows, where
/// `dirs::home_dir()` reads the shell profile folder and cannot be
/// redirected); otherwise it is `dirs::home_dir()`.
pub fn home_dir() -> std::path::PathBuf {
    if let Some(root) = std::env::var_os("HARNESSKIT_HOME").filter(|v| !v.is_empty()) {
        return std::path::PathBuf::from(root);
    }
    dirs::home_dir().unwrap_or_default()
}
