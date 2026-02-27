import { invoke } from '@tauri-apps/api/core';
import type { BrowserProfile, AppSettings, RunningStatus, McpConfig, McpToolInfo } from '../types/profile';
import type { BrowserSource, CftVersionInfo } from '../types/profile';

export const tauriApi = {
  // Profile management
  async getProfiles(): Promise<BrowserProfile[]> {
    return invoke('get_profiles');
  },

  async addProfile(profile: BrowserProfile): Promise<void> {
    return invoke('add_profile', { profile });
  },

  async updateProfile(profile: BrowserProfile): Promise<void> {
    return invoke('update_profile', { profile });
  },

  async deleteProfile(profileId: string): Promise<void> {
    return invoke('delete_profile', { profileId });
  },

  // Process management
  async launchProfile(profileId: string): Promise<number> {
    return invoke('launch_profile', { profileId });
  },

  async activateProfile(profileId: string): Promise<void> {
    return invoke('activate_profile', { profileId });
  },

  async killProfile(profileId: string): Promise<void> {
    return invoke('kill_profile', { profileId });
  },

  async getRunningProfiles(): Promise<RunningStatus> {
    return invoke('get_running_profiles');
  },

  // Settings
  async getChromePath(): Promise<string> {
    return invoke('get_chrome_path');
  },

  async updateChromePath(path: string): Promise<void> {
    return invoke('update_chrome_path', { path });
  },

  async getBrowserSource(): Promise<BrowserSource> {
    return invoke('get_browser_source');
  },

  async updateBrowserSource(source: BrowserSource): Promise<void> {
    return invoke('update_browser_source', { source });
  },

  async getCftVersions(): Promise<CftVersionInfo[]> {
    return invoke('get_cft_versions');
  },

  async downloadCftVersion(
    channel: string,
    version: string,
    downloadDir?: string
  ): Promise<string> {
    return invoke('download_cft_version', {
      channel,
      version,
      download_dir: downloadDir ?? null,
    });
  },

  async getSettings(): Promise<AppSettings> {
    return invoke('get_settings');
  },

  async updateSettings(settings: AppSettings): Promise<void> {
    return invoke('update_settings', { settings });
  },

  // MCP / API Server
  async getMcpConfig(): Promise<McpConfig> {
    return invoke('get_mcp_config');
  },

  async updateMcpConfig(mcp: McpConfig): Promise<void> {
    return invoke('update_mcp_config', { mcp });
  },

  // MCP Tool Config Writer
  async detectMcpTools(): Promise<McpToolInfo[]> {
    return invoke('detect_mcp_tools');
  },

  async writeBrowsionToTool(
    toolId: string,
    binaryPath: string,
    apiPort: number,
    apiKey?: string,
    projectDir?: string
  ): Promise<string> {
    return invoke('write_browsion_to_tool', {
      toolId,
      binaryPath,
      projectDir: projectDir ?? null,
      apiPort,
      apiKey: apiKey ?? null,
    });
  },

  async findMcpBinary(): Promise<string | null> {
    return invoke('find_mcp_binary');
  },
};
