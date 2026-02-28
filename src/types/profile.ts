export interface BrowserProfile {
  id: string;
  name: string;
  description: string;
  user_data_dir: string;
  proxy_server?: string;
  lang: string;
  timezone?: string;
  fingerprint?: string;
  color?: string;
  custom_args: string[];
  tags: string[];
  headless?: boolean;
}

export type CftChannel = 'Stable' | 'Beta' | 'Dev' | 'Canary';

export type BrowserSource =
  | {
      type: 'chrome_for_testing';
      channel: CftChannel;
      version?: string;
      download_dir?: string;
    }
  | { type: 'custom'; path: string; fingerprint_chromium?: boolean };

export interface CftVersionInfo {
  channel: string;
  version: string;
  url: string;
  platform: string;
}

export interface AppConfig {
  browser_source: BrowserSource;
  profiles: BrowserProfile[];
  settings: AppSettings;
}

export interface AppSettings {
  auto_start: boolean;
  minimize_to_tray: boolean;
}

export interface ProcessInfo {
  profile_id: string;
  pid: number;
  cdp_port?: number;
  launched_at: number;
}

export type RunningStatus = Record<string, boolean>;

export interface McpConfig {
  enabled: boolean;
  api_port: number;
  api_key?: string;
}

export type ToolScope = 'global' | 'project_scoped';

export interface ProxyPreset {
  id: string;
  name: string;
  url: string;
}

export interface SnapshotInfo {
  name: string;
  created_at_ts: number;
  size_bytes: number;
}

export interface ActionEntry {
  id: string;
  ts: number;
  profile_id: string;
  tool: string;
  duration_ms: number;
  success: boolean;
  error?: string;
}

export interface McpToolInfo {
  id: string;
  name: string;
  config_path: string;
  found: boolean;
  scope: ToolScope;
}

// Workflow types
export type StepType =
  | 'navigate'
  | 'go_back'
  | 'go_forward'
  | 'reload'
  | 'wait_for_url'
  | 'wait_for_navigation'
  | 'click'
  | 'click_at'
  | 'hover'
  | 'double_click'
  | 'right_click'
  | 'drag'
  | 'type'
  | 'slow_type'
  | 'press_key'
  | 'select_option'
  | 'upload_file'
  | 'scroll'
  | 'scroll_element'
  | 'scroll_into_view'
  | 'wait_for_element'
  | 'wait_for_text'
  | 'screenshot'
  | 'screenshot_element'
  | 'get_page_state'
  | 'get_page_text'
  | 'get_cookies'
  | 'extract'
  | 'new_tab'
  | 'switch_tab'
  | 'close_tab'
  | 'wait_for_new_tab'
  | 'get_console_logs'
  | 'sleep'
  | 'set_variable'
  | 'condition';

export interface WorkflowStep {
  id: string;
  name: string;
  description: string;
  type: StepType;
  params: Record<string, unknown>;
  continue_on_error: boolean;
  timeout_ms: number;
}

export interface Workflow {
  id: string;
  name: string;
  description: string;
  steps: WorkflowStep[];
  variables: Record<string, unknown>;
  created_at: number;
  updated_at: number;
}

export type ExecutionStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'paused'
  | 'cancelled';

export interface StepResult {
  step_id: string;
  status: ExecutionStatus;
  duration_ms: number;
  output: unknown;
  error: string | null;
  started_at: number;
  completed_at: number;
}

export interface WorkflowExecution {
  id: string;
  workflow_id: string;
  profile_id: string;
  status: ExecutionStatus;
  current_step_index: number;
  step_results: StepResult[];
  variables: Record<string, unknown>;
  started_at: number;
  completed_at: number | null;
  error: string | null;
}

export interface StepTypeInfo {
  type: StepType;
  name: string;
  description: string;
  params: ParamInfo[];
}

export interface ParamInfo {
  name: string;
  type: string;
  required: boolean;
  description: string;
}

// Recording types
export type RecordedActionType =
  | 'navigate'
  | 'go_back'
  | 'go_forward'
  | 'reload'
  | 'click'
  | 'hover'
  | 'double_click'
  | 'right_click'
  | 'type'
  | 'slow_type'
  | 'press_key'
  | 'select_option'
  | 'upload_file'
  | 'scroll'
  | 'scroll_into_view'
  | 'new_tab'
  | 'switch_tab'
  | 'close_tab'
  | 'sleep'
  | 'wait_for_text'
  | 'wait_for_element'
  | 'screenshot'
  | 'get_console_logs'
  | 'extract';

export interface RecordedAction {
  index: number;
  type: RecordedActionType;
  params: Record<string, unknown>;
  timestamp_ms: number;
  screenshot_base64: string | null;
}

export interface Recording {
  id: string;
  name: string;
  description: string;
  profile_id: string;
  actions: RecordedAction[];
  created_at: number;
  duration_ms: number;
}
