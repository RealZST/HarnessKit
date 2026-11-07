use hk_core::adapter;
use hk_core::store::Store;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
pub struct WebState {
    pub store: Arc<Mutex<Store>>,
    pub adapters: Arc<Vec<Box<dyn adapter::AgentAdapter>>>,
    /// None means no auth required (localhost-only mode)
    pub token: Option<String>,
    /// Human-readable name for this node, shown in the web UI so multiple
    /// tabs pointing at different remote hosts are distinguishable. Defaults
    /// to the machine hostname; overridable via `hk serve --name`.
    pub node_name: String,
    /// Home directory that path-allowlist checks accept as a root.
    /// `hk_core::home_dir()` in production; tests inject a fixture directory
    /// so the check is deterministic even where the OS temp dir lives under
    /// the user profile (Windows).
    pub home: std::path::PathBuf,
}
