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
}

export interface AppConfig {
  chrome_path: string;
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
  launched_at: number;
}

export type RunningStatus = Record<string, boolean>;
