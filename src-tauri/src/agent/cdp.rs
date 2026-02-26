use crate::agent::types::{AXNode, CookieInfo, DOMContext, DOMElement, PageState, TabInfo};
use crate::config::schema::BrowserProfile;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

/// CDP Client using raw WebSocket for better Chrome compatibility
#[allow(clippy::type_complexity)]
pub struct CDPClient {
    /// WebSocket sender
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
    /// Response receiver (command id → one-shot sender)
    responses: Arc<Mutex<HashMap<u32, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    /// CDP event listeners (method name → list of one-shot senders)
    events: Arc<Mutex<HashMap<String, Vec<tokio::sync::oneshot::Sender<serde_json::Value>>>>>,
    /// AX ref cache: "e1" → backend_node_id. Cleared on navigation.
    ax_ref_cache: Arc<Mutex<HashMap<String, i64>>>,
    /// Persistent network request log (max 200 entries). Filled by WS reader.
    network_log: Arc<Mutex<VecDeque<serde_json::Value>>>,
    /// Chrome process ID
    chrome_pid: Option<u32>,
    /// Profile being used
    profile_id: String,
    /// Current URL
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
            ax_ref_cache: Arc::new(Mutex::new(HashMap::new())),
            network_log: Arc::new(Mutex::new(VecDeque::new())),
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
            ax_ref_cache: Arc::new(Mutex::new(HashMap::new())),
            network_log: Arc::new(Mutex::new(VecDeque::new())),
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
    async fn connect_websocket(&mut self) -> Result<(), String> {
        let mut retries = 0u32;
        const MAX_RETRIES: u32 = 30;
        let mut last_error = String::new();

        while retries < MAX_RETRIES {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let list_url = format!("http://localhost:{}/json/list", self.cdp_port);
            match reqwest::get(&list_url).await {
                Ok(response) if response.status().is_success() => {
                    match response.json::<serde_json::Value>().await {
                        Ok(targets) => {
                            let page_target = targets
                                .as_array()
                                .and_then(|arr| {
                                    arr.iter().find(|t| {
                                        t.get("type").and_then(|v| v.as_str()) == Some("page")
                                    })
                                });

                            if let Some(target) = page_target {
                                if let Some(ws_url) =
                                    target.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
                                {
                                    return self.setup_ws_connection(ws_url).await;
                                } else {
                                    last_error =
                                        "No webSocketDebuggerUrl in page target".to_string();
                                }
                            } else {
                                last_error = "No page target found".to_string();
                            }
                        }
                        Err(e) => {
                            last_error = format!("Failed to parse targets response: {}", e);
                        }
                    }
                }
                Ok(response) => {
                    last_error = format!("HTTP error: {}", response.status());
                }
                Err(e) => {
                    last_error = format!("Connection error: {}", e);
                }
            }

            retries += 1;
            tracing::debug!("Retry {}/{}: {}", retries, MAX_RETRIES, last_error);
        }

        Err(format!(
            "Failed to connect to Chrome CDP on port {} after {} retries: {}",
            self.cdp_port, MAX_RETRIES, last_error
        ))
    }

    /// Perform the actual WebSocket connection + enable CDP domains.
    async fn setup_ws_connection(&mut self, ws_url: &str) -> Result<(), String> {
        tracing::info!("Connecting to CDP WebSocket: {}", ws_url);

        let (ws_stream, _) =
            connect_async(ws_url)
                .await
                .map_err(|e| format!("Failed to connect WebSocket: {}", e))?;

        let (tx, mut rx) = StreamExt::split(ws_stream);
        self.ws_tx = Some(Arc::new(Mutex::new(tx)));

        let responses = self.responses.clone();
        let events = self.events.clone();
        let network_log = self.network_log.clone();
        tokio::spawn(async move {
            while let Some(msg) = StreamExt::next(&mut rx).await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(id) = json.get("id").and_then(|i| i.as_u64()) {
                                // Command response
                                if let Some(sender) =
                                    responses.lock().await.remove(&(id as u32))
                                {
                                    let _ = sender.send(json);
                                }
                            } else if let Some(method_val) = json.get("method") {
                                // CDP event
                                let method = method_val
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                let params = json
                                    .get("params")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);

                                // Persistent network log capture (independent of subscribers)
                                match method.as_str() {
                                    "Network.requestWillBeSent" => {
                                        let url = params.get("request").and_then(|r| r.get("url")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let req_method = params.get("request").and_then(|r| r.get("method")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let request_id = params.get("requestId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let mut log = network_log.lock().await;
                                        if log.len() >= 200 { log.pop_front(); }
                                        log.push_back(serde_json::json!({
                                            "type": "request",
                                            "url": url,
                                            "method": req_method,
                                            "requestId": request_id
                                        }));
                                    }
                                    "Network.responseReceived" => {
                                        let url = params.get("response").and_then(|r| r.get("url")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let status = params.get("response").and_then(|r| r.get("status")).and_then(|v| v.as_i64()).unwrap_or(0);
                                        let mime = params.get("response").and_then(|r| r.get("mimeType")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let request_id = params.get("requestId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let mut log = network_log.lock().await;
                                        if log.len() >= 200 { log.pop_front(); }
                                        log.push_back(serde_json::json!({
                                            "type": "response",
                                            "url": url,
                                            "status": status,
                                            "mimeType": mime,
                                            "requestId": request_id
                                        }));
                                    }
                                    _ => {}
                                }

                                // One-shot event subscribers
                                let mut ev = events.lock().await;
                                if let Some(senders) = ev.remove(&method) {
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

        tracing::info!("CDP client connected for profile {}", self.profile_id);

        self.send_command("Page.enable", json!({})).await?;
        self.send_command("Runtime.enable", json!({})).await?;
        self.send_command("Network.enable", json!({})).await?;
        tracing::info!("CDP domains enabled");

        Ok(())
    }

    /// Send a CDP command and wait for response
    async fn send_command(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let tx = self.ws_tx.as_ref().ok_or("WebSocket not connected")?;

        let (id, rx) = {
            let mut msg_id = self.msg_id.lock().await;
            *msg_id += 1;
            let id = *msg_id - 1;

            let (tx, rx) = tokio::sync::oneshot::channel();
            self.responses.lock().await.insert(id, tx);
            (id, rx)
        };

        let command = json!({
            "id": id,
            "method": method,
            "params": params
        });

        let mut tx_guard = tx.lock().await;
        tx_guard
            .send(WsMessage::Text(command.to_string()))
            .await
            .map_err(|e| format!("Failed to send command: {}", e))?;
        drop(tx_guard);

        // Wait for response with timeout
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err("Response channel closed".to_string()),
            Err(_) => Err("Command timeout".to_string()),
        }
    }

    /// Register a one-shot listener for a CDP event method.
    /// Must be called BEFORE triggering the action that fires the event.
    async fn subscribe_event(
        &self,
        method: &str,
    ) -> Result<tokio::sync::oneshot::Receiver<serde_json::Value>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.events
            .lock()
            .await
            .entry(method.to_string())
            .or_default()
            .push(tx);
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
    /// `wait_until`: "load" | "domcontentloaded" | "none"
    pub async fn navigate_wait(
        &self,
        url: &str,
        wait_until: &str,
        timeout_ms: u64,
    ) -> Result<(), String> {
        // Subscribe to event BEFORE sending navigate to avoid race conditions
        let event_rx = match wait_until {
            "load" | "networkidle" => {
                Some(self.subscribe_event("Page.loadEventFired").await?)
            }
            "domcontentloaded" => {
                Some(self.subscribe_event("Page.domContentEventFired").await?)
            }
            _ => None,
        };

        let _ = self
            .send_command("Page.navigate", json!({"url": url}))
            .await?;
        *self.current_url.lock().await = url.to_string();

        // Clear AX ref cache on navigation
        self.ax_ref_cache.lock().await.clear();

        if let Some(rx) = event_rx {
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

    /// Get the viewport-relative center of a DOM node by its **frontend** node ID.
    /// Uses Runtime.callFunctionOn → getBoundingClientRect (viewport-relative, not document-absolute).
    async fn get_node_center(&self, node_id: i64) -> Result<(f64, f64), String> {
        // Resolve frontend node_id → Runtime RemoteObject
        let resolve_result = self
            .send_command("DOM.resolveNode", json!({"nodeId": node_id}))
            .await?;
        let object_id = resolve_result
            .get("result")
            .and_then(|r| r.get("object"))
            .and_then(|o| o.get("objectId"))
            .and_then(|v| v.as_str())
            .ok_or("Failed to resolve node to runtime object")?
            .to_string();

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
            .ok_or("Failed to get node center")?;

        if value.is_null() {
            return Err("Element has no layout dimensions (hidden or detached)".to_string());
        }

        let x = value.get("x").and_then(|v| v.as_f64()).ok_or("Failed to get node center x")?;
        let y = value.get("y").and_then(|v| v.as_f64()).ok_or("Failed to get node center y")?;
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
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let text_json = serde_json::to_string(text).unwrap_or_default();

        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": format!(
                        "(function() {{ \
                            const el = document.querySelector('{escaped_selector}'); \
                            if(!el) return false; \
                            el.focus(); \
                            const nativeInputValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value') \
                                || Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value'); \
                            if(nativeInputValueSetter && nativeInputValueSetter.set) {{ \
                                nativeInputValueSetter.set.call(el, {text_json}); \
                            }} else {{ \
                                el.value = {text_json}; \
                            }} \
                            el.dispatchEvent(new Event('input', {{bubbles: true}})); \
                            el.dispatchEvent(new Event('change', {{bubbles: true}})); \
                            return true; \
                        }})()"
                    ),
                    "returnByValue": true
                }),
            )
            .await?;

        let typed = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
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
                json!({"type": "keyDown", "key": ch_str, "text": ch_str}),
            )
            .await?;
            self.send_command(
                "Input.dispatchKeyEvent",
                json!({"type": "char", "key": ch_str, "text": ch_str}),
            )
            .await?;
            self.send_command(
                "Input.dispatchKeyEvent",
                json!({"type": "keyUp", "key": ch_str, "text": ch_str}),
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
    pub async fn upload_file(&self, selector: &str, file_path: &str) -> Result<(), String> {
        let doc_result = self
            .send_command("DOM.getDocument", json!({"depth": 0, "pierce": true}))
            .await?;
        let doc_node_id = doc_result
            .get("result")
            .and_then(|r| r.get("root"))
            .and_then(|r| r.get("nodeId"))
            .and_then(|v| v.as_i64())
            .ok_or("Failed to get document nodeId")?;

        let query_result = self
            .send_command(
                "DOM.querySelector",
                json!({"nodeId": doc_node_id, "selector": selector}),
            )
            .await?;
        let node_id = query_result
            .get("result")
            .and_then(|r| r.get("nodeId"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| format!("File input not found: {}", selector))?;

        if node_id == 0 {
            return Err(format!("File input not found: {}", selector));
        }

        self.send_command(
            "DOM.setFileInputFiles",
            json!({"nodeId": node_id, "files": [file_path]}),
        )
        .await?;

        tracing::debug!("Uploaded file '{}' to: {}", file_path, selector);
        Ok(())
    }

    // ── Scroll / Wait ───────────────────────────────────────────────

    /// Scroll an element into view (centers it in the viewport).
    pub async fn scroll_into_view(&self, selector: &str) -> Result<(), String> {
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function() {{ const el = document.querySelector('{}'); if(!el) return false; el.scrollIntoView({{behavior:'instant',block:'center'}}); return true; }})()",
            escaped
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
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let value_json = serde_json::to_string(value).unwrap_or_default();
        let js = format!(
            "(function() {{ \
                const el = document.querySelector('{escaped}'); \
                if(!el) return 'not_found'; \
                const opt = Array.from(el.options).find(o => o.value === {value_json} || o.text.trim() === {value_json}); \
                if(!opt) return 'no_option'; \
                el.value = opt.value; \
                el.dispatchEvent(new Event('input', {{bubbles:true}})); \
                el.dispatchEvent(new Event('change', {{bubbles:true}})); \
                return 'ok'; \
            }})()"
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

    /// Scroll the page
    pub async fn scroll(&self, direction: &str, amount: u32) -> Result<(), String> {
        let scroll_amount = amount as i32;
        let (x, y) = match direction {
            "up" => (0, -scroll_amount),
            "down" => (0, scroll_amount),
            "left" => (-scroll_amount, 0),
            "right" => (scroll_amount, 0),
            _ => (0, scroll_amount),
        };

        self.send_command(
            "Runtime.evaluate",
            json!({
                "expression": format!("window.scrollBy({}, {})", x, y)
            }),
        )
        .await?;

        tracing::debug!("Scrolled: {} by {}", direction, amount);
        Ok(())
    }

    /// Wait for an element to appear in the DOM.
    pub async fn wait_for_element(&self, selector: &str, timeout_ms: u64) -> Result<(), String> {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");

        loop {
            let result = self
                .send_command(
                    "Runtime.evaluate",
                    json!({
                        "expression": format!("!!document.querySelector('{}')", escaped),
                        "returnByValue": true
                    }),
                )
                .await?;

            let found = result
                .get("result")
                .and_then(|r| r.get("result"))
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_bool())
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

    // ── Screenshot ──────────────────────────────────────────────────

    /// Take a screenshot.
    /// - `full_page`: capture the entire scrollable page (not just the visible viewport).
    /// - `format`: "png" (default), "jpeg", or "webp".
    /// - `quality`: compression quality for jpeg/webp (0–100, ignored for png).
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

    /// Evaluate arbitrary JavaScript and return the stringified result.
    pub async fn evaluate_js(&self, expression: &str) -> Result<serde_json::Value, String> {
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
            )
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

    /// Resolve an AX ref to a frontend node ID via DOM.pushNodesByBackendIdsToFrontend.
    async fn resolve_ref_to_node_id(&self, ref_id: &str) -> Result<i64, String> {
        let backend_node_id = {
            let cache = self.ax_ref_cache.lock().await;
            cache
                .get(ref_id)
                .copied()
                .ok_or_else(|| format!("Unknown ref '{}'. Call get_ax_tree first.", ref_id))?
        };

        let push_result = self
            .send_command(
                "DOM.pushNodesByBackendIdsToFrontend",
                json!({"backendNodeIds": [backend_node_id]}),
            )
            .await?;

        let node_ids = push_result
            .get("result")
            .and_then(|r| r.get("nodeIds"))
            .and_then(|v| v.as_array())
            .ok_or("Failed to resolve backend node")?;

        let node_id = node_ids
            .first()
            .and_then(|v| v.as_i64())
            .ok_or("Failed to get frontend node ID")?;

        if node_id == 0 {
            return Err(format!(
                "Node for ref '{}' no longer exists (page may have changed).",
                ref_id
            ));
        }

        Ok(node_id)
    }

    /// Click an element by its AX tree ref (e.g. "e1"). Call `get_ax_tree` first.
    pub async fn click_ref(&self, ref_id: &str) -> Result<(), String> {
        let node_id = self.resolve_ref_to_node_id(ref_id).await?;
        let (cx, cy) = self.get_node_center(node_id).await?;
        self.dispatch_mouse_click(cx, cy).await?;
        tracing::debug!("click_ref {}", ref_id);
        Ok(())
    }

    /// Type text into an element by its AX tree ref. Call `get_ax_tree` first.
    pub async fn type_ref(&self, ref_id: &str, text: &str) -> Result<(), String> {
        let node_id = self.resolve_ref_to_node_id(ref_id).await?;

        // Focus via CDP DOM.focus
        self.send_command("DOM.focus", json!({"nodeId": node_id}))
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
        let node_id = self.resolve_ref_to_node_id(ref_id).await?;
        self.send_command("DOM.focus", json!({"nodeId": node_id}))
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

    /// List all browser tabs via /json/list HTTP endpoint.
    pub async fn list_tabs(&self) -> Result<Vec<TabInfo>, String> {
        let url = format!("http://localhost:{}/json/list", self.cdp_port);
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("Failed to list tabs: {}", e))?;
        let targets: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse tab list: {}", e))?;

        let tabs = targets
            .iter()
            .filter(|t| {
                t.get("type").and_then(|v| v.as_str()) == Some("page")
            })
            .map(|t| TabInfo {
                id: t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                url: t.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                title: t.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                target_type: "page".to_string(),
            })
            .collect();
        Ok(tabs)
    }

    /// Open a new tab with a URL via /json/new endpoint.
    pub async fn new_tab(&self, url: &str) -> Result<TabInfo, String> {
        let endpoint = format!("http://localhost:{}/json/new?{}", self.cdp_port, url);
        let resp = reqwest::get(&endpoint)
            .await
            .map_err(|e| format!("Failed to create new tab: {}", e))?;
        let target: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse new tab response: {}", e))?;
        Ok(TabInfo {
            id: target.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: target.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: target.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            target_type: "page".to_string(),
        })
    }

    /// Close a tab by its target ID via /json/close endpoint.
    pub async fn close_tab(&self, target_id: &str) -> Result<(), String> {
        let endpoint = format!(
            "http://localhost:{}/json/close/{}",
            self.cdp_port, target_id
        );
        reqwest::get(&endpoint)
            .await
            .map_err(|e| format!("Failed to close tab: {}", e))?;
        Ok(())
    }

    /// Activate (switch to) a tab by its target ID via /json/activate endpoint.
    pub async fn switch_tab(&self, target_id: &str) -> Result<(), String> {
        let endpoint = format!(
            "http://localhost:{}/json/activate/{}",
            self.cdp_port, target_id
        );
        reqwest::get(&endpoint)
            .await
            .map_err(|e| format!("Failed to switch tab: {}", e))?;
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

    /// Delete all cookies.
    pub async fn delete_cookies(&self) -> Result<(), String> {
        self.send_command("Network.clearBrowserCookies", json!({}))
            .await?;
        Ok(())
    }

    // ── Advanced: Console Logs ─────────────────────────────────────

    /// Get recent console log entries via JS.
    pub async fn get_console_logs(&self) -> Result<serde_json::Value, String> {
        let js = r#"
            (function() {
                if (!window.__browsion_console_logs) return [];
                return window.__browsion_console_logs.slice(-100);
            })()
        "#;
        self.evaluate_js(js).await
    }

    /// Install a console log interceptor that stores messages in-page.
    pub async fn enable_console_capture(&self) -> Result<(), String> {
        let js = r#"
            (function() {
                if (window.__browsion_console_logs) return;
                window.__browsion_console_logs = [];
                const orig = console.log;
                console.log = function() {
                    window.__browsion_console_logs.push({
                        type: 'log',
                        args: Array.from(arguments).map(a => String(a)),
                        ts: Date.now()
                    });
                    orig.apply(console, arguments);
                };
                const origErr = console.error;
                console.error = function() {
                    window.__browsion_console_logs.push({
                        type: 'error',
                        args: Array.from(arguments).map(a => String(a)),
                        ts: Date.now()
                    });
                    origErr.apply(console, arguments);
                };
                const origWarn = console.warn;
                console.warn = function() {
                    window.__browsion_console_logs.push({
                        type: 'warn',
                        args: Array.from(arguments).map(a => String(a)),
                        ts: Date.now()
                    });
                    origWarn.apply(console, arguments);
                };
            })()
        "#;
        self.evaluate_js(js).await?;
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
