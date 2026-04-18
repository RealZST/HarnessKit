mod auth;
mod handlers;
mod router;
pub mod state;

use hk_core::{adapter, store::Store};
use parking_lot::Mutex;
use state::WebState;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct ServeOptions {
    pub port: u16,
    pub host: String,
    pub token: Option<String>,
}

pub async fn serve(options: ServeOptions) -> anyhow::Result<()> {
    let data_dir = dirs::home_dir()
        .expect("Cannot determine home directory")
        .join(".harnesskit");
    std::fs::create_dir_all(&data_dir)?;
    let store = Store::open(&data_dir.join("metadata.db"))?;

    let state = WebState {
        store: Arc::new(Mutex::new(store)),
        adapters: Arc::new(adapter::all_adapters()),
        pending_clones: Arc::new(Mutex::new(HashMap::new())),
        token: options.token.clone(),
    };

    let app = router::build_router(state);
    let addr: SocketAddr = format!("{}:{}", options.host, options.port).parse()?;

    eprintln!("HarnessKit Web UI running at http://{addr}");
    if options.host == "127.0.0.1" {
        eprintln!("Access via SSH tunnel: ssh -L {p}:localhost:{p} your-server", p = options.port);
    }
    if let Some(token) = &options.token {
        eprintln!("Auth token: {token}");
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
