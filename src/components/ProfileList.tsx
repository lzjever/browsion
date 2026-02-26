import React, { useEffect, useState } from 'react';
import { ProfileItem } from './ProfileItem';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile, RunningStatus } from '../types/profile';
import { useToast } from './Toast';
import { ConfirmDialog } from './ConfirmDialog';

interface ProfileListProps {
  onEditProfile: (profile: BrowserProfile) => void;
  onCloneProfile: (profile: BrowserProfile) => void;
  refreshTrigger: number;
}

export const ProfileList: React.FC<ProfileListProps> = ({ onEditProfile, onCloneProfile, refreshTrigger }) => {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [runningStatus, setRunningStatus] = useState<RunningStatus>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tagFilter, setTagFilter] = useState('');

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

    // Refresh status every 2 seconds
    const interval = setInterval(async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus(status);
      } catch (err) {
        console.error('Failed to refresh status:', err);
      }
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    if (refreshTrigger > 0) {
      loadProfiles();
    }
  }, [refreshTrigger]);

  const handleLaunch = async (id: string) => {
    try {
      await tauriApi.launchProfile(id);
      const status = await tauriApi.getRunningProfiles();
      setRunningStatus(status);
      showToast('Browser launched', 'success');
    } catch (err) {
      showToast(`Failed to launch: ${err}`, 'error');
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

  // Filter profiles by tags
  const filteredProfiles = profiles.filter((profile) => {
    if (!tagFilter.trim()) return true;
    const keywords = tagFilter.trim().toLowerCase().split(/\s+/);
    return keywords.some((kw) =>
      (profile.tags || []).some((tag) => tag.toLowerCase().includes(kw))
    );
  });

  return (
    <>
      <div className="profile-filter">
        <input
          type="text"
          className="filter-input"
          placeholder="Filter by tags (e.g., work testing)"
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
              onLaunch={handleLaunch}
              onActivate={handleActivate}
              onKill={handleKill}
              onEdit={onEditProfile}
              onClone={onCloneProfile}
              onDelete={handleDelete}
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
    </>
  );
};
