mod helpers;
pub mod extensions;
pub mod marketplace;
pub mod install;
pub mod agents;
pub mod audit;
pub mod projects;
pub mod settings;

// Re-export shared types that appear in Tauri command signatures.
// The Tauri proc macro requires these types to be publicly reachable.
#[allow(unused_imports)]
pub use helpers::{FileEntry, list_dir_entries};

// Re-export all commands at top level so main.rs doesn't need to change
pub use extensions::*;
pub use marketplace::*;
pub use install::*;
pub use agents::*;
pub use audit::*;
pub use projects::*;
pub use settings::*;

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use hk_core::store::Store;

pub struct PendingClone {
    pub _temp_dir: tempfile::TempDir,
    pub clone_dir: std::path::PathBuf,
    pub url: String,
    pub created_at: std::time::Instant,
}

pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub pending_clones: Arc<Mutex<HashMap<String, PendingClone>>>,
}
