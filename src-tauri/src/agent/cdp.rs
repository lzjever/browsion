use crate::agent::types::{AXNode, CookieInfo, DOMContext, DOMElement, PageState};
use crate::config::schema::BrowserProfile;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

/// Simple glob/wildcard matching: `*` matches any sequence of characters.
/// e.g. `"*/form*"` matches `"http://example.com/form?x=1"`
fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        // No wildcard — literal substring match
        return text.contains(pattern);
    }
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            None => return false,
            Some(idx) => {
                // The first segment must match at the start of the remaining text
                // only if the pattern does NOT start with '*'
                if i == 0 && !pattern.starts_with('*') && idx != 0 {
                    return false;
                }
                pos += idx + part.len();
            }
        }
    }
    // If the pattern does NOT end with '*', the last segment must reach the end
    if !pattern.ends_with('*') {
        let last = parts.last().unwrap();
        if !last.is_empty() && !text.ends_with(last) {
            return false;
        }
    }
    true
}

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

/// CDP Client using raw WebSocket for better Chrome compatibility
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

impl CDPClient {
    /// Create a new CDP client (allocates a fresh port).
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

    /// Attach to an already-running Chrome instance by its CDP port.
    /// Does **not** launch Chrome — the browser must already be running
    /// with `--remote-debugging-port={cdp_port}`.
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
            chrome_pid: None, // not owned; won't kill on drop
            profile_id,
            current_url: Arc::new(Mutex::new(String::new())),
            msg_id: Arc::new(Mutex::new(1)),
            cdp_port,
        };
        client.connect_websocket().await?;
        Ok(client)
    }

    /// Connect to the CDP WebSocket endpoint for the current `cdp_port`.
    /// Polls `/json/list`, finds the first "page" target, and connects.
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
                let tab = TabState {
                    session_id,
                    url,
                    ..Default::default()
                };
                reg.insert(target_id.clone(), tab);
            }
            *self.active_target_id.lock().await = target_id.clone();

            // Step 8: enable CDP domains in this session
            self.send_command("Page.enable", json!({})).await?;
            self.send_command("Runtime.enable", json!({})).await?;
            self.send_command("Network.enable", json!({})).await?;
            self.send_command("Log.enable", json!({})).await?;
            self.send_command("DOM.enable", json!({})).await?;

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

    /// Perform the actual WebSocket connection + enable CDP domains.
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
                                        let mut reg = tab_registry.lock().await;
                                        let tab = reg.entry(target_id.clone()).or_default();
                                        tab.session_id = new_session_id;
                                        if !url.is_empty() { tab.url = url; }
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
                                                TabState {
                                                    url,
                                                    ..Default::default()
                                                }
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
                                        let mut reg = tab_registry.lock().await;
                                        if let Some(tab) = reg.get_mut(&target_id) {
                                            if !url.is_empty() { tab.url = url; }
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
                                        drop(log);
                                        let mut count = inflight_clone.lock().await;
                                        *count = count.saturating_add(1);
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
                                        drop(log);
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
                                            .find(|r| glob_match(&r.url_pattern, &url))
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

    /// Subscribe to a CDP event in the active tab session.
    /// Must be called BEFORE triggering the action that fires the event.
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

    /// Launch Chrome with CDP enabled and connect.
    pub async fn launch(
        &mut self,
        chrome_path: &Path,
        profile: &BrowserProfile,
        headless: bool,
    ) -> Result<(), String> {
        let mut cmd = Command::new(chrome_path);

        cmd.arg(format!(
            "--user-data-dir={}",
            profile.user_data_dir.display()
        ));
        cmd.arg(format!("--remote-debugging-port={}", self.cdp_port));

        if headless {
            cmd.arg("--headless=new");
        }

        cmd.arg("--no-first-run");
        cmd.arg("--no-default-browser-check");
        cmd.arg("--disable-background-networking");
        cmd.arg("--disable-sync");

        if let Some(proxy) = &profile.proxy_server {
            cmd.arg(format!("--proxy-server={}", proxy));
        }

        cmd.arg(format!("--lang={}", profile.lang));

        if let Some(fp) = &profile.fingerprint {
            cmd.arg(format!("--fingerprint={}", fp));
        }

        if let Some(tz) = &profile.timezone {
            cmd.arg(format!("--timezone={}", tz));
            cmd.env("TZ", tz);
        }

        for arg in &profile.custom_args {
            cmd.arg(arg);
        }

        cmd.arg("about:blank");

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to launch Chrome: {}", e))?;
        self.chrome_pid = Some(child.id());

        self.connect_websocket().await
    }

    // ── Navigation ─────────────────────────────────────────────────

    /// Navigate to a URL, waiting for Page.loadEventFired (up to 15s).
    pub async fn navigate(&self, url: &str) -> Result<(), String> {
        self.navigate_wait(url, "load", 15000).await
    }

    /// Navigate with explicit wait strategy and timeout.
    /// `wait_until`: "load" | "domcontentloaded" | "networkidle" | "none"
    pub async fn navigate_wait(
        &self,
        url: &str,
        wait_until: &str,
        timeout_ms: u64,
    ) -> Result<(), String> {
        // Subscribe to event BEFORE sending navigate to avoid race conditions
        let event_rx = match wait_until {
            "load" => {
                Some(self.subscribe_event("Page.loadEventFired").await?)
            }
            "domcontentloaded" => {
                Some(self.subscribe_event("Page.domContentEventFired").await?)
            }
            "networkidle" => {
                Some(self.subscribe_event("Page.loadEventFired").await?)
            }
            _ => None,
        };

        let _ = self
            .send_command("Page.navigate", json!({"url": url}))
            .await?;
        *self.current_url.lock().await = url.to_string();

        // Clear AX ref cache on navigation
        self.ax_ref_cache.lock().await.clear();
        *self.inflight_requests.lock().await = 0;

        if wait_until == "networkidle" {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
            // Wait for load event first (with remaining time budget)
            if let Some(rx) = event_rx {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                let _ = tokio::time::timeout(remaining, rx).await;
            }
            // Then wait until inflight == 0 for 500ms consecutively (same deadline)
            let idle_target = std::time::Duration::from_millis(500);
            let mut idle_since: Option<std::time::Instant> = None;
            loop {
                if std::time::Instant::now() > deadline {
                    tracing::warn!("networkidle timeout ({}ms): {}", timeout_ms, url);
                    break;
                }
                let count = *self.inflight_requests.lock().await;
                if count == 0 {
                    match idle_since {
                        None => { idle_since = Some(std::time::Instant::now()); }
                        Some(t) if t.elapsed() >= idle_target => { break; }
                        _ => {}
                    }
                } else {
                    idle_since = None;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
            tracing::debug!("Network idle reached: {}", url);
        } else if let Some(rx) = event_rx {
            match tokio::time::timeout(tokio::time::Duration::from_millis(timeout_ms), rx).await {
                Ok(_) => tracing::debug!("Navigation complete: {}", url),
                Err(_) => tracing::warn!("Navigation timeout ({}ms): {}", timeout_ms, url),
            }
        }

        tracing::info!("Navigated to: {}", url);
        Ok(())
    }

    /// Wait for page load to complete (use after actions that trigger navigation).
    pub async fn wait_for_navigation(&self, timeout_ms: u64) -> Result<(), String> {
        let rx = self.subscribe_event("Page.loadEventFired").await?;
        match tokio::time::timeout(tokio::time::Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err("Navigation event channel closed".to_string()),
            Err(_) => Err(format!("wait_for_navigation timeout after {}ms", timeout_ms)),
        }
    }

    /// Get current URL from browser
    pub async fn get_url(&self) -> Result<String, String> {
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": "window.location.href",
                    "returnByValue": true
                }),
            )
            .await?;

        if let Some(url) = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
        {
            *self.current_url.lock().await = url.to_string();
            Ok(url.to_string())
        } else {
            Ok(self.current_url.lock().await.clone())
        }
    }

    /// Get page title
    pub async fn get_title(&self) -> Result<Option<String>, String> {
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": "document.title",
                    "returnByValue": true
                }),
            )
            .await?;

        if let Some(title) = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
        {
            Ok(Some(title.to_string()))
        } else {
            Ok(None)
        }
    }

    /// Go back in browser history. Uses Page.navigateToHistoryEntry so it works for SPA pushState.
    pub async fn go_back(&self) -> Result<(), String> {
        let nav_result = self
            .send_command("Page.getNavigationHistory", json!({}))
            .await?;
        let current_index = nav_result
            .get("result")
            .and_then(|r| r.get("currentIndex"))
            .and_then(|v| v.as_i64())
            .ok_or("Failed to get navigation history")?;
        let entries = nav_result
            .get("result")
            .and_then(|r| r.get("entries"))
            .and_then(|v| v.as_array())
            .ok_or("Failed to get navigation entries")?;

        let target_index = current_index - 1;
        if target_index < 0 {
            return Err("Already at the beginning of navigation history".to_string());
        }
        let entry_id = entries
            .get(target_index as usize)
            .and_then(|e| e.get("id"))
            .and_then(|v| v.as_i64())
            .ok_or("Failed to get history entry ID")?;

        // Subscribe to frameNavigated (works for both full-page and SPA navigation)
        let rx = self.subscribe_event("Page.frameNavigated").await?;
        self.ax_ref_cache.lock().await.clear();

        self.send_command("Page.navigateToHistoryEntry", json!({"entryId": entry_id}))
            .await?;

        let _ = tokio::time::timeout(tokio::time::Duration::from_millis(15000), rx).await;
        tracing::debug!("Navigated back");
        Ok(())
    }

    /// Go forward in browser history. Uses Page.navigateToHistoryEntry so it works for SPA pushState.
    pub async fn go_forward(&self) -> Result<(), String> {
        let nav_result = self
            .send_command("Page.getNavigationHistory", json!({}))
            .await?;
        let current_index = nav_result
            .get("result")
            .and_then(|r| r.get("currentIndex"))
            .and_then(|v| v.as_i64())
            .ok_or("Failed to get navigation history")?;
        let entries = nav_result
            .get("result")
            .and_then(|r| r.get("entries"))
            .and_then(|v| v.as_array())
            .ok_or("Failed to get navigation entries")?;

        let target_index = current_index + 1;
        if target_index >= entries.len() as i64 {
            return Err("Already at the end of navigation history".to_string());
        }
        let entry_id = entries
            .get(target_index as usize)
            .and_then(|e| e.get("id"))
            .and_then(|v| v.as_i64())
            .ok_or("Failed to get history entry ID")?;

        // Subscribe to frameNavigated (works for both full-page and SPA navigation)
        let rx = self.subscribe_event("Page.frameNavigated").await?;
        self.ax_ref_cache.lock().await.clear();

        self.send_command("Page.navigateToHistoryEntry", json!({"entryId": entry_id}))
            .await?;

        let _ = tokio::time::timeout(tokio::time::Duration::from_millis(15000), rx).await;
        tracing::debug!("Navigated forward");
        Ok(())
    }

    /// Reload the current page, waiting for page load.
    pub async fn reload(&self) -> Result<(), String> {
        let rx = self.subscribe_event("Page.loadEventFired").await?;
        self.ax_ref_cache.lock().await.clear();

        self.send_command("Page.reload", json!({})).await?;

        let _ = tokio::time::timeout(tokio::time::Duration::from_millis(15000), rx).await;
        tracing::debug!("Page reloaded");
        Ok(())
    }

    // ── Element helpers (shared by click/hover/double_click/right_click) ──────

    /// Get the center coordinates of a DOM element using JS getBoundingClientRect.
    /// Returns **viewport-relative** coordinates (correct for scrolled pages).
    /// Uses deep shadow DOM traversal to pierce shadow roots.
    async fn get_element_center(&self, selector: &str) -> Result<(f64, f64), String> {
        let selector_json = serde_json::to_string(selector).unwrap_or_default();
        let js = format!(
            r#"(function() {{
                function deepQuery(root, sel) {{
                    let el = root.querySelector(sel);
                    if (el) return el;
                    for (const host of root.querySelectorAll('*')) {{
                        if (host.shadowRoot) {{
                            el = deepQuery(host.shadowRoot, sel);
                            if (el) return el;
                        }}
                    }}
                    return null;
                }}
                const el = deepQuery(document, {selector_json});
                if (!el) return {{ error: 'not_found' }};
                const rect = el.getBoundingClientRect();
                if (rect.width === 0 && rect.height === 0) return {{ error: 'no_layout' }};
                return {{ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }};
            }})()"#
        );
        let result = self.evaluate_js(&js).await?;
        if let Some(err) = result.get("error").and_then(|v| v.as_str()) {
            return Err(format!("Element not found: {} ({})", selector, err));
        }
        let x = result.get("x").and_then(|v| v.as_f64()).ok_or("Failed to get element x")?;
        let y = result.get("y").and_then(|v| v.as_f64()).ok_or("Failed to get element y")?;
        Ok((x, y))
    }

    /// Dispatch a real left-click at coordinates via CDP Input domain.
    async fn dispatch_mouse_click(&self, x: f64, y: f64) -> Result<(), String> {
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseMoved", "x": x, "y": y, "button": "none", "clickCount": 0}),
        )
        .await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1}),
        )
        .await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseReleased", "x": x, "y": y, "button": "left", "clickCount": 1}),
        )
        .await?;
        Ok(())
    }

    // ── Mouse interactions ──────────────────────────────────────────

    /// Click an element by CSS selector using real mouse events (supports hover-triggered dropdowns, React synthetic events).
    pub async fn click(&self, selector: &str) -> Result<(), String> {
        let (cx, cy) = self.get_element_center(selector).await?;
        self.dispatch_mouse_click(cx, cy).await?;
        tracing::debug!("Clicked element: {}", selector);
        Ok(())
    }

    /// Hover over an element (dispatches mouseMoved — triggers :hover, tooltip, dropdown reveal).
    pub async fn hover(&self, selector: &str) -> Result<(), String> {
        let (cx, cy) = self.get_element_center(selector).await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseMoved", "x": cx, "y": cy, "button": "none", "clickCount": 0}),
        )
        .await?;
        tracing::debug!("Hovered element: {}", selector);
        Ok(())
    }

    /// Double-click an element by CSS selector.
    pub async fn double_click(&self, selector: &str) -> Result<(), String> {
        let (cx, cy) = self.get_element_center(selector).await?;
        // First click
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": cx, "y": cy, "button": "left", "clickCount": 1}),
        )
        .await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseReleased", "x": cx, "y": cy, "button": "left", "clickCount": 1}),
        )
        .await?;
        // Second click (clickCount: 2 signals double-click)
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": cx, "y": cy, "button": "left", "clickCount": 2}),
        )
        .await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseReleased", "x": cx, "y": cy, "button": "left", "clickCount": 2}),
        )
        .await?;
        tracing::debug!("Double-clicked element: {}", selector);
        Ok(())
    }

    /// Right-click an element by CSS selector (opens context menu).
    pub async fn right_click(&self, selector: &str) -> Result<(), String> {
        let (cx, cy) = self.get_element_center(selector).await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": cx, "y": cy, "button": "right", "clickCount": 1}),
        )
        .await?;
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseReleased", "x": cx, "y": cy, "button": "right", "clickCount": 1}),
        )
        .await?;
        tracing::debug!("Right-clicked element: {}", selector);
        Ok(())
    }

    // ── Keyboard / Input ────────────────────────────────────────────

    /// Type text into an element (fast mode: sets value + fires input/change events).
    /// Works for most inputs including React controlled components.
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

    /// Slow-type text character-by-character using real key events.
    /// Use for inputs that validate on each keystroke (e.g. autocomplete, OTP fields).
    pub async fn slow_type(
        &self,
        selector: &str,
        text: &str,
        delay_ms: u64,
    ) -> Result<(), String> {
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");

        // Focus the element first
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": format!(
                        "(function() {{ const el = document.querySelector('{escaped}'); if(el) {{ el.focus(); return true; }} return false; }})()"
                    ),
                    "returnByValue": true
                }),
            )
            .await?;

        let focused = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !focused {
            return Err(format!("Element not found: {}", selector));
        }

        for ch in text.chars() {
            let ch_str = ch.to_string();
            self.send_command(
                "Input.dispatchKeyEvent",
                json!({"type": "keyDown", "key": ch_str}),
            )
            .await?;
            self.send_command(
                "Input.dispatchKeyEvent",
                json!({"type": "char", "key": ch_str, "text": ch_str}),
            )
            .await?;
            self.send_command(
                "Input.dispatchKeyEvent",
                json!({"type": "keyUp", "key": ch_str}),
            )
            .await?;

            if delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }

        tracing::debug!("Slow-typed {} chars into: {}", text.len(), selector);
        Ok(())
    }

    /// Press a key (supports modifier combos like "Ctrl+A", "Shift+Enter").
    pub async fn press_key(&self, key: &str) -> Result<(), String> {
        // Parse modifier+key combos (e.g. "Ctrl+A", "Shift+Enter")
        let parts: Vec<&str> = key.split('+').collect();
        let (modifiers, base_key) = if parts.len() > 1 {
            let mods = &parts[..parts.len() - 1];
            let mut modifier_mask: u32 = 0;
            for m in mods {
                modifier_mask |= match *m {
                    "Ctrl" | "Control" => 2,
                    "Alt" => 1,
                    "Shift" => 8,
                    "Meta" | "Cmd" | "Command" => 4,
                    _ => 0,
                };
            }
            (modifier_mask, parts[parts.len() - 1])
        } else {
            (0, key)
        };

        let key_code = match base_key {
            "Enter" => 13,
            "Tab" => 9,
            "Escape" => 27,
            "Backspace" => 8,
            "Delete" => 46,
            "ArrowUp" => 38,
            "ArrowDown" => 40,
            "ArrowLeft" => 37,
            "ArrowRight" => 39,
            "Home" => 36,
            "End" => 35,
            "PageUp" => 33,
            "PageDown" => 34,
            "F1" => 112,
            "F2" => 113,
            "F3" => 114,
            "F4" => 115,
            "F5" => 116,
            "F12" => 123,
            "a" | "A" => 65,
            "c" | "C" => 67,
            "v" | "V" => 86,
            "x" | "X" => 88,
            "z" | "Z" => 90,
            _ => base_key.chars().next().map(|c| c as i32).unwrap_or(0),
        };

        self.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": base_key,
                "code": base_key,
                "windowsVirtualKeyCode": key_code,
                "modifiers": modifiers
            }),
        )
        .await?;

        // For single printable characters with no Ctrl/Alt/Meta modifier,
        // send a "char" event — required for rich text editors (CodeMirror, Monaco, contenteditable).
        // Ctrl+A, Alt+F etc. do NOT produce a char event.
        let is_printable = base_key.chars().count() == 1 && modifiers & (2 | 1 | 4) == 0;
        if is_printable {
            self.send_command(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "char",
                    "key": base_key,
                    "text": base_key,
                    "modifiers": modifiers
                }),
            )
            .await?;
        }

        self.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": base_key,
                "code": base_key,
                "windowsVirtualKeyCode": key_code,
                "modifiers": modifiers
            }),
        )
        .await?;

        tracing::debug!("Pressed key: {}", key);
        Ok(())
    }

    // ── File upload ─────────────────────────────────────────────────

    /// Upload a file to an `<input type="file">` element.
    pub async fn upload_file(&self, selector: &str, file_paths: Vec<String>) -> Result<(), String> {
        let selector_json = serde_json::to_string(selector).unwrap_or_default();
        // Step 1: resolve element via deep shadow DOM JS query
        let js = format!(r#"(function() {{
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
            return deepQuery(document, {selector_json});
        }})()"#);

        let result = self.send_command("Runtime.evaluate", json!({
            "expression": js,
            "returnByValue": false,
        })).await?;

        let object_id = result["result"]["result"]["objectId"]
            .as_str()
            .ok_or_else(|| format!("upload_file: element not found for selector '{}'", selector))?
            .to_string();

        // Step 2: get backendNodeId from objectId
        let describe_result = self.send_command("DOM.describeNode", json!({
            "objectId": object_id,
        })).await?;

        let backend_node_id = describe_result["result"]["node"]["backendNodeId"]
            .as_i64()
            .ok_or("upload_file: could not get backendNodeId")?;

        // Step 3: set files via backendNodeId
        self.send_command("DOM.setFileInputFiles", json!({
            "backendNodeId": backend_node_id,
            "files": file_paths,
        })).await?;

        tracing::debug!("Uploaded files to: {}", selector);
        Ok(())
    }

    // ── Scroll / Wait ───────────────────────────────────────────────

    /// Scroll an element into view (centers it in the viewport).
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

    /// Select an option in a <select> element by value or visible text.
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

    /// Scroll the page in the given direction.
    /// Uses real mouse wheel events at the viewport center.
    pub async fn scroll(&self, direction: &str, amount: u32) -> Result<(), String> {
        let amount = amount as f64;
        let (delta_x, delta_y) = match direction {
            "up" => (0.0, -amount),
            "down" => (0.0, amount),
            "left" => (-amount, 0.0),
            "right" => (amount, 0.0),
            _ => (0.0, amount),
        };
        // Dispatch at viewport center
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

    /// Scroll within a specific element using real mouse wheel events.
    /// `selector` can be "window" or empty for page-level scroll.
    /// `delta_x`: horizontal scroll (positive = right), `delta_y`: vertical scroll (positive = down).
    pub async fn scroll_element(&self, selector: &str, delta_x: f64, delta_y: f64) -> Result<(), String> {
        let (cx, cy) = if selector.is_empty() || selector == "window" {
            // Scroll at viewport center
            (640.0f64, 400.0f64)
        } else {
            self.get_element_center(selector).await?
        };

        self.send_command("Input.dispatchMouseEvent", json!({
            "type": "mouseWheel",
            "x": cx,
            "y": cy,
            "deltaX": delta_x,
            "deltaY": delta_y,
            "modifiers": 0,
        })).await?;

        tracing::debug!("Scrolled element '{}' by ({}, {})", selector, delta_x, delta_y);
        Ok(())
    }

    /// Wait for an element to appear in the DOM.
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

    /// Wait for duration
    pub async fn wait(&self, duration_ms: u64) -> Result<(), String> {
        tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
        Ok(())
    }

    /// Wait for text to appear anywhere in `document.body.innerText`.
    pub async fn wait_for_text(&self, text: &str, timeout_ms: u64) -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            if std::time::Instant::now() > deadline {
                return Err(format!("wait_for_text: '{}' not found within {}ms", text, timeout_ms));
            }
            let js = r#"(function(){ try { return document.body ? document.body.innerText : ''; } catch(e) { return ''; } })()"#;
            if let Ok(val) = self.evaluate_js(js).await {
                if val.as_str().map(|s| s.contains(text)).unwrap_or(false) {
                    return Ok(());
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    /// Wait until the page URL contains `pattern`.
    /// Returns the current URL when matched.
    /// Essential for SPA navigation (React Router, Vue Router).
    pub async fn wait_for_url(&self, pattern: &str, timeout_ms: u64) -> Result<String, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            if std::time::Instant::now() > deadline {
                return Err(format!("wait_for_url: URL matching '{}' not found within {}ms", pattern, timeout_ms));
            }
            if let Ok(val) = self.evaluate_js("window.location.href").await {
                if let Some(href) = val.as_str() {
                    if href.contains(pattern) {
                        return Ok(href.to_string());
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    // ── Screenshot ──────────────────────────────────────────────────

    /// Take a screenshot.
    ///
    /// - `full_page`: capture the entire scrollable page (not just the visible viewport).
    /// - `format`: "png" (default), "jpeg", or "webp".
    /// - `quality`: compression quality for jpeg/webp (0–100, ignored for png).
    ///
    /// Returns a base64-encoded image.
    pub async fn screenshot(
        &self,
        full_page: bool,
        format: &str,
        quality: Option<u32>,
    ) -> Result<String, String> {
        let mut params = serde_json::json!({ "format": format });

        if let Some(q) = quality {
            params["quality"] = serde_json::Value::Number(serde_json::Number::from(q));
        }

        if full_page {
            // Get the full content size, then capture beyond the viewport
            let metrics = self
                .send_command("Page.getLayoutMetrics", json!({}))
                .await?;
            let content_width = metrics
                .get("result")
                .and_then(|r| r.get("cssContentSize"))
                .and_then(|s| s.get("width"))
                .and_then(|v| v.as_f64())
                .unwrap_or(1280.0);
            let content_height = metrics
                .get("result")
                .and_then(|r| r.get("cssContentSize"))
                .and_then(|s| s.get("height"))
                .and_then(|v| v.as_f64())
                .unwrap_or(768.0);
            params["captureBeyondViewport"] = serde_json::Value::Bool(true);
            params["clip"] = serde_json::json!({
                "x": 0,
                "y": 0,
                "width": content_width,
                "height": content_height,
                "scale": 1
            });
        }

        let result = self.send_command("Page.captureScreenshot", params).await?;

        if let Some(data) = result
            .get("result")
            .and_then(|r| r.get("data"))
            .and_then(|d| d.as_str())
        {
            tracing::debug!("Screenshot taken (full_page={}, format={})", full_page, format);
            Ok(data.to_string())
        } else {
            Err("Failed to capture screenshot".to_string())
        }
    }

    /// Capture a screenshot of a specific element identified by a CSS selector.
    /// Uses deep shadow DOM traversal to find the element.
    /// Returns base64-encoded image.
    pub async fn screenshot_element(&self, selector: &str, format: &str, quality: Option<u32>) -> Result<String, String> {
        // 1. Get element bounding rect including scroll offset (page-absolute)
        let selector_json = serde_json::to_string(selector).unwrap_or_default();
        let js = format!(r#"(function() {{
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
            if (!el) return null;
            const rect = el.getBoundingClientRect();
            return {{
                x: rect.left + window.scrollX,
                y: rect.top + window.scrollY,
                width: rect.width,
                height: rect.height
            }};
        }})()"#);

        let result = self.evaluate_js(&js).await?;
        if result.is_null() {
            return Err(format!("screenshot_element: element not found for '{}'", selector));
        }

        let x = result["x"].as_f64().ok_or("screenshot_element: no x")?;
        let y = result["y"].as_f64().ok_or("screenshot_element: no y")?;
        let width = result["width"].as_f64().ok_or("screenshot_element: no width")?;
        let height = result["height"].as_f64().ok_or("screenshot_element: no height")?;

        if width < 1.0 || height < 1.0 {
            return Err(format!("screenshot_element: element '{}' has zero size", selector));
        }

        // 2. Capture with clip
        let fmt = match format { "jpeg" => "jpeg", "webp" => "webp", _ => "png" };
        let mut params = json!({
            "format": fmt,
            "clip": {
                "x": x,
                "y": y,
                "width": width,
                "height": height,
                "scale": 1.0
            }
        });
        if fmt == "jpeg" || fmt == "webp" {
            params["quality"] = json!(quality.unwrap_or(85));
        }

        let response = self.send_command("Page.captureScreenshot", params).await?;
        response["result"]["data"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or("screenshot_element: no data in response".to_string())
    }

    // ── Coordinate-based & drag interactions ────────────────────────

    /// Click at specific viewport coordinates. Use for canvas, image maps, or vision-based automation.
    pub async fn click_at(&self, x: f64, y: f64) -> Result<(), String> {
        self.dispatch_mouse_click(x, y).await?;
        tracing::debug!("Clicked at ({:.1}, {:.1})", x, y);
        Ok(())
    }

    /// Drag from one element to another using real mouse events.
    /// Sends intermediate moves to trigger dragover/dragenter handlers.
    pub async fn drag(&self, from_selector: &str, to_selector: &str) -> Result<(), String> {
        let (from_x, from_y) = self.get_element_center(from_selector).await?;
        let (to_x, to_y) = self.get_element_center(to_selector).await?;

        // Move to source
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseMoved", "x": from_x, "y": from_y, "button": "none", "clickCount": 0}),
        ).await?;

        // Press (begin drag)
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": from_x, "y": from_y, "button": "left", "clickCount": 1}),
        ).await?;

        // Intermediate moves (triggers dragover/dragenter)
        const STEPS: u32 = 10;
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let mx = from_x + (to_x - from_x) * t;
            let my = from_y + (to_y - from_y) * t;
            self.send_command(
                "Input.dispatchMouseEvent",
                json!({"type": "mouseMoved", "x": mx, "y": my, "button": "left", "buttons": 1, "clickCount": 0}),
            ).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
        }

        // Release at destination
        self.send_command(
            "Input.dispatchMouseEvent",
            json!({"type": "mouseReleased", "x": to_x, "y": to_y, "button": "left", "clickCount": 1}),
        ).await?;

        tracing::debug!("Dragged '{}' → '{}'", from_selector, to_selector);
        Ok(())
    }

    /// Generate a PDF of the current page.
    /// Returns base64-encoded PDF data.
    /// `landscape`: print in landscape orientation (default false)
    /// `print_background`: include CSS backgrounds (default true)
    /// `scale`: scale factor 0.1-2.0 (default 1.0)
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

    // ── Dialog handling ─────────────────────────────────────────────

    /// Handle a JavaScript dialog (alert / confirm / prompt).
    /// `action`: "accept" or "dismiss".
    /// `prompt_text`: text to type into a prompt dialog (ignored for alert/confirm).
    pub async fn handle_dialog(
        &self,
        action: &str,
        prompt_text: Option<&str>,
    ) -> Result<(), String> {
        let accept = action != "dismiss";
        let mut params = json!({ "accept": accept });
        if let Some(text) = prompt_text {
            params["promptText"] = serde_json::Value::String(text.to_string());
        }
        self.send_command("Page.handleJavaScriptDialog", params)
            .await?;
        tracing::debug!("Handled dialog: {}", action);
        Ok(())
    }

    // ── Network log ─────────────────────────────────────────────────

    /// Return recent network requests captured since the last connect.
    /// Includes requests and responses (max 200 entries combined).
    pub async fn get_network_log(&self) -> Vec<serde_json::Value> {
        self.network_log.lock().await.iter().cloned().collect()
    }

    /// Clear the network log.
    pub async fn clear_network_log(&self) {
        self.network_log.lock().await.clear();
    }

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
        tracing::debug!("Added mock rule: {} -> {}", url_pattern, status);
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
        self.send_command(
            "Fetch.enable",
            json!({"handleAuthRequests": false}),
        )
        .await?;
        Ok(())
    }

    // ── DOM context ─────────────────────────────────────────────────

    /// Get DOM context for LLM
    pub async fn get_dom_context(&self) -> Result<DOMContext, String> {
        let url = self.get_url().await?;

        let js = r##"
            (function() {
                const elements = [];
                const forms = [];
                const links = [];

                function getSelector(el) {
                    if (el.id) return "#" + el.id;
                    if (el.className && typeof el.className === "string") {
                        const classes = el.className.trim().split(/\s+/).filter(c => c);
                        if (classes.length > 0) {
                            return el.tagName.toLowerCase() + "." + classes.slice(0, 2).join(".");
                        }
                    }
                    return el.tagName.toLowerCase();
                }

                const interactives = document.querySelectorAll(
                    "a, button, input, select, textarea, [onclick], [role=\"button\"], [tabindex]"
                );

                interactives.forEach(el => {
                    const rect = el.getBoundingClientRect();
                    const visible = rect.width > 0 && rect.height > 0;

                    const element = {
                        tag: el.tagName.toLowerCase(),
                        id: el.id || null,
                        classes: (el.className && typeof el.className === "string")
                            ? el.className.trim().split(/\s+/).filter(c => c)
                            : [],
                        selector: getSelector(el),
                        text: (el.innerText || el.value || el.placeholder || "").substring(0, 100),
                        input_type: el.type || null,
                        placeholder: el.placeholder || null,
                        aria_label: el.getAttribute("aria-label") || null,
                        visible: visible,
                        clickable: visible && !el.disabled
                    };

                    elements.push(element);

                    if (el.tagName === "A") {
                        links.push(element);
                    } else if (["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(el.tagName)) {
                        forms.push(element);
                    }
                });

                return { elements, forms, links, title: document.title };
            })();
        "##;

        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": js,
                    "returnByValue": true
                }),
            )
            .await?;

        let dom_data = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let title = dom_data
            .get("title")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let elements: Vec<DOMElement> =
            serde_json::from_value(dom_data.get("elements").cloned().unwrap_or_default())
                .unwrap_or_default();

        let forms: Vec<DOMElement> =
            serde_json::from_value(dom_data.get("forms").cloned().unwrap_or_default())
                .unwrap_or_default();

        let links: Vec<DOMElement> =
            serde_json::from_value(dom_data.get("links").cloned().unwrap_or_default())
                .unwrap_or_default();

        Ok(DOMContext {
            url,
            title,
            elements,
            forms,
            links,
        })
    }

    /// Extract data using CSS selectors
    pub async fn extract_data(
        &self,
        selectors: &std::collections::HashMap<String, String>,
    ) -> Result<serde_json::Value, String> {
        let mut result = serde_json::Map::new();

        for (name, selector) in selectors {
            let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
            let js = format!(
                "(function() {{ const el = document.querySelector('{}'); return el ? (el.innerText || el.value || el.textContent || '') : ''; }})()",
                escaped
            );

            let res = self
                .send_command(
                    "Runtime.evaluate",
                    json!({
                        "expression": js,
                        "returnByValue": true
                    }),
                )
                .await?;

            let value = res
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            result.insert(name.clone(), serde_json::Value::String(value.to_string()));
        }

        Ok(serde_json::Value::Object(result))
    }

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

    /// Evaluate arbitrary JavaScript and return the stringified result.
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

        let result = self
            .send_command("Runtime.evaluate", params)
            .await?;

        let value = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        if let Some(exception) = result
            .get("result")
            .and_then(|r| r.get("exceptionDetails"))
        {
            let msg = exception
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("JS evaluation error");
            return Err(msg.to_string());
        }

        Ok(value)
    }

    // ── Accessibility Tree ──────────────────────────────────────────

    /// Returns true if an AX node should be included in the filtered tree.
    fn is_interesting_ax_node(role: &str, name: &str, ignored: bool) -> bool {
        if ignored {
            return false;
        }

        // Always include interactive roles (even with empty name)
        const INTERACTIVE: &[&str] = &[
            "button",
            "link",
            "textbox",
            "combobox",
            "listbox",
            "option",
            "checkbox",
            "radio",
            "switch",
            "slider",
            "spinbutton",
            "searchbox",
            "menuitem",
            "menuitemcheckbox",
            "menuitemradio",
            "tab",
            "treeitem",
            "columnheader",
            "rowheader",
            "gridcell",
            "cell",
            "row",
            "select",
            "input",
            "textarea",
        ];

        if INTERACTIVE.contains(&role) {
            return true;
        }

        // Include structural/landmark roles only when they have an accessible name
        const STRUCTURAL: &[&str] = &[
            "heading",
            "img",
            "figure",
            "main",
            "navigation",
            "region",
            "complementary",
            "banner",
            "contentinfo",
            "form",
            "alert",
            "dialog",
            "status",
            "tooltip",
            "table",
            "list",
            "listitem",
        ];

        if STRUCTURAL.contains(&role) && !name.is_empty() {
            return true;
        }

        // Skip noisy / structural-only roles with no name
        const SKIP: &[&str] = &[
            "none",
            "generic",
            "presentation",
            "InlineTextBox",
            "LineBreak",
            "StaticText",
            "SvgRoot",
            "Canvas",
            "Iframe",
            "RootWebArea",
            "WebArea",
        ];

        if SKIP.contains(&role) {
            return false;
        }

        // For anything else, include only if it has a name
        !name.is_empty()
    }

    /// Get the filtered Accessibility Tree for the current page.
    /// Assigns stable `ref_id`s ("e1", "e2", …) to each included node.
    /// The ref cache is updated so that `click_ref` / `type_ref` / `focus_ref` work.
    pub async fn get_ax_tree(&self) -> Result<Vec<AXNode>, String> {
        // Enable the Accessibility domain (idempotent)
        self.send_command("Accessibility.enable", json!({})).await?;

        let result = self
            .send_command("Accessibility.getFullAXTree", json!({}))
            .await?;

        let nodes_val = result
            .get("result")
            .and_then(|r| r.get("nodes"))
            .and_then(|n| n.as_array())
            .ok_or("Failed to get AX tree nodes")?;

        let mut ax_nodes: Vec<AXNode> = Vec::new();
        let mut ref_counter: u32 = 1;
        let mut new_cache: HashMap<String, i64> = HashMap::new();

        for node_val in nodes_val {
            let ignored = node_val
                .get("ignored")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let role = node_val
                .get("role")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let name = node_val
                .get("name")
                .and_then(|n| n.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !Self::is_interesting_ax_node(role, name, ignored) {
                continue;
            }

            let backend_node_id = node_val
                .get("backendDOMNodeId")
                .and_then(|v| v.as_i64());

            let description = node_val
                .get("description")
                .and_then(|d| d.get("value"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            // Extract properties array
            let props = node_val.get("properties").and_then(|p| p.as_array());
            let mut focused: Option<bool> = None;
            let mut disabled: Option<bool> = None;
            let mut value: Option<String> = None;
            let mut checked: Option<String> = None;

            if let Some(props) = props {
                for prop in props {
                    let prop_name =
                        prop.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let prop_val = prop.get("value").and_then(|v| v.get("value"));
                    match prop_name {
                        "focused" => focused = prop_val.and_then(|v| v.as_bool()),
                        "disabled" => disabled = prop_val.and_then(|v| v.as_bool()),
                        "value" => {
                            value = prop_val
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                        }
                        "checked" => {
                            checked = prop_val.and_then(|v| v.as_str()).map(|s| s.to_string())
                        }
                        _ => {}
                    }
                }
            }

            // Also check the top-level value field
            if value.is_none() {
                value = node_val
                    .get("value")
                    .and_then(|v| v.get("value"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
            }

            let ref_id = format!("e{}", ref_counter);
            ref_counter += 1;

            if let Some(bid) = backend_node_id {
                new_cache.insert(ref_id.clone(), bid);
            }

            // Only include focused/disabled if they're true (to reduce noise)
            ax_nodes.push(AXNode {
                ref_id,
                role: role.to_string(),
                name: name.to_string(),
                description,
                focused: if focused == Some(true) { Some(true) } else { None },
                disabled: if disabled == Some(true) { Some(true) } else { None },
                value,
                checked,
                backend_node_id,
            });
        }

        // Update the ref cache
        *self.ax_ref_cache.lock().await = new_cache;

        Ok(ax_nodes)
    }

    /// Get combined page state: URL, title, and filtered AX tree.
    /// One-shot call for AI agents to understand the current page.
    pub async fn get_page_state(&self) -> Result<PageState, String> {
        let ax_tree = self.get_ax_tree().await?;
        let url = self.get_url().await?;
        let title = self.get_title().await.unwrap_or(None);
        let element_count = ax_tree.len();
        Ok(PageState {
            url,
            title,
            ax_tree,
            element_count,
        })
    }

    // ── Ref-based interactions (use after get_ax_tree) ───────────────

    /// Resolve an AX ref to a Runtime RemoteObject objectId via DOM.resolveNode(backendNodeId).
    /// This bypasses the deprecated DOM.pushNodesByBackendIdsToFrontend.
    async fn resolve_ref_to_object_id(&self, ref_id: &str) -> Result<String, String> {
        let backend_node_id = {
            let cache = self.ax_ref_cache.lock().await;
            cache
                .get(ref_id)
                .copied()
                .ok_or_else(|| format!("Unknown ref '{}'. Call get_ax_tree first.", ref_id))?
        };

        let resolve_result = self
            .send_command(
                "DOM.resolveNode",
                json!({"backendNodeId": backend_node_id}),
            )
            .await?;

        if let Some(error) = resolve_result.get("error") {
            return Err(format!(
                "CDP error resolving ref '{}': {}",
                ref_id,
                serde_json::to_string(error).unwrap_or_else(|_| "unknown error".to_string())
            ));
        }

        let object_id = resolve_result
            .get("result")
            .and_then(|r| r.get("object"))
            .and_then(|o| o.get("objectId"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("No objectId for ref '{}' in DOM.resolveNode response", ref_id))?
            .to_string();

        Ok(object_id)
    }

    /// Get the viewport center of a node via its Runtime objectId.
    async fn get_node_center_from_object(&self, object_id: &str) -> Result<(f64, f64), String> {
        let call_result = self
            .send_command(
                "Runtime.callFunctionOn",
                json!({
                    "objectId": object_id,
                    "functionDeclaration": "function() { const rect = this.getBoundingClientRect(); if (rect.width === 0 && rect.height === 0) return null; return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }; }",
                    "returnByValue": true
                }),
            )
            .await?;

        let value = call_result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .ok_or("Failed to get node center from object")?;

        if value.is_null() {
            return Err("Element has no layout dimensions (hidden or detached)".to_string());
        }

        let x = value.get("x").and_then(|v| v.as_f64()).ok_or("Failed to get node center x")?;
        let y = value.get("y").and_then(|v| v.as_f64()).ok_or("Failed to get node center y")?;
        Ok((x, y))
    }

    /// Click an element by its AX tree ref (e.g. "e1"). Call `get_ax_tree` first.
    pub async fn click_ref(&self, ref_id: &str) -> Result<(), String> {
        let object_id = self.resolve_ref_to_object_id(ref_id).await?;
        let (cx, cy) = self.get_node_center_from_object(&object_id).await?;
        self.dispatch_mouse_click(cx, cy).await?;
        tracing::debug!("click_ref {}", ref_id);
        Ok(())
    }

    /// Type text into an element by its AX tree ref. Call `get_ax_tree` first.
    pub async fn type_ref(&self, ref_id: &str, text: &str) -> Result<(), String> {
        let backend_node_id = {
            let cache = self.ax_ref_cache.lock().await;
            cache
                .get(ref_id)
                .copied()
                .ok_or_else(|| format!("Unknown ref '{}'. Call get_ax_tree first.", ref_id))?
        };

        // Focus via CDP DOM.focus (accepts backendNodeId directly)
        self.send_command("DOM.focus", json!({"backendNodeId": backend_node_id}))
            .await?;

        let text_json = serde_json::to_string(text).unwrap_or_default();
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": format!(
                        "(function() {{ \
                            const el = document.activeElement; \
                            if(!el) return false; \
                            const niv = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value') \
                                || Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value'); \
                            if(niv && niv.set) {{ niv.set.call(el, {text_json}); }} \
                            else {{ el.value = {text_json}; }} \
                            el.dispatchEvent(new Event('input', {{bubbles: true}})); \
                            el.dispatchEvent(new Event('change', {{bubbles: true}})); \
                            return true; \
                        }})()"
                    ),
                    "returnByValue": true
                }),
            )
            .await?;

        let ok = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if ok {
            tracing::debug!("type_ref {} → {} chars", ref_id, text.len());
            Ok(())
        } else {
            Err(format!("Failed to type into ref '{}'", ref_id))
        }
    }

    /// Focus an element by its AX tree ref. Call `get_ax_tree` first.
    pub async fn focus_ref(&self, ref_id: &str) -> Result<(), String> {
        let backend_node_id = {
            let cache = self.ax_ref_cache.lock().await;
            cache
                .get(ref_id)
                .copied()
                .ok_or_else(|| format!("Unknown ref '{}'. Call get_ax_tree first.", ref_id))?
        };
        self.send_command("DOM.focus", json!({"backendNodeId": backend_node_id}))
            .await?;
        tracing::debug!("focus_ref {}", ref_id);
        Ok(())
    }

    // ── Accessors ───────────────────────────────────────────────────

    /// Get the CDP port this client is connected to.
    pub fn get_cdp_port(&self) -> u16 {
        self.cdp_port
    }

    /// Check if the WebSocket connection is active.
    pub fn is_connected(&self) -> bool {
        self.ws_tx.is_some()
    }

    // ── Advanced: Tabs ─────────────────────────────────────────────

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

    // ── Advanced: Cookies ──────────────────────────────────────────

    /// Get cookies for the current page.
    pub async fn get_cookies(&self) -> Result<Vec<CookieInfo>, String> {
        let result = self
            .send_command("Network.getCookies", json!({}))
            .await?;
        let cookies_val = result
            .get("result")
            .and_then(|r| r.get("cookies"))
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![]));
        let cookies: Vec<CookieInfo> =
            serde_json::from_value(cookies_val).unwrap_or_default();
        Ok(cookies)
    }

    /// Set a cookie.
    pub async fn set_cookie(
        &self,
        name: &str,
        value: &str,
        domain: &str,
        path: &str,
    ) -> Result<(), String> {
        let result = self
            .send_command(
                "Network.setCookie",
                json!({
                    "name": name,
                    "value": value,
                    "domain": domain,
                    "path": path,
                }),
            )
            .await?;
        let success = result
            .get("result")
            .and_then(|r| r.get("success"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if success {
            Ok(())
        } else {
            Err("Failed to set cookie".to_string())
        }
    }

    /// Set a cookie with all fields (secure, httpOnly, expires).
    pub async fn set_cookie_full(&self, cookie: &CookieInfo) -> Result<(), String> {
        let mut params = json!({
            "name": cookie.name,
            "value": cookie.value,
            "domain": cookie.domain,
            "path": cookie.path,
            "secure": cookie.secure,
            "httpOnly": cookie.http_only,
        });
        if cookie.expires > 0.0 {
            params["expires"] = json!(cookie.expires);
        }
        let result = self.send_command("Network.setCookie", params).await?;
        let success = result
            .get("result")
            .and_then(|r| r.get("success"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if success {
            Ok(())
        } else {
            Err("Failed to set cookie (full)".to_string())
        }
    }

    /// Delete all cookies.
    pub async fn delete_cookies(&self) -> Result<(), String> {
        self.send_command("Network.clearBrowserCookies", json!({}))
            .await?;
        Ok(())
    }

    /// Delete a specific cookie by name, domain, and/or path.
    /// If only name is provided, deletes all cookies with that name across all domains/paths.
    pub async fn delete_cookie_named(
        &self,
        name: String,
        domain: Option<String>,
        path: Option<String>,
    ) -> Result<(), String> {
        let mut params = json!({ "name": name });
        if let Some(d) = domain {
            params["domain"] = json!(d);
        }
        if let Some(p) = path {
            params["path"] = json!(p);
        }
        self.send_command("Network.deleteCookies", params).await?;
        Ok(())
    }

    // ── Advanced: Console Logs ─────────────────────────────────────

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

    // ── Emulation ───────────────────────────────────────────────────

    /// Set viewport size and mobile emulation.
    pub async fn set_viewport(&self, width: u32, height: u32, device_scale_factor: f64, mobile: bool) -> Result<(), String> {
        self.send_command("Emulation.setDeviceMetricsOverride", json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": device_scale_factor,
            "mobile": mobile,
        })).await?;
        Ok(())
    }

    /// Override the User-Agent string.
    pub async fn set_user_agent(&self, user_agent: &str) -> Result<(), String> {
        self.send_command("Network.setUserAgentOverride", json!({
            "userAgent": user_agent,
        })).await?;
        Ok(())
    }

    /// Grant browser permissions (e.g., geolocation) for a specific origin.
    pub async fn grant_permissions(&self, permissions: &[&str], origin: &str) -> Result<(), String> {
        self.send_browser_command("Browser.grantPermissions", json!({
            "permissions": permissions,
            "origin": origin,
        })).await?;
        Ok(())
    }

    /// Override geolocation coordinates.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64, accuracy: f64) -> Result<(), String> {
        self.send_command("Emulation.setGeolocationOverride", json!({
            "latitude": latitude,
            "longitude": longitude,
            "accuracy": accuracy,
        })).await?;
        Ok(())
    }

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

    // ── Storage (localStorage / sessionStorage) ─────────────────────────────

    /// Get all key-value pairs from localStorage or sessionStorage.
    /// `storage_type`: "local" (default) or "session".
    pub async fn get_storage(&self, storage_type: &str) -> Result<serde_json::Value, String> {
        let store = if storage_type == "session" { "sessionStorage" } else { "localStorage" };
        let js = format!(r#"(function() {{
            const store = window.{store};
            const result = {{}};
            for (let i = 0; i < store.length; i++) {{
                const key = store.key(i);
                result[key] = store.getItem(key);
            }}
            return result;
        }})()"#);
        self.evaluate_js(&js).await
    }

    /// Set a key-value pair in localStorage or sessionStorage.
    pub async fn set_storage_item(&self, storage_type: &str, key: &str, value: &str) -> Result<(), String> {
        let store = if storage_type == "session" { "sessionStorage" } else { "localStorage" };
        let key_json = serde_json::to_string(key).unwrap_or_default();
        let val_json = serde_json::to_string(value).unwrap_or_default();
        let js = format!(r#"window.{store}.setItem({key_json}, {val_json})"#);
        self.evaluate_js(&js).await?;
        Ok(())
    }

    /// Remove a single key from localStorage or sessionStorage.
    pub async fn remove_storage_item(&self, storage_type: &str, key: &str) -> Result<(), String> {
        let store = if storage_type == "session" { "sessionStorage" } else { "localStorage" };
        let key_json = serde_json::to_string(key).unwrap_or_default();
        let js = format!(r#"window.{store}.removeItem({key_json})"#);
        self.evaluate_js(&js).await?;
        Ok(())
    }

    /// Clear all entries from localStorage or sessionStorage.
    pub async fn clear_storage(&self, storage_type: &str) -> Result<(), String> {
        let store = if storage_type == "session" { "sessionStorage" } else { "localStorage" };
        let js = format!(r#"window.{store}.clear()"#);
        self.evaluate_js(&js).await?;
        Ok(())
    }

    // ── Close ───────────────────────────────────────────────────────

    /// Close the browser
    pub async fn close(&mut self) -> Result<(), String> {
        if let Some(tx) = self.ws_tx.take() {
            let mut tx_guard = tx.lock().await;
            let _ = tx_guard.close().await;
        }

        if let Some(pid) = self.chrome_pid.take() {
            #[cfg(unix)]
            {
                let _ = Command::new("kill").arg(pid.to_string()).spawn();
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .spawn();
            }
        }

        tracing::info!("CDP client closed for profile {}", self.profile_id);
        Ok(())
    }
}

impl Drop for CDPClient {
    fn drop(&mut self) {
        if let Some(pid) = self.chrome_pid {
            #[cfg(unix)]
            {
                let _ = Command::new("kill").arg(pid.to_string()).spawn();
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .spawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn test_glob_match_no_wildcard() {
        assert!(glob_match("/form", "http://example.com/form?x=1"));
        assert!(!glob_match("/form", "http://example.com/other"));
    }

    #[test]
    fn test_glob_match_leading_star() {
        assert!(glob_match("*/form*", "http://example.com/form?x=1"));
        assert!(glob_match("*/api/v1/*", "http://example.com/api/v1/users"));
        assert!(!glob_match("*/api/v1/*", "http://example.com/api/v2/users"));
    }

    #[test]
    fn test_glob_match_trailing_star() {
        assert!(glob_match("http://example.com/*", "http://example.com/page"));
        assert!(!glob_match("http://example.com/*", "http://other.com/page"));
    }

    #[test]
    fn test_glob_match_star_only() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_glob_match_multiple_wildcards() {
        assert!(glob_match("*/images/*.png", "http://cdn.com/images/photo.png"));
        assert!(!glob_match("*/images/*.png", "http://cdn.com/images/photo.jpg"));
    }
}
