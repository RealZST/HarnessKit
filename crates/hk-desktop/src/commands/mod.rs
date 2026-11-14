pub mod agents;
pub mod audit;
pub mod extensions;
mod helpers;
pub mod install;
pub mod kits;
pub mod marketplace;
pub mod projects;
pub mod repositories;
pub mod settings;

// Re-export shared types that appear in Tauri command signatures.
// The Tauri proc macro requires these types to be publicly reachable.
#[allow(unused_imports)]
pub use helpers::{FileEntry, list_dir_entries};

// Re-export all commands at top level so main.rs doesn't need to change
pub use agents::*;
pub use audit::*;
pub use extensions::*;
pub use install::*;
pub use kits::*;
pub use marketplace::*;
pub use projects::*;
pub use repositories::*;
pub use settings::*;

use hk_core::adapter;
use hk_core::store::Store;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub adapters: Arc<Vec<Box<dyn adapter::AgentAdapter>>>,
}
