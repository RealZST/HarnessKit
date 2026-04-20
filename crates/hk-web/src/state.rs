use hk_core::adapter;
use hk_core::store::Store;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct PendingClone {
    pub _temp_dir: tempfile::TempDir,
    pub clone_dir: std::path::PathBuf,
    pub url: String,
    pub created_at: std::time::Instant,
}

#[derive(Clone)]
pub struct WebState {
    pub store: Arc<Mutex<Store>>,
    pub adapters: Arc<Vec<Box<dyn adapter::AgentAdapter>>>,
    pub pending_clones: Arc<Mutex<HashMap<String, PendingClone>>>,
    /// None means no auth required (localhost-only mode)
    pub token: Option<String>,
}
