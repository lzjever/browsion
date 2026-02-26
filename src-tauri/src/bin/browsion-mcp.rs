//! Browsion MCP Server — browser automation via Chrome DevTools Protocol.
//! Communicates with the Browsion HTTP API over localhost.
//!
//! Usage: browsion-mcp  (stdio transport; set BROWSION_API_PORT if not 38472)

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::*,
    schemars, tool, tool_handler, tool_router, ServerHandler,
    service::RequestContext,
    RoleServer, ServiceExt,
    ErrorData as McpError,
};
use reqwest::Client;
use serde_json::json;
use std::env;

const DEFAULT_PORT: u16 = 38472;

fn api_base() -> String {
    let port = env::var("BROWSION_API_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    format!("http://127.0.0.1:{}", port)
}

// ---------------------------------------------------------------------------
// Parameter types
// ---------------------------------------------------------------------------

// ── Profile management ──────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ProfileIdOnlyParam {
    /// Profile ID (from list_profiles)
    id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateProfileParam {
    /// Human-readable display name
    name: String,
    /// Absolute path to the Chrome user-data directory for this profile.
    /// Each profile must have a unique directory (e.g. ~/.browsion/profiles/work).
    user_data_dir: String,
    /// Accept-Language header and UI locale (default: "en-US")
    #[serde(default = "default_lang")]
    lang: String,
    /// Optional proxy server URL (e.g. "http://127.0.0.1:8080" or "socks5://host:1080")
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy_server: Option<String>,
    /// Optional IANA timezone identifier (e.g. "America/Los_Angeles", "Asia/Shanghai")
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    /// Optional fingerprint seed (numeric string) for browser fingerprinting
    #[serde(skip_serializing_if = "Option::is_none")]
    fingerprint: Option<String>,
    /// Tags for grouping / filtering profiles (e.g. ["work", "vpn"])
    #[serde(default)]
    tags: Vec<String>,
    /// Extra Chrome command-line arguments (advanced; most use cases don't need this)
    #[serde(default)]
    custom_args: Vec<String>,
}

fn default_lang() -> String { "en-US".to_string() }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UpdateProfileParam {
    /// Profile ID — must match an existing profile
    id: String,
    /// New display name
    name: String,
    /// Short description (may be empty string)
    #[serde(default)]
    description: String,
    /// Absolute path to the Chrome user-data directory
    user_data_dir: String,
    /// Accept-Language and UI locale (default: "en-US")
    #[serde(default = "default_lang")]
    lang: String,
    /// Proxy server URL (null to remove)
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy_server: Option<String>,
    /// IANA timezone (null to remove)
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    /// Fingerprint seed (null to remove)
    #[serde(skip_serializing_if = "Option::is_none")]
    fingerprint: Option<String>,
    /// Tags list (replaces existing tags)
    #[serde(default)]
    tags: Vec<String>,
    /// Extra Chrome arguments (replaces existing)
    #[serde(default)]
    custom_args: Vec<String>,
}

// ── Browser lifecycle ───────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct BrowserParam {
    /// Profile ID whose browser to control
    profile_id: String,
}

// ── Navigation ──────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct NavigateParam {
    /// Profile ID
    profile_id: String,
    /// Fully-qualified URL to navigate to (include https://)
    url: String,
    /// When to consider navigation complete:
    /// - "load" (default): wait for window.onload — safe for most pages
    /// - "domcontentloaded": wait for DOMContentLoaded only — faster, DOM ready but resources may still load
    /// - "none": fire-and-forget, return immediately after sending navigation command
    #[serde(default = "default_wait_until")]
    wait_until: String,
    /// Maximum time to wait for the load event in milliseconds (default: 15000)
    #[serde(default = "default_nav_timeout")]
    timeout_ms: u64,
}

fn default_wait_until() -> String { "load".to_string() }
fn default_nav_timeout() -> u64 { 15000 }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WaitForNavParam {
    /// Profile ID
    profile_id: String,
    /// Maximum wait time in milliseconds (default: 15000)
    #[serde(default = "default_nav_timeout")]
    timeout_ms: u64,
}

// ── Element interaction ─────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SelectorParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector identifying the target element (e.g. "#submit", ".btn-primary", "input[name=email]")
    selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TypeTextParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector of the input/textarea element
    selector: String,
    /// Text to type (entire string set at once; triggers input+change events)
    text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SlowTypeParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector of the input element
    selector: String,
    /// Text to type character-by-character
    text: String,
    /// Delay between keystrokes in milliseconds (default: 50)
    #[serde(default = "default_key_delay")]
    delay_ms: u64,
}
fn default_key_delay() -> u64 { 50 }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SelectOptionParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector of the <select> element
    selector: String,
    /// Option to select — matched against the option's value attribute first, then visible text
    value: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PressKeyParam {
    /// Profile ID
    profile_id: String,
    /// Key or key combination to press.
    /// Single keys: "Enter", "Tab", "Escape", "Backspace", "Delete",
    ///              "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
    ///              "Home", "End", "PageUp", "PageDown", "F1"…"F12"
    /// Combos: "Ctrl+A", "Ctrl+C", "Ctrl+V", "Ctrl+Z", "Shift+Enter", "Ctrl+Shift+I"
    key: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UploadFileParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector of the <input type="file"> element
    selector: String,
    /// Absolute path to the file on the local filesystem
    file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ScrollParam {
    /// Profile ID
    profile_id: String,
    /// Scroll direction: "up", "down", "left", "right"
    direction: String,
    /// Number of pixels to scroll (default: 500)
    #[serde(default = "default_scroll")]
    amount: u32,
}
fn default_scroll() -> u32 { 500 }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WaitForParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector to wait for
    selector: String,
    /// Timeout in milliseconds (default: 5000)
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}
fn default_timeout() -> u64 { 5000 }

// ── AX tree ref-based actions ───────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RefParam {
    /// Profile ID
    profile_id: String,
    /// Element ref from get_page_state / get_ax_tree (e.g. "e1", "e12")
    ref_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TypeRefParam {
    /// Profile ID
    profile_id: String,
    /// Element ref from get_page_state / get_ax_tree (e.g. "e3")
    ref_id: String,
    /// Text to set in the element
    text: String,
}

// ── Data / JS ───────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ExtractParam {
    /// Profile ID
    profile_id: String,
    /// Map of label → CSS selector. Each selector's innerText (or value) is returned.
    selectors: std::collections::HashMap<String, String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EvaluateParam {
    /// Profile ID
    profile_id: String,
    /// JavaScript expression to evaluate in the page context. Supports await.
    expression: String,
}

// ── Tabs ────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct NewTabParam {
    /// Profile ID
    profile_id: String,
    /// URL to open in the new tab (default: "about:blank")
    #[serde(default = "default_new_tab_url")]
    url: String,
}
fn default_new_tab_url() -> String { "about:blank".to_string() }

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TabTargetParam {
    /// Profile ID
    profile_id: String,
    /// CDP target ID of the tab (from list_tabs)
    target_id: String,
}

// ── Cookies ─────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetCookieParam {
    /// Profile ID
    profile_id: String,
    /// Cookie name
    name: String,
    /// Cookie value
    value: String,
    /// Cookie domain (e.g. ".github.com" or "github.com")
    domain: String,
    /// Cookie path (default: "/")
    #[serde(default = "default_cookie_path")]
    path: String,
}
fn default_cookie_path() -> String { "/".to_string() }

// ── Utility ─────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WaitParam {
    /// Duration to wait in milliseconds
    duration_ms: u64,
}

// ── Screenshot ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ScreenshotParam {
    /// Profile ID
    profile_id: String,
    /// Capture the full scrollable page, not just the visible viewport (default: false)
    #[serde(default)]
    full_page: bool,
    /// Image format: "png" (lossless, default), "jpeg" (smaller), or "webp" (modern, smaller)
    #[serde(default = "default_screenshot_format")]
    format: String,
    /// Compression quality for jpeg/webp (0–100). Ignored for png. Default: 80.
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<u32>,
}
fn default_screenshot_format() -> String { "png".to_string() }

// ── Dialog / coordinate / drag / network / text-wait ─────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct HandleDialogParam {
    /// Profile ID
    profile_id: String,
    /// "accept" to click OK/Confirm, "dismiss" to click Cancel/Close
    action: String,
    /// Text to type into a prompt() dialog before accepting (ignored for alert/confirm)
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_text: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ClickAtParam {
    /// Profile ID
    profile_id: String,
    /// Viewport X coordinate in CSS pixels (0 = left edge)
    x: f64,
    /// Viewport Y coordinate in CSS pixels (0 = top edge)
    y: f64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DragParam {
    /// Profile ID
    profile_id: String,
    /// CSS selector of the element to drag from
    from_selector: String,
    /// CSS selector of the element to drop onto
    to_selector: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WaitForTextParam {
    /// Profile ID
    profile_id: String,
    /// Text to wait for anywhere in document.body.innerText
    text: String,
    /// Maximum wait in milliseconds (default: 30000)
    #[serde(default = "default_wait_for_text_timeout")]
    timeout_ms: u64,
}
fn default_wait_for_text_timeout() -> u64 { 30000 }

// ---------------------------------------------------------------------------
// MCP Server
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct BrowsionMcpServer {
    client: Client,
    base: String,
    api_key: Option<String>,
    tool_router: ToolRouter<Self>,
}

impl BrowsionMcpServer {
    fn new() -> Self {
        Self {
            client: Client::new(),
            base: api_base(),
            api_key: env::var("BROWSION_API_KEY").ok().filter(|s| !s.is_empty()),
            tool_router: Self::tool_router(),
        }
    }

    fn apply_key(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => builder.header("X-API-Key", key),
            None => builder,
        }
    }

    async fn api_post(&self, path: &str, body: &serde_json::Value) -> Result<String, McpError> {
        let req = self.client.post(format!("{}{}", self.base, path)).json(body);
        let resp = self.apply_key(req).send().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let status = resp.status();
        let text = resp.text().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if !status.is_success() {
            return Err(McpError::internal_error(text, None));
        }
        Ok(text)
    }

    async fn api_get(&self, path: &str) -> Result<String, McpError> {
        let req = self.client.get(format!("{}{}", self.base, path));
        let resp = self.apply_key(req).send().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let status = resp.status();
        let text = resp.text().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if !status.is_success() {
            return Err(McpError::internal_error(text, None));
        }
        Ok(text)
    }

    async fn api_delete(&self, path: &str) -> Result<String, McpError> {
        let req = self.client.delete(format!("{}{}", self.base, path));
        let resp = self.apply_key(req).send().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let status = resp.status();
        let text = resp.text().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if !status.is_success() {
            return Err(McpError::internal_error(text, None));
        }
        Ok(text)
    }

    async fn api_put(&self, path: &str, body: &serde_json::Value) -> Result<String, McpError> {
        let req = self.client.put(format!("{}{}", self.base, path)).json(body);
        let resp = self.apply_key(req).send().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let status = resp.status();
        let text = resp.text().await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if !status.is_success() {
            return Err(McpError::internal_error(text, None));
        }
        Ok(text)
    }

    fn text_result(text: impl Into<String>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

use rmcp::handler::server::wrapper::Parameters;

#[tool_router]
impl BrowsionMcpServer {
    // ── 1. Profile Management ──────────────────────────────────────────────
    //
    // Browsion profiles are isolated Chrome instances. Each profile has its own
    // cookies, localStorage, history, and optionally a proxy or timezone.
    // Use list_profiles to discover existing profiles and their IDs.

    /// List all profiles with running status
    #[tool(description = "List all browser profiles. Returns an array of profile objects each containing: id, name, description, user_data_dir, lang, proxy_server, timezone, tags, is_running. Use the id field with all other tools.")]
    async fn list_profiles(&self) -> Result<CallToolResult, McpError> {
        let body = self.api_get("/api/profiles").await?;
        Self::text_result(body)
    }

    /// Get a single profile by ID
    #[tool(description = "Get the full details of a single profile by its ID. Returns the profile object including all configuration fields.")]
    async fn get_profile(
        &self,
        Parameters(p): Parameters<ProfileIdOnlyParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self.api_get(&format!("/api/profiles/{}", p.id)).await?;
        Self::text_result(body)
    }

    /// Create a new browser profile
    #[tool(description = "Create a new browser profile with an isolated Chrome user-data directory. Each profile is a separate browser identity (cookies, history, localStorage are isolated). Returns the created profile including its auto-generated id. Use that id with launch_browser and all browser control tools.")]
    async fn create_profile(
        &self,
        Parameters(p): Parameters<CreateProfileParam>,
    ) -> Result<CallToolResult, McpError> {
        let id = uuid::Uuid::new_v4().to_string();
        let profile = json!({
            "id": id,
            "name": p.name,
            "description": "",
            "user_data_dir": p.user_data_dir,
            "lang": p.lang,
            "proxy_server": p.proxy_server,
            "timezone": p.timezone,
            "fingerprint": p.fingerprint,
            "tags": p.tags,
            "custom_args": p.custom_args,
        });
        let body = self.api_post("/api/profiles", &profile).await?;
        Self::text_result(body)
    }

    /// Update an existing profile
    #[tool(description = "Update an existing browser profile. All fields are required (use get_profile to read current values first, then modify what you need). The id must match an existing profile. Returns the updated profile.")]
    async fn update_profile(
        &self,
        Parameters(p): Parameters<UpdateProfileParam>,
    ) -> Result<CallToolResult, McpError> {
        let profile = json!({
            "id": p.id,
            "name": p.name,
            "description": p.description,
            "user_data_dir": p.user_data_dir,
            "lang": p.lang,
            "proxy_server": p.proxy_server,
            "timezone": p.timezone,
            "fingerprint": p.fingerprint,
            "tags": p.tags,
            "custom_args": p.custom_args,
        });
        let body = self.api_put(&format!("/api/profiles/{}", p.id), &profile).await?;
        Self::text_result(body)
    }

    /// Delete a profile by ID
    #[tool(description = "Permanently delete a browser profile by ID. Fails if the browser is currently running (call kill_browser first). Does NOT delete the user-data directory from disk.")]
    async fn delete_profile(
        &self,
        Parameters(p): Parameters<ProfileIdOnlyParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_delete(&format!("/api/profiles/{}", p.id)).await?;
        Self::text_result(format!("Profile {} deleted", p.id))
    }

    // ── 2. Browser Lifecycle ───────────────────────────────────────────────

    /// Launch a profile's browser
    #[tool(description = "Launch Chrome for a profile with CDP enabled. Returns { pid, cdp_port }. If the browser is already running, returns an error — check get_running_browsers first. After launching, use navigate and other browser control tools with the same profile_id.")]
    async fn launch_browser(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self.api_post(&format!("/api/launch/{}", p.profile_id), &json!({})).await?;
        Self::text_result(body)
    }

    /// Kill a running browser
    #[tool(description = "Kill the Chrome browser for a profile. Closes all tabs and terminates the process. Returns an error if the browser is not running.")]
    async fn kill_browser(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_post(&format!("/api/kill/{}", p.profile_id), &json!({})).await?;
        Self::text_result(format!("Browser for profile {} stopped", p.profile_id))
    }

    /// List all running browsers
    #[tool(description = "List all currently running browsers. Returns an array of { profile_id, pid, cdp_port, launched_at }. Use this to check which profiles are active before issuing browser control commands.")]
    async fn get_running_browsers(&self) -> Result<CallToolResult, McpError> {
        let body = self.api_get("/api/running").await?;
        Self::text_result(body)
    }

    // ── 3. Navigation ──────────────────────────────────────────────────────

    /// Navigate to a URL
    #[tool(description = "Navigate to a URL and wait for the page to load. Returns { url, title } after the load event fires (up to timeout_ms). Use wait_until='domcontentloaded' for faster SPAs, or 'none' to fire-and-forget. Always call this after launch_browser before any page interaction.")]
    async fn navigate(
        &self,
        Parameters(p): Parameters<NavigateParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/navigate_wait", p.profile_id),
                &json!({ "url": p.url, "wait_until": p.wait_until, "timeout_ms": p.timeout_ms }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Go back in browser history
    #[tool(description = "Navigate to the previous page in browser history (equivalent to clicking the Back button). Waits for the page to load. Returns { url, title }.")]
    async fn go_back(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(&format!("/api/browser/{}/back", p.profile_id), &json!({}))
            .await?;
        Self::text_result(body)
    }

    /// Go forward in browser history
    #[tool(description = "Navigate to the next page in browser history (equivalent to clicking the Forward button). Waits for the page to load. Returns { url, title }.")]
    async fn go_forward(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(&format!("/api/browser/{}/forward", p.profile_id), &json!({}))
            .await?;
        Self::text_result(body)
    }

    /// Reload the current page
    #[tool(description = "Reload (refresh) the current page and wait for it to finish loading. Returns { url, title }.")]
    async fn reload(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(&format!("/api/browser/{}/reload", p.profile_id), &json!({}))
            .await?;
        Self::text_result(body)
    }

    /// Wait for navigation triggered by a page action
    #[tool(description = "Wait for a page-load event that was triggered by a prior action (e.g. clicking a submit button that navigates). Subscribe BEFORE the action if possible; use navigate instead when you control the URL. Returns { url, title } when load fires or an error on timeout.")]
    async fn wait_for_navigation(
        &self,
        Parameters(p): Parameters<WaitForNavParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/wait_for_nav", p.profile_id),
                &json!({ "timeout_ms": p.timeout_ms }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Get the current page URL
    #[tool(description = "Get the current URL of the active page in a profile's browser.")]
    async fn get_current_url(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/url", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Get the current page title
    #[tool(description = "Get the title of the active page (document.title) in a profile's browser.")]
    async fn get_page_title(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/title", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    // ── 4. Mouse Interactions ──────────────────────────────────────────────

    /// Click an element
    #[tool(description = "Click an element by CSS selector using real mouse events (mouseMoved → mousePressed → mouseReleased). Works with hover-triggered dropdowns, React synthetic events, and Shadow DOM. Fails if element is not found or has no layout box.")]
    async fn click(
        &self,
        Parameters(p): Parameters<SelectorParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/click", p.profile_id),
                &json!({ "selector": p.selector }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Hover over an element
    #[tool(description = "Move the mouse cursor over an element (mouseMoved event). Use to reveal dropdown menus, tooltips, or :hover-only UI elements before clicking them.")]
    async fn hover(
        &self,
        Parameters(p): Parameters<SelectorParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/hover", p.profile_id),
                &json!({ "selector": p.selector }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Double-click an element
    #[tool(description = "Double-click an element by CSS selector. Use for text selection, opening items in file trees, or triggering dblclick event handlers.")]
    async fn double_click(
        &self,
        Parameters(p): Parameters<SelectorParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/double_click", p.profile_id),
                &json!({ "selector": p.selector }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Right-click an element
    #[tool(description = "Right-click an element by CSS selector to open its context menu.")]
    async fn right_click(
        &self,
        Parameters(p): Parameters<SelectorParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/right_click", p.profile_id),
                &json!({ "selector": p.selector }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 5. Keyboard & Text Input ───────────────────────────────────────────

    /// Type text into an input field
    #[tool(description = "Set the value of an input or textarea element and fire input+change events. Works with React controlled inputs. For fields that validate on each keystroke (autocomplete, OTP), use slow_type instead.")]
    async fn type_text(
        &self,
        Parameters(p): Parameters<TypeTextParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/type", p.profile_id),
                &json!({ "selector": p.selector, "text": p.text }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Type text character-by-character (keystroke simulation)
    #[tool(description = "Type text one character at a time using real keyDown/char/keyUp events with a configurable delay. Use for: autocomplete/suggestion fields, OTP inputs, rich-text editors, or any field that must process each keystroke individually.")]
    async fn slow_type(
        &self,
        Parameters(p): Parameters<SlowTypeParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/slow_type", p.profile_id),
                &json!({ "selector": p.selector, "text": p.text, "delay_ms": p.delay_ms }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Press a key or key combination
    #[tool(description = "Dispatch a keyDown+keyUp event for a key or modifier combination. Examples: 'Enter', 'Tab', 'Escape', 'Backspace', 'Delete', 'ArrowDown', 'F5', 'Ctrl+A', 'Ctrl+C', 'Ctrl+V', 'Ctrl+Z', 'Shift+Enter', 'Ctrl+Shift+I'. Use after type_text to submit forms (press 'Enter').")]
    async fn press_key(
        &self,
        Parameters(p): Parameters<PressKeyParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/press_key", p.profile_id),
                &json!({ "key": p.key }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 6. Form Controls ───────────────────────────────────────────────────

    /// Select an option in a <select> dropdown
    #[tool(description = "Select an option in a <select> element by its value attribute or visible label text. Fires input and change events. Example: select_option(selector='#country', value='US') or select_option(selector='select[name=size]', value='Large').")]
    async fn select_option(
        &self,
        Parameters(p): Parameters<SelectOptionParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/select_option", p.profile_id),
                &json!({ "selector": p.selector, "value": p.value }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Upload a file to a file input
    #[tool(description = "Set a file on an <input type='file'> element using the CDP DOM API (bypasses the OS file picker). Provide the absolute path to the file. After upload, trigger form submission or further interaction as needed.")]
    async fn upload_file(
        &self,
        Parameters(p): Parameters<UploadFileParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/upload_file", p.profile_id),
                &json!({ "selector": p.selector, "file_path": p.file_path.clone() }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 7. Scroll & Wait ───────────────────────────────────────────────────

    /// Scroll the page
    #[tool(description = "Scroll the viewport by a pixel amount in a direction. Use scroll_into_view to bring a specific element into the viewport before interacting with it.")]
    async fn scroll(
        &self,
        Parameters(p): Parameters<ScrollParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/scroll", p.profile_id),
                &json!({ "direction": p.direction, "amount": p.amount }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Scroll an element into the viewport
    #[tool(description = "Scroll the page until the specified element is visible and centered in the viewport. Call before click/hover if an element might be outside the visible area (e.g. in a long list or below the fold).")]
    async fn scroll_into_view(
        &self,
        Parameters(p): Parameters<SelectorParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/scroll_into_view", p.profile_id),
                &json!({ "selector": p.selector }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Wait for an element to appear in the DOM
    #[tool(description = "Poll until a CSS selector is present in the DOM, up to timeout_ms. Useful for waiting for lazy-loaded content, modal dialogs, or async UI updates. Returns { ok: true } when found, error on timeout.")]
    async fn wait_for_element(
        &self,
        Parameters(p): Parameters<WaitForParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/wait_for", p.profile_id),
                &json!({ "selector": p.selector, "timeout_ms": p.timeout_ms }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 8. Page Observation ────────────────────────────────────────────────

    /// Get page state: URL, title, and accessibility tree (recommended)
    #[tool(description = "One-shot page observation: returns { url, title, element_count, ax_tree: [...] }. The ax_tree is a compact list of interactive and landmark elements, each with a stable ref_id (e.g. 'e1'). Use ref_ids with click_ref, type_ref, focus_ref. Typically 200-400 tokens vs thousands for DOM context. Call this as the first observation step on any new page.")]
    async fn get_page_state(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/page_state", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Get only the accessibility tree (without re-fetching URL/title)
    #[tool(description = "Get the filtered Accessibility Tree for the current page. Returns an array of nodes with { ref_id, role, name, description, value, focused, disabled, checked }. Use ref_ids with click_ref / type_ref / focus_ref. Call get_page_state instead if you also need URL and title.")]
    async fn get_ax_tree(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/ax_tree", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Take a screenshot
    #[tool(description = "Capture a screenshot of the current page and return it as an inline image. Options: full_page=true to capture the entire scrollable page; format='jpeg'/'webp' for smaller files; quality=80 for jpeg/webp compression. Use for visual verification, debugging, or when the page has visual content (charts, captchas) not captured in the AX tree.")]
    async fn screenshot(
        &self,
        Parameters(p): Parameters<ScreenshotParam>,
    ) -> Result<CallToolResult, McpError> {
        let mut url = format!("/api/browser/{}/screenshot?format={}", p.profile_id, p.format);
        if p.full_page { url.push_str("&full_page=true"); }
        if let Some(q) = p.quality { url.push_str(&format!("&quality={}", q)); }
        let body = self.api_get(&url).await?;
        let parsed: serde_json::Value =
            serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
        if let Some(image) = parsed.get("image").and_then(|v| v.as_str()) {
            let mime = match p.format.as_str() {
                "jpeg" => "image/jpeg",
                "webp" => "image/webp",
                _ => "image/png",
            };
            Ok(CallToolResult::success(vec![Content::image(image, mime)]))
        } else {
            Self::text_result(body)
        }
    }

    /// Get raw DOM context (legacy; prefer get_page_state)
    #[tool(description = "Get interactive DOM elements, forms, and links as structured JSON. Legacy alternative to get_page_state — produces more tokens and less semantic information. Prefer get_page_state for AI tasks. Useful when you need raw DOM attributes not captured in the AX tree.")]
    async fn get_dom_context(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/dom_context", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Extract text content by CSS selectors
    #[tool(description = "Extract text/value from multiple elements at once using a map of { label: 'CSS selector' }. Returns a map of { label: 'extracted text' }. Efficient for scraping structured data from known selectors.")]
    async fn extract_data(
        &self,
        Parameters(p): Parameters<ExtractParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/extract", p.profile_id),
                &json!({ "selectors": p.selectors }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 9. AX-Ref Actions ─────────────────────────────────────────────────

    /// Click by accessibility tree ref
    #[tool(description = "Click an element using its ref_id from get_page_state / get_ax_tree (e.g. 'e5'). Uses the same real mouse events as click(). Preferred over click() because refs are semantically grounded — they map directly to accessible interactive elements.")]
    async fn click_ref(
        &self,
        Parameters(p): Parameters<RefParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/click_ref", p.profile_id),
                &json!({ "ref_id": p.ref_id }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Type into element by accessibility tree ref
    #[tool(description = "Set text in an input element identified by its ref_id from get_page_state / get_ax_tree. Focuses the element, sets its value, and fires input+change events.")]
    async fn type_ref(
        &self,
        Parameters(p): Parameters<TypeRefParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/type_ref", p.profile_id),
                &json!({ "ref_id": p.ref_id, "text": p.text }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Focus element by accessibility tree ref
    #[tool(description = "Move keyboard focus to an element identified by its ref_id. After focusing, use press_key to send keystrokes directly to that element.")]
    async fn focus_ref(
        &self,
        Parameters(p): Parameters<RefParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/focus_ref", p.profile_id),
                &json!({ "ref_id": p.ref_id }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 10. JavaScript ────────────────────────────────────────────────────

    /// Execute JavaScript in the page
    #[tool(description = "Evaluate a JavaScript expression in the page context and return the result. Supports await for async operations. Use for: reading DOM state, triggering custom events, interacting with page APIs, or extracting data not reachable via CSS selectors. Returns { result: <value> }.")]
    async fn evaluate_js(
        &self,
        Parameters(p): Parameters<EvaluateParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/evaluate", p.profile_id),
                &json!({ "expression": p.expression }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 11. Tabs ──────────────────────────────────────────────────────────

    /// List all open tabs
    #[tool(description = "List all open browser tabs in a profile's browser. Returns an array of { id, url, title, type }. Use the id (CDP target ID) with switch_tab and close_tab.")]
    async fn list_tabs(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/tabs", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Open a new tab
    #[tool(description = "Open a new browser tab, optionally at a specific URL. Returns { id, url, title }. Note: the CDP session remains connected to the original tab; use switch_tab to focus the new one.")]
    async fn new_tab(
        &self,
        Parameters(p): Parameters<NewTabParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/tabs/new", p.profile_id),
                &json!({ "url": p.url }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Switch to a tab by its target ID
    #[tool(description = "Bring a specific tab into focus by its CDP target ID (from list_tabs). This makes the tab active in the browser UI but does not reconnect the CDP session to it.")]
    async fn switch_tab(
        &self,
        Parameters(p): Parameters<TabTargetParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_post(
            &format!("/api/browser/{}/tabs/switch", p.profile_id),
            &json!({ "target_id": p.target_id }),
        )
        .await?;
        Self::text_result(format!("Switched to tab {}", p.target_id))
    }

    /// Close a tab
    #[tool(description = "Close a browser tab by its CDP target ID (from list_tabs).")]
    async fn close_tab(
        &self,
        Parameters(p): Parameters<TabTargetParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_post(
            &format!("/api/browser/{}/tabs/close", p.profile_id),
            &json!({ "target_id": p.target_id }),
        )
        .await?;
        Self::text_result(format!("Tab {} closed", p.target_id))
    }

    // ── 12. Cookies ───────────────────────────────────────────────────────

    /// Get all cookies for the current page
    #[tool(description = "Get all cookies accessible to the current page. Returns an array of { name, value, domain, path, secure, httpOnly, expires }.")]
    async fn get_cookies(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/cookies", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Set a cookie
    #[tool(description = "Set a cookie in the browser. Use to inject session tokens or auth cookies before navigating to a page. Domain should match the target site (e.g. '.github.com' for all GitHub subdomains).")]
    async fn set_cookie(
        &self,
        Parameters(p): Parameters<SetCookieParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_post(
            &format!("/api/browser/{}/cookies/set", p.profile_id),
            &json!({ "name": p.name, "value": p.value, "domain": p.domain, "path": p.path }),
        )
        .await?;
        Self::text_result("Cookie set")
    }

    /// Delete all cookies
    #[tool(description = "Delete all cookies in the browser (equivalent to clearing browser cookies). Use to force a fresh unauthenticated state.")]
    async fn delete_cookies(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_post(
            &format!("/api/browser/{}/cookies/clear", p.profile_id),
            &json!({}),
        )
        .await?;
        Self::text_result("All cookies cleared")
    }

    // ── 13. Console ───────────────────────────────────────────────────────

    /// Enable console log capture
    #[tool(description = "Inject a console interceptor into the current page to capture console.log / console.error / console.warn output. Must be called once per page (re-call after navigation). Then use get_console_logs to read captured output.")]
    async fn enable_console_capture(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        self.api_post(
            &format!("/api/browser/{}/console/enable", p.profile_id),
            &json!({}),
        )
        .await?;
        Self::text_result("Console capture enabled")
    }

    /// Get captured console logs
    #[tool(description = "Read the last 100 console.log / console.error / console.warn messages captured since enable_console_capture was called. Returns an array of { type, args, ts }.")]
    async fn get_console_logs(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/console", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    // ── 14. Dialog Handling ───────────────────────────────────────────────

    /// Handle a JavaScript dialog (alert/confirm/prompt)
    #[tool(description = "Accept or dismiss a browser dialog (alert / confirm / prompt). Call immediately after the action that triggers the dialog — automation is blocked until the dialog is handled. For prompt() dialogs, supply prompt_text to type into the input before accepting. Returns { ok: true }.")]
    async fn handle_dialog(
        &self,
        Parameters(p): Parameters<HandleDialogParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/handle_dialog", p.profile_id),
                &json!({ "action": p.action, "prompt_text": p.prompt_text }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 15. Coordinate & Drag ─────────────────────────────────────────────

    /// Click at specific viewport coordinates
    #[tool(description = "Click at exact viewport (x, y) pixel coordinates. Use for: canvas elements, image maps, coordinate-based vision automation, or clicking areas without a selectable DOM element. Coordinates are relative to the visible viewport (not the document).")]
    async fn click_at(
        &self,
        Parameters(p): Parameters<ClickAtParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/click_at", p.profile_id),
                &json!({ "x": p.x, "y": p.y }),
            )
            .await?;
        Self::text_result(body)
    }

    /// Drag one element onto another
    #[tool(description = "Drag an element from one position to another using real mouse events (mousemove → mousedown → intermediate moves → mouseup). Use for drag-and-drop UI components, sortable lists, kanban boards, file drop zones. Returns { ok: true }.")]
    async fn drag(
        &self,
        Parameters(p): Parameters<DragParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/drag", p.profile_id),
                &json!({ "from_selector": p.from_selector, "to_selector": p.to_selector }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 16. Network Log ───────────────────────────────────────────────────

    /// Get recent network requests
    #[tool(description = "Return the last 200 network requests and responses captured since the browser connected. Each entry has { type: 'request'|'response', url, method, status, requestId }. Useful for debugging API calls, understanding what data a page fetches, or waiting for specific network activity.")]
    async fn get_network_log(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_get(&format!("/api/browser/{}/network_log", p.profile_id))
            .await?;
        Self::text_result(body)
    }

    /// Clear the network log
    #[tool(description = "Clear all captured network log entries. Call before an action to isolate the network requests made by that specific action.")]
    async fn clear_network_log(
        &self,
        Parameters(p): Parameters<BrowserParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/network_log/clear", p.profile_id),
                &json!({}),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 17. Text Wait ─────────────────────────────────────────────────────

    /// Wait for text to appear on the page
    #[tool(description = "Poll until the specified text appears anywhere in document.body.innerText. Useful for waiting for async content to load, confirmation messages, or dynamic UI updates. Returns { ok: true } when found, error on timeout.")]
    async fn wait_for_text(
        &self,
        Parameters(p): Parameters<WaitForTextParam>,
    ) -> Result<CallToolResult, McpError> {
        let body = self
            .api_post(
                &format!("/api/browser/{}/wait_for_text", p.profile_id),
                &json!({ "text": p.text, "timeout_ms": p.timeout_ms }),
            )
            .await?;
        Self::text_result(body)
    }

    // ── 18. Utility ───────────────────────────────────────────────────────

    /// Wait for a fixed duration
    #[tool(description = "Pause for a fixed number of milliseconds. Prefer wait_for_element, wait_for_text, or wait_for_navigation over fixed waits when possible.")]
    async fn wait(
        &self,
        Parameters(p): Parameters<WaitParam>,
    ) -> Result<CallToolResult, McpError> {
        tokio::time::sleep(tokio::time::Duration::from_millis(p.duration_ms)).await;
        Self::text_result(format!("Waited {}ms", p.duration_ms))
    }
}

#[tool_handler]
impl ServerHandler for BrowsionMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "browsion-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some("Browsion Browser Automation".into()),
                description: Some(
                    "Automate isolated Chrome browser profiles via Chrome DevTools Protocol (CDP)".into(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "# Browsion MCP — Browser Automation via CDP\n\
                 \n\
                 ## Core Concepts\n\
                 - **Profile**: an isolated Chrome identity with its own cookies, localStorage, history.\n\
                 - Each profile maps to a unique Chrome user-data directory on disk.\n\
                 - Multiple profiles can run simultaneously and are fully isolated.\n\
                 - All browser-control tools require `profile_id` — the browser for that profile must be running.\n\
                 \n\
                 ## Standard Workflow\n\
                 ```\n\
                 1. list_profiles          → find or create a profile\n\
                 2. launch_browser         → start Chrome for that profile\n\
                 3. navigate               → go to a URL (waits for page load)\n\
                 4. get_page_state         → observe page: URL + title + AX tree with refs\n\
                 5. click_ref / type_ref   → interact using semantic element refs from step 4\n\
                 6. screenshot             → visual verification when needed\n\
                 7. kill_browser           → shut down when done\n\
                 ```\n\
                 \n\
                 ## Tool Groups\n\
                 **Profiles (CRUD):** list_profiles, get_profile, create_profile, update_profile, delete_profile\n\
                 **Lifecycle:** launch_browser, kill_browser, get_running_browsers\n\
                 **Navigation:** navigate, go_back, go_forward, reload, wait_for_navigation, get_current_url, get_page_title\n\
                 **Mouse:** click, hover, double_click, right_click, click_at, drag\n\
                 **Keyboard/Input:** type_text, slow_type, press_key\n\
                 **Forms:** select_option, upload_file\n\
                 **Scroll/Wait:** scroll, scroll_into_view, wait_for_element, wait_for_text\n\
                 **Observe (recommended):** get_page_state, get_ax_tree, screenshot\n\
                 **AX-Ref Actions:** click_ref, type_ref, focus_ref\n\
                 **DOM (legacy):** get_dom_context, extract_data\n\
                 **JS:** evaluate_js\n\
                 **Tabs:** list_tabs, new_tab, switch_tab, close_tab\n\
                 **Cookies:** get_cookies, set_cookie, delete_cookies\n\
                 **Console:** enable_console_capture, get_console_logs\n\
                 **Network:** get_network_log, clear_network_log\n\
                 **Dialog:** handle_dialog\n\
                 **Utility:** wait\n\
                 \n\
                 ## AX Tree Workflow (recommended for AI agents)\n\
                 `get_page_state` returns a filtered Accessibility Tree with `ref_id`s:\n\
                 ```json\n\
                 { \"ref_id\": \"e3\", \"role\": \"textbox\", \"name\": \"Email\" }\n\
                 ```\n\
                 Then: `type_ref(ref_id='e3', text='user@example.com')` — no fragile CSS selectors needed.\n\
                 Refs are valid until the next navigation.\n\
                 \n\
                 ## Tips\n\
                 - `navigate` waits for the full page load event (default 15s timeout).\n\
                 - Use `scroll_into_view` before clicking elements that may be off-screen.\n\
                 - Use `slow_type` for autocomplete/OTP fields; `type_text` for everything else.\n\
                 - `select_option` works by value attribute OR visible label text.\n\
                 - If a page shows alert/confirm/prompt, call `handle_dialog` to unblock automation.\n\
                 - `click_at(x, y)` for canvas or vision-mode coordinate clicks.\n\
                 - `screenshot(full_page=true)` for full-page captures; use `format='jpeg'` for smaller files.\n\
                 - `get_network_log` to debug API calls or verify network activity.\n\
                 - `evaluate_js` is the escape hatch for anything else."
                    .into(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let base = api_base();
    let client = Client::new();

    match client.get(format!("{}/api/health", base)).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Browsion API reachable at {}", base);
        }
        Ok(resp) => {
            eprintln!("Browsion API returned HTTP {}: {}", resp.status(), base);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Browsion API not reachable at {}: {}", base, e);
            eprintln!("Start Browsion with the API server enabled in Settings (default port 38472).");
            std::process::exit(1);
        }
    }

    let server = BrowsionMcpServer::new();
    let transport = tokio::io::join(tokio::io::stdin(), tokio::io::stdout());
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}
