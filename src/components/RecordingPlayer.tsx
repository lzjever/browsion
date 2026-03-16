import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { tauriApi } from '../api/tauri';
import type { Recording, BrowserProfile } from '../types/profile';
import { useToast } from './Toast';

interface RecordingPlayerProps {
  recording: Recording;
  profiles: BrowserProfile[];
  onClose: () => void;
}

interface PlaybackProgressEvent {
  recording_id: string;
  profile_id: string;
  action_index: number;
  total_actions: number;
  action_type: string;
  status: 'running' | 'failed' | 'completed';
  error?: string | null;
}

export const RecordingPlayer: React.FC<RecordingPlayerProps> = ({
  recording,
  profiles,
  onClose,
}) => {
  const { showToast } = useToast();
  const [selectedProfileId, setSelectedProfileId] = useState('');
  const [playing, setPlaying] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [currentActionIndex, setCurrentActionIndex] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<PlaybackProgressEvent>('recording-playback-progress', (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (payload.recording_id !== recording.id) return;
      if (selectedProfileId && payload.profile_id !== selectedProfileId) return;

      if (payload.status === 'running') {
        setCurrentActionIndex(payload.action_index);
      } else if (payload.status === 'failed') {
        setCurrentActionIndex(payload.action_index);
        setError(payload.error ?? 'Playback failed');
      } else if (payload.status === 'completed') {
        setCurrentActionIndex(recording.actions.length > 0 ? recording.actions.length - 1 : null);
      }
    });

    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, [recording.id, recording.actions.length, selectedProfileId]);

  const handlePlay = async () => {
    if (!selectedProfileId) {
      showToast('Please select a profile', 'warning');
      return;
    }

    setPlaying(true);
    setCompleted(false);
    setError(null);
    setCurrentActionIndex(0);

    try {
      const result = await tauriApi.playRecording(recording.id, selectedProfileId);
      setCompleted(true);
      showToast(
        `Playback completed: ${result.completed_actions}/${result.total_actions} actions`,
        'success'
      );
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
      showToast(message, 'error');
    } finally {
      setPlaying(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content recording-player" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Playing: {recording.name}</h3>
          <button className="modal-close" onClick={onClose} aria-label="Close modal">✕</button>
        </div>

        <div className="recording-player-body">
          <div className="form-group">
            <label>Select Profile *</label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              disabled={playing}
            >
              <option value="">-- Choose a profile --</option>
              {profiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name}
                </option>
              ))}
            </select>
          </div>

          <div className="recording-progress">
            <div className="progress-bar">
              <div
                className="progress-fill"
                style={{
                  width:
                    completed
                      ? '100%'
                      : currentActionIndex !== null
                        ? `${Math.min(((currentActionIndex + 1) / recording.actions.length) * 100, 100)}%`
                        : playing
                          ? '10%'
                          : '0%',
                }}
              />
            </div>
            <div className="progress-text">
              {playing
                ? currentActionIndex !== null
                  ? `Action ${Math.min(currentActionIndex + 1, recording.actions.length)} of ${recording.actions.length}`
                  : 'Playing in Rust backend...'
                : completed
                  ? `Completed ${recording.actions.length} actions`
                  : 'Ready to play'}
            </div>
          </div>

          <div className="recording-actions-list">
            {recording.actions.map((action, index) => (
              <div
                key={index}
                className={`recorded-action-item ${index === currentActionIndex ? 'current' : ''} ${((completed || (currentActionIndex !== null && index < currentActionIndex)) ? 'done' : '')}`}
              >
                <span className="action-number">{index + 1}</span>
                <span className="action-type">{action.type}</span>
                <span className="action-time">{action.timestamp_ms}ms</span>
                {index === currentActionIndex && playing && <span className="action-status">Running...</span>}
                {(completed || (currentActionIndex !== null && index < currentActionIndex)) && (
                  <span className="action-status done">✓</span>
                )}
              </div>
            ))}
          </div>

          {error && <div className="error-message">{error}</div>}
          {completed && !error && (
            <div className="success-message">
              Playback completed! {recording.actions.length} actions executed.
            </div>
          )}
        </div>

        <div className="modal-footer">
          <button className="btn btn-secondary" onClick={onClose}>
            {completed ? 'Close' : 'Cancel'}
          </button>
          {!completed && !playing && (
            <button
              className="btn btn-primary"
              onClick={handlePlay}
              disabled={!selectedProfileId}
            >
              ▶ Play Recording
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
