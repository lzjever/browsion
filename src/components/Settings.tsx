import React, { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { tauriApi } from '../api/tauri';
import type { AppSettings, BrowserSource, CftVersionInfo, McpConfig } from '../types/profile';
import { open } from '@tauri-apps/plugin-dialog';

type CftDownloadProgress =
  | { phase: 'download'; loaded: number; total: number | null }
  | { phase: 'extracting' };

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export const Settings: React.FC = () => {
  const [effectivePath, setEffectivePath] = useState<string>('');
  const [browserSource, setBrowserSource] = useState<BrowserSource | null>(null);
  const [cftVersions, setCftVersions] = useState<CftVersionInfo[]>([]);
  const [customPath, setCustomPath] = useState('');
  const [fingerprintChromium, setFingerprintChromium] = useState(false);
  const [cftChannel, setCftChannel] = useState<string>('Stable');
  const [cftVersion, setCftVersion] = useState<string>('');
  const [downloadDir, setDownloadDir] = useState('');
  const [settings, setSettings] = useState<AppSettings>({
    auto_start: false,
    minimize_to_tray: true,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<CftDownloadProgress | null>(null);
  const [downloadCompleteMessage, setDownloadCompleteMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const successRef = React.useRef<HTMLDivElement>(null);

  // MCP / API Server
  const [mcpConfig, setMcpConfig] = useState<McpConfig>({
    enabled: true,
    api_port: 38472,
  });
  const [mcpSaving, setMcpSaving] = useState(false);
  const [mcpStatus, setMcpStatus] = useState<'checking' | 'running' | 'stopped'>('checking');

  useEffect(() => {
    loadSettings();
  }, []);

  useEffect(() => {
    const unlisten = listen<{ phase: string; loaded?: number; total?: number }>(
      'cft-download-progress',
      (event) => {
        const p = event.payload;
        if (p.phase === 'extracting') {
          setDownloadProgress({ phase: 'extracting' });
        } else if (p.phase === 'download' && typeof p.loaded === 'number') {
          setDownloadProgress({
            phase: 'download',
            loaded: p.loaded,
            total: typeof p.total === 'number' ? p.total : null,
          });
        }
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Load CfT versions on mount so Channel/Version dropdowns work regardless of current browser source
  useEffect(() => {
    loadCftVersions();
  }, []);

  const loadSettings = async () => {
    try {
      setLoading(true);
      const [path, source, appSettings, mcp] = await Promise.all([
        tauriApi.getChromePath(),
        tauriApi.getBrowserSource(),
        tauriApi.getSettings(),
        tauriApi.getMcpConfig(),
      ]);
      setEffectivePath(path);
      setBrowserSource(source);
      setSettings(appSettings);
      setMcpConfig(mcp);
      if (source?.type === 'custom') {
        setCustomPath(source.path);
        setFingerprintChromium(source.fingerprint_chromium ?? false);
      }
      if (source?.type === 'chrome_for_testing') {
        setCftChannel(source.channel);
        setCftVersion(source.version ?? '');
        setDownloadDir(source.download_dir);
      }
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const loadCftVersions = async () => {
    try {
      setLoadingVersions(true);
      const versions = await tauriApi.getCftVersions();
      setCftVersions(versions);
      if (versions.length > 0 && !cftVersion) {
        const stable = versions.find((v) => v.channel === 'Stable');
        if (stable) setCftVersion(stable.version);
      }
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingVersions(false);
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
        setCustomPath(selected);
      }
    } catch (err) {
      console.error('Failed to open file dialog:', err);
    }
  };

  const handleSaveCustomPath = async () => {
    try {
      setSaving(true);
      setError(null);
      setSuccess(null);
      const newSource: BrowserSource = {
        type: 'custom',
        path: customPath,
        fingerprint_chromium: fingerprintChromium,
      };
      await tauriApi.updateBrowserSource(newSource);
      setBrowserSource(newSource);
      const path = await tauriApi.getChromePath();
      setEffectivePath(path);
      setSuccess('Custom Chrome path saved');
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleUseCft = async () => {
    try {
      setSaving(true);
      setError(null);
      setSuccess(null);
      const newSource: BrowserSource = {
        type: 'chrome_for_testing',
        channel: cftChannel as 'Stable' | 'Beta' | 'Dev' | 'Canary',
        version: cftVersion || undefined,
        download_dir: downloadDir || (browserSource?.type === 'chrome_for_testing' ? browserSource.download_dir : undefined),
      };
      await tauriApi.updateBrowserSource(newSource);
      setBrowserSource(newSource);
      const path = await tauriApi.getChromePath();
      setEffectivePath(path);
      setSuccess(`Now using Chrome for Testing (${cftChannel} ${cftVersion || 'latest'}). Path updated above.`);
      setTimeout(() => setSuccess(null), 5000);
      successRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDownloadCft = async () => {
    const versionInfo = cftVersions.find(
      (v) => v.channel === cftChannel && v.version === cftVersion
    ) || cftVersions.find((v) => v.channel === cftChannel);
    if (!versionInfo) {
      setError('Select a channel and version first');
      return;
    }
    try {
      setDownloading(true);
      setDownloadProgress(null);
      setError(null);
      const dir = downloadDir || (browserSource?.type === 'chrome_for_testing' ? browserSource.download_dir : undefined);
      const path = await tauriApi.downloadCftVersion(
        versionInfo.channel,
        versionInfo.version,
        dir || undefined
      );
      setEffectivePath(path);
      setSuccess(`Downloaded: ${versionInfo.channel} ${versionInfo.version}`);
      setDownloadCompleteMessage(`${versionInfo.channel} ${versionInfo.version} ready. Click "Use Chrome for Testing" to switch.`);
      setTimeout(() => setSuccess(null), 5000);
      setTimeout(() => setDownloadCompleteMessage(null), 5000);
      successRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      await loadSettings();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDownloading(false);
      setDownloadProgress(null);
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
      setSettings(settings);
    }
  };

  const checkMcpStatus = useCallback(async () => {
    if (!mcpConfig.enabled || mcpConfig.api_port === 0) {
      setMcpStatus('stopped');
      return;
    }
    try {
      const resp = await fetch(`http://127.0.0.1:${mcpConfig.api_port}/api/health`);
      setMcpStatus(resp.ok ? 'running' : 'stopped');
    } catch {
      setMcpStatus('stopped');
    }
  }, [mcpConfig.enabled, mcpConfig.api_port]);

  useEffect(() => {
    if (!loading) {
      checkMcpStatus();
    }
  }, [loading, checkMcpStatus]);

  const handleMcpSave = async () => {
    try {
      setMcpSaving(true);
      setError(null);
      await tauriApi.updateMcpConfig(mcpConfig);
      setSuccess('MCP / API server configuration saved');
      setTimeout(() => setSuccess(null), 3000);
      // Give server a moment to start, then check status
      setTimeout(() => checkMcpStatus(), 500);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMcpSaving(false);
    }
  };

  const generateApiKey = () => {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    const array = new Uint8Array(32);
    crypto.getRandomValues(array);
    let key = 'sk-';
    for (let i = 0; i < 32; i++) {
      key += chars[array[i] % chars.length];
    }
    setMcpConfig({ ...mcpConfig, api_key: key });
  };

  const mcpClientConfig = JSON.stringify(
    {
      mcpServers: {
        browsion: {
          command: 'browsion-mcp',
          env: {
            BROWSION_API_PORT: String(mcpConfig.api_port),
            ...(mcpConfig.api_key ? { BROWSION_API_KEY: mcpConfig.api_key } : {}),
          },
        },
      },
    },
    null,
    2
  );

  if (loading) {
    return <div className="loading">Loading settings...</div>;
  }

  return (
    <div className="settings-container">
      <h2>Settings</h2>

      {error && <div className="error-message">{error}</div>}
      {success && (
        <div ref={successRef} className="success-message success-message-prominent">
          {success}
        </div>
      )}

      <div className="settings-content">
        <div className="settings-section">
          <h3>Browser (Chrome)</h3>
          <p className="settings-hint">
            Current selection is shown above; choose an option below to change it.
          </p>

          <div className="browser-current-selection">
            <div className="browser-current-selection-label">Current selection</div>
            <div className="form-group">
              <label>Effective Chrome path</label>
              {browserSource && (
                <div className="effective-path-source">
                  {browserSource.type === 'chrome_for_testing'
                    ? `Chrome for Testing (${browserSource.channel}${browserSource.version ? ` ${browserSource.version}` : ''})`
                    : 'Custom path'}
                </div>
              )}
              <input
                type="text"
                readOnly
                value={effectivePath}
                className="readonly"
              />
            </div>
          </div>

          <div className="browser-source-options-label">Choose browser source</div>
          <div className="browser-source-options">
            <div className="browser-option-block">
              <h4 className="browser-option-title">Chrome for Testing (official)</h4>
          <div className="form-group">
            <label>Channel</label>
            <select
              value={cftChannel}
              onChange={(e) => {
                setCftChannel(e.target.value);
                const v = cftVersions.find((x) => x.channel === e.target.value);
                if (v) setCftVersion(v.version);
              }}
            >
              <option value="Stable">Stable</option>
              <option value="Beta">Beta</option>
              <option value="Dev">Dev</option>
              <option value="Canary">Canary</option>
            </select>
          </div>
          <div className="form-group">
            <label>Version</label>
            <select
              value={cftVersion}
              onChange={(e) => setCftVersion(e.target.value)}
              disabled={loadingVersions}
            >
              {cftVersions
                .filter((v) => v.channel === cftChannel)
                .map((v) => (
                  <option key={v.version} value={v.version}>
                    {v.version}
                  </option>
                ))}
            </select>
            {loadingVersions && <span className="muted">Loading versions…</span>}
          </div>
          <div className="form-group">
            <label>Download directory</label>
            <input
              type="text"
              value={downloadDir}
              onChange={(e) => setDownloadDir(e.target.value)}
              placeholder="~/.browsion/cft (default)"
            />
          </div>
          {downloadProgress && (
            <div className="cft-download-progress form-group">
              <div className="cft-download-progress-label">
                {downloadProgress.phase === 'extracting'
                  ? 'Extracting…'
                  : `Downloading… ${formatBytes(downloadProgress.loaded)}${downloadProgress.total != null ? ` / ${formatBytes(downloadProgress.total)}` : ''}`}
              </div>
              <div className="cft-download-progress-bar">
                <div
                  className="cft-download-progress-fill"
                  style={{
                    width:
                      downloadProgress.phase === 'download' &&
                      downloadProgress.total != null &&
                      downloadProgress.total > 0
                        ? `${Math.round((downloadProgress.loaded / downloadProgress.total) * 100)}%`
                        : downloadProgress.phase === 'extracting'
                          ? '100%'
                          : '0%',
                  }}
                />
              </div>
            </div>
          )}
          {downloadCompleteMessage && !downloadProgress && (
            <div className="cft-download-complete form-group">
              <span className="cft-download-complete-icon">✓</span>
              <span className="cft-download-complete-text">{downloadCompleteMessage}</span>
            </div>
          )}
          <div className="button-row">
            <button
              className="btn btn-primary"
              onClick={handleDownloadCft}
              disabled={loadingVersions || downloading}
            >
              {downloading ? 'Downloading…' : 'Download this version'}
            </button>
            <button
              className="btn btn-secondary"
              onClick={handleUseCft}
              disabled={saving}
            >
              {saving ? 'Switching…' : 'Use Chrome for Testing'}
            </button>
          </div>
            </div>

            <div className="browser-option-block">
              <h4 className="browser-option-title">Custom path (e.g. ungoogled)</h4>
          <div className="form-group">
            <label htmlFor="chrome-path">Chrome executable path</label>
            <div className="input-with-button">
              <input
                type="text"
                id="chrome-path"
                value={customPath}
                onChange={(e) => setCustomPath(e.target.value)}
                placeholder="/path/to/chromium"
              />
              <button className="btn btn-secondary" onClick={handleBrowseChrome}>
                Browse
              </button>
            </div>
          </div>
          <div className="form-group checkbox-group">
            <label>
              <input
                type="checkbox"
                checked={fingerprintChromium}
                onChange={(e) => setFingerprintChromium(e.target.checked)}
              />
              <span>This is adryfish/fingerprint-chromium</span>
            </label>
            <p className="form-hint">
              When enabled, profile edit will show fingerprint, timezone and language options per{' '}
              <a href="https://github.com/adryfish/fingerprint-chromium" target="_blank" rel="noopener noreferrer">
                fingerprint-chromium
              </a>.
            </p>
          </div>
          <button
            className="btn btn-primary"
            onClick={handleSaveCustomPath}
            disabled={saving}
          >
            {saving ? 'Saving…' : 'Use custom path'}
          </button>
            </div>
          </div>
        </div>

        <div className="settings-section">
          <h3>MCP / API Server</h3>

          <div className="checkbox-group">
            <label>
              <input
                type="checkbox"
                checked={mcpConfig.enabled}
                onChange={(e) =>
                  setMcpConfig({ ...mcpConfig, enabled: e.target.checked })
                }
              />
              <span>Enable API server</span>
            </label>
          </div>

          <div className="form-group">
            <label>Port</label>
            <input
              type="number"
              min={1024}
              max={65535}
              value={mcpConfig.api_port}
              onChange={(e) =>
                setMcpConfig({
                  ...mcpConfig,
                  api_port: parseInt(e.target.value) || 38472,
                })
              }
              disabled={!mcpConfig.enabled}
            />
          </div>

          <div className="form-group">
            <label>API Key (optional)</label>
            <div className="input-with-buttons">
              <input
                type="text"
                value={mcpConfig.api_key ?? ''}
                onChange={(e) =>
                  setMcpConfig({
                    ...mcpConfig,
                    api_key: e.target.value || undefined,
                  })
                }
                placeholder="No authentication"
                disabled={!mcpConfig.enabled}
              />
              <button
                className="btn btn-secondary btn-small"
                onClick={generateApiKey}
                disabled={!mcpConfig.enabled}
              >
                Generate
              </button>
              <button
                className="btn btn-secondary btn-small"
                onClick={() => {
                  if (mcpConfig.api_key) {
                    navigator.clipboard.writeText(mcpConfig.api_key);
                  }
                }}
                disabled={!mcpConfig.enabled || !mcpConfig.api_key}
              >
                Copy
              </button>
            </div>
          </div>

          <div className="mcp-status-row">
            <span className="mcp-status-label">Status:</span>
            <span className={`mcp-status-badge mcp-status-${mcpStatus}`}>
              {mcpStatus === 'running'
                ? `Running on http://127.0.0.1:${mcpConfig.api_port}`
                : mcpStatus === 'checking'
                  ? 'Checking…'
                  : 'Stopped'}
            </span>
          </div>

          <button
            className="btn btn-primary"
            onClick={handleMcpSave}
            disabled={mcpSaving}
            style={{ marginBottom: '16px' }}
          >
            {mcpSaving ? 'Saving…' : 'Apply'}
          </button>

          <div className="mcp-config-snippet">
            <label>MCP Client Config (for Claude Desktop / Cursor)</label>
            <pre className="mcp-config-code">{mcpClientConfig}</pre>
            <button
              className="btn btn-secondary btn-small"
              onClick={() => navigator.clipboard.writeText(mcpClientConfig)}
            >
              Copy Config
            </button>
          </div>
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
      </div>
    </div>
  );
};
