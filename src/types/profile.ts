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

export interface LocalApiConfig {
  enabled: boolean;
  api_port: number;
  api_key?: string;
}

export interface ProcessInfo {
  profile_id: string;
  pid: number;
  cdp_port?: number;
  launched_at: number;
}

export type RunningStatus = Record<string, boolean>;

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

export interface RecordingSessionInfo {
  id: string;
  profile_id: string;
  started_at: number;
  action_count: number;
  is_recording: boolean;
}
