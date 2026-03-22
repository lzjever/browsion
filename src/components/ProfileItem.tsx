import React from 'react';
import type { BrowserProfile } from '../types/profile';
import { areProfilesEqual } from '../utils';

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
}

export const ProfileItem = React.memo<ProfileItemProps>(({ profile, isRunning, isLaunching = false, onLaunch, onActivate, onKill, onEdit, onClone, onDelete }) => {
  const directoryName = profile.user_data_dir.split(/[/\\]/).filter(Boolean).pop() ?? profile.user_data_dir;
  const visibleTags = profile.tags?.slice(0, 4) ?? [];

  return (
    <div className="profile-item">
      <div className="profile-header">
        <div
          className="profile-color"
          style={{ backgroundColor: profile.color || '#666' }}
        />
        <div className="profile-main">
          <div className="profile-title-row">
            <div className="profile-info">
              <h3>{profile.name}</h3>
              {profile.description && <p className="description">{profile.description}</p>}
            </div>
            <div className="profile-status">
              <span className={`status-indicator ${isRunning ? 'running' : 'stopped'}`}>
                {isRunning ? '● Running' : '○ Stopped'}
              </span>
            </div>
          </div>

          <div className="profile-details">
            <span className="detail">Lang: {profile.lang}</span>
            {profile.timezone && (
              <span className="detail">TZ: {profile.timezone}</span>
            )}
            <span
              className="detail detail-muted"
              title={profile.user_data_dir}
            >
              Dir: …/{directoryName}
            </span>
          </div>

          {(visibleTags.length > 0 || profile.proxy_server) && (
            <div className="profile-supporting">
              {visibleTags.length > 0 && (
                <div className="profile-tags-row">
                  {visibleTags.map((tag) => (
                    <span key={tag} className="profile-tag">
                      {tag}
                    </span>
                  ))}
                  {profile.tags.length > visibleTags.length && (
                    <span className="profile-tag">+{profile.tags.length - visibleTags.length}</span>
                  )}
                </div>
              )}

              {profile.proxy_server && (
                <div className="profile-proxy" title={profile.proxy_server}>
                  Proxy: {profile.proxy_server}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <div className="profile-actions">
        {!isRunning ? (
          <button
            className="btn btn-primary btn-sm profile-action-btn profile-action-primary"
            onClick={() => onLaunch(profile.id)}
            disabled={isLaunching}
          >
            {isLaunching ? 'Launching…' : 'Launch'}
          </button>
        ) : (
          <>
            <button className="btn btn-success btn-sm profile-action-btn profile-action-primary" onClick={() => onActivate(profile.id)}>
              Activate
            </button>
            <button className="btn btn-danger btn-sm profile-action-btn" onClick={() => onKill(profile.id)}>
              Kill
            </button>
          </>
        )}
        <button className="btn btn-secondary btn-sm profile-action-btn" onClick={() => onEdit(profile)}>
          Edit
        </button>
        <button className="btn btn-info btn-sm profile-action-btn" onClick={() => onClone(profile)}>
          Clone
        </button>
        <button
          className="btn btn-danger-outline btn-sm profile-action-btn"
          onClick={() => onDelete(profile.id)}
          disabled={isRunning}
        >
          Delete
        </button>
      </div>
    </div>
  );
}, (prevProps, nextProps) => (
  prevProps.isRunning === nextProps.isRunning &&
  prevProps.isLaunching === nextProps.isLaunching &&
  prevProps.onLaunch === nextProps.onLaunch &&
  prevProps.onActivate === nextProps.onActivate &&
  prevProps.onKill === nextProps.onKill &&
  prevProps.onEdit === nextProps.onEdit &&
  prevProps.onClone === nextProps.onClone &&
  prevProps.onDelete === nextProps.onDelete &&
  areProfilesEqual(prevProps.profile, nextProps.profile)
));
