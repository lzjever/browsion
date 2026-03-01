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
// Browser control: comprehensive not-running error paths
// ---------------------------------------------------------------------------

macro_rules! browser_not_running_get {
    ($name:ident, $path:literal) => {
        #[tokio::test]
        async fn $name() {
            let app = make_app_no_auth();
            let req = axum::http::Request::builder()
                .uri(concat!("/api/browser/fake-id/", $path))
                .body(axum::body::Body::empty())
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
    };
}

macro_rules! browser_not_running_post {
    ($name:ident, $path:literal) => {
        #[tokio::test]
        async fn $name() {
            let app = make_app_no_auth();
            let req = axum::http::Request::builder()
                .method("POST")
                .uri(concat!("/api/browser/fake-id/", $path))
                .header("content-type", "application/json")
                .body(json_body(&serde_json::json!({})))
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
    };
    ($name:ident, $path:literal, $body:expr) => {
        #[tokio::test]
        async fn $name() {
            let app = make_app_no_auth();
            let req = axum::http::Request::builder()
                .method("POST")
                .uri(concat!("/api/browser/fake-id/", $path))
                .header("content-type", "application/json")
                .body(json_body(&$body))
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
    };
}

macro_rules! browser_not_running_delete {
    ($name:ident, $path:literal) => {
        #[tokio::test]
        async fn $name() {
            let app = make_app_no_auth();
            let req = axum::http::Request::builder()
                .method("DELETE")
                .uri(concat!("/api/browser/fake-id/", $path))
                .body(axum::body::Body::empty())
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
    };
    ($name:ident, $path:literal, $body:expr) => {
        #[tokio::test]
        async fn $name() {
            let app = make_app_no_auth();
            let req = axum::http::Request::builder()
                .method("DELETE")
                .uri(concat!("/api/browser/fake-id/", $path))
                .header("content-type", "application/json")
                .body(json_body(&$body))
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            assert_eq!(res.status(), StatusCode::CONFLICT);
        }
    };
}

// GET routes
browser_not_running_get!(test_browser_url_not_running, "url");
browser_not_running_get!(test_browser_title_not_running, "title");
browser_not_running_get!(test_browser_ax_tree_not_running, "ax_tree");
browser_not_running_get!(test_browser_page_state_not_running, "page_state");
browser_not_running_get!(test_browser_screenshot_element_not_running, "screenshot_element?selector=body");
browser_not_running_get!(test_browser_network_log_not_running, "network_log");
browser_not_running_get!(test_browser_console_not_running, "console");
browser_not_running_get!(test_browser_page_text_not_running, "page_text");
browser_not_running_get!(test_browser_storage_get_not_running, "storage");
browser_not_running_get!(test_browser_pdf_not_running, "pdf");
browser_not_running_get!(test_browser_frames_not_running, "frames");
browser_not_running_get!(test_browser_export_cookies_not_running, "cookies/export");

// POST routes — no required fields (empty {} is valid)
browser_not_running_post!(test_browser_back_not_running, "back");
browser_not_running_post!(test_browser_forward_not_running, "forward");
browser_not_running_post!(test_browser_reload_not_running, "reload");
browser_not_running_post!(test_browser_wait_for_nav_not_running, "wait_for_nav");
browser_not_running_post!(test_browser_new_tab_not_running, "tabs/new");
browser_not_running_post!(test_browser_wait_new_tab_not_running, "tabs/wait_new");
browser_not_running_post!(test_browser_delete_cookies_not_running, "cookies/clear");
browser_not_running_post!(test_browser_enable_console_not_running, "console/enable");
browser_not_running_post!(test_browser_clear_console_not_running, "console/clear");
browser_not_running_post!(test_browser_clear_network_log_not_running, "network_log/clear");
browser_not_running_post!(test_browser_emulate_not_running, "emulate");
browser_not_running_post!(test_browser_main_frame_not_running, "main_frame");
browser_not_running_post!(test_browser_navigate_wait_not_running, "navigate_wait",
    serde_json::json!({ "url": "https://example.com" }));

// POST routes — required fields: selector
browser_not_running_post!(test_browser_click_not_running, "click",
    serde_json::json!({ "selector": "button" }));
browser_not_running_post!(test_browser_hover_not_running, "hover",
    serde_json::json!({ "selector": "button" }));
browser_not_running_post!(test_browser_double_click_not_running, "double_click",
    serde_json::json!({ "selector": "button" }));
browser_not_running_post!(test_browser_right_click_not_running, "right_click",
    serde_json::json!({ "selector": "button" }));
browser_not_running_post!(test_browser_scroll_into_view_not_running, "scroll_into_view",
    serde_json::json!({ "selector": "button" }));
browser_not_running_post!(test_browser_wait_for_not_running, "wait_for",
    serde_json::json!({ "selector": "button" }));
browser_not_running_post!(test_browser_tap_not_running, "tap",
    serde_json::json!({ "selector": "button" }));

// POST routes — required fields: selector + text
browser_not_running_post!(test_browser_type_not_running, "type",
    serde_json::json!({ "selector": "input", "text": "hello" }));
browser_not_running_post!(test_browser_slow_type_not_running, "slow_type",
    serde_json::json!({ "selector": "input", "text": "hello" }));

// POST routes — required fields: selector + other
browser_not_running_post!(test_browser_select_option_not_running, "select_option",
    serde_json::json!({ "selector": "select", "value": "opt" }));
browser_not_running_post!(test_browser_upload_file_not_running, "upload_file",
    serde_json::json!({ "selector": "input", "file_path": "/tmp/file.txt" }));
browser_not_running_post!(test_browser_scroll_element_not_running, "scroll_element",
    serde_json::json!({ "selector": "div" }));
browser_not_running_post!(test_browser_swipe_not_running, "swipe",
    serde_json::json!({ "selector": "div", "direction": "up" }));

// POST routes — required fields: key / ref_id
browser_not_running_post!(test_browser_press_key_not_running, "press_key",
    serde_json::json!({ "key": "Enter" }));
browser_not_running_post!(test_browser_click_ref_not_running, "click_ref",
    serde_json::json!({ "ref_id": "e1" }));
browser_not_running_post!(test_browser_focus_ref_not_running, "focus_ref",
    serde_json::json!({ "ref_id": "e1" }));
browser_not_running_post!(test_browser_type_ref_not_running, "type_ref",
    serde_json::json!({ "ref_id": "e1", "text": "hello" }));

// POST routes — required fields: direction
browser_not_running_post!(test_browser_scroll_not_running, "scroll",
    serde_json::json!({ "direction": "down" }));

// POST routes — other required fields
browser_not_running_post!(test_browser_extract_not_running, "extract",
    serde_json::json!({ "selectors": {} }));
browser_not_running_post!(test_browser_switch_tab_not_running, "tabs/switch",
    serde_json::json!({ "target_id": "some-target" }));
browser_not_running_post!(test_browser_close_tab_not_running, "tabs/close",
    serde_json::json!({ "target_id": "some-target" }));
browser_not_running_post!(test_browser_set_cookie_not_running, "cookies/set",
    serde_json::json!({ "name": "test", "value": "val", "domain": "example.com" }));
browser_not_running_post!(test_browser_handle_dialog_not_running, "handle_dialog",
    serde_json::json!({ "action": "accept" }));
browser_not_running_post!(test_browser_click_at_not_running, "click_at",
    serde_json::json!({ "x": 100.0, "y": 200.0 }));
browser_not_running_post!(test_browser_drag_not_running, "drag",
    serde_json::json!({ "from_selector": "#a", "to_selector": "#b" }));
browser_not_running_post!(test_browser_wait_for_text_not_running, "wait_for_text",
    serde_json::json!({ "text": "hello" }));
browser_not_running_post!(test_browser_wait_for_url_not_running, "wait_for_url",
    serde_json::json!({ "pattern": "example.com" }));
browser_not_running_post!(test_browser_intercept_block_not_running, "intercept/block",
    serde_json::json!({ "url_pattern": "*.js" }));
browser_not_running_post!(test_browser_intercept_mock_not_running, "intercept/mock",
    serde_json::json!({ "url_pattern": "*.js", "status": 200, "body": "{}" }));
browser_not_running_post!(test_browser_switch_frame_not_running, "switch_frame",
    serde_json::json!({ "frame_id": "frame-1" }));

// POST routes — cookies/import (requires format + data)
browser_not_running_post!(test_browser_import_cookies_not_running, "cookies/import",
    serde_json::json!({ "format": "json", "data": "[]" }));

// POST routes — storage set (requires key + value)
browser_not_running_post!(test_browser_set_storage_not_running, "storage",
    serde_json::json!({ "key": "myKey", "value": "myVal" }));

// DELETE routes
browser_not_running_delete!(test_browser_clear_intercepts_not_running, "intercept");
browser_not_running_delete!(test_browser_clear_storage_not_running, "storage",
    serde_json::json!({}));

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
