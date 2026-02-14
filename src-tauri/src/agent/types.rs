use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// API type for the provider
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApiType {
    #[default]
    Openai,
    Anthropic,
    Ollama,
}

/// AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// Default LLM model to use (format: "provider_id:model_name")
    pub default_llm: Option<String>,
    /// Default VLM model for visual tasks (format: "provider_id:model_name")
    pub default_vlm: Option<String>,
    /// Enable automatic VLM escalation when stuck
    #[serde(default = "default_escalation")]
    pub escalation_enabled: bool,
    /// Maximum retries before escalating
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Task timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// AI providers configuration
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            default_llm: None,
            default_vlm: None,
            escalation_enabled: true,
            max_retries: 3,
            timeout_seconds: 300,
            providers: HashMap::new(),
        }
    }
}

fn default_escalation() -> bool {
    true
}

fn default_max_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    300
}

/// Provider-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Display name for this provider
    pub name: String,
    /// API type (determines request format)
    #[serde(default)]
    pub api_type: ApiType,
    /// Base URL for the API
    pub base_url: String,
    /// API key (optional for local providers like Ollama)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Available models for this provider
    #[serde(default)]
    pub models: Vec<String>,
}

// Conversion from config types to agent types
impl From<crate::config::ProviderConfig> for ProviderConfig {
    fn from(config: crate::config::ProviderConfig) -> Self {
        Self {
            name: config.name,
            api_type: match config.api_type {
                crate::config::ApiType::Openai => ApiType::Openai,
                crate::config::ApiType::Anthropic => ApiType::Anthropic,
                crate::config::ApiType::Ollama => ApiType::Ollama,
            },
            base_url: config.base_url,
            api_key: config.api_key,
            models: config.models,
        }
    }
}

impl From<crate::config::AIConfig> for AIConfig {
    fn from(config: crate::config::AIConfig) -> Self {
        Self {
            default_llm: config.default_llm,
            default_vlm: config.default_vlm,
            escalation_enabled: config.escalation_enabled,
            max_retries: config.max_retries,
            timeout_seconds: config.timeout_seconds,
            providers: config
                .providers
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

/// Agent run options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOptions {
    /// Run in headless mode
    #[serde(default)]
    pub headless: bool,
    /// Custom timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// Maximum steps before stopping
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    /// Start URL (optional, overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            headless: false,
            timeout: None,
            max_steps: 50,
            start_url: None,
        }
    }
}

fn default_max_steps() -> u32 {
    50
}

/// Agent status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    /// Agent is initializing
    Initializing,
    /// Agent is running
    Running,
    /// Agent is paused
    Paused,
    /// Agent completed successfully
    Completed,
    /// Agent failed with error
    Failed,
    /// Agent was stopped by user
    Stopped,
}

/// Current mode of the agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    /// Using text-based LLM
    Llm,
    /// Using vision model (VLM)
    Vlm,
}

/// A single step in the agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    /// Step number
    pub step: u32,
    /// Current URL
    pub url: String,
    /// Action being performed
    pub action: String,
    /// Current mode (LLM/VLM)
    pub mode: AgentMode,
    /// Timestamp
    pub timestamp: u64,
    /// Optional screenshot (base64)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

/// Agent progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProgress {
    /// Agent ID
    pub agent_id: String,
    /// Current status
    pub status: AgentStatus,
    /// Current step info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step: Option<AgentStep>,
    /// Total steps completed
    pub steps_completed: u32,
    /// Current mode
    pub mode: AgentMode,
    /// Accumulated cost in USD
    pub cost: f64,
    /// Progress message
    pub message: String,
    /// Result data (when completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AgentResult>,
    /// Error message (when failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch execution progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    /// Batch ID
    pub batch_id: String,
    /// Total profiles in batch
    pub total: u32,
    /// Completed count
    pub completed: u32,
    /// Failed count
    pub failed: u32,
    /// Current running profile ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_profile: Option<String>,
    /// Individual agent IDs mapped to profile IDs
    pub agents: std::collections::HashMap<String, String>,
    /// Results for each profile
    pub results: std::collections::HashMap<String, AgentResult>,
    /// Errors for each profile
    pub errors: std::collections::HashMap<String, String>,
    /// Total cost across all agents
    pub total_cost: f64,
}

/// Agent execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Summary of what was accomplished
    pub summary: String,
    /// Extracted data (if any)
    #[serde(default)]
    pub data: serde_json::Value,
    /// Final URL
    pub final_url: String,
    /// Total steps taken
    pub total_steps: u32,
    /// Total cost in USD
    pub total_cost: f64,
    /// Execution time in seconds
    pub duration_seconds: u64,
}

/// LLM action decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMDecision {
    /// Action to take
    pub action: AgentAction,
    /// Reasoning for this action
    pub reasoning: String,
    /// Whether task is complete
    pub is_complete: bool,
    /// Result to extract (if complete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Actions the agent can take
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentAction {
    /// Navigate to a URL
    Navigate { url: String },
    /// Click an element
    Click { selector: String },
    /// Type text into an element
    Type { selector: String, text: String },
    /// Press a key
    PressKey { key: String },
    /// Scroll the page
    Scroll {
        direction: ScrollDirection,
        amount: u32,
    },
    /// Wait for an element or time
    Wait {
        duration_ms: Option<u64>,
        selector: Option<String>,
    },
    /// Extract data from the page
    Extract { selectors: HashMap<String, String> },
    /// Take a screenshot
    Screenshot,
    /// Go back in history
    GoBack,
    /// No action needed (task complete or waiting)
    None,
}

/// Scroll direction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// DOM element info for LLM context
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

/// Simplified DOM structure for LLM
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

/// LLM message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

/// LLM request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRequest {
    pub model: String,
    pub messages: Vec<LLMMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Agent session state
#[derive(Debug, Clone)]
pub struct AgentSession {
    /// Unique session ID
    pub id: String,
    /// Profile ID being used
    pub profile_id: String,
    /// Original task description
    pub task: String,
    /// Agent options
    pub options: AgentOptions,
    /// Current status
    pub status: AgentStatus,
    /// Current mode
    pub mode: AgentMode,
    /// Steps completed
    pub steps_completed: u32,
    /// Accumulated cost
    pub cost: f64,
    /// Conversation history
    pub history: Vec<LLMMessage>,
    /// Start time
    pub started_at: std::time::Instant,
    /// Stop flag
    pub should_stop: bool,
    /// Pause flag
    pub is_paused: bool,
}

impl AgentSession {
    pub fn new(id: String, profile_id: String, task: String, options: AgentOptions) -> Self {
        Self {
            id,
            profile_id,
            task,
            options,
            status: AgentStatus::Initializing,
            mode: AgentMode::Llm,
            steps_completed: 0,
            cost: 0.0,
            history: Vec::new(),
            started_at: std::time::Instant::now(),
            should_stop: false,
            is_paused: false,
        }
    }
}

/// Task template for reusable automation tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    /// Unique template ID
    pub id: String,
    /// Template name
    pub name: String,
    /// Template description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Task description (the prompt)
    pub task: String,
    /// Default start URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    /// Whether to use headless mode by default
    #[serde(default)]
    pub headless: bool,
    /// Template category
    #[serde(default)]
    pub category: TemplateCategory,
    /// Creation timestamp
    pub created_at: u64,
    /// Last used timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<u64>,
    /// Usage count
    #[serde(default)]
    pub usage_count: u32,
}

/// Template category for organization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TemplateCategory {
    #[default]
    General,
    Login,
    Scraping,
    Form,
    Navigation,
    Screenshot,
    Custom,
}

impl TaskTemplate {
    /// Create a new template
    pub fn new(name: String, task: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: None,
            task,
            start_url: None,
            headless: false,
            category: TemplateCategory::default(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_used: None,
            usage_count: 0,
        }
    }
}
