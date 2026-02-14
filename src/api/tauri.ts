import { invoke } from '@tauri-apps/api/core';
import type { BrowserProfile, AppSettings, RunningStatus } from '../types/profile';
import type { AgentOptions, AgentProgress, AIConfig, ProviderConfig, TemplateInfo, ScheduledTask } from '../types/agent';

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

  // AI Agent
  async runAgent(profileId: string, task: string, options?: AgentOptions): Promise<string> {
    return invoke('run_agent', { profileId, task, options });
  },

  async stopAgent(agentId: string): Promise<void> {
    return invoke('stop_agent', { agentId });
  },

  async pauseAgent(agentId: string): Promise<void> {
    return invoke('pause_agent', { agentId });
  },

  async resumeAgent(agentId: string): Promise<void> {
    return invoke('resume_agent', { agentId });
  },

  async getAgentStatus(agentId: string): Promise<AgentProgress | null> {
    return invoke('get_agent_status', { agentId });
  },

  // AI Configuration
  async getAIConfig(): Promise<AIConfig> {
    return invoke('get_ai_config');
  },

  async updateAIConfig(aiConfig: AIConfig): Promise<void> {
    return invoke('update_ai_config', { aiConfig });
  },

  async testAIProvider(providerConfig: ProviderConfig, model: string): Promise<string> {
    return invoke('test_ai_provider', { providerConfig, model });
  },

  // Task Templates (File-based)
  async getTemplates(): Promise<TemplateInfo[]> {
    return invoke('get_templates');
  },

  async getTemplate(id: string): Promise<TemplateInfo> {
    return invoke('get_template', { id });
  },

  async saveTemplate(
    id: string,
    name: string,
    content: string,
    startUrl?: string,
    headless?: boolean
  ): Promise<void> {
    return invoke('save_template', { id, name, content, startUrl, headless: headless ?? false });
  },

  async deleteTemplate(id: string): Promise<void> {
    return invoke('delete_template', { id });
  },

  async openTemplatesDir(): Promise<void> {
    return invoke('open_templates_dir');
  },

  // Scheduled Tasks
  async getScheduledTasks(): Promise<ScheduledTask[]> {
    return invoke('get_scheduled_tasks');
  },

  async addScheduledTask(task: ScheduledTask): Promise<void> {
    return invoke('add_scheduled_task', { task });
  },

  async updateScheduledTask(task: ScheduledTask): Promise<void> {
    return invoke('update_scheduled_task', { task });
  },

  async deleteScheduledTask(taskId: string): Promise<void> {
    return invoke('delete_scheduled_task', { taskId });
  },

  async toggleScheduledTask(taskId: string, enabled: boolean): Promise<void> {
    return invoke('toggle_scheduled_task', { taskId, enabled });
  },
};
