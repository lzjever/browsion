// E2E test for the agent system with real LLM
// Run with: cargo test --test e2e_agent_test -- --nocapture

use std::collections::HashMap;
use std::path::PathBuf;

#[tokio::test]
async fn test_cdp_navigation() {
    // Initialize logging
    let _ = tracing_subscriber::fmt::try_init();

    // Test basic CDP connection and navigation
    use browsion_lib::agent::cdp::CDPClient;
    use browsion_lib::config::schema::BrowserProfile;

    let chrome_path =
        PathBuf::from("/home/percy/tools/ungoogled-chromium-139.0.7258.154-1-x86_64_linux/chrome");

    if !chrome_path.exists() {
        eprintln!("Chrome not found at {:?}, skipping test", chrome_path);
        return;
    }

    let profile = BrowserProfile {
        id: "test-e2e".to_string(),
        name: "E2E Test".to_string(),
        description: String::new(),
        user_data_dir: std::env::temp_dir().join("browsion-e2e-test"),
        proxy_server: None,
        lang: "en-US".to_string(),
        timezone: None,
        fingerprint: None,
        color: Some("#4A90E2".to_string()),
        custom_args: vec![],
        tags: vec![],
    };

    // Create temp user data dir
    std::fs::create_dir_all(&profile.user_data_dir).ok();

    let mut client = CDPClient::new("test-e2e".to_string());

    // Launch browser (not headless so we can see it)
    println!("Launching Chrome...");
    match client.launch(&chrome_path, &profile, false).await {
        Ok(_) => println!("Chrome launched successfully"),
        Err(e) => {
            eprintln!("Failed to launch Chrome: {}", e);
            panic!("Chrome launch failed");
        }
    }

    // Test navigation
    println!("Navigating to https://www.163.com...");
    match client.navigate("https://www.163.com").await {
        Ok(_) => println!("Navigation successful"),
        Err(e) => {
            eprintln!("Failed to navigate: {}", e);
            let _ = client.close().await;
            panic!("Navigation failed");
        }
    }

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get URL
    match client.get_url().await {
        Ok(url) => println!("Current URL: {}", url),
        Err(e) => eprintln!("Failed to get URL: {}", e),
    }

    // Get title
    match client.get_title().await {
        Ok(title) => println!("Page title: {:?}", title),
        Err(e) => eprintln!("Failed to get title: {}", e),
    }

    // Get DOM context
    println!("Getting DOM context...");
    match client.get_dom_context().await {
        Ok(ctx) => {
            println!("DOM Context - URL: {}, Title: {:?}", ctx.url, ctx.title);
            println!("Found {} interactive elements", ctx.elements.len());
            if !ctx.elements.is_empty() {
                println!("First element: {:?}", ctx.elements[0]);
            }
        }
        Err(e) => {
            eprintln!("Failed to get DOM context: {}", e);
        }
    }

    // Close browser
    println!("Closing browser...");
    match client.close().await {
        Ok(_) => println!("Browser closed"),
        Err(e) => eprintln!("Failed to close browser: {}", e),
    }

    // Cleanup
    std::fs::remove_dir_all(&profile.user_data_dir).ok();
}

#[tokio::test]
async fn test_llm_connection() {
    // Initialize logging
    let _ = tracing_subscriber::fmt::try_init();

    use browsion_lib::agent::llm::LLMClient;
    use browsion_lib::agent::types::{AIConfig, ApiType, ProviderConfig};

    // Load config from file
    let config_path = dirs::config_dir()
        .expect("No config dir")
        .join("browsion")
        .join("config.toml");

    println!("Loading config from {:?}", config_path);

    let config_content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read config: {}", e);
            panic!("Config not found");
        }
    };

    let raw_config: toml::Value = toml::from_str(&config_content).expect("Failed to parse config");

    // Extract AI config
    let ai_section = raw_config.get("ai").expect("No ai section");
    let providers_section = ai_section.get("providers").expect("No providers section");

    // Build AIConfig
    let mut providers = HashMap::new();

    for (provider_id, provider_value) in providers_section.as_table().unwrap() {
        let provider_table = provider_value.as_table().unwrap();

        let api_type = match provider_table.get("api_type").and_then(|v| v.as_str()) {
            Some("openai") => ApiType::Openai,
            Some("anthropic") => ApiType::Anthropic,
            Some("ollama") => ApiType::Ollama,
            _ => ApiType::Openai,
        };

        let config = ProviderConfig {
            name: provider_table
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(provider_id)
                .to_string(),
            api_type,
            base_url: provider_table
                .get("base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            api_key: provider_table
                .get("api_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            models: provider_table
                .get("models")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        };

        providers.insert(provider_id.clone(), config);
    }

    let ai_config = AIConfig {
        default_llm: ai_section
            .get("default_llm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        default_vlm: ai_section
            .get("default_vlm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        escalation_enabled: ai_section
            .get("escalation_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        max_retries: ai_section
            .get("max_retries")
            .and_then(|v| v.as_integer())
            .unwrap_or(3) as u32,
        timeout_seconds: ai_section
            .get("timeout_seconds")
            .and_then(|v| v.as_integer())
            .unwrap_or(300) as u64,
        providers,
    };

    println!(
        "Loaded AI config with {} providers",
        ai_config.providers.len()
    );
    println!("Default LLM: {:?}", ai_config.default_llm);
    println!("Default VLM: {:?}", ai_config.default_vlm);

    // Create LLM client
    let llm_client = LLMClient::new(ai_config.clone());

    // Get default LLM provider
    println!("\nGetting default LLM provider...");
    let provider = match llm_client.get_default_llm() {
        Ok(p) => {
            println!("Provider: {} (model: {})", p.name(), p.model());
            p
        }
        Err(e) => {
            eprintln!("Failed to get provider: {}", e);
            panic!("No LLM provider configured");
        }
    };

    // Test a simple completion
    use browsion_lib::agent::types::LLMMessage;

    let messages = vec![
        LLMMessage {
            role: "system".to_string(),
            content: "You are a browser automation assistant. Respond with JSON only.".to_string(),
            images: None,
        },
        LLMMessage {
            role: "user".to_string(),
            content: r#"I need to navigate to https://www.baidu.com. Respond with JSON like: {"action": {"type": "navigate", "url": "..."}, "reasoning": "...", "is_complete": false}"#.to_string(),
            images: None,
        },
    ];

    println!("\nSending test message to LLM...");
    match provider.complete(&messages).await {
        Ok(response) => {
            println!("LLM Response: {}", response.content);
            println!(
                "Usage: {} prompt + {} completion = {} total tokens",
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                response.usage.total_tokens
            );

            // Parse the decision
            let decision = LLMClient::parse_decision(&response.content);
            println!("Parsed decision: {:?}", decision);
        }
        Err(e) => {
            eprintln!("LLM call failed: {}", e);
            panic!("LLM connection failed");
        }
    }
}

#[tokio::test]
async fn test_full_agent_flow() {
    // Initialize logging
    let _ = tracing_subscriber::fmt::try_init();

    use browsion_lib::agent::action::execute_action;
    use browsion_lib::agent::cdp::CDPClient;
    use browsion_lib::agent::llm::{build_context_prompt, build_system_prompt, LLMClient};
    use browsion_lib::agent::types::{AIConfig, ApiType, LLMMessage, ProviderConfig};
    use browsion_lib::config::schema::BrowserProfile;

    // Load config
    let config_path = dirs::config_dir()
        .expect("No config dir")
        .join("browsion")
        .join("config.toml");

    let config_content = match std::fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read config: {}", e);
            return;
        }
    };

    let raw_config: toml::Value = toml::from_str(&config_content).expect("Failed to parse config");
    let ai_section = raw_config.get("ai").expect("No ai section");
    let providers_section = ai_section.get("providers").expect("No providers section");

    let mut providers = HashMap::new();
    for (provider_id, provider_value) in providers_section.as_table().unwrap() {
        let provider_table = provider_value.as_table().unwrap();
        let api_type = match provider_table.get("api_type").and_then(|v| v.as_str()) {
            Some("openai") => ApiType::Openai,
            Some("anthropic") => ApiType::Anthropic,
            Some("ollama") => ApiType::Ollama,
            _ => ApiType::Openai,
        };

        providers.insert(
            provider_id.clone(),
            ProviderConfig {
                name: provider_table
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(provider_id)
                    .to_string(),
                api_type,
                base_url: provider_table
                    .get("base_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                api_key: provider_table
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                models: provider_table
                    .get("models")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            },
        );
    }

    let ai_config = AIConfig {
        default_llm: ai_section
            .get("default_llm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        default_vlm: ai_section
            .get("default_vlm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        escalation_enabled: true,
        max_retries: 3,
        timeout_seconds: 300,
        providers,
    };

    // Chrome setup
    let chrome_path =
        PathBuf::from("/home/percy/tools/ungoogled-chromium-139.0.7258.154-1-x86_64_linux/chrome");
    if !chrome_path.exists() {
        eprintln!("Chrome not found, skipping test");
        return;
    }

    let profile = BrowserProfile {
        id: "test-e2e-full".to_string(),
        name: "E2E Full Test".to_string(),
        description: String::new(),
        user_data_dir: std::env::temp_dir().join("browsion-e2e-full-test"),
        proxy_server: Some("http://192.168.0.220:8889".to_string()), // Use proxy
        lang: "en-US".to_string(),
        timezone: Some("America/Los_Angeles".to_string()),
        fingerprint: Some("9999".to_string()),
        color: Some("#4A90E2".to_string()),
        custom_args: vec![],
        tags: vec![],
    };

    std::fs::create_dir_all(&profile.user_data_dir).ok();

    // Launch browser
    println!("\n=== Starting Full Agent Flow Test ===\n");
    println!("Launching Chrome...");
    let mut client = CDPClient::new("test-e2e-full".to_string());
    if let Err(e) = client.launch(&chrome_path, &profile, false).await {
        eprintln!("Failed to launch: {}", e);
        return;
    }
    println!("Chrome launched!");

    // Create LLM client
    let llm_client = LLMClient::new(ai_config);
    let provider = match llm_client.get_default_llm() {
        Ok(p) => {
            println!("Using LLM: {} ({})", p.name(), p.model());
            p
        }
        Err(e) => {
            eprintln!("Failed to get LLM: {}", e);
            let _ = client.close().await;
            return;
        }
    };

    // Task to execute
    let task = "Navigate to https://www.baidu.com";
    println!("\nTask: {}", task);

    // Build messages
    let mut messages = vec![
        LLMMessage {
            role: "system".to_string(),
            content: build_system_prompt(),
            images: None,
        },
        LLMMessage {
            role: "user".to_string(),
            content: format!("My task is: {}", task),
            images: None,
        },
    ];

    // Run a few steps
    for step in 1..=3 {
        println!("\n--- Step {} ---", step);

        // Get DOM context
        let dom_context = match client.get_dom_context().await {
            Ok(ctx) => {
                println!("Current URL: {}", ctx.url);
                ctx
            }
            Err(e) => {
                eprintln!("Failed to get DOM: {}", e);
                continue;
            }
        };

        // Check if already done
        if dom_context.url.contains("baidu.com") {
            println!("SUCCESS! Already on baidu.com");
            break;
        }

        // Build context message
        let context_msg = build_context_prompt(task, &dom_context);
        messages.push(LLMMessage {
            role: "user".to_string(),
            content: context_msg,
            images: None,
        });

        // Get LLM decision
        println!("Calling LLM...");
        let response = match provider.complete(&messages).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("LLM failed: {}", e);
                continue;
            }
        };

        println!(
            "LLM response: {}",
            response.content.chars().take(200).collect::<String>()
        );

        // Add to history
        messages.push(LLMMessage {
            role: "assistant".to_string(),
            content: response.content.clone(),
            images: None,
        });

        // Parse decision
        let decision = match LLMClient::parse_decision(&response.content) {
            Ok(d) => {
                println!(
                    "Decision: action={:?}, is_complete={}",
                    d.action, d.is_complete
                );
                d
            }
            Err(e) => {
                eprintln!("Failed to parse: {}", e);
                continue;
            }
        };

        if decision.is_complete {
            println!("Task marked as complete!");
            break;
        }

        // Execute action
        println!("Executing action: {:?}", decision.action);
        let result = execute_action(&client, &decision.action).await;
        println!(
            "Result: success={}, message={}",
            result.as_ref().map(|r| r.success).unwrap_or(false),
            result
                .as_ref()
                .map(|r| r.message.as_str())
                .unwrap_or("error")
        );

        // Wait a bit
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    // Final state
    println!("\n=== Final State ===");
    match client.get_url().await {
        Ok(url) => {
            println!("Final URL: {}", url);
            if url.contains("baidu.com") {
                println!("TEST PASSED: Successfully navigated to baidu.com");
            } else {
                println!("TEST INCOMPLETE: Did not reach baidu.com");
            }
        }
        Err(e) => eprintln!("Failed to get final URL: {}", e),
    }

    // Close
    println!("\nClosing browser...");
    let _ = client.close().await;
    std::fs::remove_dir_all(&profile.user_data_dir).ok();
    println!("Done!");
}
