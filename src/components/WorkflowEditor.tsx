import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { Workflow, WorkflowStep, StepType, StepTypeInfo } from '../types/profile';

interface WorkflowEditorProps {
  workflow: Workflow;
  onSave: () => void;
  onCancel: () => void;
}

export const WorkflowEditor: React.FC<WorkflowEditorProps> = ({
  workflow,
  onSave,
  onCancel,
}) => {
  const [name, setName] = useState(workflow.name);
  const [description, setDescription] = useState(workflow.description);
  const [steps, setSteps] = useState<WorkflowStep[]>(workflow.steps);
  const [variables, setVariables] = useState<Record<string, string>>(
    Object.fromEntries(
      Object.entries(workflow.variables).map(([k, v]) => [k, String(v)])
    )
  );
  const [stepTypes, setStepTypes] = useState<StepTypeInfo[]>([]);
  const [selectedStepIndex, setSelectedStepIndex] = useState<number | null>(null);

  useEffect(() => {
    tauriApi.getStepTypes().then(setStepTypes).catch(console.error);
  }, []);

  const handleAddStep = (type: StepType) => {
    const newStep: WorkflowStep = {
      id: `step-${steps.length}`,
      name: `${type} ${steps.length + 1}`,
      description: '',
      type,
      params: {},
      continue_on_error: false,
      timeout_ms: 30000,
    };
    setSteps([...steps, newStep]);
    setSelectedStepIndex(steps.length);
  };

  const handleUpdateStep = (index: number, updates: Partial<WorkflowStep>) => {
    const newSteps = [...steps];
    newSteps[index] = { ...newSteps[index], ...updates };
    setSteps(newSteps);
  };

  const handleDeleteStep = (index: number) => {
    const newSteps = steps.filter((_, i) => i !== index);
    setSteps(newSteps);
    if (selectedStepIndex === index) {
      setSelectedStepIndex(null);
    }
  };

  const handleSave = async () => {
    if (!name.trim()) {
      alert('Workflow name is required');
      return;
    }

    const now = Date.now();
    const workflowToSave: Workflow = {
      id: workflow.id || `workflow-${Date.now()}`,
      name: name.trim(),
      description: description.trim(),
      steps: steps.map((step, i) => ({ ...step, id: step.id || `step-${i}` })),
      variables,
      created_at: workflow.created_at || now,
      updated_at: now,
    };

    try {
      await tauriApi.saveWorkflow(workflowToSave);
      onSave();
    } catch (e) {
      alert(`Failed to save workflow: ${e}`);
    }
  };

  const selectedStep = selectedStepIndex !== null ? steps[selectedStepIndex] : null;
  const selectedStepType = stepTypes.find((st) => st.type === selectedStep?.type);

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal-content workflow-editor" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>{workflow.id ? 'Edit Workflow' : 'New Workflow'}</h3>
          <button className="modal-close" onClick={onCancel}>✕</button>
        </div>

        <div className="workflow-editor-body">
          <div className="form-group">
            <label>Name *</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My workflow"
            />
          </div>

          <div className="form-group">
            <label>Description</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What this workflow does..."
              rows={2}
            />
          </div>

          <div className="form-group">
            <label>Variables</label>
            <div className="variables-editor">
              {Object.entries(variables).map(([key, value]) => (
                <div key={key} className="variable-row">
                  <input
                    type="text"
                    value={key}
                    onChange={(e) => {
                      const newVars = { ...variables };
                      delete newVars[key];
                      newVars[e.target.value] = value;
                      setVariables(newVars);
                    }}
                    placeholder="variable_name"
                  />
                  <input
                    type="text"
                    value={value}
                    onChange={(e) => {
                      setVariables({ ...variables, [key]: e.target.value });
                    }}
                    placeholder="default value"
                  />
                  <button
                    className="btn btn-danger-outline btn-sm"
                    onClick={() => {
                      const newVars = { ...variables };
                      delete newVars[key];
                      setVariables(newVars);
                    }}
                  >
                    ✕
                  </button>
                </div>
              ))}
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setVariables({ ...variables, '': '' })}
              >
                + Add Variable
              </button>
            </div>
          </div>

          <div className="form-group">
            <label>Steps</label>
            <div className="steps-editor">
              <div className="steps-list">
                {steps.map((step, index) => (
                  <div
                    key={step.id}
                    className={`step-item ${selectedStepIndex === index ? 'selected' : ''}`}
                    onClick={() => setSelectedStepIndex(index)}
                  >
                    <span className="step-number">{index + 1}</span>
                    <span className="step-name">{step.name}</span>
                    <span className="step-type">{step.type}</span>
                    <button
                      className="btn btn-danger-outline btn-sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteStep(index);
                      }}
                    >
                      ✕
                    </button>
                  </div>
                ))}
              </div>

              <div className="add-step-buttons">
                {stepTypes.map((stepType) => (
                  <button
                    key={stepType.type}
                    className="btn btn-secondary btn-sm"
                    onClick={() => handleAddStep(stepType.type)}
                  >
                    + {stepType.name}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {selectedStep && selectedStepType && (
            <div className="form-group step-details">
              <label>Step Details</label>
              <div className="step-details-form">
                <div className="form-group">
                  <label>Name</label>
                  <input
                    type="text"
                    value={selectedStep.name}
                    onChange={(e) =>
                      handleUpdateStep(selectedStepIndex!, { name: e.target.value })
                    }
                  />
                </div>

                <div className="form-group">
                  <label>Parameters</label>
                  {selectedStepType.params.map((param) => (
                    <div key={param.name} className="param-field">
                      <label>
                        {param.name}
                        {param.required && ' *'}
                      </label>
                      <input
                        type={param.type === 'number' ? 'number' : 'text'}
                        value={(selectedStep.params[param.name] as string) || ''}
                        onChange={(e) => {
                          const value = param.type === 'number'
                            ? parseFloat(e.target.value) || 0
                            : e.target.value;
                          handleUpdateStep(selectedStepIndex!, {
                            params: { ...selectedStep.params, [param.name]: value },
                          });
                        }}
                        placeholder={param.description}
                      />
                    </div>
                  ))}
                </div>

                <div className="form-group">
                  <label>
                    <input
                      type="checkbox"
                      checked={selectedStep.continue_on_error}
                      onChange={(e) =>
                        handleUpdateStep(selectedStepIndex!, {
                          continue_on_error: e.target.checked,
                        })
                      }
                    />
                    Continue on error
                  </label>
                </div>

                <div className="form-group">
                  <label>Timeout (ms)</label>
                  <input
                    type="number"
                    value={selectedStep.timeout_ms}
                    onChange={(e) =>
                      handleUpdateStep(selectedStepIndex!, {
                        timeout_ms: parseInt(e.target.value) || 30000,
                      })
                    }
                  />
                </div>
              </div>
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={handleSave}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
};
