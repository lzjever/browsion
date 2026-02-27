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

export interface McpToolInfo {
  id: string;
  name: string;
  config_path: string;
  found: boolean;
  scope: ToolScope;
}
