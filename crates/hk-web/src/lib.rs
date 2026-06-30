mod auth;
mod handlers;
pub mod router;
pub mod state;

use hk_core::{adapter, store::Store};
use parking_lot::Mutex;
use state::WebState;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

pub struct ServeOptions {
    pub port: u16,
    pub host: String,
    pub token: Option<String>,
    /// Optional node label override; falls back to the machine hostname.
    pub name: Option<String>,
}

/// Resolve the display name for this node: the explicit `--name` if given,
/// otherwise the machine hostname (best-effort; "unknown-host" if unavailable).
fn resolve_node_name(name: Option<String>) -> String {
    name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "unknown-host".to_string())
    })
}

/// Non-loopback, non-link-local IPv4 addresses of this machine, used to print
/// reachable URLs when bound to 0.0.0.0 (which itself isn't a usable address).
fn reachable_ipv4_addrs() -> Vec<Ipv4Addr> {
    let Ok(ifaces) = local_ip_address::list_afinet_netifas() else {
        return Vec::new();
    };
    let mut addrs: Vec<Ipv4Addr> = ifaces
        .into_iter()
        .filter_map(|(_, ip)| match ip {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => Some(v4),
            _ => None,
        })
        .collect();
    addrs.sort();
    addrs.dedup();
    addrs
}

pub async fn serve(options: ServeOptions) -> anyhow::Result<()> {
    let data_dir = dirs::home_dir()
        .expect("Cannot determine home directory")
        .join(".harnesskit");
    std::fs::create_dir_all(&data_dir)?;
    let store = Store::open(&data_dir.join("metadata.db"))?;

    let node_name = resolve_node_name(options.name.clone());

    let state = WebState {
        store: Arc::new(Mutex::new(store)),
        adapters: Arc::new(adapter::all_adapters()),
        pending_clones: Arc::new(Mutex::new(HashMap::new())),
        token: options.token.clone(),
        node_name: node_name.clone(),
    };

    let app = router::build_router(state);
    let addr: SocketAddr = format!("{}:{}", options.host, options.port).parse()?;

    // When auth is enabled, embed the token in the URL so the user can paste a
    // single link and be logged in — the frontend reads it and strips it from
    // the address bar. Mirrors Jupyter's `?token=` flow.
    let token_query = options
        .token
        .as_deref()
        .map(|t| format!("/?token={t}"))
        .unwrap_or_default();

    match options.host.as_str() {
        "127.0.0.1" => {
            eprintln!("HarnessKit Web UI [{node_name}] running at http://{addr}{token_query}");
            eprintln!("Access via SSH tunnel: ssh -L {p}:localhost:{p} your-server", p = options.port);
        }
        // 0.0.0.0 binds every interface but is not itself a reachable address,
        // so don't present it as a clickable URL — print the actual LAN IPs.
        "0.0.0.0" => {
            eprintln!("HarnessKit Web UI [{node_name}] listening on all interfaces (port {})", options.port);
            let addrs = reachable_ipv4_addrs();
            if addrs.is_empty() {
                eprintln!("Use this machine's LAN IP at port {}", options.port);
            } else {
                for ip in addrs {
                    eprintln!("Reachable on your network at http://{ip}:{}{token_query}", options.port);
                }
            }
        }
        _ => {
            eprintln!("HarnessKit Web UI [{node_name}] running at http://{addr}{token_query}");
        }
    }
    if let Some(token) = &options.token {
        eprintln!("Auth token: {token}");
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
