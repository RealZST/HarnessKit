use axum::body::Body;
use axum::http::{Request, StatusCode};
use hk_core::{adapter, store::Store};
use hk_web::state::WebState;
use parking_lot::Mutex;
use std::sync::Arc;
use tower::ServiceExt;

// Keep TempDir alive so the database file isn't deleted during the test.
fn test_state() -> (WebState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let store = Store::open(&db_path).unwrap();
    // The allowlist's home is a sibling of the fixtures, not the real home:
    // on Windows the OS temp dir lives under the user profile, so fixture
    // paths would otherwise be allowed by the home-prefix check.
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let state = WebState {
        store: Arc::new(Mutex::new(store)),
        adapters: Arc::new(adapter::all_adapters()),
        token: None,
        node_name: "test-node".to_string(),
        home,
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
async fn server_info_returns_node_name() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    let response = app
        .oneshot(
            Request::post("/api/server_info")
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
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["node_name"], "test-node");
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

/// install_to_agent requires target_scope: an old client omitting the field
/// must get a client error, never a silent Global-scope install.
#[tokio::test]
async fn install_to_agent_missing_target_scope_is_client_error() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    let response = app
        .oneshot(
            Request::post("/api/install_to_agent")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"extension_id":"x","target_agent":"claude","hermes_category":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        response.status().is_client_error(),
        "missing target_scope must be rejected, got {}",
        response.status()
    );
}

/// Both ConfigScope wire shapes deserialize. The requests then 404 on the
/// unknown extension — reaching the handler body proves the scope parsed
/// (a shape error would be a 4xx from the Json extractor instead).
#[tokio::test]
async fn install_to_agent_accepts_both_scope_shapes() {
    for scope in [
        r#"{"type":"global"}"#,
        r#"{"type":"project","name":"p","path":"/tmp/p"}"#,
    ] {
        let (state, _tmp) = test_state();
        let app = hk_web::router::build_router(state);
        let body = format!(
            r#"{{"extension_id":"missing","target_agent":"claude","hermes_category":null,"target_scope":{scope}}}"#
        );
        let response = app
            .oneshot(
                Request::post("/api/install_to_agent")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "scope shape {scope} should parse and 404 on the missing extension"
        );
    }
}

/// Single-file extensions (e.g. Oh My Pi `.ts` plugins) live directly in the
/// agent's extension directory, so `list_skill_files` must return a one-entry
/// tree instead of a 404 — the Documentation panel walks
/// the tree and previews each file it finds.
#[tokio::test]
async fn list_skill_files_returns_single_entry_for_file() {
    let (state, tmp) = test_state();
    // Adapters' real skill dirs are not writable in tests; register the
    // fixture root as an allowed project so the path check passes.
    {
        let store = state.store.lock();
        store
            .insert_project(&hk_core::models::Project {
                id: "proj-test".into(),
                name: "test".into(),
                path: tmp.path().to_string_lossy().to_string(),
                created_at: chrono::Utc::now(),
                exists: true,
            })
            .unwrap();
    }
    let ext_dir = tmp.path().join("extensions");
    std::fs::create_dir_all(&ext_dir).unwrap();
    let file_path = ext_dir.join("my-plugin.ts");
    std::fs::write(&file_path, "// plugin\n").unwrap();

    let app = hk_web::router::build_router(state);
    let response = app
        .oneshot(
            Request::post("/api/list_skill_files")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": file_path.to_string_lossy() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = value.as_array().expect("array of FileEntry");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "my-plugin.ts");
    assert_eq!(entries[0]["is_dir"], false);
    assert_eq!(entries[0]["children"], serde_json::Value::Null);
}

/// The single-file branch must stay BEHIND the path allowlist: an existing
/// file outside every adapter root and registered project is rejected, never
/// listed. Locks the check ordering against future reordering.
#[tokio::test]
async fn list_skill_files_rejects_file_outside_allowed_roots() {
    let (state, tmp) = test_state();
    // No project registered for this fixture dir, and the state's home is a
    // sibling directory (see `test_state`), so the path is not allowed.
    let file_path = tmp.path().join("stray-plugin.ts");
    std::fs::write(&file_path, "// plugin\n").unwrap();

    let app = hk_web::router::build_router(state);
    let response = app
        .oneshot(
            Request::post("/api/list_skill_files")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": file_path.to_string_lossy() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        response.status().is_client_error(),
        "file outside allowed roots must be rejected, got {}",
        response.status()
    );
}

/// Directories keep the existing tree behavior.
#[tokio::test]
async fn list_skill_files_lists_directory_entries() {
    let (state, tmp) = test_state();
    {
        let store = state.store.lock();
        store
            .insert_project(&hk_core::models::Project {
                id: "proj-test".into(),
                name: "test".into(),
                path: tmp.path().to_string_lossy().to_string(),
                created_at: chrono::Utc::now(),
                exists: true,
            })
            .unwrap();
    }
    let skill_dir = tmp.path().join("my-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# skill\n").unwrap();

    let app = hk_web::router::build_router(state);
    let response = app
        .oneshot(
            Request::post("/api/list_skill_files")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": skill_dir.to_string_lossy() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let entries = value.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "SKILL.md");
}

/// Missing paths still 404.
#[tokio::test]
async fn list_skill_files_missing_path_is_not_found() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);
    let response = app
        .oneshot(
            Request::post("/api/list_skill_files")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"path":"/does/not/exist-xyz"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn repositories_api_crud() {
    let (state, _tmp) = test_state();
    let app = hk_web::router::build_router(state);

    // List initially empty
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/list_repositories")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(items.is_empty());

    // Add repository (with auth_token and trailing slash)
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/add_repository")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"url":"https://github.com/foo/bar.git/","name":"foo/bar","branch":"main","auth_token":"ghp_secret_token"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let repo: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let repo_id = repo["id"].as_str().unwrap().to_string();
    assert_eq!(repo["name"], "foo/bar");
    assert!(
        repo.get("auth_token").is_none(),
        "auth_token must never be serialized in add_repository response"
    );

    // Duplicate add_repository via owner/repo shorthand returns 409 Conflict
    let dup_res = app
        .clone()
        .oneshot(
            Request::post("/api/add_repository")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"url":"foo/bar"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dup_res.status(), StatusCode::CONFLICT);

    // List has 1 repository and omits auth_token
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/list_repositories")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["repository"]["id"], repo_id);
    assert!(
        items[0]["repository"].get("auth_token").is_none(),
        "auth_token must never be serialized in list_repositories response"
    );

    // set_repository_item_status rejects invalid kind and invalid status with 400 Bad Request
    let bad_kind_res = app
        .clone()
        .oneshot(
            Request::post("/api/set_repository_item_status")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"repo_id":"{repo_id}","item_id":"x","kind":"invalid_kind","status":"ignored"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_kind_res.status(), StatusCode::BAD_REQUEST);

    let bad_status_res = app
        .clone()
        .oneshot(
            Request::post("/api/set_repository_item_status")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"repo_id":"{repo_id}","item_id":"x","kind":"skill","status":"bogus"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_status_res.status(), StatusCode::BAD_REQUEST);

    // delete_repository and sync_repository reject invalid repo_id ("../escape") with 400 Bad Request
    for endpoint in ["/api/delete_repository", "/api/sync_repository"] {
        let bad_id_res = app
            .clone()
            .oneshot(
                Request::post(endpoint)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"id":"../escape"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            bad_id_res.status(),
            StatusCode::BAD_REQUEST,
            "{endpoint} must reject traversal id with 400"
        );
    }

    // update_repository_items rejects invalid repo_id with 400 Bad Request
    let bad_update_res = app
        .clone()
        .oneshot(
            Request::post("/api/update_repository_items")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_id":"../escape"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_update_res.status(), StatusCode::BAD_REQUEST);

    // update_repository_items returns 404 Not Found for non-existent repo_id
    let missing_update_res = app
        .clone()
        .oneshot(
            Request::post("/api/update_repository_items")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"repo_id":"missing-repo"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_update_res.status(), StatusCode::NOT_FOUND);

    // update_repository_items returns 0 when no items need updates (both per-repo and global)
    for body_json in [
        format!(r#"{{"repo_id":"{repo_id}"}}"#),
        r#"{"repo_id":null}"#.to_string(),
        r#"{}"#.to_string(),
    ] {
        let update_res = app
            .clone()
            .oneshot(
                Request::post("/api/update_repository_items")
                    .header("content-type", "application/json")
                    .body(Body::from(body_json))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update_res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(update_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let count: u32 = serde_json::from_slice(&body).unwrap();
        assert_eq!(count, 0);
    }

    // Delete repository
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/delete_repository")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"id":"{repo_id}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // List empty again
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/list_repositories")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(items.is_empty());
}
