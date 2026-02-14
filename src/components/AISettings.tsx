import { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { AIConfig, ProviderConfig, ApiType } from '../types/agent';

const API_TYPES: { value: ApiType; label: string; description: string }[] = [
  { value: 'openai', label: 'OpenAI Compatible', description: 'OpenAI, Azure, or any OpenAI-compatible API' },
  { value: 'anthropic', label: 'Anthropic', description: 'Claude API' },
  { value: 'ollama', label: 'Ollama', description: 'Local Ollama server' },
];

const DEFAULT_URLS: Record<ApiType, string> = {
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com',
  ollama: 'http://localhost:11434',
};

interface TestStatus {
  [key: string]: 'idle' | 'testing' | 'success' | 'error';
}

interface TestMessage {
  [key: string]: string;
}

export function AISettings() {
  const [config, setConfig] = useState<AIConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [testStatus, setTestStatus] = useState<TestStatus>({});
  const [testMessage, setTestMessage] = useState<TestMessage>({});

  // New provider form
  const [showAddProvider, setShowAddProvider] = useState(false);
  const [newProviderId, setNewProviderId] = useState('');
  const [newProviderName, setNewProviderName] = useState('');
  const [newProviderType, setNewProviderType] = useState<ApiType>('openai');
  const [newProviderUrl, setNewProviderUrl] = useState('https://api.openai.com/v1');
  const [newProviderKey, setNewProviderKey] = useState('');
  const [newProviderModels, setNewProviderModels] = useState('');

  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    setLoading(true);
    setError(null);
    try {
      const cfg = await tauriApi.getAIConfig();
      setConfig(cfg);
    } catch (e) {
      setError(`Failed to load AI config: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!config) return;

    setSaving(true);
    setError(null);
    setSuccess(false);

    try {
      await tauriApi.updateAIConfig(config);
      setSuccess(true);
      setTimeout(() => setSuccess(false), 3000);
    } catch (e) {
      setError(`Failed to save: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const testConnection = async (providerId: string) => {
    if (!config?.providers[providerId]) return;
    const provider = config.providers[providerId];
    const model = provider.models[0] || '';

    setTestStatus((prev) => ({ ...prev, [providerId]: 'testing' }));
    setTestMessage((prev) => ({ ...prev, [providerId]: '' }));

    try {
      const result = await tauriApi.testAIProvider(provider, model);
      setTestStatus((prev) => ({ ...prev, [providerId]: 'success' }));
      setTestMessage((prev) => ({ ...prev, [providerId]: result }));
    } catch (e) {
      setTestStatus((prev) => ({ ...prev, [providerId]: 'error' }));
      setTestMessage((prev) => ({ ...prev, [providerId]: String(e) }));
    }
  };

  const addProvider = () => {
    if (!config || !newProviderId.trim() || !newProviderName.trim()) return;

    const providerId = newProviderId.trim().toLowerCase().replace(/[^a-z0-9_-]/g, '_');
    if (config.providers[providerId]) {
      setError(`Provider ID "${providerId}" already exists`);
      return;
    }

    const models = newProviderModels
      .split(',')
      .map(m => m.trim())
      .filter(m => m.length > 0);

    const newProvider: ProviderConfig = {
      name: newProviderName.trim(),
      api_type: newProviderType,
      base_url: newProviderUrl.trim(),
      api_key: newProviderKey.trim() || undefined,
      models,
    };

    setConfig({
      ...config,
      providers: {
        ...config.providers,
        [providerId]: newProvider,
      },
    });

    // Reset form
    setNewProviderId('');
    setNewProviderName('');
    setNewProviderType('openai');
    setNewProviderUrl('https://api.openai.com/v1');
    setNewProviderKey('');
    setNewProviderModels('');
    setShowAddProvider(false);
  };

  const removeProvider = (providerId: string) => {
    if (!config) return;

    const newProviders = { ...config.providers };
    delete newProviders[providerId];

    setConfig({
      ...config,
      providers: newProviders,
      // Clear default selections if removed provider was selected
      default_llm: config.default_llm?.startsWith(providerId + ':') ? undefined : config.default_llm,
      default_vlm: config.default_vlm?.startsWith(providerId + ':') ? undefined : config.default_vlm,
    });

    // Clean up test status
    setTestStatus((prev) => {
      const newStatus = { ...prev };
      delete newStatus[providerId];
      return newStatus;
    });
  };

  const updateProvider = (providerId: string, updates: Partial<ProviderConfig>) => {
    if (!config) return;

    setConfig({
      ...config,
      providers: {
        ...config.providers,
        [providerId]: {
          ...config.providers[providerId],
          ...updates,
        },
      },
    });

    setTestStatus((prev) => ({ ...prev, [providerId]: 'idle' }));
  };

  const getModelOptions = (): string[] => {
    if (!config) return [];
    const options: string[] = [];
    for (const [providerId, provider] of Object.entries(config.providers)) {
      for (const model of provider.models) {
        options.push(`${providerId}:${model}`);
      }
    }
    return options;
  };

  const getProviderStatusLabel = (providerId: string) => {
    const provider = config?.providers[providerId];
    if (!provider) return null;

    const hasKey = provider.api_key && provider.api_key.length > 0;
    const isOllama = provider.api_type === 'ollama';
    const hasModels = provider.models.length > 0;

    if (!hasModels) return <span className="status-badge warning">No models</span>;
    if (!isOllama && !hasKey) return <span className="status-badge warning">No API key</span>;

    const status = testStatus[providerId];
    switch (status) {
      case 'testing':
        return <span className="status-badge testing">Testing...</span>;
      case 'success':
        return <span className="status-badge success">Connected</span>;
      case 'error':
        return <span className="status-badge error">Failed</span>;
      default:
        return <span className="status-badge idle">Ready</span>;
    }
  };

  if (loading) {
    return <div className="loading">Loading AI configuration...</div>;
  }

  if (!config) {
    return <div className="error">Failed to load AI configuration</div>;
  }

  const modelOptions = getModelOptions();

  return (
    <div className="ai-settings">
      <h3>AI Configuration</h3>
      <p className="settings-description">
        Add AI providers (OpenAI, Anthropic, or custom endpoints), then select which model to use for LLM and VLM.
      </p>

      {error && <div className="error-message">{error}</div>}
      {success && <div className="success-message">Settings saved successfully!</div>}

      {/* Model Selection */}
      <div className="settings-section">
        <h4>Model Selection</h4>

        <div className="form-group">
          <label>Default LLM (Text Model)</label>
          <select
            value={config.default_llm || ''}
            onChange={(e) => setConfig({ ...config, default_llm: e.target.value || undefined })}
          >
            <option value="">-- Select a model --</option>
            {modelOptions.map((option) => {
              const [providerId, model] = option.split(':');
              const provider = config.providers[providerId];
              return (
                <option key={option} value={option}>
                  {provider?.name || providerId} / {model}
                </option>
              );
            })}
          </select>
          <span className="form-hint">Primary model for understanding pages and making decisions</span>
        </div>

        <div className="form-group">
          <label>Default VLM (Vision Model)</label>
          <select
            value={config.default_vlm || ''}
            onChange={(e) => setConfig({ ...config, default_vlm: e.target.value || undefined })}
          >
            <option value="">-- Select a model --</option>
            {modelOptions.map((option) => {
              const [providerId, model] = option.split(':');
              const provider = config.providers[providerId];
              return (
                <option key={option} value={option}>
                  {provider?.name || providerId} / {model}
                </option>
              );
            })}
          </select>
          <span className="form-hint">Vision model for screenshots (when LLM gets stuck)</span>
        </div>
      </div>

      {/* Providers List */}
      <div className="settings-section">
        <div className="section-header">
          <h4>Providers</h4>
          <button
            className="btn btn-sm btn-primary"
            onClick={() => setShowAddProvider(!showAddProvider)}
          >
            Add Provider
          </button>
        </div>

        {/* Add Provider Form */}
        {showAddProvider && (
          <div className="add-provider-form">
            <div className="form-row">
              <div className="form-group">
                <label>Provider ID *</label>
                <input
                  type="text"
                  value={newProviderId}
                  onChange={(e) => setNewProviderId(e.target.value)}
                  placeholder="e.g., openai, my-custom-llm"
                />
                <span className="form-hint">Unique identifier (lowercase, no spaces)</span>
              </div>
              <div className="form-group">
                <label>Display Name *</label>
                <input
                  type="text"
                  value={newProviderName}
                  onChange={(e) => setNewProviderName(e.target.value)}
                  placeholder="e.g., OpenAI, My Custom LLM"
                />
              </div>
            </div>

            <div className="form-group">
              <label>API Type</label>
              <select
                value={newProviderType}
                onChange={(e) => {
                  const type = e.target.value as ApiType;
                  setNewProviderType(type);
                  setNewProviderUrl(DEFAULT_URLS[type]);
                }}
              >
                {API_TYPES.map((t) => (
                  <option key={t.value} value={t.value}>
                    {t.label}
                  </option>
                ))}
              </select>
              <span className="form-hint">{API_TYPES.find(t => t.value === newProviderType)?.description}</span>
            </div>

            <div className="form-group">
              <label>Base URL</label>
              <input
                type="text"
                value={newProviderUrl}
                onChange={(e) => setNewProviderUrl(e.target.value)}
                placeholder="https://api.example.com/v1"
              />
            </div>

            <div className="form-group">
              <label>API Key {newProviderType === 'ollama' && '(optional for Ollama)'}</label>
              <input
                type="password"
                value={newProviderKey}
                onChange={(e) => setNewProviderKey(e.target.value)}
                placeholder={newProviderType === 'ollama' ? 'Not required for local Ollama' : 'Enter API key...'}
              />
            </div>

            <div className="form-group">
              <label>Models (comma-separated)</label>
              <input
                type="text"
                value={newProviderModels}
                onChange={(e) => setNewProviderModels(e.target.value)}
                placeholder="e.g., gpt-4o, gpt-4o-mini"
              />
              <span className="form-hint">Enter model names available from this provider</span>
            </div>

            <div className="form-actions">
              <button className="btn btn-secondary" onClick={() => setShowAddProvider(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={addProvider}
                disabled={!newProviderId.trim() || !newProviderName.trim()}
              >
                Add Provider
              </button>
            </div>
          </div>
        )}

        {/* Provider Cards */}
        {Object.entries(config.providers).map(([providerId, provider]) => (
          <div key={providerId} className="provider-card">
            <div className="provider-header">
              <div className="provider-title">
                <h5>{provider.name}</h5>
                <span className="provider-type">{provider.api_type}</span>
                {getProviderStatusLabel(providerId)}
              </div>
              <button
                className="btn-icon btn-danger"
                onClick={() => removeProvider(providerId)}
                title="Remove provider"
              >
                ×
              </button>
            </div>

            <div className="form-group">
              <label>API Key</label>
              <input
                type="password"
                value={provider.api_key || ''}
                onChange={(e) => updateProvider(providerId, { api_key: e.target.value || undefined })}
                placeholder={provider.api_type === 'ollama' ? 'Not required for Ollama' : 'Enter API key...'}
              />
            </div>

            <div className="form-group">
              <label>Base URL</label>
              <input
                type="text"
                value={provider.base_url}
                onChange={(e) => updateProvider(providerId, { base_url: e.target.value })}
              />
            </div>

            <div className="form-group">
              <label>Models (comma-separated)</label>
              <input
                type="text"
                value={provider.models.join(', ')}
                onChange={(e) =>
                  updateProvider(providerId, {
                    models: e.target.value
                      .split(',')
                      .map((m) => m.trim())
                      .filter((m) => m.length > 0),
                  })
                }
                placeholder="e.g., gpt-4o, gpt-4o-mini"
              />
            </div>

            <div className="provider-actions">
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => testConnection(providerId)}
                disabled={testStatus[providerId] === 'testing'}
              >
                {testStatus[providerId] === 'testing' ? 'Testing...' : 'Test Connection'}
              </button>
            </div>

            {testMessage[providerId] && (
              <div className={`test-result ${testStatus[providerId] === 'success' ? 'success' : 'error'}`}>
                {testMessage[providerId]}
              </div>
            )}
          </div>
        ))}

        {Object.keys(config.providers).length === 0 && (
          <div className="no-providers">
            <p>No providers configured. Click "Add Provider" to get started.</p>
          </div>
        )}
      </div>

      {/* General Settings */}
      <div className="settings-section">
        <h4>General Settings</h4>

        <div className="form-row">
          <div className="form-group">
            <label>Max Retries</label>
            <input
              type="number"
              value={config.max_retries}
              onChange={(e) => setConfig({ ...config, max_retries: parseInt(e.target.value) || 3 })}
              min={1}
              max={10}
            />
            <span className="form-hint">Retries before escalating to VLM</span>
          </div>

          <div className="form-group">
            <label>Timeout (seconds)</label>
            <input
              type="number"
              value={config.timeout_seconds}
              onChange={(e) => setConfig({ ...config, timeout_seconds: parseInt(e.target.value) || 300 })}
              min={60}
              max={3600}
            />
            <span className="form-hint">Maximum task duration</span>
          </div>
        </div>

        <div className="form-group">
          <label>
            <input
              type="checkbox"
              checked={config.escalation_enabled}
              onChange={(e) => setConfig({ ...config, escalation_enabled: e.target.checked })}
            />
            Enable VLM Escalation
          </label>
          <span className="form-hint">Automatically use vision model when text model gets stuck</span>
        </div>
      </div>

      {/* Save Button */}
      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
        <button className="btn btn-secondary" onClick={loadConfig} disabled={saving}>
          Reset
        </button>
      </div>
    </div>
  );
}
