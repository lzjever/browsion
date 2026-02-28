import React, { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile, ActionEntry } from '../types/profile';

interface Thumbnail {
  profileId: string;
  url: string;
  title: string;
  src: string; // data:image/jpeg;base64,...
}

export const MonitorPage: React.FC = () => {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [runningIds, setRunningIds] = useState<string[]>([]);
  const [thumbnails, setThumbnails] = useState<Record<string, Thumbnail>>({});
  const [allActions, setAllActions] = useState<ActionEntry[]>([]);
  const [mcpConfig, setMcpConfig] = useState<{ api_port: number; api_key?: string } | null>(null);
  const [paused, setPaused] = useState(false);
  const [profileFilter, setProfileFilter] = useState('');
  const [searchTerm, setSearchTerm] = useState('');

  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  const mcpConfigRef = useRef(mcpConfig);
  mcpConfigRef.current = mcpConfig;

  const runningIdsRef = useRef(runningIds);
  runningIdsRef.current = runningIds;

  // Load initial data
  useEffect(() => {
    tauriApi.getProfiles().then(setProfiles).catch(console.error);
    tauriApi.getMcpConfig().then((cfg) => setMcpConfig(cfg)).catch(console.error);

    const loadRunning = async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningIds(Object.entries(status).filter(([, v]) => v).map(([k]) => k));
      } catch {
        // ignore
      }
    };
    loadRunning();
  }, []);

  // Listen for browser status changes
  useEffect(() => {
    const unlisten = listen('browser-status-changed', async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningIds(Object.entries(status).filter(([, v]) => v).map(([k]) => k));
      } catch {
        // ignore
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Screenshot polling (3s)
  const pollScreenshots = useCallback(async () => {
    const cfg = mcpConfigRef.current;
    const ids = runningIdsRef.current;
    if (!cfg || ids.length === 0 || pausedRef.current) return;

    const base = `http://127.0.0.1:${cfg.api_port}`;
    const headers: Record<string, string> = {};
    if (cfg.api_key) headers['X-API-Key'] = cfg.api_key;

    for (const id of ids) {
      try {
        const res = await fetch(
          `${base}/api/browser/${id}/screenshot?format=jpeg&quality=60`,
          { headers }
        );
        if (res.ok) {
          const data = await res.json();
          if (data.screenshot) {
            // Also grab URL + title
            let url = '';
            let title = '';
            try {
              const pageRes = await fetch(`${base}/api/browser/${id}/url`, { headers });
              if (pageRes.ok) {
                const d = await pageRes.json();
                url = d.url || '';
              }
              const titleRes = await fetch(`${base}/api/browser/${id}/title`, { headers });
              if (titleRes.ok) {
                const d = await titleRes.json();
                title = d.title || '';
              }
            } catch {
              // ignore
            }
            setThumbnails((prev) => ({
              ...prev,
              [id]: {
                profileId: id,
                src: `data:image/jpeg;base64,${data.screenshot}`,
                url,
                title,
              },
            }));
          }
        }
      } catch {
        // browser may not be ready
      }
    }
  }, []);

  // Action log polling (5s)
  const pollActions = useCallback(async () => {
    const cfg = mcpConfigRef.current;
    if (!cfg || pausedRef.current) return;

    const base = `http://127.0.0.1:${cfg.api_port}`;
    const headers: Record<string, string> = {};
    if (cfg.api_key) headers['X-API-Key'] = cfg.api_key;

    try {
      const res = await fetch(`${base}/api/action_log?limit=200`, { headers });
      if (res.ok) {
        const data: ActionEntry[] = await res.json();
        setAllActions(data);
      }
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    if (!mcpConfig) return;

    // Initial fetch
    pollScreenshots();
    pollActions();

    const ssInterval = setInterval(pollScreenshots, 3000);
    const actInterval = setInterval(pollActions, 5000);

    // Pause when tab hidden
    const onVisChange = () => {
      setPaused(document.hidden);
    };
    document.addEventListener('visibilitychange', onVisChange);

    return () => {
      clearInterval(ssInterval);
      clearInterval(actInterval);
      document.removeEventListener('visibilitychange', onVisChange);
    };
  }, [mcpConfig, pollScreenshots, pollActions]);

  const getProfileName = (id: string) =>
    profiles.find((p) => p.id === id)?.name ?? id;

  // Per-profile top-5 actions
  const topActions = (profileId: string): ActionEntry[] =>
    allActions.filter((a) => a.profile_id === profileId).slice(0, 5);

  // Cookie export helper
  const handleExportCookies = async (profileId: string, format: 'json' | 'netscape') => {
    const cfg = mcpConfigRef.current;
    if (!cfg) return;
    const headers: Record<string, string> = {};
    if (cfg.api_key) headers['X-API-Key'] = cfg.api_key;
    try {
      const res = await fetch(
        `http://127.0.0.1:${cfg.api_port}/api/browser/${profileId}/cookies/export?format=${format}`,
        { headers }
      );
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = format === 'json' ? 'cookies.json' : 'cookies.txt';
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      alert(`Export failed: ${e}`);
    }
  };

  // Cookie import helper
  const handleImportCookies = async (profileId: string, format: 'json' | 'netscape') => {
    const cfg = mcpConfigRef.current;
    if (!cfg) return;

    const input = document.createElement('input');
    input.type = 'file';
    input.accept = format === 'json' ? '.json' : '.txt';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const data = await file.text();
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };
      if (cfg.api_key) headers['X-API-Key'] = cfg.api_key;
      try {
        const res = await fetch(
          `http://127.0.0.1:${cfg.api_port}/api/browser/${profileId}/cookies/import`,
          {
            method: 'POST',
            headers,
            body: JSON.stringify({ format, data }),
          }
        );
        const result = await res.json();
        alert(`Imported ${result.imported} cookies${result.errors?.length ? `, ${result.errors.length} errors` : ''}`);
      } catch (e) {
        alert(`Import failed: ${e}`);
      }
    };
    input.click();
  };

  // Filtered action log
  const filteredActions = allActions.filter((a) => {
    if (profileFilter && a.profile_id !== profileFilter) return false;
    if (searchTerm) {
      const q = searchTerm.toLowerCase();
      return (
        a.tool.toLowerCase().includes(q) ||
        a.profile_id.toLowerCase().includes(q) ||
        getProfileName(a.profile_id).toLowerCase().includes(q)
      );
    }
    return true;
  });

  return (
    <div className="monitor-page">
      <h2>Activity Monitor</h2>

      {runningIds.length === 0 ? (
        <p className="muted">No browsers running. Launch a profile to see live data.</p>
      ) : (
        <div className="monitor-thumbnails">
          {runningIds.map((id) => {
            const thumb = thumbnails[id];
            const actions = topActions(id);
            return (
              <div key={id} className="monitor-card">
                <div className="monitor-card-header">
                  <span className="monitor-profile-name">{getProfileName(id)}</span>
                  <span className="status-indicator running">● Running</span>
                </div>

                {thumb ? (
                  <img
                    src={thumb.src}
                    alt="screenshot"
                    className="monitor-thumbnail"
                  />
                ) : (
                  <div className="monitor-thumbnail-placeholder">Loading…</div>
                )}

                {thumb && (
                  <div className="monitor-url" title={thumb.url}>
                    {thumb.title && <strong>{thumb.title}</strong>}
                    <small>{thumb.url}</small>
                  </div>
                )}

                {actions.length > 0 && (
                  <div className="monitor-recent-actions">
                    <div className="monitor-actions-label">Recent actions</div>
                    {actions.map((a) => (
                      <div key={a.id} className={`monitor-action-row ${a.success ? '' : 'failed'}`}>
                        <span className="monitor-action-tool">{a.tool}</span>
                        <span className="monitor-action-dur">{a.duration_ms}ms</span>
                        <span className={`monitor-action-status ${a.success ? 'ok' : 'err'}`}>
                          {a.success ? '✓' : '✕'}
                        </span>
                      </div>
                    ))}
                  </div>
                )}

                <div className="monitor-card-footer">
                  <div className="monitor-cookie-btns">
                    <span className="monitor-label">Cookies:</span>
                    <button className="btn btn-secondary btn-sm" onClick={() => handleExportCookies(id, 'json')}>Export JSON</button>
                    <button className="btn btn-secondary btn-sm" onClick={() => handleExportCookies(id, 'netscape')}>Export TXT</button>
                    <button className="btn btn-secondary btn-sm" onClick={() => handleImportCookies(id, 'json')}>Import JSON</button>
                    <button className="btn btn-secondary btn-sm" onClick={() => handleImportCookies(id, 'netscape')}>Import TXT</button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <div className="monitor-log-section">
        <div className="monitor-log-header">
          <h3>Action Log</h3>
          <div className="monitor-log-controls">
            <select
              value={profileFilter}
              onChange={(e) => setProfileFilter(e.target.value)}
            >
              <option value="">All profiles</option>
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
            <input
              type="text"
              placeholder="Search tool / profile…"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
            <button
              className={`btn btn-secondary btn-sm ${paused ? 'active' : ''}`}
              onClick={() => setPaused((p) => !p)}
            >
              {paused ? '▶ Resume' : '⏸ Pause'}
            </button>
          </div>
        </div>

        {filteredActions.length === 0 ? (
          <p className="muted">No actions logged yet. API calls appear here automatically.</p>
        ) : (
          <table className="action-log-table">
            <thead>
              <tr>
                <th>Time</th>
                <th>Profile</th>
                <th>Tool</th>
                <th>Duration</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {filteredActions.map((a) => (
                <tr key={a.id} className={a.success ? '' : 'row-error'}>
                  <td className="log-ts">{new Date(a.ts).toLocaleTimeString()}</td>
                  <td>{getProfileName(a.profile_id) || a.profile_id}</td>
                  <td className="log-tool">{a.tool}</td>
                  <td>{a.duration_ms}ms</td>
                  <td>
                    {a.success ? (
                      <span className="log-ok">✓</span>
                    ) : (
                      <span className="log-err" title={a.error}>✕ {a.error}</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
};
