import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { tauriApi } from '../api/tauri';
import type { AppSettings, BrowserSource, CftVersionInfo, ProxyPreset } from '../types/profile';
import { open } from '@tauri-apps/plugin-dialog';
import { UI_CONSTANTS } from './constants';

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

  // Proxy presets
  const [proxyPresets, setProxyPresets] = useState<ProxyPreset[]>([]);
  const [newProxyName, setNewProxyName] = useState('');
  const [newProxyUrl, setNewProxyUrl] = useState('');
  const [testingProxy, setTestingProxy] = useState<string | null>(null);
  const [proxyLatencies, setProxyLatencies] = useState<Record<string, number | 'error'>>({});

  useEffect(() => {
    loadSettings();
    loadProxyPresets();
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
      const [path, source, appSettings] = await Promise.all([
        tauriApi.getChromePath(),
        tauriApi.getBrowserSource(),
        tauriApi.getSettings(),
      ]);
      setEffectivePath(path);
      setBrowserSource(source);
      setSettings(appSettings);
      if (source?.type === 'custom') {
        setCustomPath(source.path);
        setFingerprintChromium(source.fingerprint_chromium ?? false);
      }
      if (source?.type === 'chrome_for_testing') {
        setCftChannel(source.channel);
        setCftVersion(source.version ?? '');
        setDownloadDir(source.download_dir ?? '');
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
      setTimeout(() => setSuccess(null), UI_CONSTANTS.SUCCESS_MESSAGE_DURATION_MS);
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
      setTimeout(() => setSuccess(null), UI_CONSTANTS.LONG_SUCCESS_MESSAGE_DURATION_MS);
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
      setTimeout(() => setSuccess(null), UI_CONSTANTS.LONG_SUCCESS_MESSAGE_DURATION_MS);
      setTimeout(() => setDownloadCompleteMessage(null), UI_CONSTANTS.LONG_SUCCESS_MESSAGE_DURATION_MS);
      successRef.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      await loadSettings();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDownloading(false);
      setDownloadProgress(null);
    }
  };

  const loadProxyPresets = async () => {
    try {
      const presets = await tauriApi.getProxyPresets();
      setProxyPresets(presets);
    } catch {
      // non-fatal
    }
  };

  const handleAddProxy = async () => {
    const name = newProxyName.trim();
    const url = newProxyUrl.trim();
    if (!name || !url) return;
    try {
      await tauriApi.addProxyPreset(name, url);
      setNewProxyName('');
      setNewProxyUrl('');
      await loadProxyPresets();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDeleteProxy = async (id: string) => {
    try {
      await tauriApi.deleteProxyPreset(id);
      await loadProxyPresets();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleTestProxy = async (preset: ProxyPreset) => {
    setTestingProxy(preset.id);
    try {
      const ms = await tauriApi.testProxy(preset.url);
      setProxyLatencies((prev) => ({ ...prev, [preset.id]: ms }));
    } catch {
      setProxyLatencies((prev) => ({ ...prev, [preset.id]: 'error' }));
    } finally {
      setTestingProxy(null);
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
                        ? `${Math.round((downloadProgress.loaded / downloadProgress.total) * UI_CONSTANTS.PERCENTAGE_MULTIPLIER)}%`
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

        <div className="settings-section">
          <h3>Proxy Presets</h3>
          <p className="settings-hint">Save proxy URLs for quick reuse in profiles.</p>

          {proxyPresets.length > 0 && (
            <table className="proxy-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>URL</th>
                  <th>Latency</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {proxyPresets.map((p) => {
                  const lat = proxyLatencies[p.id];
                  return (
                    <tr key={p.id}>
                      <td>{p.name}</td>
                      <td className="proxy-url">{p.url}</td>
                      <td>
                        {lat === undefined ? '' : lat === 'error' ? (
                          <span className="proxy-latency-error">✕ fail</span>
                        ) : (
                          <span className={`proxy-latency ${lat < 500 ? 'fast' : lat < 1500 ? 'ok' : 'slow'}`}>{lat} ms</span>
                        )}
                      </td>
                      <td>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleTestProxy(p)}
                          disabled={testingProxy === p.id}
                        >
                          {testingProxy === p.id ? '…' : 'Test'}
                        </button>
                        <button
                          className="btn btn-danger-outline btn-sm"
                          onClick={() => handleDeleteProxy(p.id)}
                        >
                          Delete
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}

          <div className="proxy-add-row">
            <input
              type="text"
              value={newProxyName}
              onChange={(e) => setNewProxyName(e.target.value)}
              placeholder="Name (e.g. Home)"
            />
            <input
              type="text"
              value={newProxyUrl}
              onChange={(e) => setNewProxyUrl(e.target.value)}
              placeholder="http://user:pass@host:port"
              style={{ flex: 2 }}
            />
            <button
              className="btn btn-primary"
              onClick={handleAddProxy}
              disabled={!newProxyName.trim() || !newProxyUrl.trim()}
            >
              Add
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
