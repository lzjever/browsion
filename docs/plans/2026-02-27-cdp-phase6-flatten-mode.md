# Browsion CDP Phase 6: Flatten Mode Multi-Tab + Bug Fixes + New Features

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor CDPClient to CDP Flatten Mode for real multi-tab support, fix 4 known bugs, and add 7 new capabilities (console CDP events, network interception, PDF, page text, touch, frames).

**Architecture:** Replace per-page WebSocket (page-specific WS URL) with a single browser-level WebSocket connection using CDP Flatten Mode. All tab operations route via `sessionId`. Per-tab state (AX cache, inflight counter, current URL) is stored in a `TabState` registry and swapped on tab switch. Bug fixes are self-contained changes on top of the refactored base. New features follow the same pattern as existing ones.

**Tech Stack:** Rust, tokio, tokio-tungstenite, serde_json, CDP (Chrome DevTools Protocol)

---

## Overview of Changes

| Phase | What | Why |
|-------|------|-----|
| 1 | Flatten Mode architecture | Real multi-tab: one WS, N sessions |
| 2 | Tab management rewrite | wait_for_new_tab, true switch_tab |
| 3 | Bug fixes (4) | Selector safety, scroll, console |
| 4 | New features (7) | Console CDP, intercept, PDF, text, touch, frames |
| 5 | API + MCP wiring | Expose all new capabilities |

**Compile check command** (run after every task):
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | head -50
```

---

## Phase 1 — CDP Flatten Mode Architecture

### Task 1: Add new types to types.rs

**Files:**
- Modify: `src-tauri/src/agent/types.rs`

Add `active` flag to `TabInfo`, add `FrameInfo`, `ConsoleLogEntry`, `InterceptRule`, `InterceptAction`.

**Step 1: Replace and extend types.rs**

```rust
// In types.rs — update TabInfo to add `active` field:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(rename = "type")]
    pub target_type: String,
    /// Whether this is the currently active (CDP-connected) tab
    #[serde(default)]
    pub active: bool,
}

// Add after TabInfo:

/// A browser frame (main frame or iframe).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameInfo {
    pub id: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

/// A console log entry captured via Runtime.consoleAPICalled or Log.entryAdded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleLogEntry {
    /// "log" | "error" | "warn" | "info" | "debug" | "table" | "trace" | "exception"
    #[serde(rename = "type")]
    pub entry_type: String,
    /// The formatted arguments as strings
    pub args: Vec<String>,
    /// Unix timestamp in milliseconds
    pub timestamp: f64,
    /// "console" | "exception" | "network" | "other"
    pub source: String,
}

/// An intercept rule: match URL by substring, then block or mock.
#[derive(Debug, Clone)]
pub struct InterceptRule {
    /// Substring match against the request URL
    pub url_pattern: String,
    pub action: InterceptAction,
}

#[derive(Debug, Clone)]
pub enum InterceptAction {
    /// Return a network error (Fetch.failRequest with errorReason=Failed)
    Block,
    /// Return a synthetic HTTP response
    Mock {
        status: u16,
        body: String,
        content_type: String,
    },
}
```

**Step 2: Compile check**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -20
```
Expected: no new errors (TabInfo change may cause unused field warnings — OK).

**Step 3: Commit**
```bash
git add src-tauri/src/agent/types.rs
git commit -m "feat(types): add TabInfo.active, FrameInfo, ConsoleLogEntry, InterceptRule"
```

---

### Task 2: Add new fields to CDPClient struct

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` (lines 14–48, struct definition only)

**Step 1: Add `TabState` struct just before CDPClient**

Insert between the `use` block and the `CDPClient` struct:

```rust
/// Per-tab state saved/restored on tab switch.
#[derive(Default, Clone)]
struct TabState {
    /// Flatten-mode CDP session ID for this tab. Empty string = not yet attached.
    session_id: String,
    /// Current URL of this tab.
    url: String,
    /// AX ref cache: ref_id ("e1"…) → backend_node_id
    ax_ref_cache: HashMap<String, i64>,
    /// Count of in-flight network requests
    inflight_requests: u32,
}
```

**Step 2: Replace CDPClient struct fields**

Replace the entire `pub struct CDPClient { … }` block with:

```rust
#[allow(clippy::type_complexity)]
pub struct CDPClient {
    /// Browser-level WebSocket sender (single connection for all tabs)
    ws_tx: Option<
        Arc<
            Mutex<
                futures::stream::SplitSink<
                    tokio_tungstenite::WebSocketStream<
                        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                    >,
                    WsMessage,
                >,
            >,
        >,
    >,
    /// Command response routing: (session_id, msg_id) → sender
    /// session_id="" for browser-level commands (Target domain)
    responses: Arc<Mutex<HashMap<(String, u32), tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    /// Event routing: (session_id, method) → one-shot senders
    /// session_id="" for browser-level events (Target.*)
    events: Arc<Mutex<HashMap<(String, String), Vec<tokio::sync::oneshot::Sender<serde_json::Value>>>>>,
    /// Tab registry: target_id → TabState
    tab_registry: Arc<Mutex<HashMap<String, TabState>>>,
    /// Currently active target_id (which tab CDP commands go to)
    active_target_id: Arc<Mutex<String>>,
    /// AX ref cache for the active tab (synced from/to tab_registry on switch)
    ax_ref_cache: Arc<Mutex<HashMap<String, i64>>>,
    /// Persistent network request log (max 500 entries, all tabs combined)
    network_log: Arc<Mutex<VecDeque<serde_json::Value>>>,
    /// Inflight network requests for the active tab
    inflight_requests: Arc<Mutex<u32>>,
    /// Persistent console log (max 500 entries, all tabs combined)
    console_log: Arc<Mutex<VecDeque<serde_json::Value>>>,
    /// Network intercept rules (Fetch domain)
    intercept_rules: Arc<Mutex<Vec<crate::agent::types::InterceptRule>>>,
    /// Frame execution contexts: frame_id → context_id
    frame_contexts: Arc<Mutex<HashMap<String, i64>>>,
    /// Currently active frame_id (None = main frame)
    active_frame_id: Arc<Mutex<Option<String>>>,
    /// Chrome process ID (owned launches only)
    chrome_pid: Option<u32>,
    /// Profile being used
    profile_id: String,
    /// Current URL of active tab
    current_url: Arc<Mutex<String>>,
    /// Message ID counter
    msg_id: Arc<Mutex<u32>>,
    /// CDP port being used
    cdp_port: u16,
}
```

**Step 3: Update `CDPClient::new()` and `CDPClient::attach()`**

Replace the `new()` constructor:

```rust
pub fn new(profile_id: String) -> Self {
    Self {
        ws_tx: None,
        responses: Arc::new(Mutex::new(HashMap::new())),
        events: Arc::new(Mutex::new(HashMap::new())),
        tab_registry: Arc::new(Mutex::new(HashMap::new())),
        active_target_id: Arc::new(Mutex::new(String::new())),
        ax_ref_cache: Arc::new(Mutex::new(HashMap::new())),
        network_log: Arc::new(Mutex::new(VecDeque::new())),
        inflight_requests: Arc::new(Mutex::new(0)),
        console_log: Arc::new(Mutex::new(VecDeque::new())),
        intercept_rules: Arc::new(Mutex::new(Vec::new())),
        frame_contexts: Arc::new(Mutex::new(HashMap::new())),
        active_frame_id: Arc::new(Mutex::new(None)),
        chrome_pid: None,
        profile_id,
        current_url: Arc::new(Mutex::new(String::new())),
        msg_id: Arc::new(Mutex::new(1)),
        cdp_port: crate::process::port::allocate_cdp_port(),
    }
}
```

Update `attach()` the same way (same fields, `cdp_port` from parameter):

```rust
pub async fn attach(profile_id: String, cdp_port: u16) -> Result<Self, String> {
    let mut client = Self {
        ws_tx: None,
        responses: Arc::new(Mutex::new(HashMap::new())),
        events: Arc::new(Mutex::new(HashMap::new())),
        tab_registry: Arc::new(Mutex::new(HashMap::new())),
        active_target_id: Arc::new(Mutex::new(String::new())),
        ax_ref_cache: Arc::new(Mutex::new(HashMap::new())),
        network_log: Arc::new(Mutex::new(VecDeque::new())),
        inflight_requests: Arc::new(Mutex::new(0)),
        console_log: Arc::new(Mutex::new(VecDeque::new())),
        intercept_rules: Arc::new(Mutex::new(Vec::new())),
        frame_contexts: Arc::new(Mutex::new(HashMap::new())),
        active_frame_id: Arc::new(Mutex::new(None)),
        chrome_pid: None,
        profile_id,
        current_url: Arc::new(Mutex::new(String::new())),
        msg_id: Arc::new(Mutex::new(1)),
        cdp_port,
    };
    client.connect_websocket().await?;
    Ok(client)
}
```

**Step 4: Compile check**

Expect many errors because `responses`, `events`, `ax_ref_cache` types changed. That's fine — we fix them in the next tasks.

```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error\[" | wc -l
```

**Step 5: Commit**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "refactor(cdp): add TabState, multi-session struct fields for flatten mode"
```

---

### Task 3: Rewrite `send_command` and `subscribe_event` for session routing

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` (send_command, subscribe_event sections)

**Step 1: Replace `send_command` with session-aware version**

Find and replace the entire `send_command` function (lines ~261–297) with:

```rust
/// Send a CDP command to the currently active tab session.
/// If no tab is active yet (during initial setup), sends at browser level (no sessionId).
async fn send_command(
    &self,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let session_id = self.active_session_id().await;
    self.send_command_in_session(&session_id, method, params).await
}

/// Send a CDP command explicitly at browser level (no sessionId).
/// Use for Target.* domain commands regardless of active tab.
async fn send_browser_command(
    &self,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    self.send_command_in_session("", method, params).await
}

/// Core command sender — routes by (session_id, msg_id).
async fn send_command_in_session(
    &self,
    session_id: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let tx = self.ws_tx.as_ref().ok_or("WebSocket not connected")?;

    let (id, rx) = {
        let mut msg_id = self.msg_id.lock().await;
        *msg_id += 1;
        let id = *msg_id - 1;
        let (resp_tx, rx) = tokio::sync::oneshot::channel();
        let key = (session_id.to_string(), id);
        self.responses.lock().await.insert(key, resp_tx);
        (id, rx)
    };

    let mut command = serde_json::json!({
        "id": id,
        "method": method,
        "params": params
    });
    if !session_id.is_empty() {
        command["sessionId"] = serde_json::Value::String(session_id.to_string());
    }

    let mut tx_guard = tx.lock().await;
    tx_guard
        .send(WsMessage::Text(command.to_string()))
        .await
        .map_err(|e| format!("Failed to send command: {}", e))?;
    drop(tx_guard);

    match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => Err("Response channel closed".to_string()),
        Err(_) => Err(format!("Command timeout: {}", method)),
    }
}

/// Returns the CDP session_id for the currently active tab.
/// Returns empty string if no tab is active (browser-level).
async fn active_session_id(&self) -> String {
    let target_id = self.active_target_id.lock().await.clone();
    if target_id.is_empty() {
        return String::new();
    }
    self.tab_registry
        .lock()
        .await
        .get(&target_id)
        .map(|t| t.session_id.clone())
        .unwrap_or_default()
}
```

**Step 2: Replace `subscribe_event` with session-aware version**

Find and replace the `subscribe_event` function (lines ~299–313) with:

```rust
/// Subscribe to a CDP event in the active tab session.
async fn subscribe_event(
    &self,
    method: &str,
) -> Result<tokio::sync::oneshot::Receiver<serde_json::Value>, String> {
    let session_id = self.active_session_id().await;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let key = (session_id, method.to_string());
    self.events.lock().await.entry(key).or_default().push(tx);
    Ok(rx)
}

/// Subscribe to a browser-level CDP event (Target.* domain, no sessionId).
async fn subscribe_browser_event(
    &self,
    method: &str,
) -> Result<tokio::sync::oneshot::Receiver<serde_json::Value>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let key = ("".to_string(), method.to_string());
    self.events.lock().await.entry(key).or_default().push(tx);
    Ok(rx)
}
```

**Step 3: Compile check**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -30
```

**Step 4: Commit**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "refactor(cdp): session-aware send_command + subscribe_event for flatten mode"
```

---

### Task 4: Rewrite the WS reader for multi-session dispatch

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` — `setup_ws_connection` function

**Step 1: Replace the entire `setup_ws_connection` function**

The new version handles `(session_id, id)` response routing and persistent events.

```rust
async fn setup_ws_connection(&mut self, ws_url: &str) -> Result<(), String> {
    tracing::info!("Connecting to CDP WebSocket: {}", ws_url);

    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("Failed to connect WebSocket: {}", e))?;

    let (tx, mut rx) = StreamExt::split(ws_stream);
    let ws_tx_arc = Arc::new(Mutex::new(tx));
    self.ws_tx = Some(ws_tx_arc.clone());

    // Clone all shared state for the reader task
    let responses = self.responses.clone();
    let events = self.events.clone();
    let network_log = self.network_log.clone();
    let console_log = self.console_log.clone();
    let inflight_clone = self.inflight_requests.clone();
    let tab_registry = self.tab_registry.clone();
    let intercept_rules = self.intercept_rules.clone();
    let frame_contexts_clone = self.frame_contexts.clone();
    let ws_tx_for_reader = ws_tx_arc.clone();
    let msg_id_for_reader = self.msg_id.clone();

    tokio::spawn(async move {
        while let Some(msg) = StreamExt::next(&mut rx).await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        // Extract sessionId ("" = browser-level)
                        let session_id = json
                            .get("sessionId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if let Some(id) = json.get("id").and_then(|i| i.as_u64()) {
                            // ── Command response ──────────────────────────
                            let key = (session_id.clone(), id as u32);
                            if let Some(sender) = responses.lock().await.remove(&key) {
                                let _ = sender.send(json.clone());
                            }
                        } else if let Some(method_val) = json.get("method") {
                            // ── CDP Event ─────────────────────────────────
                            let method = method_val.as_str().unwrap_or("").to_string();
                            let params = json
                                .get("params")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);

                            // Persistent event handlers
                            match method.as_str() {
                                // ── Target: tab lifecycle ─────────────────
                                "Target.attachedToTarget" => {
                                    let target_id = params["targetInfo"]["targetId"]
                                        .as_str().unwrap_or("").to_string();
                                    let new_session_id = params["sessionId"]
                                        .as_str().unwrap_or("").to_string();
                                    let url = params["targetInfo"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    let title = params["targetInfo"]["title"]
                                        .as_str().unwrap_or("").to_string();
                                    let mut reg = tab_registry.lock().await;
                                    let tab = reg.entry(target_id.clone()).or_default();
                                    tab.session_id = new_session_id;
                                    tab.url = url;
                                    // title may be empty initially — set if non-empty
                                    if !title.is_empty() { tab.url = title; }
                                    tracing::debug!("Tab attached: {}", target_id);
                                }
                                "Target.targetCreated" => {
                                    let target_id = params["targetInfo"]["targetId"]
                                        .as_str().unwrap_or("").to_string();
                                    let target_type = params["targetInfo"]["type"]
                                        .as_str().unwrap_or("");
                                    let url = params["targetInfo"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    if target_type == "page" {
                                        let mut reg = tab_registry.lock().await;
                                        reg.entry(target_id.clone()).or_insert_with(|| {
                                            let mut s = TabState::default();
                                            s.url = url;
                                            s
                                        });
                                        tracing::debug!("New tab discovered: {}", target_id);
                                    }
                                }
                                "Target.targetDestroyed" => {
                                    let target_id = params["targetId"]
                                        .as_str().unwrap_or("").to_string();
                                    tab_registry.lock().await.remove(&target_id);
                                    tracing::debug!("Tab destroyed: {}", target_id);
                                }
                                "Target.targetInfoChanged" => {
                                    let target_id = params["targetInfo"]["targetId"]
                                        .as_str().unwrap_or("").to_string();
                                    let url = params["targetInfo"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    let title = params["targetInfo"]["title"]
                                        .as_str().unwrap_or("").to_string();
                                    let mut reg = tab_registry.lock().await;
                                    if let Some(tab) = reg.get_mut(&target_id) {
                                        tab.url = url;
                                        if !title.is_empty() {
                                            // store in url field for now — title not in TabState
                                            // (title is fetched on demand)
                                        }
                                    }
                                    tracing::debug!("Tab info changed: {}", target_id);
                                }
                                // ── Network: inflight tracking ────────────
                                "Network.requestWillBeSent" => {
                                    let url = params["request"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    let req_method = params["request"]["method"]
                                        .as_str().unwrap_or("").to_string();
                                    let request_id = params["requestId"]
                                        .as_str().unwrap_or("").to_string();
                                    let mut log = network_log.lock().await;
                                    if log.len() >= 500 { log.pop_front(); }
                                    log.push_back(serde_json::json!({
                                        "type": "request",
                                        "url": url,
                                        "method": req_method,
                                        "requestId": request_id,
                                        "sessionId": session_id
                                    }));
                                    *inflight_clone.lock().await =
                                        inflight_clone.lock().await.saturating_add(1);
                                }
                                "Network.responseReceived" => {
                                    let url = params["response"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    let status = params["response"]["status"]
                                        .as_i64().unwrap_or(0);
                                    let mime = params["response"]["mimeType"]
                                        .as_str().unwrap_or("").to_string();
                                    let request_id = params["requestId"]
                                        .as_str().unwrap_or("").to_string();
                                    let mut log = network_log.lock().await;
                                    if log.len() >= 500 { log.pop_front(); }
                                    log.push_back(serde_json::json!({
                                        "type": "response",
                                        "url": url,
                                        "status": status,
                                        "mimeType": mime,
                                        "requestId": request_id,
                                        "sessionId": session_id
                                    }));
                                    let mut count = inflight_clone.lock().await;
                                    *count = count.saturating_sub(1);
                                }
                                "Network.loadingFailed" => {
                                    let mut count = inflight_clone.lock().await;
                                    *count = count.saturating_sub(1);
                                }
                                // ── Console: Runtime.consoleAPICalled ──────
                                "Runtime.consoleAPICalled" => {
                                    let console_type = params["type"]
                                        .as_str().unwrap_or("log").to_string();
                                    let args: Vec<String> = params["args"]
                                        .as_array()
                                        .map(|arr| arr.iter().map(|a| {
                                            a.get("value")
                                                .and_then(|v| {
                                                    if v.is_string() { v.as_str().map(|s| s.to_string()) }
                                                    else { Some(v.to_string()) }
                                                })
                                                .or_else(|| a.get("description")
                                                    .and_then(|v| v.as_str())
                                                    .map(|s| s.to_string()))
                                                .unwrap_or_default()
                                        }).collect())
                                        .unwrap_or_default();
                                    let timestamp = params["timestamp"]
                                        .as_f64().unwrap_or(0.0);
                                    let mut log = console_log.lock().await;
                                    if log.len() >= 500 { log.pop_front(); }
                                    log.push_back(serde_json::json!({
                                        "type": console_type,
                                        "args": args,
                                        "timestamp": timestamp,
                                        "source": "console"
                                    }));
                                }
                                // ── Console: Log.entryAdded (browser logs) ─
                                "Log.entryAdded" => {
                                    let entry = &params["entry"];
                                    let level = entry["level"].as_str().unwrap_or("info");
                                    let text = entry["text"].as_str().unwrap_or("");
                                    let source = entry["source"].as_str().unwrap_or("other");
                                    let timestamp = entry["timestamp"].as_f64().unwrap_or(0.0);
                                    let mut log = console_log.lock().await;
                                    if log.len() >= 500 { log.pop_front(); }
                                    log.push_back(serde_json::json!({
                                        "type": level,
                                        "args": [text],
                                        "timestamp": timestamp,
                                        "source": source
                                    }));
                                }
                                // ── Console: Runtime.exceptionThrown ───────
                                "Runtime.exceptionThrown" => {
                                    let desc = params["exceptionDetails"]["exception"]["description"]
                                        .as_str()
                                        .or_else(|| params["exceptionDetails"]["text"].as_str())
                                        .unwrap_or("Unknown JS exception")
                                        .to_string();
                                    let url_str = params["exceptionDetails"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    let line = params["exceptionDetails"]["lineNumber"]
                                        .as_i64().unwrap_or(0);
                                    let timestamp = params["timestamp"].as_f64().unwrap_or(0.0);
                                    let mut log = console_log.lock().await;
                                    if log.len() >= 500 { log.pop_front(); }
                                    log.push_back(serde_json::json!({
                                        "type": "error",
                                        "args": [format!("Uncaught: {} ({}:{})", desc, url_str, line)],
                                        "timestamp": timestamp,
                                        "source": "exception"
                                    }));
                                }
                                // ── Frame contexts ──────────────────────────
                                "Runtime.executionContextCreated" => {
                                    let ctx = &params["context"];
                                    if let (Some(ctx_id), Some(frame_id)) = (
                                        ctx["id"].as_i64(),
                                        ctx["auxData"]["frameId"].as_str(),
                                    ) {
                                        frame_contexts_clone
                                            .lock().await
                                            .insert(frame_id.to_string(), ctx_id);
                                    }
                                }
                                "Runtime.executionContextDestroyed" => {
                                    // Clean up destroyed contexts
                                    let ctx_id = params["executionContextId"].as_i64();
                                    if let Some(cid) = ctx_id {
                                        let mut fc = frame_contexts_clone.lock().await;
                                        fc.retain(|_, v| *v != cid);
                                    }
                                }
                                // ── Network Interception (Fetch domain) ────
                                "Fetch.requestPaused" => {
                                    let request_id = params["requestId"]
                                        .as_str().unwrap_or("").to_string();
                                    let url = params["request"]["url"]
                                        .as_str().unwrap_or("").to_string();
                                    let rules = intercept_rules.lock().await;
                                    let matching = rules.iter()
                                        .find(|r| url.contains(&r.url_pattern))
                                        .cloned();
                                    drop(rules);

                                    let (resp_method, resp_params) = match matching {
                                        None => (
                                            "Fetch.continueRequest",
                                            serde_json::json!({ "requestId": request_id }),
                                        ),
                                        Some(crate::agent::types::InterceptRule {
                                            action: crate::agent::types::InterceptAction::Block,
                                            ..
                                        }) => (
                                            "Fetch.failRequest",
                                            serde_json::json!({
                                                "requestId": request_id,
                                                "errorReason": "BlockedByClient"
                                            }),
                                        ),
                                        Some(crate::agent::types::InterceptRule {
                                            action: crate::agent::types::InterceptAction::Mock {
                                                status, body, content_type
                                            },
                                            ..
                                        }) => {
                                            use base64::Engine;
                                            let body_b64 = base64::engine::general_purpose::STANDARD
                                                .encode(&body);
                                            (
                                                "Fetch.fulfillRequest",
                                                serde_json::json!({
                                                    "requestId": request_id,
                                                    "responseCode": status,
                                                    "responseHeaders": [{
                                                        "name": "Content-Type",
                                                        "value": content_type
                                                    }],
                                                    "body": body_b64
                                                }),
                                            )
                                        }
                                    };

                                    // Send response directly from the reader
                                    let ws_clone = ws_tx_for_reader.clone();
                                    let mid = msg_id_for_reader.clone();
                                    let sid = session_id.clone();
                                    let rm = resp_method;
                                    let rp = resp_params;
                                    tokio::spawn(async move {
                                        let id = {
                                            let mut m = mid.lock().await;
                                            *m += 1;
                                            *m
                                        };
                                        let mut cmd = serde_json::json!({
                                            "id": id,
                                            "method": rm,
                                            "params": rp
                                        });
                                        if !sid.is_empty() {
                                            cmd["sessionId"] =
                                                serde_json::Value::String(sid);
                                        }
                                        let _ = ws_clone
                                            .lock().await
                                            .send(WsMessage::Text(cmd.to_string()))
                                            .await;
                                    });
                                }
                                _ => {}
                            }

                            // Dispatch to one-shot subscribers (session-aware)
                            let key = (session_id.clone(), method.clone());
                            if let Some(senders) =
                                events.lock().await.remove(&key)
                            {
                                for sender in senders {
                                    let _ = sender.send(params.clone());
                                }
                            }
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    tracing::debug!("WebSocket closed");
                    break;
                }
                Err(e) => {
                    tracing::debug!("WebSocket error: {:?}", e);
                }
                _ => {}
            }
        }
    });

    tracing::info!("CDP WebSocket reader started for profile {}", self.profile_id);
    Ok(())
}
```

**Step 2: Check base64 dependency**

```bash
grep "base64" src-tauri/Cargo.toml
```

If not present, add to `[dependencies]`:
```toml
base64 = "0.22"
```

**Step 3: Compile check**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -30
```

**Step 4: Commit**
```bash
git add src-tauri/src/agent/cdp.rs src-tauri/Cargo.toml
git commit -m "refactor(cdp): WS reader with session routing, console CDP events, intercept, frames"
```

---

### Task 5: Rewrite `connect_websocket` for browser-level WS (Flatten Mode)

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` — `connect_websocket` function

**Step 1: Replace `connect_websocket` function entirely**

```rust
/// Connect to Chrome via the browser-level WebSocket (Flatten Mode).
/// Polls `/json/version` for the browser WS URL, attaches to first page target.
async fn connect_websocket(&mut self) -> Result<(), String> {
    let mut retries = 0u32;
    const MAX_RETRIES: u32 = 30;
    let mut last_error = String::new();

    while retries < MAX_RETRIES {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Step 1: get browser-level WS URL from /json/version
        let version_url = format!("http://localhost:{}/json/version", self.cdp_port);
        let version_resp = match reqwest::get(&version_url).await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                last_error = format!("HTTP {}", r.status());
                retries += 1;
                continue;
            }
            Err(e) => {
                last_error = format!("Connection error: {}", e);
                retries += 1;
                continue;
            }
        };

        let version: serde_json::Value = match version_resp.json().await {
            Ok(v) => v,
            Err(e) => {
                last_error = format!("Parse error: {}", e);
                retries += 1;
                continue;
            }
        };

        let browser_ws_url = match version
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
        {
            Some(url) => url.to_string(),
            None => {
                last_error = "No webSocketDebuggerUrl in /json/version".to_string();
                retries += 1;
                continue;
            }
        };

        // Step 2: connect WebSocket + start reader
        self.setup_ws_connection(&browser_ws_url).await?;

        // Step 3: enable target discovery
        self.send_browser_command("Target.setDiscoverTargets", json!({"discover": true}))
            .await
            .map_err(|e| format!("setDiscoverTargets failed: {}", e))?;

        // Step 4: get existing targets
        let targets_resp = self
            .send_browser_command("Target.getTargets", json!({}))
            .await
            .map_err(|e| format!("getTargets failed: {}", e))?;

        let target_infos = targets_resp["result"]["targetInfos"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        // Step 5: find the first page target
        let page_target = target_infos
            .iter()
            .find(|t| t["type"].as_str() == Some("page"));

        let target_id = match page_target {
            Some(t) => t["targetId"].as_str().unwrap_or("").to_string(),
            None => {
                last_error = "No page target found".to_string();
                retries += 1;
                continue;
            }
        };

        if target_id.is_empty() {
            last_error = "Empty target_id".to_string();
            retries += 1;
            continue;
        }

        // Step 6: attach to the page target with flatten mode
        let attach_resp = self
            .send_browser_command(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
            )
            .await
            .map_err(|e| format!("attachToTarget failed: {}", e))?;

        let session_id = match attach_resp["result"]["sessionId"].as_str() {
            Some(s) => s.to_string(),
            None => {
                last_error = "No sessionId from attachToTarget".to_string();
                retries += 1;
                continue;
            }
        };

        // Step 7: register tab + set active
        {
            let mut reg = self.tab_registry.lock().await;
            let url = page_target
                .and_then(|t| t["url"].as_str())
                .unwrap_or("")
                .to_string();
            let mut tab = TabState::default();
            tab.session_id = session_id;
            tab.url = url;
            reg.insert(target_id.clone(), tab);
        }
        *self.active_target_id.lock().await = target_id.clone();

        // Step 8: enable CDP domains in this session
        self.send_command("Page.enable", json!({})).await?;
        self.send_command("Runtime.enable", json!({})).await?;
        self.send_command("Network.enable", json!({})).await?;
        self.send_command("Log.enable", json!({})).await?;

        tracing::info!(
            "CDP flatten mode connected for profile {} (target: {})",
            self.profile_id,
            target_id
        );
        return Ok(());
    }

    Err(format!(
        "Failed to connect to Chrome CDP on port {} after {} retries: {}",
        self.cdp_port, MAX_RETRIES, last_error
    ))
}
```

**Step 2: Compile check**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -30
```

**Step 3: Commit**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "refactor(cdp): connect to browser-level WS via /json/version (flatten mode)"
```

---

## Phase 2 — Tab Management Rewrite

### Task 6: Implement `wait_for_new_tab` and rewrite `switch_tab`

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` — Tab section (lines ~1928–1999)

**Step 1: Replace the entire tab management section**

Find the comment `// ── Advanced: Tabs` and replace everything up to (not including) the Cookies section:

```rust
// ── Advanced: Tabs ─────────────────────────────────────────────────────

/// Wait for a new tab to open (e.g. triggered by clicking a target="_blank" link).
/// Call this BEFORE the action that opens the tab to avoid race conditions.
/// Returns the target_id of the new tab.
pub async fn wait_for_new_tab(&self, timeout_ms: u64) -> Result<String, String> {
    // Subscribe BEFORE the action that creates the tab
    let rx = self.subscribe_browser_event("Target.targetCreated").await?;

    match tokio::time::timeout(
        tokio::time::Duration::from_millis(timeout_ms),
        rx,
    )
    .await
    {
        Ok(Ok(params)) => {
            let target_type = params["targetInfo"]["type"].as_str().unwrap_or("");
            if target_type != "page" {
                return Err(format!(
                    "New target is type '{}', not a page tab",
                    target_type
                ));
            }
            let target_id = params["targetInfo"]["targetId"]
                .as_str()
                .ok_or("No targetId in Target.targetCreated event")?
                .to_string();
            tracing::debug!("wait_for_new_tab: new tab detected: {}", target_id);
            Ok(target_id)
        }
        Ok(Err(_)) => Err("wait_for_new_tab: event channel closed".to_string()),
        Err(_) => Err(format!(
            "wait_for_new_tab: no new tab appeared within {}ms",
            timeout_ms
        )),
    }
}

/// Switch the active CDP session to a different tab.
/// Attaches to the target if not already attached.
/// Saves/restores per-tab state (AX cache, inflight counter, URL).
pub async fn switch_tab(&self, target_id: &str) -> Result<(), String> {
    let current_target = self.active_target_id.lock().await.clone();
    if current_target == target_id {
        tracing::debug!("switch_tab: already on {}", target_id);
        return Ok(());
    }

    // Save current tab state
    if !current_target.is_empty() {
        let ax_cache = self.ax_ref_cache.lock().await.clone();
        let inflight = *self.inflight_requests.lock().await;
        let url = self.current_url.lock().await.clone();
        let mut reg = self.tab_registry.lock().await;
        if let Some(tab) = reg.get_mut(&current_target) {
            tab.ax_ref_cache = ax_cache;
            tab.inflight_requests = inflight;
            tab.url = url;
        }
    }

    // Attach to new target if not yet attached
    let has_session = {
        let reg = self.tab_registry.lock().await;
        reg.get(target_id)
            .map(|t| !t.session_id.is_empty())
            .unwrap_or(false)
    };

    if !has_session {
        let attach_resp = self
            .send_browser_command(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
            )
            .await
            .map_err(|e| format!("switch_tab: attachToTarget failed: {}", e))?;

        let session_id = attach_resp["result"]["sessionId"]
            .as_str()
            .ok_or("switch_tab: no sessionId from attachToTarget")?
            .to_string();

        let mut reg = self.tab_registry.lock().await;
        let tab = reg.entry(target_id.to_string()).or_default();
        tab.session_id = session_id;
    }

    // Switch active target
    *self.active_target_id.lock().await = target_id.to_string();

    // Restore this tab's state
    {
        let reg = self.tab_registry.lock().await;
        if let Some(tab) = reg.get(target_id) {
            *self.ax_ref_cache.lock().await = tab.ax_ref_cache.clone();
            *self.inflight_requests.lock().await = tab.inflight_requests;
            *self.current_url.lock().await = tab.url.clone();
        }
    }

    // Enable CDP domains in the new session
    // (idempotent — safe to call multiple times)
    self.send_command("Page.enable", json!({})).await?;
    self.send_command("Runtime.enable", json!({})).await?;
    self.send_command("Network.enable", json!({})).await?;
    self.send_command("Log.enable", json!({})).await?;

    // Clear AX ref cache (page may have changed)
    self.ax_ref_cache.lock().await.clear();
    *self.active_frame_id.lock().await = None;

    tracing::info!("Switched to tab: {}", target_id);
    Ok(())
}

/// List all known browser tabs.
pub async fn list_tabs(&self) -> Result<Vec<crate::agent::types::TabInfo>, String> {
    // Use Target.getTargets for fresh data
    let result = self
        .send_browser_command("Target.getTargets", json!({}))
        .await?;
    let target_infos = result["result"]["targetInfos"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let active_target = self.active_target_id.lock().await.clone();
    let tabs: Vec<crate::agent::types::TabInfo> = target_infos
        .iter()
        .filter(|t| t["type"].as_str() == Some("page"))
        .map(|t| {
            let id = t["targetId"].as_str().unwrap_or("").to_string();
            let is_active = id == active_target;
            crate::agent::types::TabInfo {
                id: id.clone(),
                url: t["url"].as_str().unwrap_or("").to_string(),
                title: t["title"].as_str().unwrap_or("").to_string(),
                target_type: "page".to_string(),
                active: is_active,
            }
        })
        .collect();
    Ok(tabs)
}

/// Open a new tab with the given URL and switch to it.
pub async fn new_tab(&self, url: &str) -> Result<crate::agent::types::TabInfo, String> {
    let result = self
        .send_browser_command(
            "Target.createTarget",
            json!({"url": url}),
        )
        .await?;
    let target_id = result["result"]["targetId"]
        .as_str()
        .ok_or("new_tab: no targetId from createTarget")?
        .to_string();

    // Small delay to let the target initialize before attaching
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    self.switch_tab(&target_id).await?;

    Ok(crate::agent::types::TabInfo {
        id: target_id,
        url: url.to_string(),
        title: String::new(),
        target_type: "page".to_string(),
        active: true,
    })
}

/// Close a tab by target_id. If it is the active tab, switches to another tab if available.
pub async fn close_tab(&self, target_id: &str) -> Result<(), String> {
    self.send_browser_command(
        "Target.closeTarget",
        json!({"targetId": target_id}),
    )
    .await?;

    self.tab_registry.lock().await.remove(target_id);

    // If we closed the active tab, switch to another if available
    let active = self.active_target_id.lock().await.clone();
    if active == target_id {
        let other = self
            .tab_registry
            .lock()
            .await
            .keys()
            .next()
            .cloned();
        if let Some(other_id) = other {
            let _ = self.switch_tab(&other_id).await;
        } else {
            *self.active_target_id.lock().await = String::new();
        }
    }

    tracing::info!("Closed tab: {}", target_id);
    Ok(())
}
```

**Step 2: Compile check**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -30
```

**Step 3: Commit**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "feat(cdp): implement wait_for_new_tab + real switch_tab via CDP flatten mode"
```

---

## Phase 3 — Bug Fixes

### Task 7: Fix selector escaping in `type_text`, `wait_for_element`, `scroll_into_view`, `select_option`

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs`

**The Problem:**
These four functions use string-interpolated selectors: `format!("querySelector('{}')")` which breaks for selectors like `input[type="email"]`. Fix: use `serde_json::to_string(selector)` to produce a properly-escaped JS string literal, and use `deepQuery` for Shadow DOM consistency.

**Step 1: Fix `type_text` (~lines 759–803)**

Replace the JS expression string inside `type_text`. The key change is:
1. Use `serde_json::to_string(selector)` not the manual-escape approach
2. Use `deepQuery` instead of `document.querySelector`

```rust
pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), String> {
    let selector_json = serde_json::to_string(selector).unwrap_or_default();
    let text_json = serde_json::to_string(text).unwrap_or_default();

    let js = format!(
        r#"(function() {{
            function deepQuery(root, sel) {{
                let el = root.querySelector(sel);
                if (el) return el;
                for (const host of root.querySelectorAll('*')) {{
                    if (host.shadowRoot) {{
                        const found = deepQuery(host.shadowRoot, sel);
                        if (found) return found;
                    }}
                }}
                return null;
            }}
            const el = deepQuery(document, {selector_json});
            if (!el) return false;
            el.focus();
            const niv = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')
                || Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value');
            if (niv && niv.set) {{
                niv.set.call(el, {text_json});
            }} else {{
                el.value = {text_json};
            }}
            el.dispatchEvent(new Event('input', {{bubbles: true}}));
            el.dispatchEvent(new Event('change', {{bubbles: true}}));
            return true;
        }})()"#
    );

    let result = self
        .send_command("Runtime.evaluate", json!({"expression": js, "returnByValue": true}))
        .await?;

    let typed = result["result"]["result"]["value"]
        .as_bool()
        .unwrap_or(false);

    if typed {
        tracing::debug!("Typed into element: {}", selector);
        Ok(())
    } else {
        Err(format!("Element not found: {}", selector))
    }
}
```

**Step 2: Fix `wait_for_element` (~lines 1103–1136)**

```rust
pub async fn wait_for_element(&self, selector: &str, timeout_ms: u64) -> Result<(), String> {
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let start = std::time::Instant::now();
    let selector_json = serde_json::to_string(selector).unwrap_or_default();

    let js = format!(
        r#"(function() {{
            function deepQuery(root, sel) {{
                if (root.querySelector(sel)) return true;
                for (const host of root.querySelectorAll('*')) {{
                    if (host.shadowRoot && deepQuery(host.shadowRoot, sel)) return true;
                }}
                return false;
            }}
            return deepQuery(document, {selector_json});
        }})()"#
    );

    loop {
        let result = self
            .send_command("Runtime.evaluate", json!({"expression": js, "returnByValue": true}))
            .await?;

        let found = result["result"]["result"]["value"]
            .as_bool()
            .unwrap_or(false);

        if found {
            tracing::debug!("Element found: {}", selector);
            return Ok(());
        }

        if start.elapsed() > timeout {
            return Err(format!("Timeout waiting for element: {}", selector));
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
```

**Step 3: Fix `scroll_into_view` (~lines 1013–1026)**

```rust
pub async fn scroll_into_view(&self, selector: &str) -> Result<(), String> {
    let selector_json = serde_json::to_string(selector).unwrap_or_default();
    let js = format!(
        r#"(function() {{
            function deepQuery(root, sel) {{
                let el = root.querySelector(sel);
                if (el) return el;
                for (const host of root.querySelectorAll('*')) {{
                    if (host.shadowRoot) {{
                        const found = deepQuery(host.shadowRoot, sel);
                        if (found) return found;
                    }}
                }}
                return null;
            }}
            const el = deepQuery(document, {selector_json});
            if (!el) return false;
            el.scrollIntoView({{behavior: 'instant', block: 'center'}});
            return true;
        }})()"#
    );
    let result = self.evaluate_js(&js).await?;
    if result.as_bool().unwrap_or(false) {
        Ok(())
    } else {
        Err(format!("Element not found: {}", selector))
    }
}
```

**Step 4: Fix `select_option` (~lines 1028–1053)**

```rust
pub async fn select_option(&self, selector: &str, value: &str) -> Result<(), String> {
    let selector_json = serde_json::to_string(selector).unwrap_or_default();
    let value_json = serde_json::to_string(value).unwrap_or_default();
    let js = format!(
        r#"(function() {{
            const el = document.querySelector({selector_json});
            if (!el) return 'not_found';
            const opt = Array.from(el.options).find(
                o => o.value === {value_json} || o.text.trim() === {value_json}
            );
            if (!opt) return 'no_option';
            el.value = opt.value;
            el.dispatchEvent(new Event('input', {{bubbles: true}}));
            el.dispatchEvent(new Event('change', {{bubbles: true}}));
            return 'ok';
        }})()"#
    );
    let result = self.evaluate_js(&js).await?;
    match result.as_str().unwrap_or("") {
        "ok" => {
            tracing::debug!("Selected '{}' in: {}", value, selector);
            Ok(())
        }
        "not_found" => Err(format!("Select element not found: {}", selector)),
        _ => Err(format!("Option '{}' not found in: {}", value, selector)),
    }
}
```

**Step 5: Compile check**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -20
```

**Step 6: Commit**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "fix(cdp): consistent selector encoding via serde_json + deepQuery in type_text, wait_for_element, scroll_into_view"
```

---

### Task 8: Fix `scroll` to use real mouse wheel events

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` — `scroll` function (~lines 1056–1076)

**The Problem:** `window.scrollBy` doesn't work on pages with custom `wheel` event handlers. `scroll` and `scroll_element` have inconsistent implementations.

**Step 1: Replace `scroll` to delegate to `scroll_element`**

```rust
/// Scroll the page in the given direction.
/// Uses real mouse wheel events at the viewport center (same as scroll_element).
pub async fn scroll(&self, direction: &str, amount: u32) -> Result<(), String> {
    let amount = amount as f64;
    let (delta_x, delta_y) = match direction {
        "up" => (0.0, -amount),
        "down" => (0.0, amount),
        "left" => (-amount, 0.0),
        "right" => (amount, 0.0),
        _ => (0.0, amount),
    };
    // Dispatch at viewport center — same mechanism as scroll_element("window", ...)
    self.send_command(
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseWheel",
            "x": 640.0,
            "y": 400.0,
            "deltaX": delta_x,
            "deltaY": delta_y,
            "modifiers": 0
        }),
    )
    .await?;
    tracing::debug!("Scrolled: {} by {}", direction, amount);
    Ok(())
}
```

**Step 2: Compile check + commit**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error"
git add src-tauri/src/agent/cdp.rs
git commit -m "fix(cdp): scroll uses Input.dispatchMouseEvent mouseWheel (consistent with scroll_element)"
```

---

### Task 9: Fix console capture — replace JS injection with CDP events

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs` — console section (~lines 2056–2106)

**The Problem:** The JS injection approach loses logs after navigation and only captures `log/error/warn`. The WS reader in Task 4 already captures `Runtime.consoleAPICalled`, `Log.entryAdded`, and `Runtime.exceptionThrown` into `self.console_log`. Now update the public API methods to use it.

**Step 1: Replace `get_console_logs` and `enable_console_capture`**

```rust
/// Get recent console log entries (captures console.*, browser errors, and JS exceptions).
/// Console capture is always active — no need to call enable_console_capture first.
/// Returns up to 500 most recent entries.
pub async fn get_console_logs(&self) -> Result<serde_json::Value, String> {
    let log = self.console_log.lock().await;
    let entries: Vec<serde_json::Value> = log.iter().cloned().collect();
    Ok(serde_json::Value::Array(entries))
}

/// Clear the console log.
pub async fn clear_console_logs(&self) {
    self.console_log.lock().await.clear();
}

/// No-op for backward compatibility. Console capture is always on via CDP events.
/// Previously this injected a JS interceptor; that approach was fragile.
pub async fn enable_console_capture(&self) -> Result<(), String> {
    // Console capture is automatic via Runtime.consoleAPICalled + Log.entryAdded.
    // This method is kept for API compatibility only.
    tracing::debug!("enable_console_capture: no-op (CDP events always active)");
    Ok(())
}
```

**Step 2: Compile check + commit**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error"
git add src-tauri/src/agent/cdp.rs
git commit -m "fix(cdp): console capture via Runtime.consoleAPICalled + Log.entryAdded (no JS injection)"
```

---

## Phase 4 — New Features

### Task 10: Add `get_page_text`

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs`

Add after the `get_dom_context` function:

```rust
/// Get the full visible text content of the page body.
/// Returns document.body.innerText — clean text as seen by the user, no HTML tags.
/// Useful for AI agents reading page content or verifying text presence.
/// Truncated to 50,000 characters to keep responses manageable.
pub async fn get_page_text(&self) -> Result<String, String> {
    let js = r#"(function() {
        try {
            const text = document.body ? document.body.innerText : '';
            return text.length > 50000 ? text.substring(0, 50000) + '\n[truncated]' : text;
        } catch(e) {
            return '';
        }
    })()"#;
    let result = self.evaluate_js(js).await?;
    Ok(result.as_str().unwrap_or("").to_string())
}
```

**Commit:**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "feat(cdp): add get_page_text - returns document.body.innerText (50k limit)"
```

---

### Task 11: Add network interception (`intercept_network`, `clear_intercepts`)

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs`

The WS reader (Task 4) already handles `Fetch.requestPaused` using `intercept_rules`. Now add the public API to add/clear rules and enable/disable interception.

Add after the network log functions:

```rust
// ── Network Interception ────────────────────────────────────────────────

/// Block requests whose URL contains `url_pattern`.
/// Enables the Fetch domain automatically on first call.
pub async fn block_url(&self, url_pattern: &str) -> Result<(), String> {
    self.ensure_fetch_enabled().await?;
    let rule = crate::agent::types::InterceptRule {
        url_pattern: url_pattern.to_string(),
        action: crate::agent::types::InterceptAction::Block,
    };
    self.intercept_rules.lock().await.push(rule);
    tracing::debug!("Added block rule: {}", url_pattern);
    Ok(())
}

/// Mock a URL pattern with a synthetic response.
pub async fn mock_url(
    &self,
    url_pattern: &str,
    status: u16,
    body: &str,
    content_type: &str,
) -> Result<(), String> {
    self.ensure_fetch_enabled().await?;
    let rule = crate::agent::types::InterceptRule {
        url_pattern: url_pattern.to_string(),
        action: crate::agent::types::InterceptAction::Mock {
            status,
            body: body.to_string(),
            content_type: content_type.to_string(),
        },
    };
    self.intercept_rules.lock().await.push(rule);
    tracing::debug!("Added mock rule: {} → {}", url_pattern, status);
    Ok(())
}

/// Remove all intercept rules. Disables the Fetch domain.
pub async fn clear_intercepts(&self) -> Result<(), String> {
    self.intercept_rules.lock().await.clear();
    self.send_command("Fetch.disable", json!({})).await?;
    tracing::debug!("Cleared all intercept rules");
    Ok(())
}

/// Enable the Fetch domain if not already enabled (idempotent).
async fn ensure_fetch_enabled(&self) -> Result<(), String> {
    // Enable with no patterns = intercept all requests
    self.send_command(
        "Fetch.enable",
        json!({"handleAuthRequests": false}),
    )
    .await?;
    Ok(())
}
```

**Commit:**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "feat(cdp): add block_url / mock_url / clear_intercepts via Fetch domain"
```

---

### Task 12: Add `print_to_pdf`

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs`

Add after `screenshot_element`:

```rust
/// Generate a PDF of the current page.
/// Returns base64-encoded PDF data.
/// `landscape`: print in landscape orientation (default false)
/// `print_background`: include CSS backgrounds (default true)
/// `scale`: scale factor 0.1–2.0 (default 1.0)
pub async fn print_to_pdf(
    &self,
    landscape: bool,
    print_background: bool,
    scale: f64,
) -> Result<String, String> {
    let result = self
        .send_command(
            "Page.printToPDF",
            json!({
                "landscape": landscape,
                "printBackground": print_background,
                "scale": scale,
                "paperWidth": 8.5,
                "paperHeight": 11.0,
                "marginTop": 0.4,
                "marginBottom": 0.4,
                "marginLeft": 0.4,
                "marginRight": 0.4
            }),
        )
        .await?;

    result["result"]["data"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "print_to_pdf: no data in response".to_string())
}
```

**Commit:**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "feat(cdp): add print_to_pdf via Page.printToPDF"
```

---

### Task 13: Add touch events (`tap`, `swipe`)

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs`

Add after the `drag` function:

```rust
// ── Touch events ─────────────────────────────────────────────────────────

/// Tap an element by CSS selector using touch events.
/// Use for mobile-emulated pages or apps with touch-only handlers.
pub async fn tap(&self, selector: &str) -> Result<(), String> {
    let (cx, cy) = self.get_element_center(selector).await?;
    self.tap_at(cx, cy).await?;
    tracing::debug!("Tapped element: {}", selector);
    Ok(())
}

/// Tap at specific viewport coordinates.
pub async fn tap_at(&self, x: f64, y: f64) -> Result<(), String> {
    let touch_point = json!([{
        "x": x,
        "y": y,
        "radiusX": 1,
        "radiusY": 1,
        "rotationAngle": 0.0,
        "force": 1.0,
        "id": 0
    }]);
    self.send_command("Input.dispatchTouchEvent", json!({
        "type": "touchStart",
        "touchPoints": touch_point,
        "modifiers": 0
    })).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    self.send_command("Input.dispatchTouchEvent", json!({
        "type": "touchEnd",
        "touchPoints": [],
        "modifiers": 0
    })).await?;
    Ok(())
}

/// Swipe from element center in a direction (up/down/left/right) by `distance` pixels.
/// Uses touch events with smooth intermediate moves.
pub async fn swipe(
    &self,
    selector: &str,
    direction: &str,
    distance: f64,
) -> Result<(), String> {
    let (start_x, start_y) = self.get_element_center(selector).await?;
    let (end_x, end_y) = match direction {
        "up" => (start_x, start_y - distance),
        "down" => (start_x, start_y + distance),
        "left" => (start_x - distance, start_y),
        "right" => (start_x + distance, start_y),
        _ => (start_x, start_y - distance),
    };

    // Touch start
    self.send_command("Input.dispatchTouchEvent", json!({
        "type": "touchStart",
        "touchPoints": [{"x": start_x, "y": start_y, "id": 0, "force": 1.0}],
        "modifiers": 0
    })).await?;

    // Move in steps
    const STEPS: u32 = 10;
    for i in 1..=STEPS {
        let t = i as f64 / STEPS as f64;
        let mx = start_x + (end_x - start_x) * t;
        let my = start_y + (end_y - start_y) * t;
        self.send_command("Input.dispatchTouchEvent", json!({
            "type": "touchMove",
            "touchPoints": [{"x": mx, "y": my, "id": 0, "force": 1.0}],
            "modifiers": 0
        })).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
    }

    // Touch end
    self.send_command("Input.dispatchTouchEvent", json!({
        "type": "touchEnd",
        "touchPoints": [],
        "modifiers": 0
    })).await?;

    tracing::debug!("Swiped element '{}' {} by {}px", selector, direction, distance);
    Ok(())
}
```

**Commit:**
```bash
git add src-tauri/src/agent/cdp.rs
git commit -m "feat(cdp): add tap / tap_at / swipe touch events via Input.dispatchTouchEvent"
```

---

### Task 14: Add frame support (`get_frames`, `switch_frame`, `main_frame`)

**Files:**
- Modify: `src-tauri/src/agent/cdp.rs`
- Modify: `src-tauri/src/agent/types.rs` (FrameInfo already added in Task 1)

**Step 1: Update `evaluate_js` to respect active frame context**

Find the `evaluate_js` function and update it to include `contextId` when a frame is active:

```rust
pub async fn evaluate_js(&self, expression: &str) -> Result<serde_json::Value, String> {
    let mut params = json!({
        "expression": expression,
        "returnByValue": true,
        "awaitPromise": true
    });

    // If switched to a frame, use its execution context
    if let Some(frame_id) = &*self.active_frame_id.lock().await {
        let contexts = self.frame_contexts.lock().await;
        if let Some(ctx_id) = contexts.get(frame_id) {
            params["contextId"] = json!(ctx_id);
        }
    }

    let result = self.send_command("Runtime.evaluate", params).await?;

    if let Some(exception) = result["result"].get("exceptionDetails") {
        let msg = exception["text"].as_str().unwrap_or("JS evaluation error");
        return Err(msg.to_string());
    }

    let value = result["result"]["result"]["value"]
        .clone();

    Ok(if value.is_null() {
        // Try description for non-serializable values
        result["result"]["result"]["description"]
            .clone()
            .unwrap_or(serde_json::Value::Null)
    } else {
        value
    })
}
```

**Step 2: Add frame methods**

Add after the Emulation section:

```rust
// ── Frames ────────────────────────────────────────────────────────────────

/// List all frames in the current page (main frame + iframes).
pub async fn get_frames(&self) -> Result<Vec<crate::agent::types::FrameInfo>, String> {
    let result = self.send_command("Page.getFrameTree", json!({})).await?;
    let frame_tree = &result["result"]["frameTree"];

    fn parse_frame(frame: &serde_json::Value) -> Vec<crate::agent::types::FrameInfo> {
        let mut frames = Vec::new();
        let f = &frame["frame"];
        frames.push(crate::agent::types::FrameInfo {
            id: f["id"].as_str().unwrap_or("").to_string(),
            url: f["url"].as_str().unwrap_or("").to_string(),
            name: f["name"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string()),
            parent_id: f["parentId"].as_str().map(|s| s.to_string()),
        });
        if let Some(children) = frame["childFrames"].as_array() {
            for child in children {
                frames.extend(parse_frame(child));
            }
        }
        frames
    }

    Ok(parse_frame(frame_tree))
}

/// Switch the JS execution context to a specific iframe (by frame_id from get_frames).
/// After switching, evaluate_js and selector-based tools operate inside that frame.
/// Only works for same-origin frames (browser security policy).
pub async fn switch_frame(&self, frame_id: &str) -> Result<(), String> {
    let has_context = self.frame_contexts.lock().await.contains_key(frame_id);
    if !has_context {
        return Err(format!(
            "Frame '{}' not found or not yet loaded. Call get_frames first.",
            frame_id
        ));
    }
    *self.active_frame_id.lock().await = Some(frame_id.to_string());
    tracing::debug!("Switched to frame: {}", frame_id);
    Ok(())
}

/// Switch back to the main frame context.
pub async fn main_frame(&self) {
    *self.active_frame_id.lock().await = None;
    tracing::debug!("Switched back to main frame");
}
```

**Step 3: Compile check + commit**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -20
git add src-tauri/src/agent/cdp.rs
git commit -m "feat(cdp): add get_frames / switch_frame / main_frame for iframe support"
```

---

## Phase 5 — HTTP API and MCP Tools

### Task 15: Update HTTP API routes and handlers

**Files:**
- Modify: `src-tauri/src/api/mod.rs`

**Step 1: Add new routes to the router**

In the `router()` function, add these new routes after the existing ones:

```rust
// Tabs — add wait_for_new_tab route
.route("/api/browser/:id/tabs/wait_new", post(browser_wait_for_new_tab))
// Page text
.route("/api/browser/:id/page_text", get(browser_get_page_text))
// Network intercept
.route("/api/browser/:id/intercept/block", post(browser_intercept_block))
.route("/api/browser/:id/intercept/mock", post(browser_intercept_mock))
.route("/api/browser/:id/intercept", delete(browser_clear_intercepts))
// PDF
.route("/api/browser/:id/pdf", get(browser_print_to_pdf))
// Touch
.route("/api/browser/:id/tap", post(browser_tap))
.route("/api/browser/:id/swipe", post(browser_swipe))
// Frames
.route("/api/browser/:id/frames", get(browser_get_frames))
.route("/api/browser/:id/switch_frame", post(browser_switch_frame))
.route("/api/browser/:id/main_frame", post(browser_main_frame))
// Console: add clear endpoint
.route("/api/browser/:id/console/clear", post(browser_clear_console))
```

Also add `routing::delete` to the axum imports if not present.

**Step 2: Add request structs**

```rust
#[derive(serde::Deserialize)]
struct WaitNewTabReq {
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

#[derive(serde::Deserialize)]
struct InterceptBlockReq {
    url_pattern: String,
}

#[derive(serde::Deserialize)]
struct InterceptMockReq {
    url_pattern: String,
    status: u16,
    body: String,
    #[serde(default = "default_content_type")]
    content_type: String,
}
fn default_content_type() -> String { "application/json".to_string() }

#[derive(serde::Deserialize)]
struct PdfReq {
    #[serde(default)]
    landscape: bool,
    #[serde(default = "default_print_bg")]
    print_background: bool,
    #[serde(default = "default_scale")]
    scale: f64,
}
fn default_print_bg() -> bool { true }
fn default_scale() -> f64 { 1.0 }

#[derive(serde::Deserialize)]
struct TapReq {
    selector: String,
}

#[derive(serde::Deserialize)]
struct SwipeReq {
    selector: String,
    direction: String,
    #[serde(default = "default_swipe_distance")]
    distance: f64,
}
fn default_swipe_distance() -> f64 { 300.0 }

#[derive(serde::Deserialize)]
struct SwitchFrameReq {
    frame_id: String,
}
```

**Step 3: Add handler functions**

```rust
async fn browser_wait_for_new_tab(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<WaitNewTabReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let target_id = client.wait_for_new_tab(req.timeout_ms).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "target_id": target_id })))
}

async fn browser_get_page_text(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let text = client.get_page_text().await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "text": text, "length": text.len() })))
}

async fn browser_intercept_block(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<InterceptBlockReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.block_url(&req.url_pattern).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true, "rule": "block", "pattern": req.url_pattern })))
}

async fn browser_intercept_mock(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<InterceptMockReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.mock_url(&req.url_pattern, req.status, &req.body, &req.content_type)
        .await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true, "rule": "mock", "pattern": req.url_pattern })))
}

async fn browser_clear_intercepts(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.clear_intercepts().await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_print_to_pdf(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    axum::extract::Query(req): axum::extract::Query<PdfReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let data = client.print_to_pdf(req.landscape, req.print_background, req.scale)
        .await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "data": data, "format": "pdf" })))
}

async fn browser_tap(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<TapReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.tap(&req.selector).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_swipe(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SwipeReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.swipe(&req.selector, &req.direction, req.distance)
        .await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_get_frames(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    let frames = client.get_frames().await.map_err(cdp_err)?;
    let value = serde_json::to_value(frames)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(value))
}

async fn browser_switch_frame(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<SwitchFrameReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.switch_frame(&req.frame_id).await.map_err(cdp_err)?;
    Ok(Json(serde_json::json!({ "ok": true, "frame_id": req.frame_id })))
}

async fn browser_main_frame(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.main_frame().await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn browser_clear_console(
    State(state): State<ApiState>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let cdp_port = require_cdp_port(&state, &id)?;
    let handle = state.session_manager.get_client(&id, cdp_port).await.map_err(cdp_err)?;
    let client = handle.lock().await;
    client.clear_console_logs().await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
```

**Step 4: Compile check + commit**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -20
git add src-tauri/src/api/mod.rs
git commit -m "feat(api): add routes for wait_for_new_tab, page_text, intercept, pdf, touch, frames"
```

---

### Task 16: Add MCP tools for all new features

**Files:**
- Modify: `src-tauri/src/bin/browsion-mcp.rs`

**Step 1: Add parameter structs (after existing param structs, before impl BrowsionMcpServer)**

```rust
// ── New Tab Wait ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WaitNewTabParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// How long to wait for a new tab (ms, default 10000)
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

// ── Page Text ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ProfileIdParam {
    /// Profile ID of the running browser
    profile_id: String,
}

// ── Network Intercept ────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct InterceptBlockParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// Substring to match against request URLs (e.g. "analytics", ".jpg", "/api/v2/")
    url_pattern: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct InterceptMockParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// Substring to match against request URLs
    url_pattern: String,
    /// HTTP status code to return (e.g. 200, 404)
    status: u16,
    /// Response body string
    body: String,
    /// Content-Type header value (default: "application/json")
    #[serde(default = "default_content_type")]
    content_type: String,
}
fn default_content_type() -> String { "application/json".to_string() }

// ── PDF ──────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PdfParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// Print in landscape orientation (default: false)
    #[serde(default)]
    landscape: bool,
    /// Include CSS background colors and images (default: true)
    #[serde(default = "default_true")]
    print_background: bool,
    /// Scale factor 0.1–2.0 (default: 1.0)
    #[serde(default = "default_scale")]
    scale: f64,
}
fn default_true() -> bool { true }
fn default_scale() -> f64 { 1.0 }

// ── Touch ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TapParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// CSS selector of the element to tap
    selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SwipeParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// CSS selector of the element to start swipe from
    selector: String,
    /// Swipe direction: "up" | "down" | "left" | "right"
    direction: String,
    /// Distance to swipe in pixels (default: 300)
    #[serde(default = "default_swipe_dist")]
    distance: f64,
}
fn default_swipe_dist() -> f64 { 300.0 }

// ── Frames ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SwitchFrameParam {
    /// Profile ID of the running browser
    profile_id: String,
    /// Frame ID from get_frames (the "id" field)
    frame_id: String,
}
```

**Step 2: Add new tool methods to `impl BrowsionMcpServer`**

Add these methods to the existing `impl BrowsionMcpServer` block:

```rust
// ── Tabs: wait_for_new_tab ────────────────────────────────────────────────

/// Wait for a new browser tab to open. Call BEFORE the action that opens the tab.
#[tool(description = "Wait for a new tab to open (e.g. clicking a target='_blank' link or window.open()). IMPORTANT: call this BEFORE the action that opens the tab to avoid race conditions. Returns the target_id of the new tab. Then call switch_tab(target_id) to start operating in it.")]
async fn wait_for_new_tab(
    &self,
    Parameters(p): Parameters<WaitNewTabParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/tabs/wait_new", p.profile_id),
            &json!({ "timeout_ms": p.timeout_ms }),
        )
        .await?;
    Self::text_result(body)
}

// ── Observe: get_page_text ────────────────────────────────────────────────

/// Get the full visible text content of the current page.
#[tool(description = "Get the full visible text content of the current page (document.body.innerText). Returns clean text without HTML tags. Useful for reading page content, verifying text presence, or feeding content to an LLM. Truncated at 50,000 characters.")]
async fn get_page_text(
    &self,
    Parameters(p): Parameters<ProfileIdParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_get(&format!("/api/browser/{}/page_text", p.profile_id))
        .await?;
    Self::text_result(body)
}

// ── Network: intercept ────────────────────────────────────────────────────

/// Block network requests matching a URL pattern.
#[tool(description = "Block all network requests whose URL contains url_pattern. Useful for blocking ads, analytics, or slow third-party resources during automation. Example: block_url(url_pattern='analytics.google.com') blocks all Google Analytics requests. Use clear_intercepts() to remove all rules.")]
async fn block_url(
    &self,
    Parameters(p): Parameters<InterceptBlockParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/intercept/block", p.profile_id),
            &json!({ "url_pattern": p.url_pattern }),
        )
        .await?;
    Self::text_result(body)
}

/// Mock a network request with a synthetic response.
#[tool(description = "Mock a network request: return a synthetic HTTP response for all requests matching url_pattern. Useful for testing, faking API responses, or bypassing paywalls in testing environments. Example: mock_url(url_pattern='/api/user', status=200, body='{\"name\":\"test\"}')")]
async fn mock_url(
    &self,
    Parameters(p): Parameters<InterceptMockParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/intercept/mock", p.profile_id),
            &json!({
                "url_pattern": p.url_pattern,
                "status": p.status,
                "body": p.body,
                "content_type": p.content_type
            }),
        )
        .await?;
    Self::text_result(body)
}

/// Remove all network intercept rules.
#[tool(description = "Remove all network intercept rules (added with block_url or mock_url) and disable network interception. Restores normal network behavior.")]
async fn clear_intercepts(
    &self,
    Parameters(p): Parameters<ProfileIdParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_delete(&format!("/api/browser/{}/intercept", p.profile_id))
        .await?;
    Self::text_result(body)
}

// ── PDF ──────────────────────────────────────────────────────────────────

/// Generate a PDF of the current page.
#[tool(description = "Generate a PDF of the current page. Returns base64-encoded PDF data. Captures the full page including backgrounds. Options: landscape (bool), print_background (bool, default true), scale (0.1-2.0, default 1.0). PDF format, A4 paper size with 0.4in margins.")]
async fn print_to_pdf(
    &self,
    Parameters(p): Parameters<PdfParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_get(&format!(
            "/api/browser/{}/pdf?landscape={}&print_background={}&scale={}",
            p.profile_id, p.landscape, p.print_background, p.scale
        ))
        .await?;
    Self::text_result(body)
}

// ── Touch ─────────────────────────────────────────────────────────────────

/// Tap an element using a touch event.
#[tool(description = "Tap an element using a touch event (touchStart + touchEnd). Use for mobile-emulated pages or apps with touch-only event handlers. For regular desktop clicks, use click() instead.")]
async fn tap(
    &self,
    Parameters(p): Parameters<TapParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/tap", p.profile_id),
            &json!({ "selector": p.selector }),
        )
        .await?;
    Self::text_result(body)
}

/// Swipe on an element (touch gesture).
#[tool(description = "Perform a swipe touch gesture on an element. direction: 'up' | 'down' | 'left' | 'right'. distance: pixels to swipe (default 300). Use for carousels, sliders, swipeable lists, pull-to-refresh. Requires mobile emulation for best results.")]
async fn swipe(
    &self,
    Parameters(p): Parameters<SwipeParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/swipe", p.profile_id),
            &json!({
                "selector": p.selector,
                "direction": p.direction,
                "distance": p.distance
            }),
        )
        .await?;
    Self::text_result(body)
}

// ── Frames ────────────────────────────────────────────────────────────────

/// List all frames (iframes) in the current page.
#[tool(description = "List all frames (main frame + iframes) in the current page. Returns each frame's id, url, name, and parent_id. Use the frame id with switch_frame() to operate inside an iframe.")]
async fn get_frames(
    &self,
    Parameters(p): Parameters<ProfileIdParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_get(&format!("/api/browser/{}/frames", p.profile_id))
        .await?;
    Self::text_result(body)
}

/// Switch the JS execution context to an iframe.
#[tool(description = "Switch execution context to an iframe so subsequent evaluate_js, click, type_text, etc. operate inside that frame. Use get_frames() to find the frame_id. Only works for same-origin iframes. Call main_frame() to switch back.")]
async fn switch_frame(
    &self,
    Parameters(p): Parameters<SwitchFrameParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/switch_frame", p.profile_id),
            &json!({ "frame_id": p.frame_id }),
        )
        .await?;
    Self::text_result(body)
}

/// Switch back to the main frame context.
#[tool(description = "Switch back to the main page frame after using switch_frame(). Must be called before navigating or using get_page_state() after iframe operations.")]
async fn main_frame(
    &self,
    Parameters(p): Parameters<ProfileIdParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/main_frame", p.profile_id),
            &json!({}),
        )
        .await?;
    Self::text_result(body)
}

// ── Console: clear ────────────────────────────────────────────────────────

/// Clear the console log buffer.
#[tool(description = "Clear all buffered console log entries. Useful before an operation you want to monitor cleanly. Console capture is always active — no need to call enable_console_capture first.")]
async fn clear_console_logs(
    &self,
    Parameters(p): Parameters<ProfileIdParam>,
) -> Result<CallToolResult, McpError> {
    let body = self
        .api_post(
            &format!("/api/browser/{}/console/clear", p.profile_id),
            &json!({}),
        )
        .await?;
    Self::text_result(body)
}
```

**Step 3: Add `api_delete` helper method** (if not present)

Add to the BrowsionMcpServer impl's helper methods:

```rust
async fn api_delete(&self, path: &str) -> Result<String, McpError> {
    let url = format!("{}{}", api_base(), path);
    let resp = self
        .client
        .delete(&url)
        .header("X-API-Key", &self.api_key)
        .send()
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    if resp.status().is_success() {
        resp.text().await.map_err(|e| McpError::internal_error(e.to_string(), None))
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(McpError::internal_error(format!("API {} → {}: {}", path, status, body), None))
    }
}
```

**Step 4: Update server instructions string and tool list**

In `get_info()`, update the instructions to add new tools to the tool groups list:

```
**Tabs:** list_tabs, new_tab, switch_tab, close_tab, wait_for_new_tab
**Observe:** get_page_state, get_ax_tree, screenshot, get_page_text
**Network:** get_network_log, clear_network_log, block_url, mock_url, clear_intercepts
**PDF:** print_to_pdf
**Touch:** tap, swipe
**Frames:** get_frames, switch_frame, main_frame
**Console:** enable_console_capture, get_console_logs, clear_console_logs
```

Also update the workflow comment to include the new tab pattern:
```
## New Tab Workflow (target="_blank" links)
1. wait_for_new_tab(timeout_ms=5000)  ← call BEFORE the click
2. click_ref("e5")                     ← click the link
3. switch_tab(target_id=<from step 1>) ← switch to new tab
4. get_page_state()                    ← observe new tab
```

**Step 5: Compile check + commit**
```bash
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep "^error" | head -20
git add src-tauri/src/bin/browsion-mcp.rs
git commit -m "feat(mcp): add 11 new tools: wait_for_new_tab, get_page_text, block_url, mock_url, clear_intercepts, print_to_pdf, tap, swipe, get_frames, switch_frame, main_frame, clear_console_logs"
```

---

## Final Verification

### Task 17: Full build, test, and smoke test

**Step 1: Full build**
```bash
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: zero errors.

**Step 2: Run existing tests**
```bash
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -20
```
Expected: all tests pass (or known-skip for integration tests requiring Chrome).

**Step 3: Manual smoke test checklist**

Start a Chrome instance and run these checks via the MCP or HTTP API:

```
[ ] launch_browser → connects via flatten mode (check logs: "CDP flatten mode connected")
[ ] list_tabs → returns tabs with `active: true` on the current tab
[ ] navigate("https://example.com") → works, page loads
[ ] get_page_state → returns AX tree with refs
[ ] click_ref / type_ref → work as before
[ ] screenshot → works
[ ] get_page_text → returns page text
[ ] new_tab("https://google.com") → opens tab, switches to it
[ ] list_tabs → shows 2 tabs, new one is active
[ ] switch_tab(old_target_id) → switches back
[ ] wait_for_new_tab pattern:
    1. wait_for_new_tab (subscribe)
    2. evaluate_js("window.open('https://example.com')")
    3. wait_for_new_tab resolves with target_id
    4. switch_tab(target_id)
    5. get_page_state → shows example.com
[ ] get_console_logs → returns entries (test: evaluate_js("console.log('test')"))
[ ] block_url("google-analytics") → check network_log shows no analytics requests
[ ] get_frames → returns frames for a page with iframes
[ ] print_to_pdf → returns base64 PDF data
[ ] tap("#some-button") → fires touch events
```

**Step 4: Update MEMORY.md**

Update the memory file to reflect the new architecture and tool count.

**Step 5: Final commit**
```bash
git add -A
git commit -m "docs: update memory with flatten mode architecture and 72 total MCP tools"
```

---

## Summary

| Phase | Tasks | Changes |
|-------|-------|---------|
| 1 | 1–5 | Flatten mode architecture (CDPClient, WS reader, send_command) |
| 2 | 6 | Tab management: wait_for_new_tab, real switch_tab, CDP-based list/new/close |
| 3 | 7–9 | Bug fixes: selector escaping, scroll mouseWheel, console CDP events |
| 4 | 10–14 | New features: get_page_text, intercept, PDF, touch, frames |
| 5 | 15–16 | HTTP API routes + MCP tools (72 total tools after this) |
| 6 | 17 | Verification |

**MCP tool count after this plan: 72** (was 61)
New tools: wait_for_new_tab, get_page_text, block_url, mock_url, clear_intercepts, print_to_pdf, tap, swipe, get_frames, switch_frame, main_frame, clear_console_logs (+1 implicit: scroll_element already existed)
