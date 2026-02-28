import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ProfileItem } from './ProfileItem';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile, RunningStatus, RecordingSessionInfo } from '../types/profile';
import { useToast } from './Toast';
import { ConfirmDialog } from './ConfirmDialog';
import { SnapshotModal } from './SnapshotModal';

interface ProfileListProps {
  onEditProfile: (profile: BrowserProfile) => void;
  onCloneProfile: (profile: BrowserProfile) => void;
  refreshTrigger: number;
}

interface RecordingDialogState {
  profile: BrowserProfile | null;
  sessionInfo: RecordingSessionInfo | null;
}

export const ProfileList: React.FC<ProfileListProps> = ({ onEditProfile, onCloneProfile, refreshTrigger }) => {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [runningStatus, setRunningStatus] = useState<RunningStatus>({});
  const [recordingSessions, setRecordingSessions] = useState<Record<string, RecordingSessionInfo>>({});
  const [loading, setLoading] = useState(true);
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tagFilter, setTagFilter] = useState('');
  const [snapshotProfile, setSnapshotProfile] = useState<BrowserProfile | null>(null);
  const [recordingDialog, setRecordingDialog] = useState<RecordingDialogState | null>(null);

  const { showToast } = useToast();
  const [confirmState, setConfirmState] = useState<{
    message: string;
    onConfirm: () => void;
    confirmLabel: string;
    confirmClassName: string;
  } | null>(null);

  const loadProfiles = async () => {
    try {
      setLoading(true);
      const [profilesData, statusData] = await Promise.all([
        tauriApi.getProfiles(),
        tauriApi.getRunningProfiles(),
      ]);
      setProfiles(profilesData);
      setRunningStatus(statusData);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      console.error('Failed to load profiles:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadProfiles();

    // Listen for real-time events from backend (MCP or tray actions)
    const unlistenProfiles = listen('profiles-changed', () => {
      loadProfiles();
    });
    const unlistenStatus = listen('browser-status-changed', async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus(status);
      } catch (err) {
        console.error('Failed to refresh status:', err);
      }
    });

    const unlistenRecording = listen('recording-status-changed', async () => {
      try {
        await loadRecordingSessions();
      } catch (err) {
        console.error('Failed to refresh recording status:', err);
      }
    });

    // Polling as fallback for process crashes not yet detected by cleanup task
    const interval = setInterval(async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus(status);
      } catch (err) {
        console.error('Failed to refresh status:', err);
      }
    }, 5000);

    return () => {
      unlistenProfiles.then((f) => f());
      unlistenStatus.then((f) => f());
      unlistenRecording.then((f) => f());
      clearInterval(interval);
    };
  }, []);

  const loadRecordingSessions = async () => {
    try {
      const activeSessions = await tauriApi.getActiveRecordingSessions();
      const sessions: Record<string, RecordingSessionInfo> = {};
      for (const [profileId, _sessionId] of Object.entries(activeSessions)) {
        const info = await tauriApi.getRecordingSessionInfo(profileId);
        if (info) {
          sessions[profileId] = info;
        }
      }
      setRecordingSessions(sessions);
    } catch (err) {
      console.error('Failed to load recording sessions:', err);
    }
  };

  useEffect(() => {
    loadRecordingSessions();
  }, []);

  useEffect(() => {
    if (refreshTrigger > 0) {
      loadProfiles();
    }
  }, [refreshTrigger]);

  const handleLaunch = async (id: string) => {
    setLaunchingId(id);
    try {
      await tauriApi.launchProfile(id);
      const status = await tauriApi.getRunningProfiles();
      setRunningStatus(status);
      showToast('Browser launched', 'success');
    } catch (err) {
      showToast(`Failed to launch: ${err}`, 'error');
    } finally {
      setLaunchingId(null);
    }
  };

  const handleActivate = async (id: string) => {
    try {
      await tauriApi.activateProfile(id);
    } catch (err) {
      showToast(`Failed to activate: ${err}`, 'error');
    }
  };

  const handleKill = (id: string) => {
    setConfirmState({
      message: 'Kill this browser? Any unsaved data will be lost.',
      confirmLabel: 'Kill',
      confirmClassName: 'btn btn-danger',
      onConfirm: async () => {
        setConfirmState(null);
        try {
          await tauriApi.killProfile(id);
          const status = await tauriApi.getRunningProfiles();
          setRunningStatus(status);
          showToast('Browser stopped', 'success');
        } catch (err) {
          showToast(`Failed to kill: ${err}`, 'error');
        }
      },
    });
  };

  const handleDelete = (id: string) => {
    setConfirmState({
      message: 'Delete this profile? This cannot be undone.',
      confirmLabel: 'Delete',
      confirmClassName: 'btn btn-danger',
      onConfirm: async () => {
        setConfirmState(null);
        try {
          await tauriApi.deleteProfile(id);
          await loadProfiles();
          showToast('Profile deleted', 'success');
        } catch (err) {
          showToast(`Failed to delete: ${err}`, 'error');
        }
      },
    });
  };

  const handleRecordToggle = async (profile: BrowserProfile) => {
    const isRecording = !!recordingSessions[profile.id];
    if (isRecording) {
      // Show dialog to name the recording before stopping
      const sessionInfo = recordingSessions[profile.id];
      setRecordingDialog({ profile, sessionInfo });
    } else {
      // Start recording
      try {
        await tauriApi.startRecording(profile.id);
        await loadRecordingSessions();
        showToast('Recording started', 'success');
      } catch (err) {
        showToast(`Failed to start recording: ${err}`, 'error');
      }
    }
  };

  const handleStopRecording = async (name: string, description: string) => {
    if (!recordingDialog?.profile) return;

    try {
      await tauriApi.stopRecording(recordingDialog.profile.id, name, description);
      await loadRecordingSessions();
      setRecordingDialog(null);
      showToast('Recording saved', 'success');
    } catch (err) {
      showToast(`Failed to stop recording: ${err}`, 'error');
    }
  };

  if (loading) {
    return <div className="loading">Loading profiles...</div>;
  }

  if (error) {
    return <div className="error">Error: {error}</div>;
  }

  if (profiles.length === 0) {
    return (
      <div className="empty-state">
        <p>No profiles yet. Click "Add Profile" to create one.</p>
      </div>
    );
  }

  // Filter profiles by name or tags
  const filteredProfiles = profiles.filter((profile) => {
    if (!tagFilter.trim()) return true;
    const keywords = tagFilter.trim().toLowerCase().split(/\s+/);
    return keywords.some((kw) =>
      profile.name.toLowerCase().includes(kw) ||
      (profile.tags || []).some((tag) => tag.toLowerCase().includes(kw))
    );
  });

  return (
    <>
      <div className="profile-filter">
        <input
          type="text"
          className="filter-input"
          placeholder="Search by name or tags…"
          value={tagFilter}
          onChange={(e) => setTagFilter(e.target.value)}
        />
      </div>
      <div className="profile-list">
        {filteredProfiles.length === 0 ? (
          <div className="empty-state">
            <p>No profiles match your filter.</p>
          </div>
        ) : (
          filteredProfiles.map((profile) => (
            <ProfileItem
              key={profile.id}
              profile={profile}
              isRunning={runningStatus[profile.id] || false}
              isLaunching={launchingId === profile.id}
              isRecording={!!recordingSessions[profile.id]}
              recordingSession={recordingSessions[profile.id] || null}
              onLaunch={handleLaunch}
              onActivate={handleActivate}
              onKill={handleKill}
              onEdit={onEditProfile}
              onClone={onCloneProfile}
              onDelete={handleDelete}
              onSnapshots={setSnapshotProfile}
              onRecordToggle={handleRecordToggle}
            />
          ))
        )}
      </div>
      {confirmState && (
        <ConfirmDialog
          message={confirmState.message}
          confirmLabel={confirmState.confirmLabel}
          confirmClassName={confirmState.confirmClassName}
          onConfirm={confirmState.onConfirm}
          onCancel={() => setConfirmState(null)}
        />
      )}
      {snapshotProfile && (
        <SnapshotModal
          profileId={snapshotProfile.id}
          profileName={snapshotProfile.name}
          onClose={() => setSnapshotProfile(null)}
        />
      )}
      {recordingDialog && (
        <RecordingSaveDialog
          profile={recordingDialog.profile}
          sessionInfo={recordingDialog.sessionInfo}
          onSave={handleStopRecording}
          onCancel={() => setRecordingDialog(null)}
        />
      )}
    </>
  );
};

// Recording save dialog component
interface RecordingSaveDialogProps {
  profile: BrowserProfile | null;
  sessionInfo: RecordingSessionInfo | null;
  onSave: (name: string, description: string) => void;
  onCancel: () => void;
}

const RecordingSaveDialog: React.FC<RecordingSaveDialogProps> = ({
  profile,
  sessionInfo,
  onSave,
  onCancel,
}) => {
  const [name, setName] = useState(`Recording ${profile?.name || 'Browser'} ${new Date().toLocaleTimeString()}`);
  const [description, setDescription] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name.trim()) {
      onSave(name.trim(), description.trim());
    }
  };

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Save Recording</h2>
          <button className="modal-close" onClick={onCancel}>
            ×
          </button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="modal-body">
            <div className="form-group">
              <label htmlFor="recording-name">Name</label>
              <input
                id="recording-name"
                type="text"
                className="form-control"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
                autoFocus
              />
            </div>
            <div className="form-group">
              <label htmlFor="recording-description">Description</label>
              <textarea
                id="recording-description"
                className="form-control"
                rows={3}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="What does this recording do?"
              />
            </div>
            {sessionInfo && (
              <div className="recording-summary">
                <p>Actions recorded: <strong>{sessionInfo.action_count}</strong></p>
                <p>Duration: <strong>{Math.floor((Date.now() - sessionInfo.started_at) / 1000)}s</strong></p>
              </div>
            )}
          </div>
          <div className="modal-footer">
            <button type="button" className="btn btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn btn-primary">
              Save Recording
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
