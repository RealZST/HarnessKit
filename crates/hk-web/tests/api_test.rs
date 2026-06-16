use axum::body::Body;
use axum::http::{Request, StatusCode};
use hk_core::{adapter, store::Store};
use hk_web::state::WebState;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

// Keep TempDir alive so the database file isn't deleted during the test.
fn test_state() -> (WebState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let store = Store::open(&db_path).unwrap();
    let state = WebState {
        store: Arc::new(Mutex::new(store)),
        adapters: Arc::new(adapter::all_adapters()),
        pending_clones: Arc::new(Mutex::new(HashMap::new())),
        token: None,
    };
    (state, tmp)
}

#[tokio::test]
async fn health_returns_ok() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    let response = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn list_extensions_returns_array() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    let response = app
        .oneshot(
            Request::post("/api/list_extensions")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_required_when_token_set() {
    let (mut state, _tmp) = test_state();
    state.token = Some("secret123".into());
    let app = hk_web::router::build_router(state);

    // Without token — should be 401
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/list_extensions")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // With token — should be 200
    let response = app
        .oneshot(
            Request::post("/api/list_extensions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer secret123")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn dashboard_stats_returns_valid_json() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    let response = app
        .oneshot(
            Request::post("/api/get_dashboard_stats")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(stats["total_extensions"].is_number());
}

/// Regression guard for the web-mode Kits outage: the frontend transport posts
/// to `/api/{command}` (e.g. `/api/list_kit_asset_candidates`), but the kit
/// routes were once registered REST-style (`GET /api/kits/candidates`). The
/// mismatch fell through to the SPA fallback, returning `200 text/html`, which
/// the frontend then failed to parse as JSON — so kits/candidates silently
/// showed empty in the browser while desktop worked. Assert every kit command
/// the frontend calls reaches a real JSON handler.
#[tokio::test]
async fn kit_command_routes_return_json_not_spa_html() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    // Read-only commands the frontend posts with an empty body. Each must hit a
    // handler (200 + application/json), not the HTML SPA fallback. `list_kits`
    // and `list_project_install_records` return JSON arrays; the candidates
    // command returns a `{ extensions, config_files }` object — so we only
    // require valid JSON of the right shape, the point being it isn't HTML.
    for command in [
        "list_kits",
        "list_kit_asset_candidates",
        "list_project_install_records",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post(format!("/api/{command}"))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "POST /api/{command} should reach a handler"
        );
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            content_type.starts_with("application/json"),
            "POST /api/{command} returned {content_type}, not JSON (SPA fallback?)"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        // The decisive guard against the SPA-fallback regression: the body must
        // parse as JSON at all (HTML would fail here) and be a real array/object
        // rather than a bare string like an HTML document slurped as text.
        let value: serde_json::Value = serde_json::from_slice(&body)
            .unwrap_or_else(|e| panic!("POST /api/{command} returned non-JSON body: {e}"));
        assert!(
            value.is_array() || value.is_object(),
            "POST /api/{command} should return a JSON array or object, got: {value}"
        );
    }
}
