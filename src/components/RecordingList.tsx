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
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

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

  // Batch selection handlers
  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selectedIds.size === recordings.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(recordings.map((r) => r.id)));
    }
  };

  const handleBatchDelete = () => {
    const count = selectedIds.size;
    setConfirmState({
      message: `Delete ${count} recording${count > 1 ? 's' : ''}?`,
      onConfirm: async () => {
        setConfirmState(null);
        let success = 0;
        let failed = 0;
        for (const id of selectedIds) {
          try {
            await tauriApi.deleteRecording(id);
            success++;
          } catch {
            failed++;
          }
        }
        setSelectedIds(new Set());
        setRefreshKey((prev) => prev + 1);
        if (failed === 0) {
          showToast(`Deleted ${success} recording${success > 1 ? 's' : ''}`, 'success');
        } else {
          showToast(`Deleted ${success}, failed ${failed}`, 'warning');
        }
      },
    });
  };

  const clearSelection = () => {
    setSelectedIds(new Set());
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
        {recordings.length > 0 && (
          <div className="recording-batch-actions">
            <label className="select-all-label">
              <input
                type="checkbox"
                checked={selectedIds.size === recordings.length && recordings.length > 0}
                onChange={toggleSelectAll}
              />
              <span>Select all</span>
            </label>
            {selectedIds.size > 0 && (
              <>
                <span className="selection-count">{selectedIds.size} selected</span>
                <button className="btn btn-danger-outline btn-sm" onClick={handleBatchDelete}>
                  Delete Selected
                </button>
                <button className="btn btn-outline btn-sm" onClick={clearSelection}>
                  Clear
                </button>
              </>
            )}
          </div>
        )}
      </div>

      {recordings.length === 0 ? (
        <div className="recording-empty-state">
          <p className="muted">No recordings yet.</p>
          <p className="muted">Recordings will be captured when you use browser automation.</p>
        </div>
      ) : (
        <div className="recording-grid">
          {recordings.map((recording) => (
            <div
              key={recording.id}
              className={`recording-card ${selectedIds.has(recording.id) ? 'selected' : ''}`}
            >
              <div className="recording-card-select">
                <input
                  type="checkbox"
                  checked={selectedIds.has(recording.id)}
                  onChange={() => toggleSelect(recording.id)}
                />
              </div>
              <div className="recording-card-content">
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
                    Play
                  </button>
                  <button
                    className="btn btn-danger-outline btn-sm"
                    onClick={() => handleDelete(recording.id, recording.name)}
                  >
                    Delete
                  </button>
                </div>
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
