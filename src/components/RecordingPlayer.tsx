import React, { useState } from 'react';
import { tauriApi } from '../api/tauri';
import type { Recording, BrowserProfile } from '../types/profile';

interface RecordingPlayerProps {
  recording: Recording;
  profiles: BrowserProfile[];
  onClose: () => void;
}

export const RecordingPlayer: React.FC<RecordingPlayerProps> = ({
  recording,
  profiles,
  onClose,
}) => {
  const [selectedProfileId, setSelectedProfileId] = useState('');
  const [currentActionIndex, setCurrentActionIndex] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const [completed, setCompleted] = useState(false);

  const handlePlay = async () => {
    if (!selectedProfileId) {
      alert('Please select a profile');
      return;
    }

    setPlaying(true);
    setCurrentActionIndex(0);

    // Execute actions sequentially
    for (let i = 0; i < recording.actions.length; i++) {
      setCurrentActionIndex(i);
      const action = recording.actions[i];

      try {
        await executeAction(action, selectedProfileId);
      } catch (e) {
        if (!confirm(`Action ${i + 1} failed: ${e}\n\nContinue?`)) {
          break;
        }
      }
    }

    setPlaying(false);
    setCompleted(true);
    setCurrentActionIndex(null);
  };

  const executeAction = async (action: any, profileId: string) => {
    const config = await tauriApi.getMcpConfig();
    if (!config?.enabled) throw new Error('API server not enabled');

    const base = `http://127.0.0.1:${config.api_port}`;
    const headers: Record<string, string> = {};
    if (config.api_key) headers['X-API-Key'] = config.api_key;

    const actionEndpoints: Record<string, string> = {
      navigate: `/api/browser/${profileId}/navigate_wait`,
      click: `/api/browser/${profileId}/click`,
      type: `/api/browser/${profileId}/type`,
      sleep: '/api/sleep', // Doesn't exist, simulate with delay
    };

    const endpoint = actionEndpoints[action.type];
    if (!endpoint) {
      throw new Error(`Action type ${action.type} not implemented`);
    }

    if (action.type === 'sleep') {
      const duration = action.params.duration_ms || 1000;
      await new Promise(resolve => setTimeout(resolve, duration));
      return;
    }

    const response = await fetch(`${base}${endpoint}`, {
      method: 'POST',
      headers: {
        ...headers,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(action.params),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content recording-player" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Playing: {recording.name}</h3>
          <button className="modal-close" onClick={onClose}>✕</button>
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
                  width: currentActionIndex !== null
                    ? `${((currentActionIndex + 1) / recording.actions.length) * 100}%`
                    : '0%',
                }}
              />
            </div>
            <div className="progress-text">
              {currentActionIndex !== null
                ? `Action ${currentActionIndex + 1} of ${recording.actions.length}`
                : 'Ready to play'}
            </div>
          </div>

          <div className="recording-actions-list">
            {recording.actions.map((action, index) => {
              const isCurrent = index === currentActionIndex;
              const isPast = currentActionIndex !== null && index < currentActionIndex;

              return (
                <div
                  key={index}
                  className={`recorded-action-item ${isCurrent ? 'current' : ''} ${isPast ? 'done' : ''}`}
                >
                  <span className="action-number">{index + 1}</span>
                  <span className="action-type">{action.type}</span>
                  <span className="action-time">{action.timestamp_ms}ms</span>
                  {isCurrent && <span className="action-status">Running...</span>}
                  {isPast && <span className="action-status done">✓</span>}
                </div>
              );
            })}
          </div>

          {completed && (
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
