//! Real end-to-end browser tests.
//!
//! These tests launch an actual Chrome process in headless mode, connect via CDP,
//! and exercise real browser operations: navigation, JS eval, clicks, forms,
//! screenshots, tabs, cookies, console capture, localStorage, AX tree, etc.
//!
//! ## Prerequisites
//! Chrome binary must be available. Discovery order:
//!   1. `CHROME_PATH` environment variable
//!   2. Common Linux / macOS / Windows paths
//!   3. PATH lookup (`which google-chrome`, `which chromium`, …)
//!
//! Tests are **skipped** (not failed) when Chrome is not found.
//!
//! ## Running
//! ```
//! cargo test --test e2e_browser_test -- --nocapture --test-threads=1
//! ```
//! Use `--test-threads=1` to avoid port conflicts between parallel tests.

use base64::Engine as _;
use browsion_lib::agent::cdp::CDPClient;
use browsion_lib::process::port::allocate_cdp_port;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command};
use tokio::sync::oneshot;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Find a Chrome binary or return None (test will be skipped).
fn find_chrome() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CHROME_PATH") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return Some(pb);
        }
    }

    let candidates: &[&str] = {
        #[cfg(target_os = "linux")]
        {
            &[
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/chromium-browser",
                "/usr/bin/chromium",
                "/usr/local/bin/google-chrome",
                "/snap/bin/chromium",
            ]
        }
        #[cfg(target_os = "macos")]
        {
            &[
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
            ]
        }
        #[cfg(target_os = "windows")]
        {
            &[
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            ]
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            &[]
        }
    };

    for path in candidates {
        let pb = PathBuf::from(path);
        if pb.exists() {
            return Some(pb);
        }
    }

    // Also try PATH
    for name in &["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"] {
        if let Ok(out) = Command::new("which").arg(name).output() {
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let pb = PathBuf::from(&p);
                if pb.exists() {
                    return Some(pb);
                }
            }
        }
    }

    None
}

/// Allocate a unique temporary user-data-dir for Chrome.
fn temp_profile_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    std::env::temp_dir().join(format!("browsion-e2e-{}", nanos))
}

/// RAII wrapper: holds Chrome child process and CDPClient; cleans up on `kill()`.
struct TestBrowser {
    child: Child,
    pub client: CDPClient,
    data_dir: PathBuf,
}

impl TestBrowser {
    /// Launch Chrome headless on `cdp_port` and attach CDPClient.
    async fn launch(chrome: &PathBuf, cdp_port: u16) -> Self {
        let data_dir = temp_profile_dir();
        std::fs::create_dir_all(&data_dir).unwrap();

        let child = Command::new(chrome)
            .arg(format!("--remote-debugging-port={}", cdp_port))
            .arg(format!("--user-data-dir={}", data_dir.display()))
            .arg("--headless=new")
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-sync")
            .arg("--disable-default-apps")
            .arg("--disable-crash-reporter")
            .arg("about:blank")
            .spawn()
            .expect("failed to spawn Chrome");

        // CDPClient polls /json/version until Chrome is ready
        let client = CDPClient::attach("e2e-test".to_string(), cdp_port)
            .await
            .expect("failed to attach CDPClient");

        TestBrowser { child, client, data_dir }
    }

    fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

/// Spin up a tiny in-process HTTP server that serves test HTML.
/// Returns `(base_url_string, shutdown_sender)`.
async fn spawn_test_server() -> (String, oneshot::Sender<()>) {
    use axum::{response::Html, routing::get, Router};

    let app = Router::new()
        .route("/", get(|| async { Html(HTML_BASIC) }))
        .route("/form", get(|| async { Html(HTML_FORM) }))
        .route("/console", get(|| async { Html(HTML_CONSOLE) }))
        .route("/storage", get(|| async { Html(HTML_STORAGE) }))
        .route("/tabs", get(|| async { Html(HTML_TABS) }))
        .route("/interact", get(|| async { Html(HTML_INTERACT) }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let base = format!("http://127.0.0.1:{}", addr.port());

    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async { let _ = rx.await; })
            .await
            .ok();
    });

    (base, tx)
}

// ── test HTML fixtures ────────────────────────────────────────────────────────

const HTML_BASIC: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Browsion Test Page</title></head>
<body>
  <h1 id="heading">Hello from Browsion</h1>
  <p id="para">This is a test paragraph.</p>
  <span id="counter">0</span>
  <button id="btn" onclick="document.getElementById('counter').textContent=String(parseInt(document.getElementById('counter').textContent)+1)">
    Click Me
  </button>
  <a id="extlink" href="/form">Go to Form</a>
</body>
</html>"#;

const HTML_FORM: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Browsion Form Test</title></head>
<body>
  <h1>Form Page</h1>
  <input id="name-input" type="text" placeholder="Enter name" aria-label="Name input" />
  <input id="email-input" type="email" placeholder="Enter email" aria-label="Email input" />
  <select id="country-select" aria-label="Country">
    <option value="">Select country</option>
    <option value="us">United States</option>
    <option value="gb">United Kingdom</option>
    <option value="de">Germany</option>
  </select>
  <button id="submit-btn" onclick="
    document.getElementById('result').textContent =
      document.getElementById('name-input').value + '|' +
      document.getElementById('email-input').value + '|' +
      document.getElementById('country-select').value;
  ">Submit</button>
  <div id="result"></div>
</body>
</html>"#;

const HTML_CONSOLE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Browsion Console Test</title></head>
<body>
  <h1>Console Test</h1>
  <script>
    console.log("hello-from-console");
    console.warn("warning-message");
    console.error("error-message");
  </script>
  <button id="log-btn" onclick="console.log('button-clicked')">Log on click</button>
</body>
</html>"#;

const HTML_STORAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Browsion Storage Test</title></head>
<body><h1>Storage Test</h1></body>
</html>"#;

const HTML_TABS: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Browsion Tab Origin</title></head>
<body>
  <h1>Tab Origin</h1>
  <a id="new-tab-link" href="/form" target="_blank">Open in new tab</a>
</body>
</html>"#;

const HTML_INTERACT: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>Browsion Interact Test</title></head>
<body>
  <h1>Interaction Page</h1>
  <input id="slow-input" type="text" />
  <pre id="keylog"></pre>
  <script>
    document.getElementById('slow-input').addEventListener('keypress', function(e) {
      document.getElementById('keylog').textContent += e.key;
    });
  </script>
</body>
</html>"#;

// ── tests ─────────────────────────────────────────────────────────────────────

/// 1. Navigate to local page, read title, URL, and heading text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_01_navigate_and_read_page_info() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let title = browser.client.get_title().await.unwrap();
    assert_eq!(title.as_deref(), Some("Browsion Test Page"), "wrong title");

    let url = browser.client.get_url().await.unwrap();
    assert!(url.contains("127.0.0.1"), "unexpected URL: {url}");

    let heading = browser.client
        .evaluate_js("document.getElementById('heading').textContent")
        .await.unwrap();
    assert_eq!(heading.as_str(), Some("Hello from Browsion"));

    browser.kill();
}

/// 2. JavaScript evaluation: arithmetic, strings, JSON, DOM.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_02_javascript_evaluation() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let n = browser.client.evaluate_js("2 + 2").await.unwrap();
    assert_eq!(n.as_i64(), Some(4), "arithmetic failed");

    let s = browser.client.evaluate_js("'hello' + ' world'").await.unwrap();
    assert_eq!(s.as_str(), Some("hello world"));

    let obj = browser.client
        .evaluate_js("JSON.stringify({a:1,b:[1,2,3]})")
        .await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(obj.as_str().unwrap()).unwrap();
    assert_eq!(parsed["a"], 1);
    assert_eq!(parsed["b"], serde_json::json!([1, 2, 3]));

    let para = browser.client
        .evaluate_js("document.getElementById('para').textContent")
        .await.unwrap();
    assert!(para.as_str().unwrap().contains("test paragraph"));

    browser.kill();
}

/// 3. Click a button repeatedly and verify the counter DOM changes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_03_click_button_changes_dom() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let before = browser.client
        .evaluate_js("document.getElementById('counter').textContent")
        .await.unwrap();
    assert_eq!(before.as_str(), Some("0"));

    for _ in 0..3 {
        browser.client.click("#btn").await.unwrap();
    }

    let after = browser.client
        .evaluate_js("document.getElementById('counter').textContent")
        .await.unwrap();
    assert_eq!(after.as_str(), Some("3"), "counter should be 3 after 3 clicks");

    browser.kill();
}

/// 4. Fill a form: type text, select dropdown, click submit, verify result div.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_04_form_fill_and_submit() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/form", base), "load", 10_000).await.unwrap();

    browser.client.type_text("#name-input", "Alice").await.unwrap();
    browser.client.type_text("#email-input", "alice@example.com").await.unwrap();
    browser.client.select_option("#country-select", "gb").await.unwrap();
    browser.client.click("#submit-btn").await.unwrap();

    browser.client.wait_for_text("Alice|", 3_000).await.unwrap();

    let result = browser.client
        .evaluate_js("document.getElementById('result').textContent")
        .await.unwrap();
    let text = result.as_str().unwrap();
    assert!(text.contains("Alice"), "name missing: {text}");
    assert!(text.contains("alice@example.com"), "email missing: {text}");
    assert!(text.contains("gb"), "country missing: {text}");

    browser.kill();
}

/// 5. Screenshot: take full-page PNG, verify it's valid base64-encoded PNG bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_05_screenshot_valid_png() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let b64 = browser.client.screenshot(false, "png", None).await.unwrap();
    assert!(!b64.is_empty(), "screenshot is empty");

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .expect("screenshot not valid base64");
    assert!(bytes.len() > 1000, "screenshot too small: {} bytes", bytes.len());
    assert_eq!(&bytes[0..4], b"\x89PNG", "screenshot is not a PNG");

    browser.kill();
}

/// 6. Element screenshot: capture a single element as PNG.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_06_screenshot_element() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let b64 = browser.client.screenshot_element("#heading", "png", None).await.unwrap();
    assert!(!b64.is_empty(), "element screenshot is empty");

    let bytes = base64::engine::general_purpose::STANDARD.decode(&b64).unwrap();
    assert_eq!(&bytes[0..4], b"\x89PNG", "element screenshot is not a PNG");

    browser.kill();
}

/// 7. Get AX tree via get_page_state, find button by name, click via ref_id.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_07_ax_tree_and_click_ref() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let state = browser.client.get_page_state().await.unwrap();
    assert_eq!(state.title.as_deref(), Some("Browsion Test Page"));
    assert!(!state.ax_tree.is_empty(), "AX tree is empty");

    // Find the "Click Me" button by its accessible name
    let btn_node = state.ax_tree.iter().find(|n| n.name.contains("Click Me"));
    assert!(
        btn_node.is_some(),
        "button 'Click Me' not found in AX tree. nodes: {:?}",
        state.ax_tree.iter().map(|n| (&n.role, &n.name)).collect::<Vec<_>>()
    );

    let btn_ref = btn_node.unwrap().ref_id.clone();

    // Click twice via semantic ref
    browser.client.click_ref(&btn_ref).await.unwrap();
    browser.client.click_ref(&btn_ref).await.unwrap();

    let counter = browser.client
        .evaluate_js("document.getElementById('counter').textContent")
        .await.unwrap();
    assert_eq!(counter.as_str(), Some("2"), "counter should be 2 after 2 ref-clicks");

    browser.kill();
}

/// 8. type_ref: find input via AX tree, type into it via ref_id, verify value.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_08_type_ref() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/form", base), "load", 10_000).await.unwrap();

    let state = browser.client.get_page_state().await.unwrap();

    // The Name input has aria-label="Name input"
    let input_node = state.ax_tree.iter().find(|n| n.name.contains("Name input"));
    assert!(input_node.is_some(), "Name input not found in AX tree. nodes: {:?}",
        state.ax_tree.iter().map(|n| (&n.role, &n.name)).collect::<Vec<_>>());

    let ref_id = input_node.unwrap().ref_id.clone();
    browser.client.type_ref(&ref_id, "Bob").await.unwrap();

    let val = browser.client
        .evaluate_js("document.getElementById('name-input').value")
        .await.unwrap();
    assert_eq!(val.as_str(), Some("Bob"), "type_ref did not set value");

    browser.kill();
}

/// 9. Tab management: open new tab, navigate it, switch back, close it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_09_tab_management() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let tabs_before = browser.client.list_tabs().await.unwrap();
    let original_count = tabs_before.len();

    // Open a new tab
    let new_tab = browser.client.new_tab("about:blank").await.unwrap();
    browser.client.switch_tab(&new_tab.id).await.unwrap();
    browser.client.navigate_wait(&format!("{}/form", base), "load", 10_000).await.unwrap();

    let new_title = browser.client.get_title().await.unwrap();
    assert_eq!(new_title.as_deref(), Some("Browsion Form Test"), "new tab wrong title");

    let tabs_after = browser.client.list_tabs().await.unwrap();
    assert_eq!(tabs_after.len(), original_count + 1, "new tab should appear in list");

    // Switch back to the original tab (not the current active one)
    let orig_tab = tabs_after.iter().find(|t| t.id != new_tab.id).unwrap();
    let orig_id = orig_tab.id.clone();
    browser.client.switch_tab(&orig_id).await.unwrap();

    let back_title = browser.client.get_title().await.unwrap();
    assert_eq!(back_title.as_deref(), Some("Browsion Test Page"), "switch back failed");

    // Close the extra tab
    browser.client.close_tab(&new_tab.id).await.unwrap();

    browser.kill();
}

/// 10. wait_for_new_tab: subscribe before clicking target="_blank" link.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_10_wait_for_new_tab() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/tabs", base), "load", 10_000).await.unwrap();

    // Subscribe BEFORE clicking to avoid race
    let (new_target, click_res) = tokio::join!(
        browser.client.wait_for_new_tab(5_000),
        browser.client.click("#new-tab-link")
    );

    click_res.unwrap();
    let new_target_id = new_target.unwrap();
    assert!(!new_target_id.is_empty(), "new tab target_id should not be empty");

    browser.client.switch_tab(&new_target_id).await.unwrap();
    browser.client.wait_for_navigation(5_000).await.ok();

    let title = browser.client.get_title().await.unwrap();
    assert_eq!(
        title.as_deref(), Some("Browsion Form Test"),
        "new tab should show form page"
    );

    browser.kill();
}

/// 11. Cookies: set, get, verify, delete, verify deletion.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_11_cookies() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    // Must navigate to a real origin for cookies to work
    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    browser.client
        .set_cookie("test-cookie", "hello-world", "127.0.0.1", "/")
        .await.unwrap();

    let cookies = browser.client.get_cookies().await.unwrap();
    let found = cookies.iter().find(|c| c.name == "test-cookie");
    assert!(
        found.is_some(),
        "cookie not found. cookies: {:?}",
        cookies.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert_eq!(found.unwrap().value, "hello-world");

    browser.client.delete_cookies().await.unwrap();
    let after = browser.client.get_cookies().await.unwrap();
    assert!(
        !after.iter().any(|c| c.name == "test-cookie"),
        "cookie should be deleted after delete_cookies()"
    );

    browser.kill();
}

/// 12. Console capture: enable before load, verify console.log entries are captured.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_12_console_capture() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    // Enable BEFORE navigating so inline scripts are captured
    browser.client.enable_console_capture().await.unwrap();
    browser.client.navigate_wait(&format!("{}/console", base), "load", 10_000).await.unwrap();
    browser.client.wait(300).await.unwrap();

    let logs = browser.client.get_console_logs().await.unwrap();
    let logs_str = logs.to_string();
    assert!(logs_str.contains("hello-from-console"), "log missing: {logs_str}");
    assert!(logs_str.contains("warning-message"), "warn missing: {logs_str}");

    // Click button to emit a runtime log
    browser.client.click("#log-btn").await.unwrap();
    browser.client.wait(200).await.unwrap();

    let logs2 = browser.client.get_console_logs().await.unwrap();
    assert!(
        logs2.to_string().contains("button-clicked"),
        "runtime log not captured: {}", logs2
    );

    // Clear and verify empty
    browser.client.clear_console_logs().await;
    let logs3 = browser.client.get_console_logs().await.unwrap();
    let arr = logs3.as_array().unwrap();
    assert!(arr.is_empty(), "logs should be empty after clear");

    browser.kill();
}

/// 13. Network log: navigation request appears in the log.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_13_network_log() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let log = browser.client.get_network_log().await;
    assert!(!log.is_empty(), "network log should not be empty after navigation");

    let has_our_request = log.iter().any(|e| {
        e.get("url").and_then(|u| u.as_str())
            .map(|u| u.contains("127.0.0.1"))
            .unwrap_or(false)
    });
    assert!(has_our_request, "no 127.0.0.1 request in log: {:?}", log);

    // Clear and verify
    browser.client.clear_network_log().await;
    let after = browser.client.get_network_log().await;
    assert!(after.is_empty(), "network log should be empty after clear");

    browser.kill();
}

/// 14. localStorage: set items, read back, remove one, clear all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_14_local_storage() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/storage", base), "load", 10_000).await.unwrap();

    browser.client.set_storage_item("local", "greeting", "hello").await.unwrap();
    browser.client.set_storage_item("local", "count", "42").await.unwrap();

    // Verify via JS
    let v = browser.client.evaluate_js("localStorage.getItem('greeting')").await.unwrap();
    assert_eq!(v.as_str(), Some("hello"), "greeting not set");

    // Verify via CDP
    let storage = browser.client.get_storage("local").await.unwrap();
    assert_eq!(
        storage.get("count").and_then(|v| v.as_str()),
        Some("42"),
        "count not in storage: {:?}", storage
    );

    // Remove one key
    browser.client.remove_storage_item("local", "greeting").await.unwrap();
    let after_remove = browser.client.evaluate_js("localStorage.getItem('greeting')").await.unwrap();
    assert!(after_remove.is_null(), "greeting should be null after removal");

    // Clear all
    browser.client.clear_storage("local").await.unwrap();
    let remaining = browser.client.get_storage("local").await.unwrap();
    assert!(remaining.as_object().unwrap().is_empty(), "storage should be empty after clear");

    browser.kill();
}

/// 15. get_page_text extracts full body text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_15_get_page_text() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let text = browser.client.get_page_text().await.unwrap();
    assert!(text.contains("Hello from Browsion"), "h1 text missing: {text}");
    assert!(text.contains("test paragraph"), "p text missing: {text}");

    browser.kill();
}

/// 16. go_back and go_forward between two pages.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_16_navigation_history() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();
    browser.client.navigate_wait(&format!("{}/form", base), "load", 10_000).await.unwrap();

    assert_eq!(
        browser.client.get_title().await.unwrap().as_deref(),
        Some("Browsion Form Test")
    );

    browser.client.go_back().await.unwrap();
    browser.client.wait_for_navigation(5_000).await.ok();
    assert_eq!(
        browser.client.get_title().await.unwrap().as_deref(),
        Some("Browsion Test Page"),
        "go_back failed"
    );

    browser.client.go_forward().await.unwrap();
    browser.client.wait_for_navigation(5_000).await.ok();
    assert_eq!(
        browser.client.get_title().await.unwrap().as_deref(),
        Some("Browsion Form Test"),
        "go_forward failed"
    );

    browser.kill();
}

/// 17. slow_type: type character-by-character and verify keypress events fire.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_17_slow_type_and_press_key() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/interact", base), "load", 10_000).await.unwrap();

    browser.client.click("#slow-input").await.unwrap();
    browser.client.slow_type("#slow-input", "hello", 20).await.unwrap();

    let keylog = browser.client
        .evaluate_js("document.getElementById('keylog').textContent")
        .await.unwrap();
    assert!(
        keylog.as_str().unwrap_or("").contains("hello"),
        "keylog should contain 'hello', got: {:?}", keylog
    );

    // Ctrl+A then Delete to clear the input
    browser.client.press_key("Ctrl+A").await.unwrap();
    browser.client.press_key("Delete").await.unwrap();

    let val = browser.client
        .evaluate_js("document.getElementById('slow-input').value")
        .await.unwrap();
    assert_eq!(val.as_str(), Some(""), "input should be empty after Ctrl+A + Delete");

    browser.kill();
}

/// 18. Reload resets DOM state to server-served HTML.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_18_reload_resets_state() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    for _ in 0..5 {
        browser.client.click("#btn").await.unwrap();
    }
    let before = browser.client
        .evaluate_js("document.getElementById('counter').textContent")
        .await.unwrap();
    assert_eq!(before.as_str(), Some("5"));

    browser.client.reload().await.unwrap();
    browser.client.wait_for_navigation(5_000).await.ok();

    let after = browser.client
        .evaluate_js("document.getElementById('counter').textContent")
        .await.unwrap();
    assert_eq!(after.as_str(), Some("0"), "counter should reset after reload");

    browser.kill();
}

/// 19. wait_for_url: detects URL change after navigation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_19_wait_for_url() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    let form_url = format!("{}/form", base);
    let (nav_res, url_res) = tokio::join!(
        browser.client.navigate(&form_url),
        browser.client.wait_for_url("/form", 5_000)
    );

    nav_res.unwrap();
    let matched = url_res.unwrap();
    assert!(matched.contains("/form"), "wait_for_url returned wrong URL: {matched}");

    browser.kill();
}

/// 20. Network interception: block a URL pattern, verify fetch fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_20_network_intercept_block() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let (base, _srv) = spawn_test_server().await;
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait(&format!("{}/", base), "load", 10_000).await.unwrap();

    // Block requests to /form (literal substring match)
    browser.client.block_url("/form").await.unwrap();

    let form_url = format!("{}/form", base);
    let blocked = browser.client
        .evaluate_js(&format!(
            r#"fetch('{}').then(r=>r.status.toString()).catch(e=>'blocked:'+e.message)"#,
            form_url
        ))
        .await.unwrap();

    let text = blocked.as_str().unwrap_or("");
    assert!(
        text.starts_with("blocked:") || text == "0",
        "request should be blocked, got: {text}"
    );

    // Clear intercepts — /form should be reachable again
    browser.client.clear_intercepts().await.unwrap();
    let ok = browser.client
        .evaluate_js(&format!(
            r#"fetch('{}').then(r=>r.status.toString()).catch(e=>'err')"#,
            form_url
        ))
        .await.unwrap();
    assert_eq!(ok.as_str(), Some("200"), "request should succeed after clear_intercepts");

    browser.kill();
}
