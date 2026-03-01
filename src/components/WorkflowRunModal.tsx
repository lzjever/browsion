import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { Workflow, BrowserProfile, WorkflowExecution, ExecutionStatus, RunningStatus } from '../types/profile';
import { useToast } from './Toast';

interface WorkflowRunModalProps {
  workflow: Workflow;
  profiles: BrowserProfile[];
  onClose: () => void;
}

export const WorkflowRunModal: React.FC<WorkflowRunModalProps> = ({
  workflow,
  profiles,
  onClose,
}) => {
  const { showToast } = useToast();
  const [selectedProfileId, setSelectedProfileId] = useState('');
  const [execution, setExecution] = useState<WorkflowExecution | null>(null);
  const [running, setRunning] = useState(false);
  const [runningStatus, setRunningStatus] = useState<RunningStatus>({});

  useEffect(() => {
    const loadRunningStatus = async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus(status);
      } catch (e) {
        console.error('Failed to load running status:', e);
      }
    };
    loadRunningStatus();
  }, []);

  // Filter profiles by running status
  const runningProfiles = profiles.filter((p) => runningStatus[p.id]);

  const handleRun = async () => {
    if (!selectedProfileId) {
      showToast('Please select a profile', 'warning');
      return;
    }

    setRunning(true);
    try {
      const result = await tauriApi.runWorkflow(
        workflow.id,
        selectedProfileId,
        workflow.variables
      );
      setExecution(result);
    } catch (e) {
      showToast(`Failed to run workflow: ${e}`, 'error');
    } finally {
      setRunning(false);
    }
  };

  const getProfileName = (id: string) => {
    return profiles.find((p) => p.id === id)?.name || id;
  };

  const getStatusBadge = (status: ExecutionStatus) => {
    const colors: Record<ExecutionStatus, string> = {
      pending: 'status-pending',
      running: 'status-running',
      completed: 'status-success',
      failed: 'status-error',
      paused: 'status-warning',
      cancelled: 'status-warning',
    };
    return <span className={`status-badge ${colors[status]}`}>{status}</span>;
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content workflow-run-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Run Workflow: {workflow.name}</h3>
          <button className="modal-close" onClick={onClose} aria-label="Close modal">✕</button>
        </div>

        <div className="workflow-run-body">
          {!execution ? (
            <div className="run-setup">
              <div className="form-group">
                <label>Select Profile *</label>
                <select
                  value={selectedProfileId}
                  onChange={(e) => setSelectedProfileId(e.target.value)}
                  disabled={running}
                >
                  <option value="">-- Choose a profile --</option>
                  {runningProfiles.map((profile) => (
                    <option key={profile.id} value={profile.id}>
                      {profile.name}
                    </option>
                  ))}
                </select>
              </div>

              {workflow.steps.length > 0 && (
                <div className="workflow-preview">
                  <h4>Steps to execute:</h4>
                  <ol>
                    {workflow.steps.map((step) => (
                      <li key={step.id}>
                        {step.name} ({step.type})
                        {step.continue_on_error && <span className="continue-on-error"> - continues on error</span>}
                      </li>
                    ))}
                  </ol>
                </div>
              )}
            </div>
          ) : (
            <div className="execution-results">
              <div className="execution-header">
                <h4>Execution Results</h4>
                {getStatusBadge(execution.status)}
              </div>

              {execution.error && (
                <div className="error-message">{execution.error}</div>
              )}

              <div className="execution-summary">
                <div>
                  <strong>Profile:</strong> {getProfileName(execution.profile_id)}
                </div>
                <div>
                  <strong>Duration:</strong> {execution.completed_at
                    ? `${execution.completed_at - execution.started_at}ms`
                    : 'In progress'}
                </div>
                <div>
                  <strong>Steps:</strong> {execution.step_results.length} / {workflow.steps.length}
                </div>
              </div>

              <div className="step-results">
                <h5>Step Results</h5>
                {execution.step_results.map((result) => {
                  const stepIndex = workflow.steps.findIndex((s) => s.id === result.step_id);
                  const step = stepIndex >= 0 ? workflow.steps[stepIndex] : null;
                  return (
                    <div key={result.step_id} className={`step-result ${result.status}`}>
                      <div className="step-result-header">
                        <span className="step-number">{(stepIndex ?? -1) + 1}</span>
                        <span className="step-name">{step?.name || result.step_id}</span>
                        {getStatusBadge(result.status)}
                        <span className="step-duration">{result.duration_ms}ms</span>
                      </div>
                      {result.error && (
                        <div className="step-error">{result.error}</div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onClose}>
            {execution ? 'Close' : 'Cancel'}
          </button>
          {!execution && (
            <button
              className="btn btn-primary"
              onClick={handleRun}
              disabled={running || !selectedProfileId}
            >
              {running ? 'Running...' : '▶ Run Workflow'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
