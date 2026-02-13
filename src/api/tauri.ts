import { invoke } from '@tauri-apps/api/core';
import type { BrowserProfile, AppSettings, RunningStatus } from '../types/profile';

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

  async getSettings(): Promise<AppSettings> {
    return invoke('get_settings');
  },

  async updateSettings(settings: AppSettings): Promise<void> {
    return invoke('update_settings', { settings });
  },
};
