import { invoke } from '@tauri-apps/api/core';
import type { BrowserProfile, AppSettings, RunningStatus, LocalApiConfig, ProxyPreset, SnapshotInfo, Recording, RecordingSessionInfo } from '../types/profile';
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

  // Local API
  async getLocalApiConfig(): Promise<LocalApiConfig> {
    return invoke('get_local_api_config');
  },

  async updateLocalApiConfig(localApi: LocalApiConfig): Promise<void> {
    return invoke('update_local_api_config', { localApi });
  },

  // Proxy presets
  async getProxyPresets(): Promise<ProxyPreset[]> {
    return invoke('get_proxy_presets');
  },

  async addProxyPreset(name: string, url: string): Promise<ProxyPreset> {
    return invoke('add_proxy_preset', { name, url });
  },

  async updateProxyPreset(preset: ProxyPreset): Promise<void> {
    return invoke('update_proxy_preset', { preset });
  },

  async deleteProxyPreset(id: string): Promise<void> {
    return invoke('delete_proxy_preset', { id });
  },

  async testProxy(url: string): Promise<number> {
    return invoke('test_proxy', { url });
  },

  // Snapshots
  async listSnapshots(profileId: string): Promise<SnapshotInfo[]> {
    return invoke('list_snapshots', { profileId });
  },

  async createSnapshot(profileId: string, name: string): Promise<SnapshotInfo> {
    return invoke('create_snapshot', { profileId, name });
  },

  async restoreSnapshot(profileId: string, name: string): Promise<void> {
    return invoke('restore_snapshot', { profileId, name });
  },

  async deleteSnapshot(profileId: string, name: string): Promise<void> {
    return invoke('delete_snapshot', { profileId, name });
  },

  // Recordings
  async listRecordings(): Promise<Recording[]> {
    return invoke('list_recordings');
  },

  async getRecording(id: string): Promise<Recording> {
    return invoke('get_recording', { id });
  },

  async saveRecording(recording: Recording): Promise<Recording> {
    return invoke('save_recording', { recording });
  },

  async deleteRecording(id: string): Promise<void> {
    return invoke('delete_recording', { id });
  },

  async playRecording(recordingId: string, profileId: string): Promise<{
    recording_id: string;
    profile_id: string;
    completed_actions: number;
    total_actions: number;
  }> {
    return invoke('play_recording', { recordingId, profileId });
  },

  // Real-time recording
  async startRecording(profileId: string): Promise<string> {
    return invoke('start_recording', { profileId });
  },

  async stopRecording(profileId: string, name: string, description: string): Promise<Recording> {
    return invoke('stop_recording', { profileId, name, description });
  },

  async getActiveRecordingSessions(): Promise<Record<string, string>> {
    return invoke('get_active_recording_sessions');
  },

  async isRecording(profileId: string): Promise<boolean> {
    return invoke('is_recording', { profileId });
  },

  async getRecordingSessionInfo(profileId: string): Promise<RecordingSessionInfo | null> {
    return invoke('get_recording_session_info', { profileId });
  },
};
