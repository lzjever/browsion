use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Chrome executable path
    pub chrome_path: PathBuf,

    /// Browser profile list
    #[serde(default)]
    pub profiles: Vec<BrowserProfile>,

    /// Application settings
    #[serde(default)]
    pub settings: AppSettings,

    /// Recently launched profile IDs (most recent first)
    #[serde(default)]
    pub recent_profiles: Vec<String>,

    /// AI configuration for agent
    #[serde(default)]
    pub ai: AIConfig,

    /// Task templates
    #[serde(default)]
    pub templates: Vec<TaskTemplate>,

    /// Scheduled tasks
    #[serde(default)]
    pub scheduled_tasks: Vec<ScheduledTask>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            chrome_path: Self::default_chrome_path(),
            profiles: Vec::new(),
            settings: AppSettings::default(),
            recent_profiles: Vec::new(),
            ai: AIConfig::default(),
            templates: Vec::new(),
            scheduled_tasks: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Get default Chrome path based on platform
    fn default_chrome_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            PathBuf::from("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe")
        }
        #[cfg(target_os = "macos")]
        {
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
        }
        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/usr/bin/google-chrome")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    /// Unique identifier
    pub id: String,

    /// Display name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// User data directory
    pub user_data_dir: PathBuf,

    /// Proxy server (e.g., "http://192.168.0.220:8889")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_server: Option<String>,

    /// Language
    #[serde(default = "default_lang")]
    pub lang: String,

    /// Timezone (e.g., "America/Los_Angeles")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// Fingerprint ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,

    /// UI color tag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Custom launch arguments
    #[serde(default)]
    pub custom_args: Vec<String>,

    /// Tags for categorization and filtering
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Auto start on system boot
    #[serde(default)]
    pub auto_start: bool,

    /// Minimize to tray on close
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            minimize_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub profile_id: String,
    pub pid: u32,
    pub launched_at: u64, // Unix timestamp
}

fn default_lang() -> String {
    "en-US".to_string()
}

fn default_true() -> bool {
    true
}

// ==================== AI Configuration ====================

/// AI provider configuration
/// API type for the provider
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiType {
    #[default]
    Openai,
    Anthropic,
    Ollama,
}

/// AI Provider configuration
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

/// AI configuration for the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// Default LLM model to use (format: "provider_id:model_name")
    #[serde(default)]
    pub default_llm: Option<String>,

    /// Default VLM model for visual tasks (format: "provider_id:model_name")
    #[serde(default)]
    pub default_vlm: Option<String>,

    /// Enable automatic VLM escalation when stuck
    #[serde(default = "default_true")]
    pub escalation_enabled: bool,

    /// Maximum retries before escalating
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Task timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// AI providers configuration (key = provider id, value = config)
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

fn default_max_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    300
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

impl Default for TaskTemplate {
    fn default() -> Self {
        Self::new("New Template".to_string(), String::new())
    }
}

/// Scheduled task for automated execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Unique task ID
    pub id: String,
    /// Task name
    pub name: String,
    /// Task description
    pub task: String,
    /// Profile IDs to run on
    pub profile_ids: Vec<String>,
    /// Schedule configuration
    pub schedule: ScheduleConfig,
    /// Whether the schedule is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Start URL (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_url: Option<String>,
    /// Use headless mode
    #[serde(default)]
    pub headless: bool,
    /// Creation timestamp
    pub created_at: u64,
    /// Last run timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<u64>,
    /// Next scheduled run timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<u64>,
    /// Run count
    #[serde(default)]
    pub run_count: u32,
}

fn default_enabled() -> bool {
    true
}

/// Schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleConfig {
    /// Run once at specific time
    Once { datetime: u64 },
    /// Run every N minutes
    Interval { minutes: u32 },
    /// Run daily at specific time
    Daily { hour: u32, minute: u32 },
    /// Run weekly on specific day and time
    Weekly {
        day_of_week: u32,
        hour: u32,
        minute: u32,
    },
    /// Cron expression (advanced)
    Cron { expression: String },
}

impl ScheduledTask {
    /// Create a new scheduled task
    pub fn new(
        name: String,
        task: String,
        profile_ids: Vec<String>,
        schedule: ScheduleConfig,
    ) -> Self {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let next_run = Self::calculate_next_run(&schedule, created_at);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            task,
            profile_ids,
            schedule,
            enabled: true,
            start_url: None,
            headless: false,
            created_at,
            last_run: None,
            next_run,
            run_count: 0,
        }
    }

    /// Calculate the next run time based on schedule
    pub fn calculate_next_run(schedule: &ScheduleConfig, from_time: u64) -> Option<u64> {
        match schedule {
            ScheduleConfig::Once { datetime } => {
                if *datetime > from_time {
                    Some(*datetime)
                } else {
                    None
                }
            }
            ScheduleConfig::Interval { minutes } => Some(from_time + (*minutes as u64 * 60)),
            ScheduleConfig::Daily { hour, minute } => {
                // Calculate next occurrence of this time
                let now = chrono::DateTime::from_timestamp(from_time as i64, 0)
                    .unwrap_or_else(chrono::Utc::now);
                let next_date = now
                    .date_naive()
                    .and_hms_opt(*hour, *minute, 0)
                    .unwrap()
                    .and_utc();
                let next = if next_date <= now {
                    next_date + chrono::Duration::days(1)
                } else {
                    next_date
                };
                Some(next.timestamp() as u64)
            }
            ScheduleConfig::Weekly {
                day_of_week,
                hour,
                minute,
            } => {
                let now = chrono::DateTime::from_timestamp(from_time as i64, 0)
                    .unwrap_or_else(chrono::Utc::now);
                let days_ahead =
                    (*day_of_week as i32 - now.weekday().num_days_from_monday() as i32 + 7) % 7;
                let next = now
                    .date_naive()
                    .and_hms_opt(*hour, *minute, 0)
                    .unwrap()
                    .and_utc()
                    + chrono::Duration::days(days_ahead as i64);
                let next = if next <= now {
                    next + chrono::Duration::weeks(1)
                } else {
                    next
                };
                Some(next.timestamp() as u64)
            }
            ScheduleConfig::Cron { .. } => {
                // Simplified - just run every hour for cron
                Some(from_time + 3600)
            }
        }
    }

    /// Update next_run after a run
    pub fn update_after_run(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_run = Some(now);
        self.run_count += 1;
        self.next_run = Self::calculate_next_run(&self.schedule, now);
    }
}
