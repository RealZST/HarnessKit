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
