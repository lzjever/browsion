//! Integration tests for the local HTTP API.
//! Tests profile CRUD, running browsers endpoint, browser control error paths, and MCP/API key auth.

use axum::http::StatusCode;
use browsion_lib::api::{app, ApiState};
use browsion_lib::config::AppConfig;
use browsion_lib::state::AppState;
use std::sync::Arc;
use tower::ServiceExt;

fn make_state() -> ApiState {
    Arc::new(AppState::new(AppConfig::default()))
}

/// App without API key (current behaviour for router(state))
fn make_app_no_auth() -> axum::Router {
    app(make_state(), None)
}

/// App with API key required (except /api/health)
fn make_app_with_auth(api_key: &str) -> axum::Router {
    app(make_state(), Some(api_key.to_string()))
}

fn json_body(val: &serde_json::Value) -> axum::body::Body {
    axum::body::Body::from(serde_json::to_vec(val).unwrap())
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_health() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/health")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"ok");
}

// ---------------------------------------------------------------------------
// Profile CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_list_profiles_empty() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(json.is_empty());
}

#[tokio::test]
async fn test_api_get_profile_not_found() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/profiles/nonexistent-id")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_api_add_and_get_profile() {
    let state = make_state();
    let api_app = app(state.clone(), None);

    let profile = serde_json::json!({
        "id": "test-001",
        "name": "Test Profile",
        "description": "",
        "user_data_dir": "/tmp/test-profile-001",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .uri("/api/profiles/test-001")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let p: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(p["name"], "Test Profile");
}

#[tokio::test]
async fn test_api_delete_profile() {
    let state = make_state();
    let api_app = app(state.clone(), None);

    let profile = serde_json::json!({
        "id": "del-001",
        "name": "To Delete",
        "description": "",
        "user_data_dir": "/tmp/del",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    let _res = api_app.oneshot(req).await.unwrap();

    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/profiles/del-001")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .uri("/api/profiles/del-001")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Running browsers endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_running_browsers_empty() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/running")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(json.is_empty());
}

// ---------------------------------------------------------------------------
// Browser control: error paths (browser not running)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_browser_navigate_not_running() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/browser/fake-id/navigate")
        .header("content-type", "application/json")
        .body(json_body(&serde_json::json!({ "url": "https://example.com" })))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_browser_screenshot_not_running() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/browser/fake-id/screenshot")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_browser_dom_context_not_running() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/browser/fake-id/dom_context")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_browser_evaluate_not_running() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/browser/fake-id/evaluate")
        .header("content-type", "application/json")
        .body(json_body(
            &serde_json::json!({ "expression": "document.title" }),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_browser_tabs_not_running() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/browser/fake-id/tabs")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_browser_cookies_not_running() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/browser/fake-id/cookies")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// List profiles includes is_running flag
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_profiles_includes_is_running() {
    let state = make_state();
    let api_app = app(state.clone(), None);

    let profile = serde_json::json!({
        "id": "flag-test",
        "name": "Flag Test",
        "description": "",
        "user_data_dir": "/tmp/flag-test",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    api_app.oneshot(req).await.unwrap();

    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(!json.is_empty());
    assert_eq!(json[0]["is_running"], false);
}

// ---------------------------------------------------------------------------
// MCP / API key authentication
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_health_bypasses_auth_when_key_set() {
    let app = make_app_with_auth("secret-key");
    let req = axum::http::Request::builder()
        .uri("/api/health")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn test_api_profiles_unauthorized_without_key() {
    let app = make_app_with_auth("secret-key");
    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_profiles_unauthorized_with_wrong_key() {
    let app = make_app_with_auth("secret-key");
    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .header("X-API-Key", "wrong-key")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_profiles_ok_with_correct_key() {
    let app = make_app_with_auth("secret-key");
    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .header("X-API-Key", "secret-key")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_running_unauthorized_without_key() {
    let app = make_app_with_auth("my-api-key");
    let req = axum::http::Request::builder()
        .uri("/api/running")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_api_running_ok_with_correct_key() {
    let app = make_app_with_auth("my-api-key");
    let req = axum::http::Request::builder()
        .uri("/api/running")
        .header("X-API-Key", "my-api-key")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// run_server: bind failure when port in use
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_run_server_fails_when_port_in_use() {
    use browsion_lib::api::run_server;
    use tokio::net::TcpListener;

    let state = make_state();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let result = tokio::spawn(async move { run_server(state, port, None).await })
        .await
        .unwrap();
    drop(listener);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to bind"));
}

// ---------------------------------------------------------------------------
// Profile update
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_update_profile() {
    let state = make_state();
    let api_app = app(state.clone(), None);

    // Create profile first
    let profile = serde_json::json!({
        "id": "upd-001",
        "name": "Original",
        "description": "",
        "user_data_dir": "/tmp/upd-001",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    api_app.oneshot(req).await.unwrap();

    // Update it
    let api_app = app(state.clone(), None);
    let updated = serde_json::json!({
        "id": "upd-001",
        "name": "Updated Name",
        "description": "new desc",
        "user_data_dir": "/tmp/upd-001",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });
    let req = axum::http::Request::builder()
        .method("PUT")
        .uri("/api/profiles/upd-001")
        .header("content-type", "application/json")
        .body(json_body(&updated))
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Verify change persisted
    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .uri("/api/profiles/upd-001")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let p: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(p["name"], "Updated Name");
    assert_eq!(p["description"], "new desc");
}

#[tokio::test]
async fn test_api_update_profile_not_found() {
    let app = make_app_no_auth();
    let updated = serde_json::json!({
        "id": "no-such",
        "name": "X",
        "description": "",
        "user_data_dir": "/tmp/x",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });
    let req = axum::http::Request::builder()
        .method("PUT")
        .uri("/api/profiles/no-such")
        .header("content-type", "application/json")
        .body(json_body(&updated))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Action log endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_action_log_get_returns_ok_with_array() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    // Response must be a valid JSON array (empty when log is fresh)
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(entries.is_empty());
}

#[tokio::test]
async fn test_api_action_log_delete_returns_no_content() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_api_action_log_entries_have_expected_shape() {
    // Push a synthetic entry by calling another API endpoint first, then read log.
    // Since the action log middleware skips /api/action_log and /api/health routes,
    // calling /api/profiles will produce an entry.
    let state = make_state();

    // Trigger a logged request via GET /api/profiles
    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .body(axum::body::Body::empty())
        .unwrap();
    api_app.oneshot(req).await.unwrap();

    // Give the spawned log task a moment to complete
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Read the action log
    let api_app = app(state.clone(), None);
    let req = axum::http::Request::builder()
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = api_app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // There should be exactly one entry from the /api/profiles call above
    assert_eq!(entries.len(), 1, "Expected 1 action log entry");
    let entry = &entries[0];

    // Verify required fields from ActionEntry struct
    assert!(entry["id"].is_string(), "id must be a string");
    assert!(entry["ts"].is_number(), "ts must be a number (Unix ms)");
    assert!(
        entry["profile_id"].is_string(),
        "profile_id must be a string"
    );
    assert!(entry["tool"].is_string(), "tool must be a string");
    assert!(
        entry["duration_ms"].is_number(),
        "duration_ms must be a number"
    );
    assert!(entry["success"].is_boolean(), "success must be a boolean");
    // `error` field is optional (skip_serializing_if = None) so no assertion needed
}

// ---------------------------------------------------------------------------
// Profile snapshots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_profile_snapshots_list_unknown_profile() {
    let app = make_app_no_auth();
    let req = axum::http::Request::builder()
        .uri("/api/profiles/ghost-profile/snapshots")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    // core_list_snapshots reads the manifest file; when none exists it returns
    // an empty HashMap (not an error), so the handler always returns 200 OK
    // with an empty JSON array for an unknown profile.
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let infos: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(infos.is_empty());
}
