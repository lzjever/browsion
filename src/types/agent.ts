// Agent types for frontend

export type ApiType = 'openai' | 'anthropic' | 'ollama';

export interface ProviderConfig {
  name: string;
  api_type: ApiType;
  base_url: string;
  api_key?: string;
  models: string[];
}

export interface AIConfig {
  default_llm?: string;  // format: "provider_id:model_name"
  default_vlm?: string;  // format: "provider_id:model_name"
  escalation_enabled: boolean;
  max_retries: number;
  timeout_seconds: number;
  providers: Record<string, ProviderConfig>;  // key = provider_id
}

export interface AgentOptions {
  headless: boolean;
  timeout?: number;
  max_steps: number;
  start_url?: string;
}

export type AgentStatus =
  | 'initializing'
  | 'running'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'stopped';

export type AgentMode = 'llm' | 'vlm';

export interface AgentStep {
  step: number;
  url: string;
  action: string;
  mode: AgentMode;
  timestamp: number;
  screenshot?: string;
}

export interface AgentProgress {
  agent_id: string;
  status: AgentStatus;
  current_step?: AgentStep;
  steps_completed: number;
  mode: AgentMode;
  cost: number;
  message: string;
  result?: AgentResult;
  error?: string;
}

export interface AgentResult {
  summary: string;
  data: Record<string, unknown> | unknown[];
  final_url: string;
  total_steps: number;
  total_cost: number;
  duration_seconds: number;
}

export type AgentActionType =
  | 'navigate'
  | 'click'
  | 'type'
  | 'press_key'
  | 'scroll'
  | 'wait'
  | 'extract'
  | 'screenshot'
  | 'go_back'
  | 'none';

export interface NavigateAction {
  type: 'navigate';
  url: string;
}

export interface ClickAction {
  type: 'click';
  selector: string;
}

export interface TypeAction {
  type: 'type';
  selector: string;
  text: string;
}

export interface PressKeyAction {
  type: 'press_key';
  key: string;
}

export interface ScrollAction {
  type: 'scroll';
  direction: 'up' | 'down' | 'left' | 'right';
  amount: number;
}

export interface WaitAction {
  type: 'wait';
  duration_ms?: number;
  selector?: string;
}

export interface ExtractAction {
  type: 'extract';
  selectors: Record<string, string>;
}

export interface ScreenshotAction {
  type: 'screenshot';
}

export interface GoBackAction {
  type: 'go_back';
}

export interface NoneAction {
  type: 'none';
}

export type AgentAction =
  | NavigateAction
  | ClickAction
  | TypeAction
  | PressKeyAction
  | ScrollAction
  | WaitAction
  | ExtractAction
  | ScreenshotAction
  | GoBackAction
  | NoneAction;

export interface LLMDecision {
  action: AgentAction;
  reasoning: string;
  is_complete: boolean;
  result?: Record<string, unknown>;
}

// Batch execution types
export interface BatchProgress {
  batch_id: string;
  total: number;
  completed: number;
  failed: number;
  current_profile?: string;
  agents: Record<string, string>; // agent_id -> profile_id
  results: Record<string, AgentResult>; // profile_id -> result
  errors: Record<string, string>; // profile_id -> error
  total_cost: number;
}

// Scheduled task types
export interface ScheduleConfig {
  type: 'once' | 'interval' | 'daily' | 'weekly' | 'cron';
  datetime?: number; // for once
  minutes?: number; // for interval
  hour?: number; // for daily/weekly
  minute?: number; // for daily/weekly
  day_of_week?: number; // for weekly (0=Mon, 6=Sun)
  expression?: string; // for cron
}

export interface ScheduledTask {
  id: string;
  name: string;
  task: string;
  profile_ids: string[];
  schedule: ScheduleConfig;
  enabled: boolean;
  start_url?: string;
  headless: boolean;
  created_at: number;
  last_run?: number;
  next_run?: number;
  run_count: number;
}

export const defaultScheduledTask: Partial<ScheduledTask> = {
  enabled: true,
  headless: true,
  run_count: 0,
};

export const defaultAgentOptions: AgentOptions = {
  headless: false,
  max_steps: 50,
};

// File-based Task Template types
export interface TemplateInfo {
  id: string;
  name: string;
  content: string;
  start_url?: string;
  headless: boolean;
  modified_at: number;
}

export const defaultTemplate: Partial<TemplateInfo> = {
  headless: false,
};
