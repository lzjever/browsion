import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { Workflow, BrowserProfile } from '../types/profile';
import { WorkflowEditor } from './WorkflowEditor';
import { WorkflowRunModal } from './WorkflowRunModal';

interface WorkflowListProps {
  profiles: BrowserProfile[];
}

export const WorkflowList: React.FC<WorkflowListProps> = ({ profiles }) => {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [editingWorkflow, setEditingWorkflow] = useState<Workflow | undefined>();
  const [runningWorkflow, setRunningWorkflow] = useState<Workflow | undefined>();
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    loadWorkflows();
  }, [refreshKey]);

  const loadWorkflows = async () => {
    try {
      const list = await tauriApi.listWorkflows();
      setWorkflows(list);
    } catch (e) {
      console.error('Failed to load workflows:', e);
    }
  };

  const handleCreateNew = () => {
    setEditingWorkflow({
      id: '',
      name: '',
      description: '',
      steps: [],
      variables: {},
      created_at: 0,
      updated_at: 0,
    });
  };

  const handleEdit = (workflow: Workflow) => {
    setEditingWorkflow(workflow);
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this workflow?')) return;
    try {
      await tauriApi.deleteWorkflow(id);
      setRefreshKey((prev) => prev + 1);
    } catch (e) {
      alert(`Failed to delete: ${e}`);
    }
  };

  const handleRun = (workflow: Workflow) => {
    setRunningWorkflow(workflow);
  };

  const handleSave = () => {
    setEditingWorkflow(undefined);
    setRefreshKey((prev) => prev + 1);
  };

  return (
    <div className="workflow-list">
      <div className="workflow-list-header">
        <h2>Workflows</h2>
        <button className="btn btn-primary" onClick={handleCreateNew}>
          + New Workflow
        </button>
      </div>

      {workflows.length === 0 ? (
        <p className="muted">No workflows yet. Create one to automate browser tasks.</p>
      ) : (
        <div className="workflow-grid">
          {workflows.map((workflow) => (
            <div key={workflow.id} className="workflow-card">
              <div className="workflow-card-header">
                <h3>{workflow.name}</h3>
                <span className="workflow-step-count">{workflow.steps.length} steps</span>
              </div>
              {workflow.description && (
                <p className="workflow-description">{workflow.description}</p>
              )}
              <div className="workflow-card-footer">
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => handleRun(workflow)}
                >
                  ▶ Run
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => handleEdit(workflow)}
                >
                  Edit
                </button>
                <button
                  className="btn btn-danger-outline btn-sm"
                  onClick={() => handleDelete(workflow.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {editingWorkflow && (
        <WorkflowEditor
          workflow={editingWorkflow}
          onSave={handleSave}
          onCancel={() => setEditingWorkflow(undefined)}
        />
      )}

      {runningWorkflow && (
        <WorkflowRunModal
          workflow={runningWorkflow}
          profiles={profiles}
          onClose={() => setRunningWorkflow(undefined)}
        />
      )}
    </div>
  );
};
