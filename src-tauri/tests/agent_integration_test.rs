// Integration tests for the agent system

#[cfg(test)]
mod agent_integration_tests {
    use browsion_lib::agent::llm::{build_context_prompt, build_system_prompt, LLMClient};
    use browsion_lib::agent::types::{
        AIConfig, AgentAction, ApiType, DOMContext, DOMElement, LLMDecision, ProviderConfig,
    };
    use std::collections::HashMap;

    /// Test that LLM decision parsing handles various formats
    #[test]
    fn test_llm_decision_parsing() {
        // Valid JSON decision
        let valid_json = r#"{"action": {"type": "navigate", "url": "https://example.com"}, "reasoning": "Need to navigate", "is_complete": false}"#;
        let decision = LLMClient::parse_decision(valid_json).unwrap();
        assert!(!decision.is_complete);
        matches!(decision.action, AgentAction::Navigate { .. });

        // Decision with completion
        let complete_json = r#"{"action": {"type": "none"}, "reasoning": "Task done", "is_complete": true, "result": {"data": "test"}}"#;
        let decision = LLMClient::parse_decision(complete_json).unwrap();
        assert!(decision.is_complete);

        // Click action (note: escape the # in selector)
        let click_json = r##"{"action": {"type": "click", "selector": "#button"}, "reasoning": "Click button", "is_complete": false}"##;
        let decision = LLMClient::parse_decision(click_json).unwrap();
        matches!(decision.action, AgentAction::Click { .. });

        // Type action
        let type_json = r##"{"action": {"type": "type", "selector": "#input", "text": "hello"}, "reasoning": "Type text", "is_complete": false}"##;
        let decision = LLMClient::parse_decision(type_json).unwrap();
        if let AgentAction::Type { selector, text } = decision.action {
            assert_eq!(selector, "#input");
            assert_eq!(text, "hello");
        } else {
            panic!("Expected Type action");
        }
    }

    /// Test system prompt generation
    #[test]
    fn test_system_prompt() {
        let prompt = build_system_prompt();
        assert!(prompt.contains("browser automation"));
        assert!(prompt.contains("JSON"));
        assert!(prompt.contains("action"));
    }

    /// Test context prompt generation
    #[test]
    fn test_context_prompt() {
        let dom_context = DOMContext {
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            elements: vec![DOMElement {
                tag: "button".to_string(),
                id: Some("submit".to_string()),
                classes: vec!["btn".to_string()],
                selector: "#submit".to_string(),
                text: Some("Submit".to_string()),
                input_type: None,
                placeholder: None,
                aria_label: None,
                visible: true,
                clickable: true,
            }],
            forms: vec![],
            links: vec![],
        };

        let prompt = build_context_prompt("Click the submit button", &dom_context);
        assert!(prompt.contains("Click the submit button"));
        assert!(prompt.contains("https://example.com"));
        assert!(prompt.contains("#submit"));
    }

    /// Test AI config validation
    #[test]
    fn test_ai_config_validation() {
        let config = AIConfig {
            default_llm: Some("openai:gpt-4".to_string()),
            default_vlm: None,
            escalation_enabled: true,
            max_retries: 3,
            timeout_seconds: 300,
            providers: HashMap::new(),
        };

        let client = LLMClient::new(config);

        // Should fail because no providers configured
        let result = client.get_default_llm();
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("Provider") || msg.contains("configured"));
        }
    }

    /// Test AI config with provider
    #[test]
    fn test_ai_config_with_provider() {
        let mut providers = HashMap::new();
        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                name: "OpenAI".to_string(),
                api_type: ApiType::Openai,
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: Some("test-key".to_string()),
                models: vec!["gpt-4".to_string()],
            },
        );

        let config = AIConfig {
            default_llm: Some("openai:gpt-4".to_string()),
            default_vlm: None,
            escalation_enabled: true,
            max_retries: 3,
            timeout_seconds: 300,
            providers,
        };

        let client = LLMClient::new(config);

        // Should succeed now
        let result = client.get_default_llm();
        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model(), "gpt-4");
    }

    /// Test action result creation
    #[test]
    fn test_action_result() {
        use browsion_lib::agent::action::ActionResult;

        let success = ActionResult::success("Test success");
        assert!(success.success);
        assert_eq!(success.message, "Test success");

        let failure = ActionResult::failure("Test failure");
        assert!(!failure.success);
        assert_eq!(failure.message, "Test failure");

        let with_data = ActionResult::with_data("Data result", serde_json::json!({"key": "value"}));
        assert!(with_data.success);
        assert!(with_data.data.is_some());
    }

    /// Test DOM element structure
    #[test]
    fn test_dom_element() {
        let element = DOMElement {
            tag: "input".to_string(),
            id: Some("email".to_string()),
            classes: vec!["form-control".to_string(), "required".to_string()],
            selector: "#email".to_string(),
            text: None,
            input_type: Some("email".to_string()),
            placeholder: Some("Enter email".to_string()),
            aria_label: Some("Email address".to_string()),
            visible: true,
            clickable: true,
        };

        assert_eq!(element.tag, "input");
        assert_eq!(element.id, Some("email".to_string()));
        assert_eq!(element.classes.len(), 2);
        assert!(element.visible);
        assert!(element.clickable);
    }

    /// Test DOM context structure
    #[test]
    fn test_dom_context() {
        let context = DOMContext {
            url: "https://test.com/page".to_string(),
            title: Some("Test Page".to_string()),
            elements: vec![],
            forms: vec![],
            links: vec![],
        };

        assert_eq!(context.url, "https://test.com/page");
        assert_eq!(context.title, Some("Test Page".to_string()));
    }

    /// Test action types serialization
    #[test]
    fn test_action_serialization() {
        // Navigate
        let action = AgentAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("navigate"));
        assert!(json.contains("example.com"));

        // Click
        let action = AgentAction::Click {
            selector: "#btn".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("click"));

        // Type
        let action = AgentAction::Type {
            selector: "#input".to_string(),
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("type"));
        assert!(json.contains("hello"));

        // None
        let action = AgentAction::None;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("none"));
    }

    /// Test LLM decision serialization
    #[test]
    fn test_decision_serialization() {
        let decision = LLMDecision {
            action: AgentAction::Navigate {
                url: "https://test.com".to_string(),
            },
            reasoning: "Test reasoning".to_string(),
            is_complete: false,
            result: Some(serde_json::json!({"key": "value"})),
        };

        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("Test reasoning"));
        assert!(json.contains("navigate"));

        let decoded: LLMDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.reasoning, "Test reasoning");
        assert!(!decoded.is_complete);
    }
}
