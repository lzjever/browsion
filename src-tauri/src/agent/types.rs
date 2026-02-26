//! Types used by the CDP client (for DOM context, cookies, tabs, AX tree, etc.).

use serde::{Deserialize, Serialize};

/// DOM element info for context (e.g. for MCP tools).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DOMElement {
    /// Tag name
    pub tag: String,
    /// Element ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// CSS classes
    #[serde(default)]
    pub classes: Vec<String>,
    /// CSS selector
    pub selector: String,
    /// Visible text content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Input type (for inputs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    /// Placeholder text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// aria-label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aria_label: Option<String>,
    /// Is element visible
    pub visible: bool,
    /// Is element clickable
    pub clickable: bool,
}

/// Simplified DOM structure (for MCP / automation context).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DOMContext {
    /// Current URL
    pub url: String,
    /// Page title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Interactive elements
    pub elements: Vec<DOMElement>,
    /// Form elements (inputs, buttons, selects)
    pub forms: Vec<DOMElement>,
    /// Links
    pub links: Vec<DOMElement>,
}

/// Browser tab (CDP Target).
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

/// Browser cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    #[serde(rename = "httpOnly")]
    pub http_only: bool,
    #[serde(default)]
    pub expires: f64,
}

/// A node in the Accessibility Tree, filtered and simplified for AI agent use.
/// Each interactive node gets a stable `ref_id` (e.g. "e1", "e2") that can be
/// passed to `click_ref`, `type_ref`, `focus_ref`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXNode {
    /// Stable element reference for use with click_ref / type_ref / focus_ref
    pub ref_id: String,
    /// ARIA role (e.g. "button", "link", "textbox", "heading")
    pub role: String,
    /// Accessible name (label text, aria-label, etc.)
    pub name: String,
    /// Accessible description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the element currently has focus
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
    /// Whether the element is disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// Current value (for inputs, selects, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Checked state for checkboxes / radios ("true" | "false" | "mixed")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<String>,
    /// Internal: CDP backend DOM node ID used to resolve element coordinates.
    /// Not serialized in JSON output.
    #[serde(skip)]
    pub backend_node_id: Option<i64>,
}

/// Combined page state: AX tree + basic page info.
/// Returned by `get_page_state` for one-shot AI context gathering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageState {
    /// Current URL
    pub url: String,
    /// Page title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Filtered accessibility tree (~200-400 tokens for typical pages)
    pub ax_tree: Vec<AXNode>,
    /// Total number of AX nodes returned (after filtering)
    pub element_count: usize,
}
