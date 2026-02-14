import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { AppSettings } from '../types/profile';
import { open } from '@tauri-apps/plugin-dialog';
import { AISettings } from './AISettings';

type SettingsTab = 'general' | 'ai';

export const Settings: React.FC = () => {
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [chromePath, setChromePath] = useState('');
  const [settings, setSettings] = useState<AppSettings>({
    auto_start: false,
    minimize_to_tray: true,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      setLoading(true);
      const [path, appSettings] = await Promise.all([
        tauriApi.getChromePath(),
        tauriApi.getSettings(),
      ]);
      setChromePath(path);
      setSettings(appSettings);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleBrowseChrome = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: 'Select Chrome Executable',
      });

      if (selected && typeof selected === 'string') {
        setChromePath(selected);
      }
    } catch (err) {
      console.error('Failed to open file dialog:', err);
    }
  };

  const handleSaveChromePath = async () => {
    try {
      setSaving(true);
      setError(null);
      setSuccess(null);

      await tauriApi.updateChromePath(chromePath);
      setSuccess('Chrome path updated successfully');

      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleSettingsChange = async (
    field: keyof AppSettings,
    value: boolean
  ) => {
    const newSettings = { ...settings, [field]: value };
    setSettings(newSettings);

    try {
      await tauriApi.updateSettings(newSettings);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      // Revert on error
      setSettings(settings);
    }
  };

  if (loading) {
    return <div className="loading">Loading settings...</div>;
  }

  return (
    <div className="settings-container">
      <h2>Settings</h2>

      {/* Tab Navigation */}
      <div className="settings-tabs">
        <button
          className={`tab-btn ${activeTab === 'general' ? 'active' : ''}`}
          onClick={() => setActiveTab('general')}
        >
          General
        </button>
        <button
          className={`tab-btn ${activeTab === 'ai' ? 'active' : ''}`}
          onClick={() => setActiveTab('ai')}
        >
          AI Configuration
        </button>
      </div>

      {error && <div className="error-message">{error}</div>}
      {success && <div className="success-message">{success}</div>}

      {/* Tab Content */}
      <div className="settings-content">
        {activeTab === 'general' && (
          <>
            <div className="settings-section">
              <h3>Chrome Configuration</h3>

              <div className="form-group">
                <label htmlFor="chrome-path">Chrome Executable Path</label>
                <div className="input-with-button">
                  <input
                    type="text"
                    id="chrome-path"
                    value={chromePath}
                    onChange={(e) => setChromePath(e.target.value)}
                    placeholder="/usr/bin/google-chrome"
                  />
                  <button className="btn btn-secondary" onClick={handleBrowseChrome}>
                    Browse
                  </button>
                </div>
              </div>

              <button
                className="btn btn-primary"
                onClick={handleSaveChromePath}
                disabled={saving}
              >
                {saving ? 'Saving...' : 'Save Chrome Path'}
              </button>
            </div>

            <div className="settings-section">
              <h3>Application Settings</h3>

              <div className="checkbox-group">
                <label>
                  <input
                    type="checkbox"
                    checked={settings.auto_start}
                    onChange={(e) =>
                      handleSettingsChange('auto_start', e.target.checked)
                    }
                  />
                  <span>Auto-start on system boot</span>
                </label>
              </div>

              <div className="checkbox-group">
                <label>
                  <input
                    type="checkbox"
                    checked={settings.minimize_to_tray}
                    onChange={(e) =>
                      handleSettingsChange('minimize_to_tray', e.target.checked)
                    }
                  />
                  <span>Minimize to tray when closing window</span>
                </label>
              </div>
            </div>
          </>
        )}

        {activeTab === 'ai' && <AISettings />}
      </div>
    </div>
  );
};
