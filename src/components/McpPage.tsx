import React, { useState, useEffect, useCallback, useRef } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { tauriApi } from '../api/tauri';
import type { McpConfig, McpToolInfo } from '../types/profile';

const TOOL_DESCRIPTIONS: Record<string, string> = {
  cursor:
    'Cursor is an AI-first code editor. Global config at ~/.cursor/mcp.json. Browsion gives Cursor real browser control for UI testing and web automation.',
  claude_code:
    "Claude Code is Anthropic's official CLI agent. Config at ~/.claude.json (note: NOT inside ~/.claude/). Browsion lets Claude Code navigate, click, screenshot, and scrape any website.",
  codex:
    'OpenAI Codex CLI is an AI coding assistant. Config at ~/.codex/config.toml in TOML format. Note: Codex only supports local stdio MCP servers.',
  openclaw:
    'OpenClaw is a self-hosted AI agent framework. Config at openclaw.json in your project directory. Specify the project directory below.',
  windsurf:
    'Windsurf (by Codeium) is an AI IDE with Cascade agent. Config at ~/.codeium/windsurf/mcp_config.json. Note: Windsurf has a 100-tool limit across all MCP servers.',
  continue_vscode:
    'Continue is a VS Code/JetBrains extension for AI-assisted coding. MCP servers are stored as separate files in .continue/mcpServers/ in your project. Specify the project directory below.',
  zed:
    "Zed is a high-performance code editor with AI agent panel. MCP servers are defined under context_servers in Zed's settings.json. Note: writing to this file will remove any existing comments, as Zed uses JSONC which cannot be round-tripped losslessly.",
};

function generateSnippet(toolId: string, mcpConfig: McpConfig, binaryPath: string): string {
  const cmd = binaryPath || 'browsion-mcp';
  const envObj: Record<string, string> = {
    BROWSION_API_PORT: String(mcpConfig.api_port),
    ...(mcpConfig.api_key ? { BROWSION_API_KEY: mcpConfig.api_key } : {}),
  };
  if (toolId === 'codex') {
    const envLines = Object.entries(envObj)
      .map(([k, v]) => `  ${k} = "${v}"`)
      .join('\n');
    return `[mcp_servers.browsion]\ncommand = "${cmd}"\n\n[mcp_servers.browsion.env]\n${envLines}`;
  }
  if (toolId === 'continue_vscode') {
    return JSON.stringify({ name: 'browsion', command: cmd, env: envObj }, null, 2);
  }
  if (toolId === 'zed') {
    return JSON.stringify(
      { context_servers: { browsion: { command: { path: cmd, env: envObj } } } },
      null,
      2
    );
  }
  return JSON.stringify({ mcpServers: { browsion: { command: cmd, env: envObj } } }, null, 2);
}

// ---------------------------------------------------------------------------
// ToolPanel
// ---------------------------------------------------------------------------

interface ToolPanelProps {
  tool: McpToolInfo;
  mcpConfig: McpConfig;
  hasUnsavedApiChanges: boolean;
  binaryPath: string;
  binaryReady: boolean;
  projectDir: string;
  onProjectDirChange: (dir: string) => void;
  onWrite: () => void;
  writing: boolean;
  writeResult?: string;
  writeError?: string;
}

const ToolPanel: React.FC<ToolPanelProps> = ({
  tool,
  mcpConfig,
  hasUnsavedApiChanges,
  binaryPath,
  binaryReady,
  projectDir,
  onProjectDirChange,
  onWrite,
  writing,
  writeResult,
  writeError,
}) => {
  const snippet = generateSnippet(tool.id, mcpConfig, binaryPath);
  const isProjectScoped = tool.scope === 'project_scoped';
  const canWrite = binaryReady && (!isProjectScoped || projectDir !== '') && !hasUnsavedApiChanges;

  const handleBrowseProject = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: 'Select project directory' });
      if (selected && typeof selected === 'string') onProjectDirChange(selected);
    } catch (err) {
      console.error('Failed to open directory dialog:', err);
    }
  };

  return (
    <div className="tool-panel">
      <div className="tool-detection-line">
        {tool.found ? (
          <span className="tool-found">✓ Config found at <code>{tool.config_path}</code></span>
        ) : (
          <span className="tool-not-found">○ Config not found — will be created at <code>{tool.config_path}</code></span>
        )}
      </div>

      <p className="tool-description">{TOOL_DESCRIPTIONS[tool.id]}</p>

      {isProjectScoped && (
        <div className="form-group">
          <label>Project directory</label>
          <div className="input-with-button">
            <input
              type="text"
              value={projectDir}
              onChange={(e) => onProjectDirChange(e.target.value)}
              placeholder="/path/to/your/project"
            />
            <button className="btn btn-secondary" onClick={handleBrowseProject}>Browse</button>
          </div>
        </div>
      )}

      <div className="mcp-config-snippet">
        <label>Config snippet ({tool.id === 'codex' ? 'TOML' : 'JSON'})</label>
        <pre className="mcp-config-code">{snippet}</pre>
        <button
          className="btn btn-secondary btn-small"
          onClick={() => navigator.clipboard.writeText(snippet)}
        >
          Copy Snippet
        </button>
      </div>

      {hasUnsavedApiChanges && (
        <p className="form-hint tool-warn-unsaved">
          ⚠ API server has unsaved changes. Press Apply above so the config file matches the running server.
        </p>
      )}
      {!binaryReady && !hasUnsavedApiChanges && (
        <p className="form-hint" style={{ marginTop: '10px', color: 'var(--danger-color)' }}>
          Set the binary path in the MCP Binary section above first.
        </p>
      )}
      {isProjectScoped && !projectDir && binaryReady && !hasUnsavedApiChanges && (
        <p className="form-hint" style={{ marginTop: '10px', color: 'var(--warning-color)' }}>
          Specify a project directory above.
        </p>
      )}

      <button
        className="btn btn-primary"
        onClick={onWrite}
        disabled={!canWrite || writing}
        style={{ marginTop: '10px' }}
      >
        {writing ? 'Writing…' : 'Write to config'}
      </button>

      {writeResult && <div className="write-success-banner">Written to: {writeResult}</div>}
      {writeError && <div className="error-message" style={{ marginTop: '8px' }}>{writeError}</div>}
    </div>
  );
};

// ---------------------------------------------------------------------------
// McpPage
// ---------------------------------------------------------------------------

export const McpPage: React.FC = () => {
  const [mcpConfig, setMcpConfig] = useState<McpConfig>({ enabled: true, api_port: 38472 });
  const [savedMcpConfig, setSavedMcpConfig] = useState<McpConfig>({ enabled: true, api_port: 38472 });
  // Separate display string for port so the input doesn't snap-to-38472 when cleared.
  // Committed to mcpConfig.api_port on blur if valid.
  const [portInput, setPortInput] = useState('38472');
  const [mcpSaving, setMcpSaving] = useState(false);
  const [mcpStatus, setMcpStatus] = useState<'checking' | 'running' | 'stopped'>('checking');

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [tools, setTools] = useState<McpToolInfo[]>([]);
  const [activeToolId, setActiveToolId] = useState('cursor');
  const [writingTool, setWritingTool] = useState<string | null>(null);
  const [writeResult, setWriteResult] = useState<Record<string, string>>({});
  const [writeError, setWriteError] = useState<Record<string, string>>({});
  const [projectDirs, setProjectDirs] = useState<Record<string, string>>({});

  const [binaryPath, setBinaryPath] = useState('');
  const [binaryFound, setBinaryFound] = useState<boolean | null>(null);
  const [showBuildInstructions, setShowBuildInstructions] = useState(false);

  // Clean up success timer on unmount
  useEffect(() => {
    return () => {
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
    };
  }, []);

  // ---------------------------------------------------------------------------
  // Init
  // ---------------------------------------------------------------------------
  useEffect(() => {
    Promise.all([tauriApi.getMcpConfig(), tauriApi.detectMcpTools(), tauriApi.findMcpBinary()])
      .then(([mcp, detected, binary]) => {
        setMcpConfig(mcp);
        setSavedMcpConfig(mcp);
        setPortInput(String(mcp.api_port));
        setTools(detected);
        if (binary) { setBinaryPath(binary); setBinaryFound(true); }
        else { setBinaryFound(false); }
        setActiveToolId(detected.find((t) => t.found)?.id ?? 'cursor');
        setLoading(false);
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
      });
  }, []);

  // ---------------------------------------------------------------------------
  // Status check — only ever uses the SAVED config, never the editing buffer
  // ---------------------------------------------------------------------------
  const checkMcpStatus = useCallback(async () => {
    if (!savedMcpConfig.enabled || savedMcpConfig.api_port === 0) {
      setMcpStatus('stopped');
      return;
    }
    try {
      const resp = await fetch(`http://127.0.0.1:${savedMcpConfig.api_port}/api/health`);
      setMcpStatus(resp.ok ? 'running' : 'stopped');
    } catch {
      setMcpStatus('stopped');
    }
  }, [savedMcpConfig.enabled, savedMcpConfig.api_port]);

  useEffect(() => {
    if (!loading) checkMcpStatus();
  }, [loading, checkMcpStatus]);

  // ---------------------------------------------------------------------------
  // API Server handlers
  // ---------------------------------------------------------------------------
  const handleMcpSave = async () => {
    try {
      setMcpSaving(true);
      setError(null);
      await tauriApi.updateMcpConfig(mcpConfig);
      setSavedMcpConfig(mcpConfig);

      if (successTimerRef.current) clearTimeout(successTimerRef.current);
      setSuccess('API server configuration saved');
      successTimerRef.current = setTimeout(() => setSuccess(null), 3000);

      // Check status against the newly saved config directly (avoid stale closure)
      const saved = mcpConfig;
      setTimeout(async () => {
        if (!saved.enabled || saved.api_port === 0) { setMcpStatus('stopped'); return; }
        try {
          const resp = await fetch(`http://127.0.0.1:${saved.api_port}/api/health`);
          setMcpStatus(resp.ok ? 'running' : 'stopped');
        } catch { setMcpStatus('stopped'); }
      }, 500);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setMcpSaving(false);
    }
  };

  const handlePortBlur = () => {
    const n = parseInt(portInput, 10);
    if (!Number.isNaN(n) && n >= 1 && n <= 65535) {
      setMcpConfig((prev) => ({ ...prev, api_port: n }));
    } else {
      // Restore display to last valid committed value
      setPortInput(String(mcpConfig.api_port));
    }
  };

  const generateApiKey = () => {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    const array = new Uint8Array(32);
    crypto.getRandomValues(array);
    let key = 'sk-';
    for (let i = 0; i < 32; i++) key += chars[array[i] % chars.length];
    setMcpConfig((prev) => ({ ...prev, api_key: key }));
  };

  const hasUnsavedApiChanges =
    mcpConfig.enabled !== savedMcpConfig.enabled ||
    mcpConfig.api_port !== savedMcpConfig.api_port ||
    (mcpConfig.api_key ?? '') !== (savedMcpConfig.api_key ?? '');

  // ---------------------------------------------------------------------------
  // Binary handlers
  // ---------------------------------------------------------------------------
  const handleBrowseBinary = async () => {
    try {
      const selected = await open({ multiple: false, directory: false, title: 'Select browsion-mcp binary' });
      if (selected && typeof selected === 'string') { setBinaryPath(selected); setBinaryFound(true); }
    } catch (err) {
      console.error('Failed to open file dialog:', err);
    }
  };

  const handleBinaryBlur = () => {
    if (binaryPath.trim() === '') setBinaryFound(false);
  };

  const binaryReady = binaryFound === true || (binaryFound === null && binaryPath.trim() !== '');

  // ---------------------------------------------------------------------------
  // Tool write handler
  // ---------------------------------------------------------------------------
  const handleWriteTool = async (toolId: string) => {
    setWritingTool(toolId);
    setWriteResult((prev) => ({ ...prev, [toolId]: '' }));
    setWriteError((prev) => ({ ...prev, [toolId]: '' }));
    try {
      const path = await tauriApi.writeBrowsionToTool(
        toolId, binaryPath, mcpConfig.api_port, mcpConfig.api_key, projectDirs[toolId]
      );
      setWriteResult((prev) => ({ ...prev, [toolId]: path }));
      // Refresh detection status (fire-and-forget, errors silently ignored —
      // failure here doesn't affect the write result)
      tauriApi.detectMcpTools()
        .then((detected) => {
          setTools((prev) =>
            prev.map((t) => {
              // Project-scoped tools always return found=false from backend (no project dir context)
              if (t.id === toolId && t.scope === 'project_scoped') return { ...t, found: true };
              return detected.find((d) => d.id === t.id) ?? t;
            })
          );
        })
        .catch(() => {/* status dot not updated, write still succeeded */});
    } catch (err) {
      setWriteError((prev) => ({
        ...prev,
        [toolId]: err instanceof Error ? err.message : String(err),
      }));
    } finally {
      setWritingTool(null);
    }
  };

  const activeTool = tools.find((t) => t.id === activeToolId) ?? null;

  if (loading) return <div className="loading">Loading MCP settings…</div>;

  return (
    <div className="mcp-page settings-container">
      <h2>MCP / AI Client Setup</h2>

      {error && <div className="error-message">{error}</div>}
      {success && <div className="success-message success-message-prominent">{success}</div>}

      <div className="settings-content">

        {/* ── Section 1: API Server ── */}
        <div className="settings-section">
          <h3>API Server</h3>

          <div className="checkbox-group">
            <label>
              <input
                type="checkbox"
                checked={mcpConfig.enabled}
                onChange={(e) => setMcpConfig((prev) => ({ ...prev, enabled: e.target.checked }))}
              />
              <span>Enable API server</span>
            </label>
          </div>

          <div className="form-group">
            <label>Port</label>
            {/* Use portInput string state to avoid the snap-to-38472 bug when clearing the field */}
            <input
              type="number"
              min={1}
              max={65535}
              value={portInput}
              onChange={(e) => setPortInput(e.target.value)}
              onBlur={handlePortBlur}
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
                  setMcpConfig((prev) => ({ ...prev, api_key: e.target.value || undefined }))
                }
                placeholder="No authentication"
                disabled={!mcpConfig.enabled}
              />
              <button className="btn btn-secondary btn-small" onClick={generateApiKey} disabled={!mcpConfig.enabled}>
                Generate
              </button>
              <button
                className="btn btn-secondary btn-small"
                onClick={() => { if (mcpConfig.api_key) navigator.clipboard.writeText(mcpConfig.api_key); }}
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
                ? `Running on http://127.0.0.1:${savedMcpConfig.api_port}`
                : mcpStatus === 'checking' ? 'Checking…' : 'Stopped'}
            </span>
          </div>

          <div className="apply-row">
            <button className="btn btn-primary" onClick={handleMcpSave} disabled={mcpSaving}>
              {mcpSaving ? 'Saving…' : 'Apply'}
            </button>
            {hasUnsavedApiChanges && <span className="unsaved-indicator">● unsaved changes</span>}
          </div>
        </div>

        {/* ── Section 2: MCP Binary ── */}
        <div className="settings-section">
          <h3>MCP Binary</h3>
          <p className="settings-hint" style={{ marginBottom: '12px' }}>
            The browsion-mcp binary is the stdio process that AI tools will run. Set its path
            before writing to any client config below.
          </p>

          <div className="binary-status-row">
            <span
              className={`binary-status-badge ${
                binaryFound === true ? 'found'
                  : binaryFound === null && binaryPath.trim() !== '' ? 'unverified'
                  : 'not-found'
              }`}
            >
              {binaryFound === true ? 'Found'
                : binaryFound === null && binaryPath.trim() !== '' ? 'Custom path (unverified)'
                : 'Binary not found'}
            </span>
          </div>

          <div className="form-group">
            <label>Binary path</label>
            <div className="input-with-button">
              <input
                type="text"
                value={binaryPath}
                onChange={(e) => { setBinaryPath(e.target.value); setBinaryFound(null); }}
                onBlur={handleBinaryBlur}
                placeholder="/path/to/browsion-mcp"
              />
              <button className="btn btn-secondary" onClick={handleBrowseBinary}>Browse</button>
            </div>
          </div>

          <button
            className="btn btn-secondary btn-small"
            onClick={() => setShowBuildInstructions(!showBuildInstructions)}
            style={{ marginBottom: '8px' }}
          >
            {showBuildInstructions ? 'Hide' : 'Show'} build instructions
          </button>

          {showBuildInstructions && (
            <div className="build-instructions">
              <pre className="mcp-config-code">{`cd /path/to/browsion/src-tauri
cargo build --release --bin browsion-mcp
# Binary at: target/release/browsion-mcp`}</pre>
            </div>
          )}
        </div>

        {/* ── Section 3: Client Setup ── */}
        <div className="settings-section">
          <h3>Client Setup</h3>
          <p style={{ marginBottom: '16px', color: 'var(--text-light)' }}>
            Choose your AI coding tool. Click "Write to config" to inject browsion-mcp into its
            configuration file automatically.
          </p>

          <div className="mcp-tool-tabs">
            {tools.map((tool) => (
              <button
                key={tool.id}
                className={`tab-btn ${activeToolId === tool.id ? 'active' : ''}`}
                onClick={() => setActiveToolId(tool.id)}
              >
                {tool.name}
                <span className={`tool-status-dot ${tool.found ? 'found' : 'not-found'}`} />
              </button>
            ))}
          </div>

          {activeTool && (
            <ToolPanel
              tool={activeTool}
              mcpConfig={mcpConfig}
              hasUnsavedApiChanges={hasUnsavedApiChanges}
              binaryPath={binaryPath}
              binaryReady={binaryReady}
              projectDir={projectDirs[activeToolId] ?? ''}
              onProjectDirChange={(dir) =>
                setProjectDirs((prev) => ({ ...prev, [activeToolId]: dir }))
              }
              onWrite={() => handleWriteTool(activeToolId)}
              writing={writingTool === activeToolId}
              writeResult={writeResult[activeToolId]}
              writeError={writeError[activeToolId]}
            />
          )}
        </div>

      </div>
    </div>
  );
};
