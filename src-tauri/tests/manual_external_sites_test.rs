//! Manual smoke tests against live external websites.
//!
//! These are intentionally `ignored` because they depend on public internet
//! reachability and third-party site behavior. Run them manually:
//!
//! `cargo test --test manual_external_sites_test -- --ignored --nocapture --test-threads=1`

use axum::http::StatusCode;
use base64::Engine as _;
use browsion_lib::config::AppConfig;
use browsion_lib::state::AppState;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn make_state() -> Arc<AppState> {
    Arc::new(AppState::new(AppConfig::default()))
}

fn run_server(state: Arc<AppState>, port: u16, api_key: Option<String>) {
    tokio::spawn(async move {
        let _ = browsion_lib::api::run_server(state, port, api_key).await;
    });
}

async fn wait_for_api_ready(port: u16, api_key: Option<&str>) {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/health", port);
    for _ in 0..50 {
        let mut request = client.get(&url);
        if let Some(api_key) = api_key {
            request = request.header("X-API-Key", api_key);
        }
        if let Ok(response) = request.send().await {
            if response.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    panic!("API server on port {} did not become ready in time", port);
}

fn find_chrome() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CHROME_PATH") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return Some(pb);
        }
    }

    for path in [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
        "/usr/local/bin/google-chrome",
        "/snap/bin/chromium",
    ] {
        let pb = PathBuf::from(path);
        if pb.exists() {
            return Some(pb);
        }
    }

    for name in ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"] {
        if let Ok(out) = Command::new("which").arg(name).output() {
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let pb = PathBuf::from(p);
                if pb.exists() {
                    return Some(pb);
                }
            }
        }
    }

    None
}

fn artifact_dir() -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/external-smoke")
        .join(ts.to_string());
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn unique_suffix() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
}

async fn api_post_json(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    path: &str,
    body: Value,
) -> Value {
    let response = client
        .post(format!("{api_base}{path}"))
        .header("X-API-Key", api_key)
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {} failed: {}", path, e));
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "POST {} failed with {}: {}",
        path,
        status,
        text
    );
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

async fn api_post_empty(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    path: &str,
) -> Value {
    let response = client
        .post(format!("{api_base}{path}"))
        .header("X-API-Key", api_key)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {} failed: {}", path, e));
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "POST {} failed with {}: {}",
        path,
        status,
        text
    );
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

async fn api_get_json(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    path: &str,
) -> Value {
    let response = client
        .get(format!("{api_base}{path}"))
        .header("X-API-Key", api_key)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {} failed: {}", path, e));
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "GET {} failed with {}: {}",
        path,
        status,
        text
    );
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

fn write_screenshot(value: &Value, path: &Path) {
    let image = value
        .get("image")
        .and_then(|v| v.as_str())
        .expect("screenshot image missing");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image)
        .expect("invalid base64 screenshot");
    fs::write(path, bytes).expect("failed to write screenshot");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "uses live external websites"]
async fn test_live_external_sites_smoke() {
    let Some(chrome_path) = find_chrome() else {
        eprintln!("SKIP: no Chrome");
        return;
    };

    let state = make_state();
    let api_port = 39540u16;
    let api_key = "external-smoke-key";
    {
        let mut config = state.config.write();
        config.chrome_path = Some(chrome_path);
        config.mcp.enabled = true;
        config.mcp.api_port = api_port;
        config.mcp.api_key = Some(api_key.to_string());
    }
    run_server(state.clone(), api_port, Some(api_key.to_string()));
    wait_for_api_ready(api_port, Some(api_key)).await;

    let artifact_dir = artifact_dir();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .unwrap();
    let api_base = format!("http://127.0.0.1:{}", api_port);
    let suffix = unique_suffix();
    let profile_id = format!("external-smoke-{}", suffix);
    let user_data_dir = std::env::temp_dir().join(format!("browsion-external-smoke-{}", suffix));
    let _ = fs::remove_dir_all(&user_data_dir);
    fs::create_dir_all(&user_data_dir).unwrap();

    let profile = serde_json::json!({
        "id": profile_id,
        "name": "External Smoke",
        "description": "Manual smoke against live websites",
        "user_data_dir": user_data_dir.to_str().unwrap(),
        "lang": "en-US",
        "tags": ["manual", "external"],
        "custom_args": ["--ignore-certificate-errors"],
        "headless": true
    });
    let create_response = client
        .post(format!("{}/api/profiles", api_base))
        .header("X-API-Key", api_key)
        .json(&profile)
        .send()
        .await
        .expect("create profile failed");
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let launch = api_post_empty(&client, &api_base, api_key, &format!("/api/launch/{}", profile_id)).await;
    assert!(
        launch.get("cdp_port").and_then(|v| v.as_u64()).is_some(),
        "launch should return cdp_port: {:?}",
        launch
    );

    // Case 1: httpbin HTML page basic navigation + screenshot
    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/navigate_wait", profile_id),
        serde_json::json!({
            "url": "http://httpbin.org/html",
            "wait_until": "load",
            "timeout_ms": 45000
        }),
    )
    .await;
    let page_text = api_get_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/page_text", profile_id),
    )
    .await;
    assert!(
        page_text
            .get("text")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("Moby-Dick"))
            .unwrap_or(false),
        "unexpected httpbin html page text: {:?}",
        page_text
    );
    let html_shot = api_get_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/screenshot?full_page=true", profile_id),
    )
    .await;
    write_screenshot(&html_shot, &artifact_dir.join("httpbin-html.png"));

    // Case 2: httpbin form recording + playback
    let start = api_post_empty(
        &client,
        &api_base,
        api_key,
        &format!("/api/recordings/start/{}", profile_id),
    )
    .await;
    let session_id = start
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("recording session_id missing")
        .to_string();
    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/navigate_wait", profile_id),
        serde_json::json!({
            "url": "http://httpbin.org/forms/post",
            "wait_until": "load",
            "timeout_ms": 45000
        }),
    )
    .await;
    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/type", profile_id),
        serde_json::json!({
            "selector": "input[name='custname']",
            "text": "OpenAI Agent"
        }),
    )
    .await;
    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/type", profile_id),
        serde_json::json!({
            "selector": "input[name='custemail']",
            "text": "agent@example.com"
        }),
    )
    .await;
    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/click", profile_id),
        serde_json::json!({ "selector": "input[name='size'][value='medium']" }),
    )
    .await;
    let form_state = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/evaluate", profile_id),
        serde_json::json!({
            "expression": "JSON.stringify({ custname: document.querySelector(\"input[name='custname']\")?.value, custemail: document.querySelector(\"input[name='custemail']\")?.value, size: document.querySelector(\"input[name='size'][value='medium']\")?.checked })"
        }),
    )
    .await;
    assert!(
        form_state
            .get("result")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("\"custname\":\"OpenAI Agent\"") && v.contains("\"custemail\":\"agent@example.com\"") && v.contains("\"size\":true"))
            .unwrap_or(false),
        "unexpected httpbin form state after fill: {:?}",
        form_state
    );
    let recording = api_post_empty(
        &client,
        &api_base,
        api_key,
        &format!("/api/recordings/stop/{}", session_id),
    )
    .await;
    let recording_id = recording
        .get("id")
        .and_then(|v| v.as_str())
        .expect("recording id missing")
        .to_string();
    let actions = recording
        .get("actions")
        .and_then(|v| v.as_array())
        .expect("recording actions missing");
    assert!(
        actions.iter().any(|action| action.get("type").and_then(|v| v.as_str()) == Some("navigate")),
        "recording should include navigate: {:?}",
        actions
    );
    assert!(
        actions.iter().any(|action| action.get("type").and_then(|v| v.as_str()) == Some("type")),
        "recording should include type: {:?}",
        actions
    );
    assert!(
        actions.iter().any(|action| action.get("type").and_then(|v| v.as_str()) == Some("click")),
        "recording should include click: {:?}",
        actions
    );
    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/kill/{}", profile_id)).await;
    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/launch/{}", profile_id)).await;
    let playback = api_post_empty(
        &client,
        &api_base,
        api_key,
        &format!("/api/recordings/{}/play/{}", recording_id, profile_id),
    )
    .await;
    assert_eq!(playback.get("completed_actions").and_then(|v| v.as_u64()), Some(actions.len() as u64));
    let playback_url = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/evaluate", profile_id),
        serde_json::json!({
            "expression": "JSON.stringify({ custname: document.querySelector(\"input[name='custname']\")?.value, custemail: document.querySelector(\"input[name='custemail']\")?.value, size: document.querySelector(\"input[name='size'][value='medium']\")?.checked })"
        }),
    )
    .await;
    assert!(
        playback_url
            .get("result")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("\"custname\":\"OpenAI Agent\"") && v.contains("\"custemail\":\"agent@example.com\"") && v.contains("\"size\":true"))
            .unwrap_or(false),
        "unexpected httpbin form state after playback: {:?}",
        playback_url
    );
    println!("external smoke artifacts: {}", artifact_dir.display());

    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/kill/{}", profile_id)).await;
    let _ = fs::remove_dir_all(&user_data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "uses live external websites"]
async fn test_live_external_tabs_smoke() {
    let Some(chrome_path) = find_chrome() else {
        eprintln!("SKIP: no Chrome");
        return;
    };

    let state = make_state();
    let api_port = 39541u16;
    let api_key = "external-tabs-key";
    {
        let mut config = state.config.write();
        config.chrome_path = Some(chrome_path);
        config.mcp.enabled = true;
        config.mcp.api_port = api_port;
        config.mcp.api_key = Some(api_key.to_string());
    }
    run_server(state.clone(), api_port, Some(api_key.to_string()));
    wait_for_api_ready(api_port, Some(api_key)).await;

    let artifact_dir = artifact_dir();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .unwrap();
    let api_base = format!("http://127.0.0.1:{}", api_port);
    let suffix = unique_suffix();
    let profile_id = format!("external-tabs-{}", suffix);
    let user_data_dir = std::env::temp_dir().join(format!("browsion-external-tabs-{}", suffix));
    let _ = fs::remove_dir_all(&user_data_dir);
    fs::create_dir_all(&user_data_dir).unwrap();

    let profile = serde_json::json!({
        "id": profile_id,
        "name": "External Tabs",
        "description": "Manual multi-tab smoke against live websites",
        "user_data_dir": user_data_dir.to_str().unwrap(),
        "lang": "en-US",
        "tags": ["manual", "external", "tabs"],
        "custom_args": ["--ignore-certificate-errors"],
        "headless": true
    });
    let create_response = client
        .post(format!("{}/api/profiles", api_base))
        .header("X-API-Key", api_key)
        .json(&profile)
        .send()
        .await
        .expect("create profile failed");
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/launch/{}", profile_id)).await;

    let html_url = "http://httpbin.org/html";
    let form_url = "http://httpbin.org/forms/post";

    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/navigate_wait", profile_id),
        serde_json::json!({
            "url": html_url,
            "wait_until": "load",
            "timeout_ms": 45000
        }),
    )
    .await;

    let new_tab = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/tabs/new", profile_id),
        serde_json::json!({ "url": form_url }),
    )
    .await;
    let form_tab_id = new_tab
        .get("id")
        .and_then(|v| v.as_str())
        .expect("new tab id missing")
        .to_string();

    let form_ready = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/wait_for_text", profile_id),
        serde_json::json!({
            "text": "Customer name",
            "timeout_ms": 45000
        }),
    )
    .await;
    assert_eq!(form_ready.get("ok").and_then(|v| v.as_bool()), Some(true));

    let tabs = api_get_json(&client, &api_base, api_key, &format!("/api/browser/{}/tabs", profile_id)).await;
    let tabs = tabs.as_array().expect("tabs response should be an array");
    assert!(
        tabs.len() >= 2,
        "expected at least 2 tabs after opening a new tab: {:?}",
        tabs
    );
    assert!(
        tabs.iter().any(|tab| {
            tab.get("url")
                .and_then(|v| v.as_str())
                .map(|url| url.contains("httpbin.org/html"))
                .unwrap_or(false)
        }),
        "expected html tab in tabs list: {:?}",
        tabs
    );
    assert!(
        tabs.iter().any(|tab| {
            tab.get("url")
                .and_then(|v| v.as_str())
                .map(|url| url.contains("httpbin.org/forms/post"))
                .unwrap_or(false)
        }),
        "expected form tab in tabs list: {:?}",
        tabs
    );

    let form_text = api_get_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/page_text", profile_id),
    )
    .await;
    assert!(
        form_text
            .get("text")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("Customer name"))
            .unwrap_or(false),
        "unexpected form page text: {:?}",
        form_text
    );

    let html_tab_id = tabs
        .iter()
        .find(|tab| {
            tab.get("url")
                .and_then(|v| v.as_str())
                .map(|url| url.contains("httpbin.org/html"))
                .unwrap_or(false)
        })
        .and_then(|tab| tab.get("id"))
        .and_then(|v| v.as_str())
        .expect("html tab id missing")
        .to_string();

    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/tabs/switch", profile_id),
        serde_json::json!({ "target_id": html_tab_id }),
    )
    .await;
    let html_text = api_get_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/page_text", profile_id),
    )
    .await;
    assert!(
        html_text
            .get("text")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("Moby-Dick"))
            .unwrap_or(false),
        "unexpected html page text after switching back: {:?}",
        html_text
    );

    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/tabs/switch", profile_id),
        serde_json::json!({ "target_id": form_tab_id }),
    )
    .await;
    let form_shot = api_get_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/screenshot?full_page=false", profile_id),
    )
    .await;
    write_screenshot(&form_shot, &artifact_dir.join("httpbin-tabs-form.png"));

    println!("external tabs artifacts: {}", artifact_dir.display());

    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/kill/{}", profile_id)).await;
    let _ = fs::remove_dir_all(&user_data_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "uses live external websites"]
async fn test_live_external_history_smoke() {
    let Some(chrome_path) = find_chrome() else {
        eprintln!("SKIP: no Chrome");
        return;
    };

    let state = make_state();
    let api_port = 39542u16;
    let api_key = "external-history-key";
    {
        let mut config = state.config.write();
        config.chrome_path = Some(chrome_path);
        config.mcp.enabled = true;
        config.mcp.api_port = api_port;
        config.mcp.api_key = Some(api_key.to_string());
    }
    run_server(state.clone(), api_port, Some(api_key.to_string()));
    wait_for_api_ready(api_port, Some(api_key)).await;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .unwrap();
    let api_base = format!("http://127.0.0.1:{}", api_port);
    let suffix = unique_suffix();
    let profile_id = format!("external-history-{}", suffix);
    let user_data_dir = std::env::temp_dir().join(format!("browsion-external-history-{}", suffix));
    let _ = fs::remove_dir_all(&user_data_dir);
    fs::create_dir_all(&user_data_dir).unwrap();

    let profile = serde_json::json!({
        "id": profile_id,
        "name": "External History",
        "description": "Manual history smoke against live websites",
        "user_data_dir": user_data_dir.to_str().unwrap(),
        "lang": "en-US",
        "tags": ["manual", "external", "history"],
        "custom_args": ["--ignore-certificate-errors"],
        "headless": true
    });
    let create_response = client
        .post(format!("{}/api/profiles", api_base))
        .header("X-API-Key", api_key)
        .json(&profile)
        .send()
        .await
        .expect("create profile failed");
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/launch/{}", profile_id)).await;

    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/navigate_wait", profile_id),
        serde_json::json!({
            "url": "http://httpbin.org/html",
            "wait_until": "load",
            "timeout_ms": 45000
        }),
    )
    .await;
    let first_text = api_get_json(&client, &api_base, api_key, &format!("/api/browser/{}/page_text", profile_id)).await;
    assert!(
        first_text
            .get("text")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("Moby-Dick"))
            .unwrap_or(false),
        "unexpected first page text: {:?}",
        first_text
    );

    api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/navigate_wait", profile_id),
        serde_json::json!({
            "url": "http://httpbin.org/forms/post",
            "wait_until": "load",
            "timeout_ms": 45000
        }),
    )
    .await;
    let second_ready = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/wait_for_text", profile_id),
        serde_json::json!({
            "text": "Customer name",
            "timeout_ms": 45000
        }),
    )
    .await;
    assert_eq!(second_ready.get("ok").and_then(|v| v.as_bool()), Some(true));

    let back = api_post_empty(&client, &api_base, api_key, &format!("/api/browser/{}/back", profile_id)).await;
    assert!(
        back.get("url")
            .and_then(|v| v.as_str())
            .map(|url| url.contains("httpbin.org/html"))
            .unwrap_or(false),
        "unexpected URL after back: {:?}",
        back
    );
    let back_text = api_get_json(&client, &api_base, api_key, &format!("/api/browser/{}/page_text", profile_id)).await;
    assert!(
        back_text
            .get("text")
            .and_then(|v| v.as_str())
            .map(|v| v.contains("Moby-Dick"))
            .unwrap_or(false),
        "unexpected page text after back: {:?}",
        back_text
    );

    let forward = api_post_empty(&client, &api_base, api_key, &format!("/api/browser/{}/forward", profile_id)).await;
    assert!(
        forward.get("url")
            .and_then(|v| v.as_str())
            .map(|url| url.contains("httpbin.org/forms/post"))
            .unwrap_or(false),
        "unexpected URL after forward: {:?}",
        forward
    );
    let forward_ready = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/wait_for_text", profile_id),
        serde_json::json!({
            "text": "Customer name",
            "timeout_ms": 45000
        }),
    )
    .await;
    assert_eq!(forward_ready.get("ok").and_then(|v| v.as_bool()), Some(true));

    let reload = api_post_empty(&client, &api_base, api_key, &format!("/api/browser/{}/reload", profile_id)).await;
    assert!(
        reload.get("url")
            .and_then(|v| v.as_str())
            .map(|url| url.contains("httpbin.org/forms/post"))
            .unwrap_or(false),
        "unexpected URL after reload: {:?}",
        reload
    );
    let reload_ready = api_post_json(
        &client,
        &api_base,
        api_key,
        &format!("/api/browser/{}/wait_for_text", profile_id),
        serde_json::json!({
            "text": "Customer name",
            "timeout_ms": 45000
        }),
    )
    .await;
    assert_eq!(reload_ready.get("ok").and_then(|v| v.as_bool()), Some(true));

    let _ = api_post_empty(&client, &api_base, api_key, &format!("/api/kill/{}", profile_id)).await;
    let _ = fs::remove_dir_all(&user_data_dir);
}
