use crate::agent::types::{
    AIConfig, ApiType, LLMDecision, LLMMessage, LLMResponse, ProviderConfig, TokenUsage,
};
use async_trait::async_trait;
use reqwest::Client;

/// LLM Provider trait
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a chat completion request
    async fn complete(&self, messages: &[LLMMessage]) -> Result<LLMResponse, String>;

    /// Send a chat completion request with image
    async fn complete_with_images(
        &self,
        messages: &[LLMMessage],
        images: &[String],
    ) -> Result<LLMResponse, String>;

    /// Get provider name
    fn name(&self) -> &str;

    /// Get model name
    fn model(&self) -> &str;
}

/// OpenAI-compatible provider (OpenAI, Azure, custom endpoints, etc.)
pub struct OpenAIProvider {
    client: Client,
    config: ProviderConfig,
    model: String,
    provider_id: String,
}

impl OpenAIProvider {
    pub fn new(provider_id: String, config: ProviderConfig, model: String) -> Self {
        Self {
            client: Client::new(),
            config,
            model,
            provider_id,
        }
    }

    fn build_messages(&self, messages: &[LLMMessage], images: &[String]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|msg| {
                if !images.is_empty() && msg.role == "user" {
                    // Build multimodal content
                    let mut content_parts = vec![serde_json::json!({
                        "type": "text",
                        "text": msg.content
                    })];

                    for img in images {
                        content_parts.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/png;base64,{}", img)
                            }
                        }));
                    }

                    serde_json::json!({
                        "role": msg.role,
                        "content": content_parts
                    })
                } else {
                    serde_json::json!({
                        "role": msg.role,
                        "content": msg.content
                    })
                }
            })
            .collect()
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn complete(&self, messages: &[LLMMessage]) -> Result<LLMResponse, String> {
        self.complete_with_images(messages, &[]).await
    }

    async fn complete_with_images(
        &self,
        messages: &[LLMMessage],
        images: &[String],
    ) -> Result<LLMResponse, String> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "messages": self.build_messages(messages, images),
            "max_tokens": 4096,
            "temperature": 0.1
        });

        let mut request = self.client.post(&url).json(&body);

        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("OpenAI API error: {}", error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage = TokenUsage {
            prompt_tokens: json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(LLMResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &str {
        &self.provider_id
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// Anthropic provider
pub struct AnthropicProvider {
    client: Client,
    config: ProviderConfig,
    model: String,
    provider_id: String,
}

impl AnthropicProvider {
    pub fn new(provider_id: String, config: ProviderConfig, model: String) -> Self {
        Self {
            client: Client::new(),
            config,
            model,
            provider_id,
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn complete(&self, messages: &[LLMMessage]) -> Result<LLMResponse, String> {
        self.complete_with_images(messages, &[]).await
    }

    async fn complete_with_images(
        &self,
        messages: &[LLMMessage],
        images: &[String],
    ) -> Result<LLMResponse, String> {
        let url = format!("{}/v1/messages", self.config.base_url);

        // Convert messages to Anthropic format
        let mut anthropic_messages = Vec::new();
        let mut system_prompt = String::new();

        for msg in messages {
            if msg.role == "system" {
                system_prompt = msg.content.clone();
            } else {
                let mut content_parts: Vec<serde_json::Value> = vec![serde_json::json!({
                    "type": "text",
                    "text": msg.content
                })];

                // Add images to the last user message
                if msg.role == "user" && !images.is_empty() {
                    for img in images {
                        content_parts.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": img
                            }
                        }));
                    }
                }

                anthropic_messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": content_parts
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": anthropic_messages,
            "max_tokens": 4096,
        });

        if !system_prompt.is_empty() {
            body["system"] = serde_json::Value::String(system_prompt);
        }

        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or_else(|| "Anthropic API key required".to_string())?;

        let response = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("Anthropic API error: {}", error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let content = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage = TokenUsage {
            prompt_tokens: json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
                + json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(LLMResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &str {
        &self.provider_id
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// Ollama provider (local)
pub struct OllamaProvider {
    client: Client,
    config: ProviderConfig,
    model: String,
    provider_id: String,
}

impl OllamaProvider {
    pub fn new(provider_id: String, config: ProviderConfig, model: String) -> Self {
        Self {
            client: Client::new(),
            config,
            model,
            provider_id,
        }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn complete(&self, messages: &[LLMMessage]) -> Result<LLMResponse, String> {
        self.complete_with_images(messages, &[]).await
    }

    async fn complete_with_images(
        &self,
        messages: &[LLMMessage],
        images: &[String],
    ) -> Result<LLMResponse, String> {
        let url = format!("{}/api/chat", self.config.base_url);

        let ollama_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                let mut msg_json = serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                });

                if msg.role == "user" && !images.is_empty() {
                    msg_json["images"] = serde_json::to_value(images).unwrap();
                }

                msg_json
            })
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "messages": ollama_messages,
            "stream": false
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("Ollama API error: {}", error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let content = json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Ollama doesn't provide token counts in the same way
        let usage = TokenUsage {
            prompt_tokens: json["prompt_eval_count"].as_u64().unwrap_or(0) as u32,
            completion_tokens: json["eval_count"].as_u64().unwrap_or(0) as u32,
            total_tokens: 0,
        };

        Ok(LLMResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &str {
        &self.provider_id
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// LLM Client factory
pub struct LLMClient {
    config: AIConfig,
}

impl LLMClient {
    pub fn new(config: AIConfig) -> Self {
        Self { config }
    }

    /// Create a provider instance with the specified model
    fn create_provider(
        &self,
        provider_id: &str,
        model: &str,
    ) -> Result<Box<dyn LLMProvider>, String> {
        let provider_config = self
            .config
            .providers
            .get(provider_id)
            .ok_or_else(|| format!("Provider '{}' not configured", provider_id))?;

        let provider: Box<dyn LLMProvider> = match provider_config.api_type {
            ApiType::Openai => Box::new(OpenAIProvider::new(
                provider_id.to_string(),
                provider_config.clone(),
                model.to_string(),
            )),
            ApiType::Anthropic => Box::new(AnthropicProvider::new(
                provider_id.to_string(),
                provider_config.clone(),
                model.to_string(),
            )),
            ApiType::Ollama => Box::new(OllamaProvider::new(
                provider_id.to_string(),
                provider_config.clone(),
                model.to_string(),
            )),
        };

        Ok(provider)
    }

    /// Get the default LLM provider (format: "provider_id:model_name")
    pub fn get_default_llm(&self) -> Result<Box<dyn LLMProvider>, String> {
        let selection = self.config.default_llm.as_ref().ok_or_else(|| {
            "No default LLM configured. Please set one in Settings > AI Configuration.".to_string()
        })?;

        let parts: Vec<&str> = selection.split(':').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid default_llm format '{}'. Expected 'provider_id:model_name'",
                selection
            ));
        }

        self.create_provider(parts[0], parts[1])
    }

    /// Get the default VLM provider (format: "provider_id:model_name")
    pub fn get_default_vlm(&self) -> Result<Box<dyn LLMProvider>, String> {
        let selection = self.config.default_vlm.as_ref().ok_or_else(|| {
            "No default VLM configured. Please set one in Settings > AI Configuration.".to_string()
        })?;

        let parts: Vec<&str> = selection.split(':').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid default_vlm format '{}'. Expected 'provider_id:model_name'",
                selection
            ));
        }

        self.create_provider(parts[0], parts[1])
    }

    /// Parse LLM decision from response
    pub fn parse_decision(content: &str) -> Result<LLMDecision, String> {
        // Try to extract JSON from the response
        let json_start = content.find('{');
        let json_end = content.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &content[start..=end];
            if let Ok(decision) = serde_json::from_str::<LLMDecision>(json_str) {
                return Ok(decision);
            }
        }

        // If JSON parsing fails, try to infer from text
        // This is a fallback for models that don't output clean JSON
        let action = if content.contains("navigate") || content.contains("go to") {
            // Try to extract URL
            let url = extract_url(content).unwrap_or_else(|| "about:blank".to_string());
            crate::agent::types::AgentAction::Navigate { url }
        } else if content.contains("click") {
            crate::agent::types::AgentAction::Click {
                selector: "button".to_string(),
            }
        } else if content.contains("type") || content.contains("enter") {
            crate::agent::types::AgentAction::Type {
                selector: "input".to_string(),
                text: "".to_string(),
            }
        } else {
            crate::agent::types::AgentAction::None
        };

        Ok(LLMDecision {
            action,
            reasoning: content.to_string(),
            is_complete: content.contains("complete")
                || content.contains("done")
                || content.contains("finished"),
            result: None,
        })
    }
}

/// Extract URL from text
fn extract_url(text: &str) -> Option<String> {
    // Simple URL extraction
    let words = text.split_whitespace();
    for word in words {
        if word.starts_with("http://") || word.starts_with("https://") {
            return Some(
                word.trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != ':' && c != '/' && c != '.' && c != '-'
                })
                .to_string(),
            );
        }
    }
    None
}

/// Build system prompt for the agent
pub fn build_system_prompt() -> String {
    r##"You are a browser automation agent. Your task is to help users perform actions in a web browser.

You will receive information about the current page state (URL, title, interactive elements, forms, links).

You must respond with a JSON object containing:
- action: The action to take (see below)
- reasoning: Why you chose this action
- is_complete: Whether the task is complete
- result: (optional) Any data to extract if task is complete

Available actions:
1. Navigate to URL: {"type": "navigate", "url": "https://example.com"}
2. Click element: {"type": "click", "selector": "#button-id"}
3. Type text: {"type": "type", "selector": "#input-id", "text": "hello"}
4. Press key: {"type": "press_key", "key": "Enter"}
5. Scroll page: {"type": "scroll", "direction": "down", "amount": 300}
6. Wait: {"type": "wait", "duration_ms": 1000}
7. Extract data: {"type": "extract", "selectors": {"name": "#name"}}
8. Go back: {"type": "go_back"}
9. No action: {"type": "none"}

Selectors should use CSS selectors (e.g., #id, .class, tag, [attr=value]).

Be concise and focused. Complete the task efficiently. If the task is done, set is_complete to true."##
        .to_string()
}

/// Build user prompt with DOM context
pub fn build_context_prompt(task: &str, dom_context: &crate::agent::types::DOMContext) -> String {
    let elements_str = dom_context
        .elements
        .iter()
        .take(50) // Limit elements to avoid token limits
        .map(|el| {
            let mut parts = vec![format!("- {}", el.selector)];
            if let Some(text) = &el.text {
                if !text.is_empty() && text.len() < 100 {
                    parts.push(format!("  text: \"{}\"", text));
                }
            }
            if let Some(placeholder) = &el.placeholder {
                parts.push(format!("  placeholder: \"{}\"", placeholder));
            }
            if let Some(aria_label) = &el.aria_label {
                parts.push(format!("  aria-label: \"{}\"", aria_label));
            }
            parts.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"Task: {}

Current page:
- URL: {}
- Title: {}

Interactive elements:
{}

What action should I take next? Respond with JSON only."#,
        task,
        dom_context.url,
        dom_context.title.as_deref().unwrap_or("N/A"),
        elements_str
    )
}

/// Test an AI provider connection
pub async fn test_provider(config: &ProviderConfig, model: &str) -> Result<String, String> {
    // Create a simple test request
    let test_message = vec![LLMMessage {
        role: "user".to_string(),
        content: "Say 'ok' if you can read this.".to_string(),
        images: None,
    }];

    let provider_id = "test".to_string();
    let model = if model.is_empty() {
        config
            .models
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_string())
    } else {
        model.to_string()
    };

    let provider: Box<dyn LLMProvider> = match config.api_type {
        ApiType::Openai => Box::new(OpenAIProvider::new(provider_id, config.clone(), model)),
        ApiType::Anthropic => Box::new(AnthropicProvider::new(provider_id, config.clone(), model)),
        ApiType::Ollama => Box::new(OllamaProvider::new(provider_id, config.clone(), model)),
    };

    match provider.complete(&test_message).await {
        Ok(response) => Ok(format!(
            "Connection successful! Model: {}, Tokens: {}",
            response.model, response.usage.total_tokens
        )),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decision_json() {
        let content = r#"{"action": {"type": "navigate", "url": "https://example.com"}, "reasoning": "Need to go to example.com", "is_complete": false}"#;
        let decision = LLMClient::parse_decision(content).unwrap();
        assert!(!decision.is_complete);
    }

    #[test]
    fn test_parse_decision_text() {
        let content = "I should navigate to https://google.com to search";
        let decision = LLMClient::parse_decision(content).unwrap();
        assert!(!decision.is_complete);
    }

    #[test]
    fn test_parse_decision_complete() {
        // Test various completion indicators (case-sensitive: must be lowercase)
        let test_cases = vec![
            ("The task is complete now.", true),
            ("I am done with this task.", true),
            ("finished extracting data.", true),
            ("Still working on it.", false),
            ("Need to click next.", false),
        ];

        for (content, expected_complete) in test_cases {
            let decision = LLMClient::parse_decision(content).unwrap();
            assert_eq!(
                decision.is_complete, expected_complete,
                "Failed for: {}",
                content
            );
        }
    }

    #[test]
    fn test_extract_url() {
        let text = "Navigate to https://example.com/page";
        let url = extract_url(text);
        assert_eq!(url, Some("https://example.com/page".to_string()));

        let text_no_url = "Click the button";
        let url = extract_url(text_no_url);
        assert_eq!(url, None);
    }

    #[test]
    fn test_build_messages_simple() {
        let config = ProviderConfig {
            name: "test".to_string(),
            api_type: ApiType::Openai,
            base_url: "https://api.example.com".to_string(),
            api_key: Some("test-key".to_string()),
            models: vec!["gpt-4".to_string()],
        };
        let provider = OpenAIProvider::new("test".to_string(), config, "gpt-4".to_string());

        let messages = vec![LLMMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            images: None,
        }];

        let built = provider.build_messages(&messages, &[]);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0]["role"], "user");
        assert_eq!(built[0]["content"], "Hello");
    }

    #[test]
    fn test_build_messages_with_images() {
        let config = ProviderConfig {
            name: "test".to_string(),
            api_type: ApiType::Openai,
            base_url: "https://api.example.com".to_string(),
            api_key: Some("test-key".to_string()),
            models: vec!["gpt-4".to_string()],
        };
        let provider = OpenAIProvider::new("test".to_string(), config, "gpt-4".to_string());

        let messages = vec![LLMMessage {
            role: "user".to_string(),
            content: "What's in this image?".to_string(),
            images: None,
        }];

        let images = vec!["base64imagedata".to_string()];
        let built = provider.build_messages(&messages, &images);

        // Content should be an array with text and image parts
        assert!(built[0]["content"].is_array());
        let content = built[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2); // text + image
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");
    }

    #[test]
    fn test_llm_client_no_default() {
        let config = AIConfig {
            default_llm: None,
            default_vlm: None,
            escalation_enabled: true,
            max_retries: 3,
            timeout_seconds: 300,
            providers: std::collections::HashMap::new(),
        };

        let client = LLMClient::new(config);

        // Should error when no default LLM is set
        let result = client.get_default_llm();
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("No default LLM configured"));
        }
    }
}
