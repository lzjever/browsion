use crate::agent::cdp::CDPClient;
use crate::agent::types::{AgentAction, ScrollDirection};
use std::collections::HashMap;

/// Execute an agent action
pub async fn execute_action(
    client: &CDPClient,
    action: &AgentAction,
) -> Result<ActionResult, String> {
    match action {
        AgentAction::Navigate { url } => {
            client.navigate(url).await?;
            Ok(ActionResult {
                success: true,
                message: format!("Navigated to {}", url),
                data: None,
            })
        }

        AgentAction::Click { selector } => match client.click(selector).await {
            Ok(_) => Ok(ActionResult {
                success: true,
                message: format!("Clicked {}", selector),
                data: None,
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                message: format!("Failed to click {}: {}", selector, e),
                data: None,
            }),
        },

        AgentAction::Type { selector, text } => match client.type_text(selector, text).await {
            Ok(_) => Ok(ActionResult {
                success: true,
                message: format!("Typed '{}' into {}", text, selector),
                data: None,
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                message: format!("Failed to type into {}: {}", selector, e),
                data: None,
            }),
        },

        AgentAction::PressKey { key } => match client.press_key(key).await {
            Ok(_) => Ok(ActionResult {
                success: true,
                message: format!("Pressed key: {}", key),
                data: None,
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                message: format!("Failed to press key {}: {}", key, e),
                data: None,
            }),
        },

        AgentAction::Scroll { direction, amount } => {
            let dir_str = match direction {
                ScrollDirection::Up => "up",
                ScrollDirection::Down => "down",
                ScrollDirection::Left => "left",
                ScrollDirection::Right => "right",
            };
            match client.scroll(dir_str, *amount).await {
                Ok(_) => Ok(ActionResult {
                    success: true,
                    message: format!("Scrolled {} by {}", dir_str, amount),
                    data: None,
                }),
                Err(e) => Ok(ActionResult {
                    success: false,
                    message: format!("Failed to scroll: {}", e),
                    data: None,
                }),
            }
        }

        AgentAction::Wait {
            duration_ms,
            selector,
        } => {
            if let Some(ms) = duration_ms {
                client.wait(*ms).await?;
                Ok(ActionResult {
                    success: true,
                    message: format!("Waited {} ms", ms),
                    data: None,
                })
            } else if let Some(sel) = selector {
                match client.wait_for_element(sel, 10000).await {
                    Ok(_) => Ok(ActionResult {
                        success: true,
                        message: format!("Element found: {}", sel),
                        data: None,
                    }),
                    Err(e) => Ok(ActionResult {
                        success: false,
                        message: format!("Element not found: {}", e),
                        data: None,
                    }),
                }
            } else {
                client.wait(1000).await?;
                Ok(ActionResult {
                    success: true,
                    message: "Waited 1000 ms (default)".to_string(),
                    data: None,
                })
            }
        }

        AgentAction::Extract { selectors } => match client.extract_data(selectors).await {
            Ok(data) => Ok(ActionResult {
                success: true,
                message: "Data extracted successfully".to_string(),
                data: Some(data),
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                message: format!("Failed to extract data: {}", e),
                data: None,
            }),
        },

        AgentAction::Screenshot => match client.screenshot().await {
            Ok(base64) => {
                let mut data = HashMap::new();
                data.insert("screenshot".to_string(), serde_json::Value::String(base64));
                Ok(ActionResult {
                    success: true,
                    message: "Screenshot taken".to_string(),
                    data: Some(serde_json::Value::Object(data.into_iter().collect())),
                })
            }
            Err(e) => Ok(ActionResult {
                success: false,
                message: format!("Failed to take screenshot: {}", e),
                data: None,
            }),
        },

        AgentAction::GoBack => match client.go_back().await {
            Ok(_) => Ok(ActionResult {
                success: true,
                message: "Navigated back".to_string(),
                data: None,
            }),
            Err(e) => Ok(ActionResult {
                success: false,
                message: format!("Failed to go back: {}", e),
                data: None,
            }),
        },

        AgentAction::None => Ok(ActionResult {
            success: true,
            message: "No action taken".to_string(),
            data: None,
        }),
    }
}

/// Result of an action execution
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Whether the action succeeded
    pub success: bool,
    /// Human-readable message
    pub message: String,
    /// Extracted data (if any)
    pub data: Option<serde_json::Value>,
}

impl ActionResult {
    pub fn success(message: &str) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data: None,
        }
    }

    pub fn failure(message: &str) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            data: None,
        }
    }

    pub fn with_data(message: &str, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data: Some(data),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_result() {
        let result = ActionResult::success("Test");
        assert!(result.success);
        assert_eq!(result.message, "Test");
        assert!(result.data.is_none());
    }
}
