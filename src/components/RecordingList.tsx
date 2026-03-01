import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { Recording, BrowserProfile } from '../types/profile';
import { RecordingPlayer } from './RecordingPlayer';
import { useToast } from './Toast';
import { ConfirmDialog } from './ConfirmDialog';

interface RecordingListProps {
  profiles: BrowserProfile[];
}

export const RecordingList: React.FC<RecordingListProps> = ({ profiles }) => {
  const { showToast } = useToast();
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [playingRecording, setPlayingRecording] = useState<Recording | undefined>();
  const [refreshKey, setRefreshKey] = useState(0);
  const [confirmState, setConfirmState] = useState<{
    message: string;
    onConfirm: () => void;
  } | null>(null);

  useEffect(() => {
    loadRecordings();
  }, [refreshKey]);

  const loadRecordings = async () => {
    try {
      const list = await tauriApi.listRecordings();
      setRecordings(list);
    } catch (e) {
      console.error('Failed to load recordings:', e);
    }
  };

  const handleDelete = (id: string, name: string) => {
    setConfirmState({
      message: `Delete recording "${name}"?`,
      onConfirm: async () => {
        setConfirmState(null);
        try {
          await tauriApi.deleteRecording(id);
          setRefreshKey((prev) => prev + 1);
          showToast('Recording deleted', 'success');
        } catch (e) {
          showToast(`Failed to delete: ${e}`, 'error');
        }
      },
    });
  };

  const handleConvertToWorkflow = async (recording: Recording) => {
    try {
      const workflowName = `Workflow from ${recording.name}`;
      const workflow = await tauriApi.recordingToWorkflow(recording.id, workflowName);
      await tauriApi.saveWorkflow(workflow);
      showToast('Workflow created successfully! Check the Workflows tab.', 'success');
    } catch (e) {
      showToast(`Failed to create workflow: ${e}`, 'error');
    }
  };

  const formatDuration = (ms: number) => {
    if (ms < 1000) return `${ms}ms`;
    const seconds = Math.floor(ms / 1000);
    return `${seconds}s`;
  };

  const formatDate = (ts: number) => {
    return new Date(ts).toLocaleString();
  };

  return (
    <div className="recording-list">
      <div className="recording-list-header">
        <h2>Recordings</h2>
      </div>

      {recordings.length === 0 ? (
        <div className="recording-empty-state">
          <p className="muted">No recordings yet.</p>
          <p className="muted">Recordings will be captured when you use browser automation.</p>
          <p className="muted">You can also convert recordings to workflows for reuse.</p>
        </div>
      ) : (
        <div className="recording-grid">
          {recordings.map((recording) => (
            <div key={recording.id} className="recording-card">
              <div className="recording-card-header">
                <h3>{recording.name}</h3>
                <span className="recording-meta">
                  {recording.actions.length} actions · {formatDuration(recording.duration_ms)}
                </span>
              </div>
              {recording.description && (
                <p className="recording-description">{recording.description}</p>
              )}
              <div className="recording-meta-info">
                <small>Profile: {recording.profile_id}</small>
                <small>Created: {formatDate(recording.created_at)}</small>
              </div>
              <div className="recording-card-footer">
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => setPlayingRecording(recording)}
                >
                  ▶ Play
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => handleConvertToWorkflow(recording)}
                >
                  Convert to Workflow
                </button>
                <button
                  className="btn btn-danger-outline btn-sm"
                  onClick={() => handleDelete(recording.id, recording.name)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {playingRecording && (
        <RecordingPlayer
          recording={playingRecording}
          profiles={profiles}
          onClose={() => setPlayingRecording(undefined)}
        />
      )}

      {confirmState && (
        <ConfirmDialog
          message={confirmState.message}
          onConfirm={confirmState.onConfirm}
          onCancel={() => setConfirmState(null)}
        />
      )}
    </div>
  );
};
