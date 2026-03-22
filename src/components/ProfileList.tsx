import React, { useDeferredValue, useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ProfileItem } from './ProfileItem';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile, RunningStatus } from '../types/profile';
import { useToast } from './Toast';
import { ConfirmDialog } from './ConfirmDialog';
import { UI_CONSTANTS } from './constants';
import { areRunningStatusesEqual, mergeProfilesById, profileMatchesFilter } from '../utils';

interface ProfileListProps {
  onEditProfile: (profile: BrowserProfile) => void;
  onCloneProfile: (profile: BrowserProfile) => void;
  refreshTrigger: number;
}

const VIRTUALIZATION_THRESHOLD = 80;
const PROFILE_CARD_MIN_WIDTH = 360;
const PROFILE_GRID_GAP = 12;
const PROFILE_CARD_HEIGHT = 190;
const PROFILE_OVERSCAN_ROWS = 2;

export const ProfileList: React.FC<ProfileListProps> = React.memo(({ onEditProfile, onCloneProfile, refreshTrigger }) => {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [runningStatus, setRunningStatus] = useState<RunningStatus>({});
  const [loading, setLoading] = useState(true);
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tagFilter, setTagFilter] = useState('');
  const deferredTagFilter = useDeferredValue(tagFilter);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [viewportWidth, setViewportWidth] = useState(0);
  const [firstVisibleRow, setFirstVisibleRow] = useState(0);

  const { showToast } = useToast();
  const [confirmState, setConfirmState] = useState<{
    message: string;
    onConfirm: () => void;
    confirmLabel: string;
    confirmClassName: string;
  } | null>(null);

  const loadProfiles = useCallback(async () => {
    try {
      setLoading(true);
      const [profilesData, statusData] = await Promise.all([
        tauriApi.getProfiles(),
        tauriApi.getRunningProfiles(),
      ]);
      setProfiles((prev) => mergeProfilesById(prev, profilesData));
      setRunningStatus((prev) =>
        areRunningStatusesEqual(prev, statusData) ? prev : statusData
      );
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      console.error('Failed to load profiles:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshProfilesSilent = useCallback(async () => {
    try {
      const [profilesData, statusData] = await Promise.all([
        tauriApi.getProfiles(),
        tauriApi.getRunningProfiles(),
      ]);
      setProfiles((prev) => mergeProfilesById(prev, profilesData));
      setRunningStatus((prev) =>
        areRunningStatusesEqual(prev, statusData) ? prev : statusData
      );
    } catch (err) {
      console.error('Failed to refresh profiles:', err);
    }
  }, []);

  useEffect(() => {
    loadProfiles();

    // Listen for real-time events from backend (local API or tray actions)
    const unlistenProfiles = listen('profiles-changed', () => {
      refreshProfilesSilent();
    });
    const unlistenStatus = listen('browser-status-changed', async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus((prev) =>
          areRunningStatusesEqual(prev, status) ? prev : status
        );
      } catch (err) {
        console.error('Failed to refresh status:', err);
      }
    });

    // Polling as fallback for process crashes not yet detected by cleanup task
    const interval = setInterval(async () => {
      try {
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus((prev) =>
          areRunningStatusesEqual(prev, status) ? prev : status
        );
      } catch (err) {
        console.error('Failed to refresh status:', err);
      }
    }, UI_CONSTANTS.POLLING_INTERVAL_MS);

    return () => {
      unlistenProfiles.then((f) => f());
      unlistenStatus.then((f) => f());
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    if (refreshTrigger > 0) {
      refreshProfilesSilent();
    }
  }, [refreshTrigger, refreshProfilesSilent]);

  const handleLaunch = useCallback(async (id: string) => {
    setLaunchingId(id);
    try {
      await tauriApi.launchProfile(id);
      const status = await tauriApi.getRunningProfiles();
      setRunningStatus((prev) =>
        areRunningStatusesEqual(prev, status) ? prev : status
      );
      showToast('Browser launched', 'success');
    } catch (err) {
      showToast(`Failed to launch: ${err}`, 'error');
    } finally {
      setLaunchingId(null);
    }
  }, [showToast]);

  const handleActivate = useCallback(async (id: string) => {
    try {
      await tauriApi.activateProfile(id);
    } catch (err) {
      showToast(`Failed to activate: ${err}`, 'error');
    }
  }, [showToast]);

  const handleKill = useCallback((id: string) => {
    setConfirmState({
      message: 'Kill this browser? Any unsaved data will be lost.',
      confirmLabel: 'Kill',
      confirmClassName: 'btn btn-danger',
      onConfirm: async () => {
        setConfirmState(null);
        try {
          await tauriApi.killProfile(id);
          const status = await tauriApi.getRunningProfiles();
          setRunningStatus((prev) =>
            areRunningStatusesEqual(prev, status) ? prev : status
          );
          showToast('Browser stopped', 'success');
        } catch (err) {
          showToast(`Failed to kill: ${err}`, 'error');
        }
      },
    });
  }, [showToast]);

  const handleDelete = useCallback((id: string) => {
    setConfirmState({
      message: 'Delete this profile? This cannot be undone.',
      confirmLabel: 'Delete',
      confirmClassName: 'btn btn-danger',
      onConfirm: async () => {
        setConfirmState(null);
        try {
          await tauriApi.deleteProfile(id);
          await refreshProfilesSilent();
          showToast('Profile deleted', 'success');
        } catch (err) {
          showToast(`Failed to delete: ${err}`, 'error');
        }
      },
    });
  }, [showToast, refreshProfilesSilent]);

  const filteredProfiles = useMemo(() => {
    if (!deferredTagFilter.trim()) {
      return profiles;
    }

    return profiles.filter((profile) => profileMatchesFilter(profile, deferredTagFilter));
  }, [profiles, deferredTagFilter]);

  const useVirtualizedList = filteredProfiles.length >= VIRTUALIZATION_THRESHOLD;

  useEffect(() => {
    if (!useVirtualizedList) {
      return undefined;
    }

    const node = viewportRef.current;
    if (!node) {
      return undefined;
    }

    const updateSize = () => {
      setViewportHeight(node.clientHeight);
      setViewportWidth(node.clientWidth);
    };

    updateSize();

    const observer = new ResizeObserver(updateSize);
    observer.observe(node);

    return () => {
      observer.disconnect();
    };
  }, [useVirtualizedList]);

  useEffect(() => {
    setFirstVisibleRow(0);
    viewportRef.current?.scrollTo({ top: 0 });
  }, [deferredTagFilter]);

  const columnCount = useMemo(() => {
    if (!useVirtualizedList || viewportWidth <= 0) {
      return 1;
    }

    return Math.max(
      1,
      Math.floor((viewportWidth + PROFILE_GRID_GAP) / (PROFILE_CARD_MIN_WIDTH + PROFILE_GRID_GAP))
    );
  }, [useVirtualizedList, viewportWidth]);

  const itemWidth = useMemo(() => {
    if (!useVirtualizedList || viewportWidth <= 0) {
      return 0;
    }

    return Math.max(0, (viewportWidth - PROFILE_GRID_GAP * (columnCount - 1)) / columnCount);
  }, [columnCount, useVirtualizedList, viewportWidth]);

  const rowStride = PROFILE_CARD_HEIGHT + PROFILE_GRID_GAP;
  const totalRows = Math.ceil(filteredProfiles.length / columnCount);
  const totalHeight = Math.max(0, totalRows * rowStride - PROFILE_GRID_GAP);
  const visibleStartRow = Math.min(firstVisibleRow, Math.max(0, totalRows - 1));
  const visibleEndRow = Math.min(
    totalRows,
    visibleStartRow + Math.ceil(viewportHeight / rowStride) + PROFILE_OVERSCAN_ROWS * 2
  );
  const visibleStartIndex = visibleStartRow * columnCount;
  const visibleEndIndex = Math.min(filteredProfiles.length, visibleEndRow * columnCount);
  const visibleProfiles = useMemo(
    () => filteredProfiles.slice(visibleStartIndex, visibleEndIndex),
    [filteredProfiles, visibleEndIndex, visibleStartIndex]
  );

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
      {filteredProfiles.length === 0 ? (
        <div className="empty-state">
          <p>No profiles match your filter.</p>
        </div>
      ) : useVirtualizedList ? (
        <div
          ref={viewportRef}
          className="profile-list-viewport"
          onScroll={(event) => {
            const nextRow = Math.max(
              0,
              Math.floor(event.currentTarget.scrollTop / rowStride) - PROFILE_OVERSCAN_ROWS
            );
            setFirstVisibleRow((prev) => (prev === nextRow ? prev : nextRow));
          }}
        >
          <div className="profile-list-virtual-spacer" style={{ height: totalHeight }}>
            {visibleProfiles.map((profile, index) => {
              const absoluteIndex = visibleStartIndex + index;
              const row = Math.floor(absoluteIndex / columnCount);
              const column = absoluteIndex % columnCount;

              return (
                <div
                  key={profile.id}
                  className="profile-list-virtual-item"
                  style={{
                    top: row * rowStride,
                    left: column * (itemWidth + PROFILE_GRID_GAP),
                    width: itemWidth,
                    height: PROFILE_CARD_HEIGHT,
                  }}
                >
                  <ProfileItem
                    profile={profile}
                    isRunning={runningStatus[profile.id] || false}
                    isLaunching={launchingId === profile.id}
                    onLaunch={handleLaunch}
                    onActivate={handleActivate}
                    onKill={handleKill}
                    onEdit={onEditProfile}
                    onClone={onCloneProfile}
                    onDelete={handleDelete}
                  />
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <div className="profile-list">
          {filteredProfiles.map((profile) => (
            <ProfileItem
              key={profile.id}
              profile={profile}
              isRunning={runningStatus[profile.id] || false}
              isLaunching={launchingId === profile.id}
              onLaunch={handleLaunch}
              onActivate={handleActivate}
              onKill={handleKill}
              onEdit={onEditProfile}
              onClone={onCloneProfile}
              onDelete={handleDelete}
            />
          ))}
        </div>
      )}
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
});
