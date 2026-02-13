import React, { useEffect, useState } from 'react';
import { ProfileItem } from './ProfileItem';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile, RunningStatus } from '../types/profile';

interface ProfileListProps {
  onEditProfile: (profile: BrowserProfile) => void;
  onCloneProfile: (profile: BrowserProfile) => void;
}

export const ProfileList: React.FC<ProfileListProps> = ({ onEditProfile, onCloneProfile }) => {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [runningStatus, setRunningStatus] = useState<RunningStatus>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tagFilter, setTagFilter] = useState('');

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

  const handleLaunch = async (id: string) => {
    try {
      await tauriApi.launchProfile(id);
      // Refresh status immediately
      const status = await tauriApi.getRunningProfiles();
      setRunningStatus(status);
    } catch (err) {
      alert(`Failed to launch profile: ${err}`);
    }
  };

  const handleActivate = async (id: string) => {
    try {
      await tauriApi.activateProfile(id);
    } catch (err) {
      alert(`Failed to activate profile: ${err}`);
    }
  };

  const handleKill = async (id: string) => {
    try {
      await tauriApi.killProfile(id);
      // Refresh status immediately
      const status = await tauriApi.getRunningProfiles();
      setRunningStatus(status);
    } catch (err) {
      alert(`Failed to kill profile: ${err}`);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Are you sure you want to delete this profile?')) {
      return;
    }

    try {
      await tauriApi.deleteProfile(id);
      await loadProfiles();
    } catch (err) {
      alert(`Failed to delete profile: ${err}`);
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
    </>
  );
};
