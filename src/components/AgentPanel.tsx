import { useState, useEffect, useCallback } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile } from '../types/profile';
import type { AgentOptions, AgentProgress, AgentStatus, TemplateInfo, BatchProgress } from '../types/agent';

interface AgentPanelProps {
  profiles: BrowserProfile[];
}

const statusLabels: Record<AgentStatus, string> = {
  initializing: 'Initializing...',
  running: 'Running',
  paused: 'Paused',
  completed: 'Completed',
  failed: 'Failed',
  stopped: 'Stopped',
};

const statusColors: Record<AgentStatus, string> = {
  initializing: 'var(--warning-color)',
  running: 'var(--success-color)',
  paused: 'var(--warning-color)',
  completed: 'var(--success-color)',
  failed: 'var(--danger-color)',
  stopped: 'var(--secondary-color)',
};

// Generate recovery suggestions based on error message
function getRecoverySuggestions(error: string | null, status: AgentStatus | null): string[] {
  if (!error || status !== 'failed') return [];

  const suggestions: string[] = [];
  const errorLower = error.toLowerCase();

  if (errorLower.includes('api key') || errorLower.includes('unauthorized') || errorLower.includes('401')) {
    suggestions.push('Check that your API key is valid and has not expired');
    suggestions.push('Verify the API key is correctly configured in Settings > AI Configuration');
    suggestions.push('Ensure you have sufficient API credits/quota');
  }

  if (errorLower.includes('timeout') || errorLower.includes('timed out')) {
    suggestions.push('The task may be too complex - try breaking it into smaller steps');
    suggestions.push('Check your network connection stability');
    suggestions.push('Increase the timeout in AI Configuration settings');
  }

  if (errorLower.includes('element not found') || errorLower.includes('selector')) {
    suggestions.push('The page structure may have changed - try refreshing and running again');
    suggestions.push('The website may be using dynamic content - try with headless mode disabled');
    suggestions.push('Consider providing more specific instructions in your task description');
  }

  if (errorLower.includes('chrome') || errorLower.includes('browser') || errorLower.includes('cdp')) {
    suggestions.push('Verify Chrome/Chromium is installed at the configured path');
    suggestions.push('Close any existing Chrome instances using the same profile');
    suggestions.push('Check that the profile directory exists and has proper permissions');
  }

  if (errorLower.includes('network') || errorLower.includes('connection') || errorLower.includes('dns')) {
    suggestions.push('Check your internet connection');
    suggestions.push('If using a proxy, verify the proxy settings are correct');
    suggestions.push('Try accessing the target URL manually in a browser');
  }

  if (errorLower.includes('rate limit') || errorLower.includes('429')) {
    suggestions.push('You have exceeded the API rate limit - wait a few minutes and try again');
    suggestions.push('Consider using a different AI provider or upgrading your plan');
  }

  if (errorLower.includes('consecutive failures') || errorLower.includes('too many')) {
    suggestions.push('The agent encountered repeated failures - try simplifying the task');
    suggestions.push('Disable headless mode to see what the agent is doing');
    suggestions.push('Try providing more detailed step-by-step instructions');
  }

  // Default suggestions if no specific matches
  if (suggestions.length === 0) {
    suggestions.push('Try running the task again - sometimes transient errors occur');
    suggestions.push('Check the AI Configuration in Settings to ensure providers are set up correctly');
    suggestions.push('Try disabling headless mode to observe the agent behavior');
    suggestions.push('Simplify your task description and break it into smaller steps');
  }

  return suggestions;
}

export function AgentPanel({ profiles }: AgentPanelProps) {
  const [selectedProfile, setSelectedProfile] = useState<string>('');
  const [selectedProfiles, setSelectedProfiles] = useState<Set<string>>(new Set());
  const [batchMode, setBatchMode] = useState(false);
  const [task, setTask] = useState('');
  const [headless, setHeadless] = useState(false);
  const [startUrl, setStartUrl] = useState('');

  const [agentId, setAgentId] = useState<string | null>(null);
  const [progress, setProgress] = useState<AgentProgress | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [logs, setLogs] = useState<string[]>([]);

  // Batch execution state
  const [batchProgress, setBatchProgress] = useState<BatchProgress | null>(null);
  const [, setBatchAgentProgress] = useState<Map<string, AgentProgress>>(new Map());

  // Template state
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [showTemplateModal, setShowTemplateModal] = useState(false);
  const [showSaveAsTemplate, setShowSaveAsTemplate] = useState(false);
  const [newTemplateName, setNewTemplateName] = useState('');

  // Load templates on mount
  useEffect(() => {
    loadTemplates();
  }, []);

  const loadTemplates = async () => {
    try {
      const loadedTemplates = await tauriApi.getTemplates();
      setTemplates(loadedTemplates);
    } catch (e) {
      console.error('Failed to load templates:', e);
    }
  };

  // Select first profile by default
  useEffect(() => {
    if (profiles.length > 0 && !selectedProfile) {
      setSelectedProfile(profiles[0].id);
    }
  }, [profiles, selectedProfile]);

  // Listen for agent progress events
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<AgentProgress>('agent-progress', (event) => {
        const progress = event.payload;
        console.log('[AgentPanel] Received progress event:', progress.agent_id, progress.status);

        // Single agent mode - accept any progress event when we're running
        if (!batchMode) {
          setProgress(progress);
          // Auto-set agentId from first progress event if not set
          setAgentId(prev => prev || progress.agent_id);
          if (progress.status === 'completed' || progress.status === 'failed' || progress.status === 'stopped') {
            setLoading(false);
            if (progress.error) {
              setError(progress.error);
            }
          }
        }

        // Batch mode - track all agent progress
        if (batchMode && batchProgress) {
          setBatchAgentProgress(prev => {
            const newMap = new Map(prev);
            newMap.set(progress.agent_id, progress);
            return newMap;
          });

          // Update batch progress based on individual completions
          if (progress.status === 'completed' || progress.status === 'failed' || progress.status === 'stopped') {
            setBatchProgress(prev => {
              if (!prev) return null;
              const profileId = prev.agents[progress.agent_id];
              if (!profileId) return prev;

              const newResults = { ...prev.results };
              const newErrors = { ...prev.errors };

              if (progress.status === 'completed' && progress.result) {
                newResults[profileId] = progress.result;
              } else if (progress.status === 'failed' && progress.error) {
                newErrors[profileId] = progress.error;
              }

              return {
                ...prev,
                completed: progress.status === 'completed' ? prev.completed + 1 : prev.completed,
                failed: progress.status === 'failed' ? prev.failed + 1 : prev.failed,
                total_cost: prev.total_cost + progress.cost,
                results: newResults,
                errors: newErrors,
              };
            });
          }
        }
      });
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [batchMode, batchProgress]);  // Removed agentId from dependencies

  // Check if batch execution is complete
  useEffect(() => {
    if (batchMode && batchProgress) {
      const totalDone = batchProgress.completed + batchProgress.failed;
      if (totalDone >= batchProgress.total) {
        setLoading(false);
        setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Batch execution complete: ${batchProgress.completed} succeeded, ${batchProgress.failed} failed`]);
      }
    }
  }, [batchProgress, batchMode]);

  // Add log entry when progress changes
  useEffect(() => {
    if (progress?.message && !batchMode) {
      setLogs((prev) => {
        const newLog = `[${new Date().toLocaleTimeString()}] ${progress.message}`;
        if (prev[prev.length - 1] !== newLog) {
          return [...prev.slice(-50), newLog];
        }
        return prev;
      });
    }
  }, [progress?.message, batchMode]);

  const handleRunAgent = useCallback(async () => {
    const profilesToRun = batchMode ? Array.from(selectedProfiles) : [selectedProfile];
    const validProfiles = profilesToRun.filter(id => id && task.trim());

    if (validProfiles.length === 0) {
      setError('Please select at least one profile and enter a task');
      return;
    }

    setLoading(true);
    setError(null);
    setLogs([]);
    setProgress(null);

    const options: AgentOptions = {
      headless,
      max_steps: 50,
      start_url: startUrl || undefined,
    };

    // Batch mode
    if (batchMode && validProfiles.length > 1) {
      const batchId = crypto.randomUUID();
      const agents: Record<string, string> = {};
      const results: Record<string, any> = {};
      const errors: Record<string, string> = {};

      setBatchProgress({
        batch_id: batchId,
        total: validProfiles.length,
        completed: 0,
        failed: 0,
        agents,
        results,
        errors,
        total_cost: 0,
      });
      setBatchAgentProgress(new Map());

      setLogs([`[${new Date().toLocaleTimeString()}] Starting batch execution for ${validProfiles.length} profiles...`]);

      // Run agents sequentially to avoid overwhelming the system
      for (const profileId of validProfiles) {
        const profileName = profiles.find(p => p.id === profileId)?.name || profileId;
        setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Starting agent for profile: ${profileName}`]);

        try {
          const id = await tauriApi.runAgent(profileId, task, options);
          agents[id] = profileId;

          // Update batch progress with new agent
          setBatchProgress(prev => prev ? { ...prev, agents: { ...prev.agents, [id]: profileId }, current_profile: profileId } : null);

          // Wait for this agent to complete before starting next
          await new Promise<void>(resolve => {
            const checkComplete = setInterval(() => {
              setBatchAgentProgress(prev => {
                const agentProgress = prev.get(id);
                if (agentProgress && (agentProgress.status === 'completed' || agentProgress.status === 'failed' || agentProgress.status === 'stopped')) {
                  clearInterval(checkComplete);
                  resolve();
                }
                return prev;
              });
            }, 500);
          });
        } catch (e) {
          errors[profileId] = String(e);
          setBatchProgress(prev => prev ? { ...prev, errors, failed: prev.failed + 1 } : null);
          setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Failed to start agent for ${profileName}: ${e}`]);
        }
      }

      return;
    }

    // Single agent mode
    try {
      const id = await tauriApi.runAgent(validProfiles[0], task, options);
      setAgentId(id);
      setLogs([`[${new Date().toLocaleTimeString()}] Agent started with ID: ${id}`]);
    } catch (e) {
      setError(e as string);
      setLoading(false);
    }
  }, [selectedProfile, selectedProfiles, batchMode, task, headless, startUrl, profiles]);

  const handleStopAgent = useCallback(async () => {
    console.log('[AgentPanel] handleStopAgent called, agentId:', agentId);
    if (!agentId) {
      console.log('[AgentPanel] No agentId, returning early');
      return;
    }

    try {
      console.log('[AgentPanel] Calling stopAgent API');
      await tauriApi.stopAgent(agentId);
      setLogs((prev) => [...prev, `[${new Date().toLocaleTimeString()}] Agent stopped`]);
    } catch (e) {
      console.error('[AgentPanel] stopAgent error:', e);
      setError(e as string);
    }
  }, [agentId]);

  const handlePauseAgent = useCallback(async () => {
    console.log('[AgentPanel] handlePauseAgent called, agentId:', agentId, 'status:', progress?.status);
    if (!agentId) {
      console.log('[AgentPanel] No agentId, returning early');
      return;
    }

    try {
      if (progress?.status === 'paused') {
        console.log('[AgentPanel] Calling resumeAgent API');
        await tauriApi.resumeAgent(agentId);
        setLogs((prev) => [...prev, `[${new Date().toLocaleTimeString()}] Agent resumed`]);
      } else {
        console.log('[AgentPanel] Calling pauseAgent API');
        await tauriApi.pauseAgent(agentId);
        setLogs((prev) => [...prev, `[${new Date().toLocaleTimeString()}] Agent paused`]);
      }
    } catch (e) {
      console.error('[AgentPanel] pauseAgent/resumeAgent error:', e);
      setError(e as string);
    }
  }, [agentId, progress?.status]);

  const isRunning = loading || progress?.status === 'running' || progress?.status === 'initializing';
  const isPaused = progress?.status === 'paused';

  // Template handlers
  const handleSelectTemplate = (template: TemplateInfo) => {
    setTask(template.content);
    setStartUrl(template.start_url || '');
    setHeadless(template.headless);
    setShowTemplateModal(false);
  };

  const handleSaveAsTemplate = async () => {
    if (!newTemplateName.trim() || !task.trim()) return;

    // Generate a safe ID from the name
    const id = newTemplateName
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');

    try {
      await tauriApi.saveTemplate(
        id,
        newTemplateName,
        task,
        startUrl || undefined,
        headless
      );
      // Reload templates
      const updatedTemplates = await tauriApi.getTemplates();
      setTemplates(updatedTemplates);
      setShowSaveAsTemplate(false);
      setNewTemplateName('');
    } catch (e) {
      setError(`Failed to save template: ${e}`);
    }
  };

  const handleDeleteTemplate = async (templateId: string) => {
    try {
      await tauriApi.deleteTemplate(templateId);
      setTemplates(templates.filter(t => t.id !== templateId));
    } catch (e) {
      setError(`Failed to delete template: ${e}`);
    }
  };

  const handleOpenTemplatesDir = async () => {
    try {
      await tauriApi.openTemplatesDir();
    } catch (e) {
      setError(`Failed to open templates directory: ${e}`);
    }
  };

  return (
    <div className="agent-panel">
      <div className="agent-config">
        <h3>Configure Agent</h3>

        {/* Batch mode toggle */}
        <div className="form-group">
          <label>
            <input
              type="checkbox"
              checked={batchMode}
              onChange={(e) => {
                setBatchMode(e.target.checked);
                setSelectedProfiles(new Set());
              }}
              disabled={isRunning}
            />
            Batch Mode (run on multiple profiles)
          </label>
        </div>

        {/* Single profile select */}
        {!batchMode && (
          <div className="form-group">
            <label>Select Profile</label>
            <select
              value={selectedProfile}
              onChange={(e) => setSelectedProfile(e.target.value)}
              disabled={isRunning}
            >
              {profiles.length === 0 ? (
                <option value="">No profiles available</option>
              ) : (
                profiles.map((profile) => (
                  <option key={profile.id} value={profile.id}>
                    {profile.name}
                  </option>
                ))
              )}
            </select>
          </div>
        )}

        {/* Batch profile selection */}
        {batchMode && (
          <div className="form-group">
            <label>Select Profiles ({selectedProfiles.size} selected)</label>
            <div className="profile-checkbox-list">
              {profiles.length === 0 ? (
                <div className="no-profiles">No profiles available</div>
              ) : (
                <>
                  <button
                    type="button"
                    className="btn btn-sm btn-link"
                    onClick={() => {
                      if (selectedProfiles.size === profiles.length) {
                        setSelectedProfiles(new Set());
                      } else {
                        setSelectedProfiles(new Set(profiles.map(p => p.id)));
                      }
                    }}
                    disabled={isRunning}
                  >
                    {selectedProfiles.size === profiles.length ? 'Deselect All' : 'Select All'}
                  </button>
                  {profiles.map((profile) => (
                    <label key={profile.id} className="profile-checkbox-item">
                      <input
                        type="checkbox"
                        checked={selectedProfiles.has(profile.id)}
                        onChange={(e) => {
                          const newSet = new Set(selectedProfiles);
                          if (e.target.checked) {
                            newSet.add(profile.id);
                          } else {
                            newSet.delete(profile.id);
                          }
                          setSelectedProfiles(newSet);
                        }}
                        disabled={isRunning}
                      />
                      <span className="profile-name">{profile.name}</span>
                      {profile.tags.length > 0 && (
                        <span className="profile-tags">{profile.tags.join(', ')}</span>
                      )}
                    </label>
                  ))}
                </>
              )}
            </div>
          </div>
        )}

        <div className="form-row">
          <div className="form-group">
            <label>
              <input
                type="checkbox"
                checked={headless}
                onChange={(e) => setHeadless(e.target.checked)}
                disabled={isRunning}
              />
              Headless Mode
            </label>
          </div>
        </div>

        <div className="form-group">
          <label>Start URL (optional)</label>
          <input
            type="text"
            value={startUrl}
            onChange={(e) => setStartUrl(e.target.value)}
            placeholder="https://example.com"
            disabled={isRunning}
          />
        </div>

        {/* Template Section */}
        <div className="template-section">
          <div className="template-header">
            <label>Task Templates</label>
            <button
              type="button"
              className="btn btn-sm btn-secondary"
              onClick={() => setShowTemplateModal(true)}
              disabled={isRunning}
            >
              Browse Templates
            </button>
          </div>

          {templates.length > 0 && (
            <div className="quick-templates">
              <span className="quick-label">Quick select:</span>
              <select
                onChange={(e) => {
                  const template = templates.find(t => t.id === e.target.value);
                  if (template) handleSelectTemplate(template);
                  e.target.value = '';
                }}
                value=""
                disabled={isRunning}
              >
                <option value="">Select a template...</option>
                {templates.map(template => (
                  <option key={template.id} value={template.id}>
                    {template.name}
                  </option>
                ))}
              </select>
            </div>
          )}

          {task.trim() && !isRunning && (
            <button
              type="button"
              className="btn btn-sm btn-link"
              onClick={() => setShowSaveAsTemplate(true)}
            >
              Save as Template
            </button>
          )}
        </div>

        <div className="form-group">
          <label>Start URL (optional)</label>
          <input
            type="text"
            value={startUrl}
            onChange={(e) => setStartUrl(e.target.value)}
            placeholder="https://example.com"
            disabled={isRunning}
          />
        </div>

        <div className="form-group">
          <label>Task Description</label>
          <textarea
            value={task}
            onChange={(e) => setTask(e.target.value)}
            placeholder="e.g., Go to amazon.com and search for 'wireless headphones'"
            rows={4}
            disabled={isRunning}
          />
        </div>

        <div className="agent-actions">
          {!isRunning ? (
            <button
              className="btn btn-primary"
              onClick={handleRunAgent}
              disabled={
                batchMode
                  ? selectedProfiles.size === 0 || !task.trim()
                  : !selectedProfile || !task.trim()
              }
            >
              {batchMode ? `Run on ${selectedProfiles.size || 'Selected'} Profiles` : 'Run Agent'}
            </button>
          ) : (
            <>
              <button className="btn btn-secondary" onClick={handlePauseAgent}>
                {isPaused ? 'Resume' : 'Pause'}
              </button>
              <button className="btn btn-danger" onClick={handleStopAgent}>
                Stop
              </button>
            </>
          )}
        </div>
      </div>

      {error && (
        <div className="error-section">
          <div className="error-message">{error}</div>
        </div>
      )}

      {/* Batch Progress */}
      {batchMode && batchProgress && (
        <div className="batch-progress">
          <div className="progress-header">
            <h3>Batch Progress</h3>
            <span className="status-badge" style={{ backgroundColor: 'var(--primary-color)' }}>
              {batchProgress.completed + batchProgress.failed} / {batchProgress.total}
            </span>
          </div>

          <div className="batch-stats">
            <div className="stat">
              <span className="stat-label">Total:</span>
              <span className="stat-value">{batchProgress.total}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Completed:</span>
              <span className="stat-value" style={{ color: 'var(--success-color)' }}>{batchProgress.completed}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Failed:</span>
              <span className="stat-value" style={{ color: 'var(--danger-color)' }}>{batchProgress.failed}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Total Cost:</span>
              <span className="stat-value">${batchProgress.total_cost.toFixed(4)}</span>
            </div>
          </div>

          {/* Results */}
          {Object.keys(batchProgress.results).length > 0 && (
            <div className="batch-results">
              <h4>Results</h4>
              {Object.entries(batchProgress.results).map(([profileId, result]) => {
                const profile = profiles.find(p => p.id === profileId);
                return (
                  <div key={profileId} className="batch-result-item">
                    <div className="profile-name">{profile?.name || profileId}</div>
                    <div className="result-summary">{result.summary}</div>
                  </div>
                );
              })}
            </div>
          )}

          {/* Errors */}
          {Object.keys(batchProgress.errors).length > 0 && (
            <div className="batch-results">
              <h4>Errors</h4>
              {Object.entries(batchProgress.errors).map(([profileId, errorMsg]) => {
                const profile = profiles.find(p => p.id === profileId);
                return (
                  <div key={profileId} className="batch-result-item failed">
                    <div className="profile-name">{profile?.name || profileId}</div>
                    <div className="error-message">{errorMsg}</div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* Recovery suggestions for failed agents */}
      {progress?.status === 'failed' && progress.error && (
        <div className="recovery-suggestions">
          <h4>Troubleshooting Suggestions</h4>
          <ul>
            {getRecoverySuggestions(progress.error, progress.status).map((suggestion, index) => (
              <li key={index}>{suggestion}</li>
            ))}
          </ul>
        </div>
      )}

      {progress && (
        <div className="agent-progress">
          <div className="progress-header">
            <h3>Agent Progress</h3>
            <span
              className="status-badge"
              style={{ backgroundColor: statusColors[progress.status] }}
            >
              {statusLabels[progress.status]}
            </span>
          </div>

          <div className="progress-stats">
            <div className="stat">
              <span className="stat-label">Mode:</span>
              <span className="stat-value">{progress.mode.toUpperCase()}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Steps:</span>
              <span className="stat-value">{progress.steps_completed}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Cost:</span>
              <span className="stat-value">${progress.cost.toFixed(4)}</span>
            </div>
          </div>

          {progress.current_step && (
            <div className="current-step">
              <div className="step-info">
                <strong>Current Action:</strong> {progress.current_step.action}
              </div>
              <div className="step-url">
                <strong>URL:</strong> {progress.current_step.url}
              </div>

              {progress.current_step.screenshot && (
                <div className="screenshot-preview">
                  <h4>Current Page</h4>
                  <img
                    src={`data:image/png;base64,${progress.current_step.screenshot}`}
                    alt="Current page screenshot"
                    className="screenshot-image"
                  />
                </div>
              )}
            </div>
          )}

          <div className="agent-logs">
            <h4>Activity Log</h4>
            <div className="logs-container">
              {logs.map((log, index) => (
                <div key={index} className="log-entry">
                  {log}
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {progress?.result && (
        <div className="agent-result">
          <div className="result-header">
            <h3>Result</h3>
            <div className="export-actions">
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => {
                  const data = JSON.stringify(progress.result, null, 2);
                  navigator.clipboard.writeText(data);
                  alert('Result copied to clipboard!');
                }}
              >
                Copy JSON
              </button>
              <button
                className="btn btn-sm btn-secondary"
                onClick={() => {
                  const data = JSON.stringify(progress.result, null, 2);
                  const blob = new Blob([data], { type: 'application/json' });
                  const url = URL.createObjectURL(blob);
                  const a = document.createElement('a');
                  a.href = url;
                  a.download = `agent-result-${new Date().toISOString().slice(0, 10)}.json`;
                  a.click();
                  URL.revokeObjectURL(url);
                }}
              >
                Download JSON
              </button>
              {progress.result.data && typeof progress.result.data === 'object' && (
                <button
                  className="btn btn-sm btn-secondary"
                  onClick={() => {
                    const result = progress.result;
                    if (!result) return;
                    // Convert data to CSV
                    const data = result.data;
                    if (!data || typeof data !== 'object') return;
                    let csv = '';

                    // Handle array of objects
                    if (Array.isArray(data) && data.length > 0 && typeof data[0] === 'object') {
                      const firstItem = data[0] as Record<string, unknown>;
                      const headers = Object.keys(firstItem);
                      csv = headers.join(',') + '\n';
                      csv += data.map(row =>
                        headers.map(h => {
                          const val = (row as Record<string, unknown>)[h];
                          const str = String(val ?? '');
                          // Escape quotes and wrap in quotes if contains comma
                          return str.includes(',') || str.includes('"')
                            ? `"${str.replace(/"/g, '""')}"`
                            : str;
                        }).join(',')
                      ).join('\n');
                    } else {
                      // Handle single object
                      const entries = Object.entries(data as Record<string, unknown>);
                      csv = 'Key,Value\n';
                      csv += entries.map(([k, v]) => {
                        const str = String(v ?? '');
                        return `${k},${str.includes(',') || str.includes('"') ? `"${str.replace(/"/g, '""')}"` : str}`;
                      }).join('\n');
                    }

                    const blob = new Blob([csv], { type: 'text/csv' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = `agent-result-${new Date().toISOString().slice(0, 10)}.csv`;
                    a.click();
                    URL.revokeObjectURL(url);
                  }}
                >
                  Download CSV
                </button>
              )}
            </div>
          </div>
          <div className="result-summary">{progress.result.summary}</div>

          {progress.result.data && Object.keys(progress.result.data).length > 0 && (
            <div className="result-data">
              <h4>Extracted Data</h4>
              <pre>{JSON.stringify(progress.result.data, null, 2)}</pre>
            </div>
          )}

          <div className="result-stats">
            <div className="stat">
              <span className="stat-label">Total Steps:</span>
              <span className="stat-value">{progress.result.total_steps}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Total Cost:</span>
              <span className="stat-value">${progress.result.total_cost.toFixed(4)}</span>
            </div>
            <div className="stat">
              <span className="stat-label">Duration:</span>
              <span className="stat-value">{progress.result.duration_seconds}s</span>
            </div>
          </div>
        </div>
      )}

      {/* Template Browser Modal */}
      {showTemplateModal && (
        <div className="modal-overlay" onClick={() => setShowTemplateModal(false)}>
          <div className="modal template-modal" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Task Templates</h3>
              <button className="btn-icon" onClick={() => setShowTemplateModal(false)}>×</button>
            </div>
            <div className="modal-body">
              {/* Open Folder Button */}
              <div className="template-folder-action">
                <button
                  className="btn btn-secondary"
                  onClick={handleOpenTemplatesDir}
                >
                  Open Templates Folder
                </button>
                <span className="hint">Templates are stored as markdown files in ~/.config/browsion/templates/</span>
              </div>

              {/* Saved Templates */}
              {templates.length > 0 ? (
                <div className="template-section">
                  <h4>Available Templates</h4>
                  <div className="template-list">
                    {templates.map(template => (
                      <div key={template.id} className="template-item">
                        <div className="template-info">
                          <span className="template-name">{template.name}</span>
                          <span className="template-id">{template.id}.md</span>
                          {template.start_url && (
                            <span className="template-url">Start: {template.start_url}</span>
                          )}
                        </div>
                        <div className="template-actions">
                          <button
                            className="btn btn-sm btn-primary"
                            onClick={() => handleSelectTemplate(template)}
                          >
                            Use
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={() => handleDeleteTemplate(template.id)}
                          >
                            Delete
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="no-templates">
                  <p>No templates yet. Create markdown files in the templates folder or save a task as a template.</p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Save as Template Modal */}
      {showSaveAsTemplate && (
        <div className="modal-overlay" onClick={() => setShowSaveAsTemplate(false)}>
          <div className="modal save-template-modal" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Save as Template</h3>
              <button className="btn-icon" onClick={() => setShowSaveAsTemplate(false)}>×</button>
            </div>
            <div className="modal-body">
              <div className="form-group">
                <label>Template Name *</label>
                <input
                  type="text"
                  value={newTemplateName}
                  onChange={(e) => setNewTemplateName(e.target.value)}
                  placeholder="e.g., Amazon Order Check"
                />
                <span className="form-hint">Will be saved as a markdown file in ~/.config/browsion/templates/</span>
              </div>
              <div className="form-group">
                <label>Task Preview</label>
                <div className="task-preview">{task}</div>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowSaveAsTemplate(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={handleSaveAsTemplate}
                disabled={!newTemplateName.trim()}
              >
                Save Template
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
