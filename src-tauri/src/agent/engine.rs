use crate::agent::action::execute_action;
use crate::agent::cdp::CDPClient;
use crate::agent::llm::{build_context_prompt, build_system_prompt, LLMClient};
use crate::agent::types::{
    AIConfig, AgentMode, AgentOptions, AgentProgress, AgentResult, AgentSession, AgentStatus,
    AgentStep, LLMMessage,
};
use crate::config::schema::BrowserProfile;
use parking_lot::Mutex as ParkingMutex;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

/// Maximum consecutive failures before stopping
const MAX_FAILURES: u32 = 3;

/// Maximum messages to keep in history (to avoid token limits)
const MAX_MESSAGES: usize = 30;

/// Trim message history to keep it within limits, always preserving system message
fn trim_messages(messages: &mut Vec<LLMMessage>) {
    if messages.len() <= MAX_MESSAGES {
        return;
    }

    // Always keep the system message (first message)
    let system_msg = messages.first().filter(|m| m.role == "system").cloned();

    // Remove oldest messages (after system), keep most recent
    let keep_count = MAX_MESSAGES - 1; // -1 for system message
    let start_idx = messages.len() - keep_count;

    let mut new_messages = Vec::with_capacity(MAX_MESSAGES);
    if let Some(system) = system_msg {
        new_messages.push(system);
    }
    new_messages.extend(messages.iter().skip(start_idx).cloned());

    *messages = new_messages;
    tracing::debug!("Trimmed message history to {} messages", messages.len());
}

/// Agent engine that orchestrates browser automation
pub struct AgentEngine {
    /// Active sessions
    sessions: Arc<RwLock<std::collections::HashMap<String, AgentSession>>>,
    /// Progress broadcasters
    progress_tx: Arc<Mutex<std::collections::HashMap<String, broadcast::Sender<AgentProgress>>>>,
    /// Tauri app handle for emitting events
    app_handle: Arc<ParkingMutex<Option<AppHandle>>>,
}

impl AgentEngine {
    /// Create a new agent engine
    pub fn new(_ai_config: AIConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            progress_tx: Arc::new(Mutex::new(std::collections::HashMap::new())),
            app_handle: Arc::new(ParkingMutex::new(None)),
        }
    }

    /// Set the app handle for event emission
    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock() = Some(handle);
    }

    /// Run an agent task
    pub async fn run(
        &self,
        profile: &BrowserProfile,
        chrome_path: &Path,
        task: String,
        options: AgentOptions,
        ai_config: AIConfig,
    ) -> Result<String, String> {
        // Generate agent ID
        let agent_id = Uuid::new_v4().to_string();

        // Create session
        let session = AgentSession::new(
            agent_id.clone(),
            profile.id.clone(),
            task.clone(),
            options.clone(),
        );

        // Create progress channel
        let (tx, _) = broadcast::channel(100);

        // Store session and channel
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(agent_id.clone(), session);
        }
        {
            let mut channels = self.progress_tx.lock().await;
            channels.insert(agent_id.clone(), tx.clone());
        }

        // Send initial progress
        let initial_progress = AgentProgress {
            agent_id: agent_id.clone(),
            status: AgentStatus::Initializing,
            current_step: None,
            steps_completed: 0,
            mode: AgentMode::Llm,
            cost: 0.0,
            message: "Starting agent...".to_string(),
            result: None,
            error: None,
        };
        let _ = tx.send(initial_progress.clone());

        // Emit Tauri event for real-time updates
        if let Some(ref handle) = *self.app_handle.lock() {
            let _ = handle.emit("agent-progress", &initial_progress);
        }

        // Create LLM client with the provided config
        let llm_client = Arc::new(LLMClient::new(ai_config));

        // Clone for async task
        let agent_id_clone = agent_id.clone();
        let sessions = self.sessions.clone();
        let progress_tx_for_spawn = self.progress_tx.clone();
        let app_handle = self.app_handle.clone();
        let profile_clone = profile.clone();
        let chrome_path_buf = chrome_path.to_path_buf();
        let task_clone = task.clone();
        let options_clone = options.clone();

        // Run agent loop in background
        tokio::spawn(async move {
            let result = Self::run_agent_loop(
                agent_id_clone.clone(),
                profile_clone,
                chrome_path_buf,
                task_clone,
                options_clone,
                sessions,
                progress_tx_for_spawn.clone(),
                llm_client,
                app_handle,
            )
            .await;

            if let Err(e) = result {
                tracing::error!("Agent loop error: {}", e);

                // Send error progress
                let error_progress = AgentProgress {
                    agent_id: agent_id_clone.clone(),
                    status: AgentStatus::Failed,
                    current_step: None,
                    steps_completed: 0,
                    mode: AgentMode::Llm,
                    cost: 0.0,
                    message: "Agent failed".to_string(),
                    result: None,
                    error: Some(e.clone()),
                };

                let channels = progress_tx_for_spawn.lock().await;
                if let Some(tx) = channels.get(&agent_id_clone) {
                    let _ = tx.send(error_progress.clone());
                }
            }
        });

        Ok(agent_id)
    }

    /// Main agent loop
    #[allow(clippy::too_many_arguments)]
    async fn run_agent_loop(
        agent_id: String,
        profile: BrowserProfile,
        chrome_path: std::path::PathBuf,
        task: String,
        options: AgentOptions,
        sessions: Arc<RwLock<std::collections::HashMap<String, AgentSession>>>,
        progress_tx: Arc<
            Mutex<std::collections::HashMap<String, broadcast::Sender<AgentProgress>>>,
        >,
        llm_client: Arc<LLMClient>,
        app_handle: Arc<ParkingMutex<Option<AppHandle>>>,
    ) -> Result<(), String> {
        tracing::info!("Starting agent loop for task: {}", task);

        // Launch browser with CDP
        tracing::info!("Launching browser...");
        let mut cdp_client = CDPClient::new(profile.id.clone());
        if let Err(e) = cdp_client
            .launch(&chrome_path, &profile, options.headless)
            .await
        {
            tracing::error!("Failed to launch browser: {}", e);
            return Err(e);
        }
        tracing::info!("Browser launched successfully");

        // Run the main loop and ensure browser is closed afterward
        let result = Self::run_agent_loop_inner(
            agent_id.clone(),
            profile,
            task,
            options,
            sessions.clone(),
            progress_tx.clone(),
            llm_client,
            app_handle,
            &mut cdp_client,
        )
        .await;

        // Always close browser, regardless of success or failure
        tracing::info!("Closing browser...");
        if let Err(e) = cdp_client.close().await {
            tracing::warn!("Failed to close browser cleanly: {}", e);
        }

        // Cleanup progress channel
        {
            let mut channels = progress_tx.lock().await;
            channels.remove(&agent_id);
        }

        result
    }

    /// Inner agent loop (separated to ensure cleanup)
    #[allow(clippy::too_many_arguments)]
    async fn run_agent_loop_inner(
        agent_id: String,
        _profile: BrowserProfile,
        task: String,
        options: AgentOptions,
        sessions: Arc<RwLock<std::collections::HashMap<String, AgentSession>>>,
        progress_tx: Arc<
            Mutex<std::collections::HashMap<String, broadcast::Sender<AgentProgress>>>,
        >,
        llm_client: Arc<LLMClient>,
        app_handle: Arc<ParkingMutex<Option<AppHandle>>>,
        cdp_client: &mut CDPClient,
    ) -> Result<(), String> {
        // Helper to emit progress both to channel and Tauri event
        let emit_progress = |progress: &AgentProgress| {
            tracing::info!(
                "Agent progress: {:?} - {}",
                progress.status,
                progress.message
            );
            if let Some(ref handle) = *app_handle.lock() {
                let _ = handle.emit("agent-progress", progress);
            }
        };

        // Navigate to start URL if specified
        if let Some(url) = &options.start_url {
            tracing::info!("Navigating to start URL: {}", url);
            if let Err(e) = cdp_client.navigate(url).await {
                tracing::error!("Failed to navigate to start URL: {}", e);
            }
        }

        // Get progress sender
        let tx = {
            let channels = progress_tx.lock().await;
            channels
                .get(&agent_id)
                .ok_or_else(|| "Progress channel not found".to_string())?
                .clone()
        };

        // Initialize session state
        {
            let mut sessions = sessions.write().await;
            if let Some(session) = sessions.get_mut(&agent_id) {
                session.status = AgentStatus::Running;
            }
        }

        // Send running status
        let running_progress = AgentProgress {
            agent_id: agent_id.clone(),
            status: AgentStatus::Running,
            current_step: None,
            steps_completed: 0,
            mode: AgentMode::Llm,
            cost: 0.0,
            message: "Agent started, processing task...".to_string(),
            result: None,
            error: None,
        };
        let _ = tx.send(running_progress.clone());
        emit_progress(&running_progress);

        // Build initial messages
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

        let mut consecutive_failures = 0;
        let mut total_cost = 0.0;
        let mut extracted_data = serde_json::Value::Null;
        let start_time = std::time::Instant::now();

        // Main loop
        for step in 1..=options.max_steps {
            tracing::info!("Starting step {}", step);

            // Check if we should stop
            let should_stop = {
                let sessions = sessions.read().await;
                if let Some(session) = sessions.get(&agent_id) {
                    session.should_stop || session.is_paused
                } else {
                    true
                }
            };

            if should_stop {
                // Check if paused
                let is_paused = {
                    let sessions = sessions.read().await;
                    sessions
                        .get(&agent_id)
                        .map(|s| s.is_paused)
                        .unwrap_or(false)
                };

                if is_paused {
                    // Wait while paused
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        let sessions = sessions.read().await;
                        if let Some(session) = sessions.get(&agent_id) {
                            if !session.is_paused || session.should_stop {
                                break;
                            }
                        }
                    }
                    continue;
                }

                // Stopped by user
                let progress = AgentProgress {
                    agent_id: agent_id.clone(),
                    status: AgentStatus::Stopped,
                    current_step: None,
                    steps_completed: step - 1,
                    mode: AgentMode::Llm,
                    cost: total_cost,
                    message: "Agent stopped by user".to_string(),
                    result: None,
                    error: None,
                };
                let _ = tx.send(progress.clone());
                emit_progress(&progress);
                break;
            }

            // Get DOM context
            tracing::debug!("Getting DOM context...");
            let dom_context = match cdp_client.get_dom_context().await {
                Ok(ctx) => ctx,
                Err(e) => {
                    tracing::error!("Failed to get DOM context: {}", e);
                    consecutive_failures += 1;
                    if consecutive_failures >= MAX_FAILURES {
                        let progress = AgentProgress {
                            agent_id: agent_id.clone(),
                            status: AgentStatus::Failed,
                            current_step: None,
                            steps_completed: step,
                            mode: AgentMode::Llm,
                            cost: total_cost,
                            message: "Failed to get page context".to_string(),
                            result: None,
                            error: Some(e.clone()),
                        };
                        let _ = tx.send(progress.clone());
                        emit_progress(&progress);
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };
            tracing::debug!("Got DOM context, URL: {}", dom_context.url);

            // Build context message
            let context_message = build_context_prompt(&task, &dom_context);
            messages.push(LLMMessage {
                role: "user".to_string(),
                content: context_message.clone(),
                images: None,
            });
            trim_messages(&mut messages);

            // Get LLM decision
            tracing::info!("Calling LLM for decision...");
            let provider = match llm_client.get_default_llm() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Failed to get LLM provider: {}", e);
                    let progress = AgentProgress {
                        agent_id: agent_id.clone(),
                        status: AgentStatus::Failed,
                        current_step: None,
                        steps_completed: step,
                        mode: AgentMode::Llm,
                        cost: total_cost,
                        message: "LLM provider not configured".to_string(),
                        result: None,
                        error: Some(e.clone()),
                    };
                    let _ = tx.send(progress.clone());
                    emit_progress(&progress);
                    break;
                }
            };

            let response = match provider.complete(&messages).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("LLM API call failed: {}", e);
                    consecutive_failures += 1;

                    // Send error progress
                    let progress = AgentProgress {
                        agent_id: agent_id.clone(),
                        status: AgentStatus::Running,
                        current_step: None,
                        steps_completed: step,
                        mode: AgentMode::Llm,
                        cost: total_cost,
                        message: format!(
                            "LLM call failed, retrying... ({}/{})",
                            consecutive_failures, MAX_FAILURES
                        ),
                        result: None,
                        error: Some(e.clone()),
                    };
                    let _ = tx.send(progress.clone());
                    emit_progress(&progress);

                    if consecutive_failures >= MAX_FAILURES {
                        let fail_progress = AgentProgress {
                            agent_id: agent_id.clone(),
                            status: AgentStatus::Failed,
                            current_step: None,
                            steps_completed: step,
                            mode: AgentMode::Llm,
                            cost: total_cost,
                            message: "Too many LLM failures".to_string(),
                            result: None,
                            error: Some(format!("LLM failed {} times", MAX_FAILURES)),
                        };
                        let _ = tx.send(fail_progress.clone());
                        emit_progress(&fail_progress);
                        break;
                    }

                    // Remove the last message before retrying
                    messages.pop();
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                }
            };

            tracing::info!(
                "LLM response received, content length: {}",
                response.content.len()
            );

            // Estimate cost (rough approximation)
            let cost = estimate_cost(&response.usage, provider.name());
            total_cost += cost;

            // Add assistant response to history
            messages.push(LLMMessage {
                role: "assistant".to_string(),
                content: response.content.clone(),
                images: None,
            });
            trim_messages(&mut messages);

            // Parse decision
            tracing::debug!("Parsing LLM decision...");
            let decision = match LLMClient::parse_decision(&response.content) {
                Ok(d) => {
                    tracing::info!(
                        "Decision parsed: action={:?}, is_complete={}",
                        d.action,
                        d.is_complete
                    );
                    d
                }
                Err(e) => {
                    let err_str = e.to_string();
                    tracing::warn!("Failed to parse LLM response: {}", err_str);
                    consecutive_failures += 1;

                    // Send warning progress
                    let warning_progress = AgentProgress {
                        agent_id: agent_id.clone(),
                        status: AgentStatus::Running,
                        current_step: None,
                        steps_completed: step,
                        mode: AgentMode::Llm,
                        cost: total_cost,
                        message: format!(
                            "Failed to parse LLM response, retrying... ({}/{})",
                            consecutive_failures, MAX_FAILURES
                        ),
                        result: None,
                        error: None,
                    };
                    let _ = tx.send(warning_progress.clone());
                    emit_progress(&warning_progress);

                    if consecutive_failures >= MAX_FAILURES {
                        // Try VLM escalation
                        tracing::info!("Max failures reached, trying VLM escalation...");
                        if let Ok(vlm_provider) = llm_client.get_default_vlm() {
                            match cdp_client.screenshot().await {
                                Ok(screenshot) => {
                                    match vlm_provider
                                        .complete_with_images(&messages, &[screenshot])
                                        .await
                                    {
                                        Ok(vlm_response) => {
                                            match LLMClient::parse_decision(&vlm_response.content) {
                                                Ok(d) => {
                                                    tracing::info!(
                                                        "VLM decision parsed successfully"
                                                    );
                                                    d
                                                }
                                                Err(e2) => {
                                                    let progress = AgentProgress {
                                                        agent_id: agent_id.clone(),
                                                        status: AgentStatus::Failed,
                                                        current_step: None,
                                                        steps_completed: step,
                                                        mode: AgentMode::Vlm,
                                                        cost: total_cost,
                                                        message: "Failed to parse LLM response"
                                                            .to_string(),
                                                        result: None,
                                                        error: Some(format!("Parse error: {}", e2)),
                                                    };
                                                    let _ = tx.send(progress.clone());
                                                    emit_progress(&progress);
                                                    break;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let progress = AgentProgress {
                                                agent_id: agent_id.clone(),
                                                status: AgentStatus::Failed,
                                                current_step: None,
                                                steps_completed: step,
                                                mode: AgentMode::Vlm,
                                                cost: total_cost,
                                                message: "VLM call failed".to_string(),
                                                result: None,
                                                error: Some(e),
                                            };
                                            let _ = tx.send(progress.clone());
                                            emit_progress(&progress);
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    let progress = AgentProgress {
                                        agent_id: agent_id.clone(),
                                        status: AgentStatus::Failed,
                                        current_step: None,
                                        steps_completed: step,
                                        mode: AgentMode::Llm,
                                        cost: total_cost,
                                        message: "Screenshot failed".to_string(),
                                        result: None,
                                        error: Some(e),
                                    };
                                    let _ = tx.send(progress.clone());
                                    emit_progress(&progress);
                                    break;
                                }
                            }
                        } else {
                            tracing::warn!("No VLM configured, continuing without escalation");
                            messages.pop(); // Remove the failed message
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    } else {
                        messages.pop(); // Remove the failed message
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        continue;
                    }
                }
            };

            // Reset failure count on success
            consecutive_failures = 0;

            // Capture screenshot for progress update
            let screenshot = cdp_client.screenshot().await.ok();

            // Send progress update
            let current_step = AgentStep {
                step,
                url: dom_context.url.clone(),
                action: format!("{:?}", decision.action),
                mode: AgentMode::Llm,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                screenshot,
            };

            let progress = AgentProgress {
                agent_id: agent_id.clone(),
                status: AgentStatus::Running,
                current_step: Some(current_step.clone()),
                steps_completed: step,
                mode: AgentMode::Llm,
                cost: total_cost,
                message: decision.reasoning.clone(),
                result: None,
                error: None,
            };
            let _ = tx.send(progress.clone());
            emit_progress(&progress);

            // Check if task is complete
            if decision.is_complete {
                if let Some(result) = &decision.result {
                    extracted_data = result.clone();
                }

                let duration = start_time.elapsed().as_secs();
                let result = AgentResult {
                    summary: decision.reasoning.clone(),
                    data: extracted_data,
                    final_url: dom_context.url,
                    total_steps: step,
                    total_cost,
                    duration_seconds: duration,
                };

                let progress = AgentProgress {
                    agent_id: agent_id.clone(),
                    status: AgentStatus::Completed,
                    current_step: None,
                    steps_completed: step,
                    mode: AgentMode::Llm,
                    cost: total_cost,
                    message: "Task completed successfully".to_string(),
                    result: Some(result),
                    error: None,
                };
                let _ = tx.send(progress.clone());
                emit_progress(&progress);

                // Update session status
                {
                    let mut sessions = sessions.write().await;
                    if let Some(session) = sessions.get_mut(&agent_id) {
                        session.status = AgentStatus::Completed;
                    }
                }

                break;
            }

            // Execute action
            let action_result = execute_action(cdp_client, &decision.action).await;

            // Handle action result
            match action_result {
                Ok(result) => {
                    if !result.success {
                        consecutive_failures += 1;

                        // Add failure context to messages
                        messages.push(LLMMessage {
                            role: "user".to_string(),
                            content: format!(
                                "The last action failed: {}. Please try a different approach.",
                                result.message
                            ),
                            images: None,
                        });
                        trim_messages(&mut messages);

                        if consecutive_failures >= MAX_FAILURES {
                            let progress = AgentProgress {
                                agent_id: agent_id.clone(),
                                status: AgentStatus::Failed,
                                current_step: None,
                                steps_completed: step,
                                mode: AgentMode::Llm,
                                cost: total_cost,
                                message: "Too many consecutive failures".to_string(),
                                result: None,
                                error: Some(result.message.clone()),
                            };
                            let _ = tx.send(progress.clone());
                            emit_progress(&progress);
                            break;
                        }
                    } else {
                        // Store extracted data
                        if let Some(data) = result.data {
                            if extracted_data.is_null() {
                                extracted_data = data;
                            } else if let serde_json::Value::Object(ref mut existing) =
                                extracted_data
                            {
                                if let serde_json::Value::Object(new_data) = data {
                                    for (k, v) in new_data {
                                        existing.insert(k, v);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::error!("Action execution error: {}", e);

                    if consecutive_failures >= MAX_FAILURES {
                        let progress = AgentProgress {
                            agent_id: agent_id.clone(),
                            status: AgentStatus::Failed,
                            current_step: None,
                            steps_completed: step,
                            mode: AgentMode::Llm,
                            cost: total_cost,
                            message: "Action execution failed".to_string(),
                            result: None,
                            error: Some(e.clone()),
                        };
                        let _ = tx.send(progress.clone());
                        emit_progress(&progress);
                        break;
                    }
                }
            }

            // Update session steps
            {
                let mut sessions = sessions.write().await;
                if let Some(session) = sessions.get_mut(&agent_id) {
                    session.steps_completed = step;
                    session.cost = total_cost;
                    session.history = messages.clone();
                }
            }

            // Small delay between steps
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        Ok(())
    }

    /// Stop an agent
    pub async fn stop(&self, agent_id: &str) -> Result<(), String> {
        tracing::info!("Stop requested for agent: {}", agent_id);
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(agent_id) {
            session.should_stop = true;
            session.status = AgentStatus::Stopped;
            tracing::info!("Agent {} stopped successfully", agent_id);
        } else {
            tracing::warn!(
                "Agent {} not found in sessions (available: {:?})",
                agent_id,
                sessions.keys().collect::<Vec<_>>()
            );
            return Err(format!("Agent {} not found", agent_id));
        }
        Ok(())
    }

    /// Pause an agent
    pub async fn pause(&self, agent_id: &str) -> Result<(), String> {
        tracing::info!("Pause requested for agent: {}", agent_id);
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(agent_id) {
            session.is_paused = true;
            session.status = AgentStatus::Paused;
            tracing::info!("Agent {} paused successfully", agent_id);
        } else {
            tracing::warn!(
                "Agent {} not found in sessions (available: {:?})",
                agent_id,
                sessions.keys().collect::<Vec<_>>()
            );
            return Err(format!("Agent {} not found", agent_id));
        }
        Ok(())
    }

    /// Resume a paused agent
    pub async fn resume(&self, agent_id: &str) -> Result<(), String> {
        tracing::info!("Resume requested for agent: {}", agent_id);
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(agent_id) {
            session.is_paused = false;
            session.status = AgentStatus::Running;
            tracing::info!("Agent {} resumed successfully", agent_id);
        } else {
            tracing::warn!("Agent {} not found in sessions", agent_id);
            return Err(format!("Agent {} not found", agent_id));
        }
        Ok(())
    }

    /// Get agent status
    pub async fn get_status(&self, agent_id: &str) -> Option<AgentProgress> {
        let sessions = self.sessions.read().await;
        sessions.get(agent_id).map(|session| AgentProgress {
            agent_id: agent_id.to_string(),
            status: session.status.clone(),
            current_step: None,
            steps_completed: session.steps_completed,
            mode: session.mode.clone(),
            cost: session.cost,
            message: String::new(),
            result: None,
            error: None,
        })
    }

    /// Subscribe to agent progress updates
    pub async fn subscribe(&self, agent_id: &str) -> Option<broadcast::Receiver<AgentProgress>> {
        let channels = self.progress_tx.lock().await;
        channels.get(agent_id).map(|tx| tx.subscribe())
    }
}

/// Estimate API cost based on token usage
fn estimate_cost(usage: &crate::agent::types::TokenUsage, provider: &str) -> f64 {
    // Rough cost estimates (USD per 1K tokens)
    let (input_cost, output_cost) = match provider {
        "openai" => (0.00015, 0.0006), // GPT-4o-mini rates
        "anthropic" => (0.003, 0.015), // Claude 3.5 Sonnet rates
        "ollama" => (0.0, 0.0),        // Local, free
        _ => (0.001, 0.002),
    };

    (usage.prompt_tokens as f64 * input_cost / 1000.0)
        + (usage.completion_tokens as f64 * output_cost / 1000.0)
}
