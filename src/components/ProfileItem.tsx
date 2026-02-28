import React from 'react';
import type { BrowserProfile } from '../types/profile';

interface ProfileItemProps {
  profile: BrowserProfile;
  isRunning: boolean;
  isLaunching?: boolean;
  onLaunch: (id: string) => void;
  onActivate: (id: string) => void;
  onKill: (id: string) => void;
  onEdit: (profile: BrowserProfile) => void;
  onClone: (profile: BrowserProfile) => void;
  onDelete: (id: string) => void;
  onSnapshots?: (profile: BrowserProfile) => void;
}

export const ProfileItem: React.FC<ProfileItemProps> = ({
  profile,
  isRunning,
  isLaunching = false,
  onLaunch,
  onActivate,
  onKill,
  onEdit,
  onClone,
  onDelete,
  onSnapshots,
}) => {
  return (
    <div className="profile-item">
      <div className="profile-header">
        <div
          className="profile-color"
          style={{ backgroundColor: profile.color || '#666' }}
        />
        <div className="profile-info">
          <h3>{profile.name}</h3>
          {profile.description && <p className="description">{profile.description}</p>}
          <div className="profile-details">
            <span className="detail">Lang: {profile.lang}</span>
            {profile.timezone && (
              <span className="detail">TZ: {profile.timezone}</span>
            )}
            <span
              className="detail detail-muted"
              title={profile.user_data_dir}
            >
              Dir: …/{profile.user_data_dir.split(/[/\\]/).filter(Boolean).pop() ?? profile.user_data_dir}
            </span>
            {profile.tags && profile.tags.length > 0 && (
              <>
                {profile.tags.slice(0, 3).map((tag) => (
                  <span key={tag} className="profile-tag">
                    {tag}
                  </span>
                ))}
                {profile.tags.length > 3 && (
                  <span className="profile-tag">+{profile.tags.length - 3}</span>
                )}
              </>
            )}
          </div>
        </div>
        <div className="profile-status">
          <span className={`status-indicator ${isRunning ? 'running' : 'stopped'}`}>
            {isRunning ? '● Running' : '○ Stopped'}
          </span>
        </div>
      </div>

      <div className="profile-actions">
        {!isRunning ? (
          <button
            className="btn btn-primary"
            onClick={() => onLaunch(profile.id)}
            disabled={isLaunching}
          >
            {isLaunching ? 'Launching…' : 'Launch'}
          </button>
        ) : (
          <>
            <button className="btn btn-success" onClick={() => onActivate(profile.id)}>
              Activate
            </button>
            <button className="btn btn-danger" onClick={() => onKill(profile.id)}>
              Kill
            </button>
          </>
        )}
        <button className="btn btn-secondary" onClick={() => onEdit(profile)}>
          Edit
        </button>
        <button className="btn btn-info" onClick={() => onClone(profile)}>
          Clone
        </button>
        {onSnapshots && (
          <button className="btn btn-secondary" onClick={() => onSnapshots(profile)}>
            Snapshots
          </button>
        )}
        <button
          className="btn btn-danger-outline"
          onClick={() => onDelete(profile.id)}
          disabled={isRunning}
        >
          Delete
        </button>
      </div>

      {profile.proxy_server && (
        <div className="profile-footer">
          <small>Proxy: {profile.proxy_server}</small>
        </div>
      )}
    </div>
  );
};
