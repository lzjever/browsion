# Final Test Coverage and Testid Standardization - v0.9.4

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete end-to-end test coverage by renaming existing tests to testid standard, adding missing workflow/recording/storage tests, and releasing v0.9.4 with comprehensive coverage.

**Architecture:** Extend existing `e2e_browser_test.rs` with Chrome launch/teardown pattern. Tests use real Chrome in headless mode, connect via CDP, and test both direct CDP calls and HTTP API endpoints.

**Tech Stack:** Rust/axum, tokio, cargo test, Chrome (headless), CDP WebSocket, Tauri commands for workflow/recording.

---

## Current State Analysis

**Existing Tests (38 total):**
- Numbered tests (20): test_01 through test_20
- Descriptive tests (18): test_lifecycle_*, test_profile_*, test_mouse_*, etc.

**Issues Identified:**
1. Inconsistent naming: 20 tests use numbers, 18 use descriptive names
2. Missing E2E coverage for workflows
3. Missing E2E coverage for recordings
4. Missing E2E coverage for storage operations (clear_storage, remove_storage)
5. Missing E2E coverage for select_option form interaction
6. Missing E2E coverage for click_at (direct coordinates)
7. Missing E2E coverage for extract_data
8. Missing E2E coverage for delete_cookies

**Target State:**
- All 48 tests follow testid standard: `test_<category>_<operation>_<variant>`
- Complete coverage of workflow/recording/storage features
- Release v0.9.4

---

## Task 1: Rename Numbered Tests to Testid Standard

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Rename test_01**

```rust
// OLD: async fn test_01_navigate_and_read_page_info()
// NEW:
async fn test_navigate_read_page_info_basic() {
```

**Step 2: Rename test_02**

```rust
// OLD: async fn test_02_javascript_evaluation()
// NEW:
async fn test_javascript_evaluate_basic_expression() {
```

**Step 3: Rename test_03**

```rust
// OLD: async fn test_03_click_button_changes_dom()
// NEW:
async fn test_mouse_click_button_changes_dom() {
```

**Step 4: Rename test_04**

```rust
// OLD: async fn test_04_form_fill_and_submit()
// NEW:
async fn test_form_fill_and_submit_basic() {
```

**Step 5: Rename test_05**

```rust
// OLD: async fn test_05_screenshot_valid_png()
// NEW:
async fn test_screenshot_full_page_png() {
```

**Step 6: Rename test_06**

```rust
// OLD: async fn test_06_screenshot_element()
// NEW:
async fn test_screenshot_element_png() {
```

**Step 7: Rename test_07**

```rust
// OLD: async fn test_07_ax_tree_and_click_ref()
// NEW:
async fn test_axref_click_button_by_ref() {
```

**Step 8: Rename test_08**

```rust
// OLD: async fn test_08_type_ref()
// NEW:
async fn test_axref_type_input_by_ref() {
```

**Step 9: Rename test_09**

```rust
// OLD: async fn test_09_tab_management()
// NEW:
async fn test_tabs_list_new_close() {
```

**Step 10: Rename test_10**

```rust
// OLD: async fn test_10_wait_for_new_tab()
// NEW:
async fn test_tabs_wait_for_new_tab() {
```

**Step 11: Rename test_11**

```rust
// OLD: async fn test_11_cookies()
// NEW:
async fn test_cookies_set_and_get() {
```

**Step 12: Rename test_12**

```rust
// OLD: async fn test_12_console_capture()
// NEW:
async fn test_console_capture_and_retrieve() {
```

**Step 13: Rename test_13**

```rust
// OLD: async fn test_13_network_log()
// NEW:
async fn test_network_log_request_response() {
```

**Step 14: Rename test_14**

```rust
// OLD: async fn test_14_local_storage()
// NEW:
async fn test_storage_set_and_get_local() {
```

**Step 15: Rename test_15**

```rust
// OLD: async fn test_15_get_page_text()
// NEW:
async fn test_observe_get_page_text() {
```

**Step 16: Rename test_16**

```rust
// OLD: async fn test_16_navigation_history()
// NEW:
async fn test_navigate_go_back_and_forward() {
```

**Step 17: Rename test_17**

```rust
// OLD: async fn test_17_slow_type_and_press_key()
// NEW:
async fn test_keyboard_slow_type_and_press_key() {
```

**Step 18: Rename test_18**

```rust
// OLD: async fn test_18_reload_resets_state()
// NEW:
async fn test_navigate_reload_page() {
```

**Step 19: Rename test_19**

```rust
// OLD: async fn test_19_wait_for_url()
// NEW:
async fn test_navigate_wait_for_url_pattern() {
```

**Step 20: Rename test_20**

```rust
// OLD: async fn test_20_network_intercept_block()
// NEW:
async fn test_network_block_url_pattern() {
```

**Step 21: Verify all tests still pass**

```bash
cd /home/percy/works/browsion/src-tauri
cargo test --test e2e_browser_test -- --test-threads=1 2>&1 | tail -10
```

Expected: 38 tests pass

**Step 22: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "refactor: rename numbered tests to follow testid standard"
```

---

## Task 2: Add Form Select Option Test

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 39. Form select option: select dropdown option.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_form_select_dropdown_option() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    // Create test page with select dropdown
    let html = r#"
    <!DOCTYPE html>
    <html><head><title>Select Test</title></head>
    <body>
        <select id="dropdown">
            <option value="opt1">Option 1</option>
            <option value="opt2">Option 2</option>
            <option value="opt3">Option 3</option>
        </select>
    </body></html>
    "#;
    let encoded = percent_encoding::utf8_percent_encode(html, percent_encoding::NON_ALPHANUMERIC).to_string();
    browser.client.navigate_wait(&format!("data:text/html,{}", encoded), "load", 5000).await.unwrap();

    // Select option by value
    browser.client.select_option("#dropdown", "opt2").await.unwrap();

    // Verify selection
    let result = browser.client.evaluate_js("document.getElementById('dropdown').value").await.unwrap();
    assert_eq!(result.as_str(), Some("opt2"), "dropdown should have opt2 selected");

    browser.kill();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_form_select_dropdown_option -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add form select option E2E test"
```

---

## Task 3: Add Mouse Click At Coordinates Test

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 40. Mouse click at: direct viewport coordinates.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mouse_click_at_viewport_coordinates() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    // Create test page with click tracker
    let html = r#"
    <!DOCTYPE html>
    <html><head><title>Click At Test</title></head>
    <body>
        <div id="output">not clicked</div>
        <script>
            document.addEventListener('click', (e) => {
                document.getElementById('output').textContent = `clicked at ${e.clientX},${e.clientY}`;
            });
        </script>
    </body></html>
    "#;
    let encoded = percent_encoding::utf8_percent_encode(html, percent_encoding::NON_ALPHANUMERIC).to_string();
    browser.client.navigate_wait(&format!("data:text/html,{}", encoded), "load", 5000).await.unwrap();

    // Click at center of viewport (400, 300)
    browser.client.click_at(400, 300).await.unwrap();

    // Verify click was recorded
    let result = browser.client.evaluate_js("document.getElementById('output').textContent").await.unwrap();
    let text = result.as_str().unwrap_or("");
    assert!(text.contains("clicked at"), "output should show clicked at coordinates: {}", text);

    browser.kill();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_mouse_click_at_viewport_coordinates -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add mouse click at coordinates E2E test"
```

---

## Task 4: Add Storage Clear and Remove Tests

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write clear_storage test**

```rust
/// 41. Storage clear: clear localStorage.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_clear_local_storage() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait("https://example.com", "load", 5000).await.unwrap();

    // Set multiple items
    browser.client.set_storage("local", "key1", "value1").await.unwrap();
    browser.client.set_storage("local", "key2", "value2").await.unwrap();

    // Clear all
    browser.client.clear_storage("local", None).await.unwrap();

    // Verify cleared
    let result = browser.client.get_storage("local").await.unwrap();
    assert_eq!(result.len(), 0, "localStorage should be empty after clear");

    browser.kill();
}
```

**Step 2: Write remove_storage test**

```rust
/// 42. Storage remove: remove specific localStorage key.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_remove_local_key() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait("https://example.com", "load", 5000).await.unwrap();

    // Set items
    browser.client.set_storage("local", "key1", "value1").await.unwrap();
    browser.client.set_storage("local", "key2", "value2").await.unwrap();

    // Remove one key
    browser.client.clear_storage("local", Some("key1".to_string())).await.unwrap();

    // Verify only one removed
    let result = browser.client.get_storage("local").await.unwrap();
    assert_eq!(result.len(), 1, "localStorage should have 1 item");
    assert_eq!(result.get("key2"), Some(&"value2".to_string()), "key2 should remain");

    browser.kill();
}
```

**Step 3: Run tests**

```bash
cargo test --test e2e_browser_test test_storage_ -- --test-threads=1
```

Expected: Both PASS

**Step 4: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add storage clear and remove E2E tests"
```

---

## Task 5: Add Cookie Delete Test

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 43. Cookies delete: delete specific cookie.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cookies_delete_specific_cookie() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait("https://example.com", "load", 5000).await.unwrap();

    // Set multiple cookies
    browser.client.set_cookie("example.com", "/", "session1", "value1", None).await.unwrap();
    browser.client.set_cookie("example.com", "/", "session2", "value2", None).await.unwrap();

    // Delete one cookie
    browser.client.delete_cookies("example.com", None, Some("session1".to_string())).await.unwrap();

    // Verify only one deleted
    let cookies = browser.client.get_cookies().await.unwrap();
    let session2_exists = cookies.iter().any(|c| c.name == "session2");
    let session1_exists = cookies.iter().any(|c| c.name == "session1");

    assert!(session2_exists, "session2 should still exist");
    assert!(!session1_exists, "session1 should be deleted");

    browser.kill();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_cookies_delete_specific_cookie -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add cookie delete E2E test"
```

---

## Task 6: Add Data Extraction Test

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 44. Data extraction: extract structured data from page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_observe_extract_structured_data() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    // Create test page with structured data
    let html = r#"
    <!DOCTYPE html>
    <html><head><title>Extract Test</title></head>
    <body>
        <div class="product" data-id="123">
            <h2 class="name">Test Product</h2>
            <span class="price">$19.99</span>
        </div>
    </body></html>
    "#;
    let encoded = percent_encoding::utf8_percent_encode(html, percent_encoding::NON_ALPHANUMERIC).to_string();
    browser.client.navigate_wait(&format!("data:text/html,{}", encoded), "load", 5000).await.unwrap();

    // Extract data using query selector
    let schema = serde_json::json!({
        "name": "string",
        "price": "string",
        "id": "string"
    });

    let result = browser.client.extract_data(".product", &schema).await.unwrap();

    assert!(result.is_object(), "result should be object");
    assert_eq!(result.get("name").and_then(|v| v.as_str()), Some("Test Product"));
    assert!(result.get("price").and_then(|v| v.as_str()).unwrap_or("").contains("$19.99"));

    browser.kill();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_observe_extract_structured_data -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add data extraction E2E test"
```

---

## Task 7: Add Geolocation Emulation Test

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 45. Emulation: set geolocation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_emulate_set_geolocation() {
    let Some(chrome) = find_chrome() else { eprintln!("SKIP: no Chrome"); return; };
    let port = allocate_cdp_port();
    let browser = TestBrowser::launch(&chrome, port).await;

    browser.client.navigate_wait("https://example.com", "load", 5000).await.unwrap();

    // Set geolocation
    browser.client.set_geolocation(37.7749, -122.4194, 10.0).await.unwrap();

    // Verify via Geolocation API
    let result = browser.client.evaluate_js(
        r#"
        new Promise(resolve => {
            navigator.geolocation.getCurrentPosition(pos => {
                resolve({lat: pos.coords.latitude, lng: pos.coords.longitude});
            }, () => resolve(null));
        })
        "#
    ).await.unwrap();

    assert!(result.is_object(), "should get geolocation");
    let lat = result.get("lat").and_then(|v| v.as_f64());
    assert!(lat.is_some() && (lat.unwrap() - 37.7749).abs() < 0.01, "latitude should be ~37.7749");

    browser.kill();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_emulate_set_geolocation -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add geolocation emulation E2E test"
```

---

## Task 8: Add Workflow Execution Test (HTTP API)

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 46. Workflow: execute simple workflow via API.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_workflow_execute_simple_navigate() {
    let chrome = find_chrome().expect("Chrome required");
    let port = allocate_cdp_port();

    // Start HTTP server
    let state = make_state();
    let api_key = Some("test-workflow-key".to_string());
    run_server(state.clone(), port, api_key.clone());
    let api_base = format!("http://127.0.0.1:{}/api", port);

    // Create profile
    let user_data_dir = std::env::temp_dir().join("browsion-e2e-workflow");
    std::fs::create_dir_all(&user_data_dir).unwrap();
    let prof_id = "workflow-test";

    let profile = serde_json::json!({
        "id": prof_id,
        "name": "Workflow Test",
        "description": "",
        "user_data_dir": user_data_dir.to_str().unwrap(),
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });

    let client = reqwest::Client::new();
    let _ = client.post(format!("{}/profiles", api_base))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .json(&profile)
        .send()
        .await
        .expect("create profile failed");

    // Create workflow
    let workflow = serde_json::json!({
        "id": "test-workflow",
        "name": "Test Navigate Workflow",
        "description": "Simple navigate workflow",
        "steps": [{
            "id": "step-1",
            "name": "Navigate to example.com",
            "description": "",
            "step_type": "navigate",
            "params": {"url": "https://example.com"},
            "continue_on_error": false,
            "timeout_ms": 10000
        }],
        "variables": {},
        "created_at": 0,
        "updated_at": 0
    });

    let _ = client.post(format!("{}/workflows", api_base))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .json(&workflow)
        .send()
        .await
        .expect("create workflow failed");

    // Launch browser
    let mut child = Command::new(&chrome)
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg(format!("--remote-debugging-port={}", port))
        .arg("--headless=new")
        .spawn()
        .expect("Failed to start Chrome");

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Run workflow
    let resp = client.post(format!("{}/workflows/test-workflow/run/{}", api_base, prof_id))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("run workflow failed");

    assert_eq!(resp.status(), 200);

    let body = resp.text().await.expect("response body missing");
    let execution: serde_json::Value = serde_json::from_str(&body).expect("invalid JSON");
    assert_eq!(execution.get("status").and_then(|v| v.as_str()), Some("completed"));

    // Cleanup
    drop(state);
    child.kill().expect("Failed to kill Chrome");
    let _ = child.wait();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_workflow_execute_simple_navigate -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add workflow execution E2E test"
```

---

## Task 9: Add Recording Lifecycle Test (HTTP API)

**Files:**
- Modify: `src-tauri/tests/e2e_browser_test.rs`

**Step 1: Write the test**

```rust
/// 47. Recording: start, check status, stop recording via API.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recording_lifecycle_via_api() {
    let chrome = find_chrome().expect("Chrome required");
    let port = allocate_cdp_port();

    // Start HTTP server
    let state = make_state();
    let api_key = Some("test-recording-key".to_string());
    run_server(state.clone(), port, api_key.clone());
    let api_base = format!("http://127.0.0.1:{}/api", port);

    // Create profile
    let user_data_dir = std::env::temp_dir().join("browsion-e2e-recording");
    std::fs::create_dir_all(&user_data_dir).unwrap();
    let prof_id = "recording-test";

    let profile = serde_json::json!({
        "id": prof_id,
        "name": "Recording Test",
        "description": "",
        "user_data_dir": user_data_dir.to_str().unwrap(),
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });

    let client = reqwest::Client::new();
    let _ = client.post(format!("{}/profiles", api_base))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .json(&profile)
        .send()
        .await
        .expect("create profile failed");

    // Launch browser
    let mut child = Command::new(&chrome)
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
        .arg(format!("--remote-debugging-port={}", port))
        .arg("--headless=new")
        .spawn()
        .expect("Failed to start Chrome");

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Start recording
    let resp = client.post(format!("{}/recordings/start/{}", api_base, prof_id))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .send()
        .await
        .expect("start recording failed");

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("response body missing");
    let start_resp: serde_json::Value = serde_json::from_str(&body).expect("invalid JSON");
    let session_id = start_resp.get("session_id").and_then(|v| v.as_str()).expect("session_id missing");

    // Check recording status
    let resp = client.get(format!("{}/profiles/{}/recording-status", api_base, prof_id))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .send()
        .await
        .expect("get recording status failed");

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("response body missing");
    let status: serde_json::Value = serde_json::from_str(&body).expect("invalid JSON");
    assert_eq!(status.get("is_recording").and_then(|v| v.as_bool()), Some(true));

    // Stop recording
    let resp = client.post(format!("{}/recordings/stop/{}", api_base, session_id))
        .header("X-API-Key", api_key.as_ref().unwrap())
        .send()
        .await
        .expect("stop recording failed");

    assert_eq!(resp.status(), 200);

    // Cleanup
    drop(state);
    child.kill().expect("Failed to kill Chrome");
    let _ = child.wait();
}
```

**Step 2: Run test**

```bash
cargo test --test e2e_browser_test test_recording_lifecycle_via_api -- --test-threads=1
```

Expected: PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/e2e_browser_test.rs
git commit -m "test: add recording lifecycle E2E test"
```

---

## Task 10: Update Documentation and Final Verification

**Step 1: Run complete test suite**

```bash
cd /home/percy/works/browsion/src-tauri
cargo test --lib 2>&1 | grep "test result"
cargo test --test e2e_browser_test -- --test-threads=1 2>&1 | grep "test result"
cargo test --test api_integration_test 2>&1 | grep "test result"
cargo test --test config_and_cft_test 2>&1 | grep "test result"
npm test 2>&1 | tail -5
```

All must pass.

**Step 2: Count total tests**

| Suite | Count |
|-------|-------|
| Frontend (Vitest) | 18 |
| Backend Lib | 79 |
| API Integration | 92 |
| Config | 6 |
| **E2E** | **48** (38 renamed + 10 new) |
| **Total** | **243** |

**Step 3: Update CHANGELOG**

```markdown
## [0.9.4] - 2026-03-01

### Testing
- **Testid standardization** — all 48 E2E tests now follow `test_<category>_<operation>_<variant>` pattern
- **Renamed 20 tests** — converted numbered tests (test_01-test_20) to descriptive testid names
- **Form interactions** — `test_form_select_dropdown_option` for dropdown selection
- **Mouse operations** — `test_mouse_click_at_viewport_coordinates` for direct coordinate clicks
- **Storage operations** — `test_storage_clear_local_storage`, `test_storage_remove_local_key` for complete storage API coverage
- **Cookie operations** — `test_cookies_delete_specific_cookie` for cookie deletion
- **Data extraction** — `test_observe_extract_structured_data` for structured data extraction
- **Geolocation emulation** — `test_emulate_set_geolocation` for location simulation
- **Workflow execution** — `test_workflow_execute_simple_navigate` for workflow automation via API
- **Recording lifecycle** — `test_recording_lifecycle_via_api` for start/stop recording via API

### Test Coverage
- **Total test count** — 243 tests (18 frontend + 79 lib + 92 API integration + 6 config + 48 E2E)
- **E2E breakdown** — All tests follow standardized naming, complete coverage of core CDP operations, HTTP API, workflow, and recording features
- **Categories covered** — navigate, mouse, keyboard, form, axref, tabs, cookies, storage, console, network, screenshot, profile, lifecycle, snapshot, emulate, touch, frames, dialog, workflow, recording

### Breaking Changes
- Test function names changed — E2E test names now follow standardized pattern (may affect external tooling that references specific test names)

### Documentation
- Updated `src-tauri/tests/README.md` with current test count and standardized naming documentation
```

**Step 4: Bump version to 0.9.4**

Edit package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json: `0.9.3` → `0.9.4`

**Step 5: Commit release**

```bash
git add CHANGELOG.md package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/tests/README.md
git commit -m "chore: release v0.9.4 — final test coverage with testid standardization"
```

---

## Task 11: Push and Monitor CI

**Step 1: Push**

```bash
git push origin main
```

**Step 2: Monitor CI**

```bash
gh run list --limit 1
```

Get the run ID, then poll every 90s:

```bash
for i in 1 10; do
  sleep 90
  result=$(gh run view <RUN_ID> --json status,conclusion -q '"status=\(.status) conclusion=\(.conclusion)"')
  echo "$(date +%H:%M): $result"
  echo "$result" | grep -q "completed" && break
done
```

**Step 3: If CI fails**

```bash
gh run view <RUN_ID> --log-failed 2>&1 | head -50
```

Fix any failures and push again.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-03-01-final-test-coverage-and-cleanup.md`.**

**Two execution options:**

**1. Subagent-Driven (this session)** — I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** — Open new session with executing-plans, batch execution with checkpoints

**Which approach?**
