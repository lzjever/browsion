use crate::agent::types::{DOMContext, DOMElement};
use crate::config::schema::BrowserProfile;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

/// Global port counter for CDP connections (starts at 9222, increments for each new client)
static CDP_PORT_COUNTER: AtomicU16 = AtomicU16::new(9222);

/// Get next available CDP port
fn get_next_cdp_port() -> u16 {
    let port = CDP_PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    // Wrap around if we exceed practical port range
    if port > 65500 {
        CDP_PORT_COUNTER.store(9222, Ordering::SeqCst);
        return 9222;
    }
    port
}

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
    /// Response receiver
    responses: Arc<Mutex<HashMap<u32, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
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
    /// Create a new CDP client
    pub fn new(profile_id: String) -> Self {
        Self {
            ws_tx: None,
            responses: Arc::new(Mutex::new(HashMap::new())),
            chrome_pid: None,
            profile_id,
            current_url: Arc::new(Mutex::new(String::new())),
            msg_id: Arc::new(Mutex::new(1)),
            cdp_port: get_next_cdp_port(),
        }
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

    /// Launch Chrome with CDP enabled and connect
    pub async fn launch(
        &mut self,
        chrome_path: &Path,
        profile: &BrowserProfile,
        headless: bool,
    ) -> Result<(), String> {
        // Build Chrome launch command
        let mut cmd = Command::new(chrome_path);

        // User data directory
        cmd.arg(format!(
            "--user-data-dir={}",
            profile.user_data_dir.display()
        ));

        // Enable remote debugging
        cmd.arg(format!("--remote-debugging-port={}", self.cdp_port));

        // Headless mode
        if headless {
            cmd.arg("--headless=new");
            cmd.arg("--disable-gpu");
        }

        // Disable some features that might interfere
        cmd.arg("--no-first-run");
        cmd.arg("--no-default-browser-check");
        cmd.arg("--disable-background-networking");
        cmd.arg("--disable-sync");

        // Proxy server
        if let Some(proxy) = &profile.proxy_server {
            cmd.arg(format!("--proxy-server={}", proxy));
        }

        // Language
        cmd.arg(format!("--lang={}", profile.lang));

        // Fingerprint
        if let Some(fp) = &profile.fingerprint {
            cmd.arg(format!("--fingerprint={}", fp));
        }

        // Timezone
        if let Some(tz) = &profile.timezone {
            cmd.arg(format!("--tz={}", tz));
        }

        // Custom arguments
        for arg in &profile.custom_args {
            cmd.arg(arg);
        }

        // Start about:blank to avoid loading a page
        cmd.arg("about:blank");

        // Launch Chrome
        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to launch Chrome: {}", e))?;
        self.chrome_pid = Some(child.id());

        // Give Chrome time to start and be ready for CDP connections
        let mut retries = 0;
        const MAX_RETRIES: u32 = 30;
        let mut last_error = String::new();

        while retries < MAX_RETRIES {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Get the list of targets to find a page target
            let list_url = format!("http://localhost:{}/json/list", self.cdp_port);

            match reqwest::get(&list_url).await {
                Ok(response) if response.status().is_success() => {
                    match response.json::<serde_json::Value>().await {
                        Ok(targets) => {
                            // Find the first page target
                            let page_target = if let Some(arr) = targets.as_array() {
                                arr.iter().find(|t| {
                                    t.get("type").and_then(|v| v.as_str()) == Some("page")
                                })
                            } else {
                                None
                            };

                            if let Some(target) = page_target {
                                if let Some(ws_url) =
                                    target.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
                                {
                                    tracing::info!(
                                        "Connecting to page target WebSocket: {}",
                                        ws_url
                                    );

                                    // Connect using raw WebSocket
                                    match connect_async(ws_url).await {
                                        Ok((ws_stream, _)) => {
                                            let (tx, mut rx) = StreamExt::split(ws_stream);

                                            self.ws_tx = Some(Arc::new(Mutex::new(tx)));

                                            // Clone for async task
                                            let responses = self.responses.clone();

                                            // Spawn a task to read responses
                                            tokio::spawn(async move {
                                                while let Some(msg) = StreamExt::next(&mut rx).await
                                                {
                                                    match msg {
                                                        Ok(WsMessage::Text(text)) => {
                                                            // Parse response and route to waiting sender
                                                            if let Ok(json) = serde_json::from_str::<
                                                                serde_json::Value,
                                                            >(
                                                                &text
                                                            ) {
                                                                if let Some(id) = json
                                                                    .get("id")
                                                                    .and_then(|i| i.as_u64())
                                                                {
                                                                    if let Some(sender) = responses
                                                                        .lock()
                                                                        .await
                                                                        .remove(&(id as u32))
                                                                    {
                                                                        let _ = sender.send(json);
                                                                    }
                                                                }
                                                            }
                                                            tracing::trace!(
                                                                "WS received: {}",
                                                                text.chars()
                                                                    .take(100)
                                                                    .collect::<String>()
                                                            );
                                                        }
                                                        Ok(WsMessage::Close(_)) => {
                                                            tracing::debug!("WebSocket closed");
                                                            break;
                                                        }
                                                        Err(e) => {
                                                            tracing::debug!(
                                                                "WebSocket error: {:?}",
                                                                e
                                                            );
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            });

                                            tracing::info!(
                                                "CDP client connected for profile {}",
                                                self.profile_id
                                            );

                                            // Enable required CDP domains
                                            self.send_command("Page.enable", json!({})).await?;
                                            self.send_command("Runtime.enable", json!({})).await?;
                                            tracing::info!("CDP domains enabled");

                                            return Ok(());
                                        }
                                        Err(e) => {
                                            last_error =
                                                format!("Failed to connect WebSocket: {}", e);
                                        }
                                    }
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
            "Failed to connect to Chrome after {} retries: {}",
            MAX_RETRIES, last_error
        ))
    }

    /// Navigate to a URL
    pub async fn navigate(&self, url: &str) -> Result<(), String> {
        let _ = self
            .send_command("Page.navigate", json!({"url": url}))
            .await?;
        *self.current_url.lock().await = url.to_string();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        tracing::info!("Navigated to: {}", url);
        Ok(())
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
            // Update stored URL
            *self.current_url.lock().await = url.to_string();
            Ok(url.to_string())
        } else {
            // Fallback to stored URL
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

    /// Click an element by selector
    pub async fn click(&self, selector: &str) -> Result<(), String> {
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let result = self.send_command("Runtime.evaluate", json!({
            "expression": format!(
                "(function() {{ const el = document.querySelector('{}'); if(el) {{ el.click(); return true; }} return false; }})()",
                escaped
            ),
            "returnByValue": true
        })).await?;

        let clicked = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if clicked {
            tracing::debug!("Clicked element: {}", selector);
            Ok(())
        } else {
            Err(format!("Element not found: {}", selector))
        }
    }

    /// Type text into an element
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), String> {
        let escaped_selector = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let escaped_text = text.replace('\\', "\\\\").replace('\'', "\\'");

        let result = self.send_command("Runtime.evaluate", json!({
            "expression": format!(
                "(function() {{ const el = document.querySelector('{}'); if(el) {{ el.focus(); el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles: true}})); return true; }} return false; }})()",
                escaped_selector, escaped_text
            ),
            "returnByValue": true
        })).await?;

        let typed = result
            .get("result")
            .and_then(|r| r.get("result"))
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if typed {
            tracing::debug!("Typed '{}' into element: {}", text, selector);
            Ok(())
        } else {
            Err(format!("Element not found: {}", selector))
        }
    }

    /// Press a key
    pub async fn press_key(&self, key: &str) -> Result<(), String> {
        let key_code = match key {
            "Enter" => 13,
            "Tab" => 9,
            "Escape" => 27,
            "Backspace" => 8,
            "ArrowUp" => 38,
            "ArrowDown" => 40,
            "ArrowLeft" => 37,
            "ArrowRight" => 39,
            _ => key.chars().next().map(|c| c as i32).unwrap_or(0),
        };

        self.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": key,
                "code": key,
                "windowsVirtualKeyCode": key_code
            }),
        )
        .await?;

        self.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": key,
                "code": key,
                "windowsVirtualKeyCode": key_code
            }),
        )
        .await?;

        tracing::debug!("Pressed key: {}", key);
        Ok(())
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

    /// Wait for an element
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

    /// Take a screenshot
    pub async fn screenshot(&self) -> Result<String, String> {
        let result = self
            .send_command(
                "Page.captureScreenshot",
                json!({
                    "format": "png"
                }),
            )
            .await?;

        if let Some(data) = result
            .get("result")
            .and_then(|r| r.get("data"))
            .and_then(|d| d.as_str())
        {
            tracing::debug!("Screenshot taken");
            Ok(data.to_string())
        } else {
            Err("Failed to capture screenshot".to_string())
        }
    }

    /// Get DOM context for LLM
    pub async fn get_dom_context(&self) -> Result<DOMContext, String> {
        let url = self.get_url().await?;

        // Use double quotes in JavaScript to avoid Rust raw string issues
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

    /// Go back in browser history
    pub async fn go_back(&self) -> Result<(), String> {
        self.send_command(
            "Page.navigate",
            json!({
                "url": "javascript:history.back()"
            }),
        )
        .await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        tracing::debug!("Navigated back");
        Ok(())
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

    /// Close the browser
    pub async fn close(&mut self) -> Result<(), String> {
        // Close WebSocket
        if let Some(tx) = self.ws_tx.take() {
            let mut tx_guard = tx.lock().await;
            let _ = tx_guard.close().await;
        }

        // Kill Chrome process if we started it
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
        // Ensure Chrome is killed when client is dropped
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
