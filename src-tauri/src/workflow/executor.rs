//! Workflow executor: runs workflow steps and tracks progress.

use crate::agent::SessionManager;
use crate::workflow::schema::{ExecutionStatus, StepResult, StepType, Workflow, WorkflowExecution};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Executes workflow steps against a running browser.
pub struct WorkflowExecutor {
    #[allow(dead_code)]
    session_manager: Arc<SessionManager>,
    /// Base URL for HTTP API (localhost:port).
    api_base: String,
    /// Optional API key for authentication.
    api_key: Option<String>,
}

impl WorkflowExecutor {
    pub fn new(session_manager: Arc<SessionManager>, api_port: u16, api_key: Option<String>) -> Self {
        Self {
            session_manager,
            api_base: format!("http://127.0.0.1:{}", api_port),
            api_key,
        }
    }

    /// Execute a workflow synchronously.
    pub async fn execute(
        &self,
        workflow: &Workflow,
        profile_id: String,
        variables: HashMap<String, serde_json::Value>,
    ) -> WorkflowExecution {
        let mut execution = WorkflowExecution::new(
            workflow.id.clone(),
            profile_id,
            variables,
            workflow.steps.len(),
        );

        execution.status = ExecutionStatus::Running;

        for (index, step) in workflow.steps.iter().enumerate() {
            execution.current_step_index = index;

            let result = self.execute_step(step, &execution, &workflow.variables).await;

            execution.step_results.push(result.clone());

            // Check if step failed
            if result.status == ExecutionStatus::Failed && !step.continue_on_error {
                execution.status = ExecutionStatus::Failed;
                execution.error = result.error;
                execution.completed_at = Some(now_ms());
                return execution;
            }
        }

        execution.status = ExecutionStatus::Completed;
        execution.completed_at = Some(now_ms());
        execution
    }

    /// Execute a single step.
    async fn execute_step(
        &self,
        step: &crate::workflow::schema::WorkflowStep,
        execution: &WorkflowExecution,
        workflow_vars: &HashMap<String, serde_json::Value>,
    ) -> StepResult {
        let started_at = now_ms();

        // Resolve variables in params
        let params = self.resolve_variables(&step.params, &execution.variables, workflow_vars);

        // Execute based on step type
        let (status, output, error) = tokio::time::timeout(
            Duration::from_millis(step.timeout_ms),
            self.execute_step_inner(&step.step_type, &execution.profile_id, &params),
        )
        .await
        .unwrap_or_else(|_| {
            (
                ExecutionStatus::Failed,
                None,
                Some(format!("Step timeout after {}ms", step.timeout_ms)),
            )
        });

        let completed_at = now_ms();
        let duration_ms = completed_at.saturating_sub(started_at);

        StepResult {
            step_id: step.id.clone(),
            status,
            duration_ms,
            output,
            error,
            started_at,
            completed_at,
        }
    }

    /// Resolve variable references in params (e.g., "${varname}").
    fn resolve_variables(
        &self,
        params: &serde_json::Value,
        exec_vars: &HashMap<String, serde_json::Value>,
        workflow_vars: &HashMap<String, serde_json::Value>,
    ) -> serde_json::Value {
        let json_str = params.to_string();

        // First resolve workflow defaults, then execution overrides
        let mut result = json_str;

        for (key, value) in workflow_vars.iter().chain(exec_vars.iter()) {
            let placeholder = format!("${{{}}}", key);
            let replacement = match value {
                serde_json::Value::String(s) => s.clone(),
                v => v.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }

        serde_json::from_str(&result).unwrap_or_else(|_| params.clone())
    }

    /// Inner step execution logic.
    async fn execute_step_inner(
        &self,
        step_type: &StepType,
        profile_id: &str,
        params: &serde_json::Value,
    ) -> (ExecutionStatus, Option<serde_json::Value>, Option<String>) {
        let client = self.http_client();

        match step_type {
            StepType::Navigate => {
                let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let url_path = format!("{}/api/browser/{}/navigate_wait", self.api_base, profile_id);
                let body = serde_json::json!({ "url": url });

                let mut req = client.post(&url_path).json(&body);
                if let Some(key) = &self.api_key {
                    req = req.header("X-API-Key", key);
                }

                match req.send().await {
                    Ok(resp) if resp.status().is_success() => {
                        (ExecutionStatus::Completed, None, None)
                    }
                    Ok(resp) => (
                        ExecutionStatus::Failed,
                        None,
                        Some(format!("HTTP {}", resp.status())),
                    ),
                    Err(e) => (ExecutionStatus::Failed, None, Some(e.to_string())),
                }
            }

            StepType::Click => {
                let selector = params.get("selector").and_then(|v| v.as_str()).unwrap_or("");
                let url_path = format!("{}/api/browser/{}/click", self.api_base, profile_id);
                let body = serde_json::json!({ "selector": selector });

                match client.post(&url_path).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        (ExecutionStatus::Completed, None, None)
                    }
                    Ok(resp) => (
                        ExecutionStatus::Failed,
                        None,
                        Some(format!("HTTP {}", resp.status())),
                    ),
                    Err(e) => (ExecutionStatus::Failed, None, Some(e.to_string())),
                }
            }

            StepType::Type => {
                let selector = params.get("selector").and_then(|v| v.as_str()).unwrap_or("");
                let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let url_path = format!("{}/api/browser/{}/type", self.api_base, profile_id);
                let body = serde_json::json!({ "selector": selector, "text": text });

                match client.post(&url_path).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        (ExecutionStatus::Completed, None, None)
                    }
                    Ok(resp) => (
                        ExecutionStatus::Failed,
                        None,
                        Some(format!("HTTP {}", resp.status())),
                    ),
                    Err(e) => (ExecutionStatus::Failed, None, Some(e.to_string())),
                }
            }

            StepType::Sleep => {
                let duration_ms = params.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(1000);
                tokio::time::sleep(Duration::from_millis(duration_ms)).await;
                (ExecutionStatus::Completed, None, None)
            }

            StepType::Screenshot => {
                let url_path = format!("{}/api/browser/{}/screenshot", self.api_base, profile_id);

                match client.get(&url_path).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            (ExecutionStatus::Completed, Some(data), None)
                        } else {
                            (ExecutionStatus::Completed, None, None)
                        }
                    }
                    Ok(resp) => (
                        ExecutionStatus::Failed,
                        None,
                        Some(format!("HTTP {}", resp.status())),
                    ),
                    Err(e) => (ExecutionStatus::Failed, None, Some(e.to_string())),
                }
            }

            StepType::GetPageState => {
                let url_path = format!("{}/api/browser/{}/page_state", self.api_base, profile_id);

                match client.get(&url_path).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(data) = resp.json::<serde_json::Value>().await {
                            (ExecutionStatus::Completed, Some(data), None)
                        } else {
                            (ExecutionStatus::Failed, None, Some("Invalid JSON".to_string()))
                        }
                    }
                    Ok(resp) => (
                        ExecutionStatus::Failed,
                        None,
                        Some(format!("HTTP {}", resp.status())),
                    ),
                    Err(e) => (ExecutionStatus::Failed, None, Some(e.to_string())),
                }
            }

            StepType::WaitForText => {
                let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let timeout_ms = params.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(10000);
                let url_path = format!("{}/api/browser/{}/wait_for", self.api_base, profile_id);
                let body = serde_json::json!({ "selector": "body", "wait_for": "text", "text": text, "timeout_ms": timeout_ms });

                match client.post(&url_path).json(&body).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        (ExecutionStatus::Completed, None, None)
                    }
                    Ok(resp) => (
                        ExecutionStatus::Failed,
                        None,
                        Some(format!("HTTP {}", resp.status())),
                    ),
                    Err(e) => (ExecutionStatus::Failed, None, Some(e.to_string())),
                }
            }

            _ => (
                ExecutionStatus::Failed,
                None,
                Some(format!("Step type {:?} not yet implemented", step_type)),
            ),
        }
    }

    /// Create HTTP client with optional API key.
    fn http_client(&self) -> reqwest::Client {
        let builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(60));

        builder.build().unwrap_or_default()
    }

    /// Add API key header to a request builder if configured.
    #[allow(dead_code)]
    fn add_api_key(
        &self,
        req: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let Some(key) = &self.api_key {
            req.header("X-API-Key", key)
        } else {
            req
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
