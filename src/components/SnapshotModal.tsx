import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { SnapshotInfo } from '../types/profile';
import { ConfirmDialog } from './ConfirmDialog';
import { UI_CONSTANTS } from './constants';

interface SnapshotModalProps {
  profileId: string;
  profileName: string;
  onClose: () => void;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTs(ts: number): string {
  return new Date(ts).toLocaleString();
}

export const SnapshotModal: React.FC<SnapshotModalProps> = ({
  profileId,
  profileName,
  onClose,
}) => {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState('');
  const [creating, setCreating] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [confirmState, setConfirmState] = useState<{
    message: string;
    onConfirm: () => void;
  } | null>(null);

  useEffect(() => {
    loadSnapshots();
  }, [profileId]);

  const loadSnapshots = async () => {
    try {
      setLoading(true);
      const list = await tauriApi.listSnapshots(profileId);
      setSnapshots(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) return;
    if (!/^[\w.-]+$/.test(name)) {
      setError('Snapshot name: letters, digits, - _ . only');
      return;
    }
    try {
      setCreating(true);
      setError(null);
      await tauriApi.createSnapshot(profileId, name);
      setNewName('');
      setSuccess(`Snapshot "${name}" created`);
      setTimeout(() => setSuccess(null), UI_CONSTANTS.SUCCESS_MESSAGE_DURATION_MS);
      await loadSnapshots();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreating(false);
    }
  };

  const handleRestore = (name: string) => {
    setConfirmState({
      message: `Restore snapshot "${name}"? This will overwrite the current profile data.`,
      onConfirm: async () => {
        setConfirmState(null);
        try {
          setRestoring(name);
          setError(null);
          await tauriApi.restoreSnapshot(profileId, name);
          setSuccess(`Restored snapshot "${name}"`);
          setTimeout(() => setSuccess(null), UI_CONSTANTS.SUCCESS_MESSAGE_DURATION_MS);
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
        } finally {
          setRestoring(null);
        }
      },
    });
  };

  const handleDelete = (name: string) => {
    setConfirmState({
      message: `Delete snapshot "${name}"?`,
      onConfirm: async () => {
        setConfirmState(null);
        try {
          setDeleting(name);
          setError(null);
          await tauriApi.deleteSnapshot(profileId, name);
          await loadSnapshots();
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
        } finally {
          setDeleting(null);
        }
      },
    });
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Snapshots — {profileName}</h3>
          <button className="modal-close" onClick={onClose} aria-label="Close modal">✕</button>
        </div>

        {error && <div className="error-message">{error}</div>}
        {success && <div className="success-message">{success}</div>}

        <div className="snapshot-create">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Snapshot name (e.g. before-login)"
            onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
          />
          <button
            className="btn btn-primary"
            onClick={handleCreate}
            disabled={creating || !newName.trim()}
          >
            {creating ? 'Creating…' : 'Create Snapshot'}
          </button>
        </div>

        {loading ? (
          <div className="loading">Loading snapshots…</div>
        ) : snapshots.length === 0 ? (
          <p className="muted">No snapshots yet. Browser must be stopped to create one.</p>
        ) : (
          <table className="snapshot-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Created</th>
                <th>Size</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {snapshots.map((s) => (
                <tr key={s.name}>
                  <td>{s.name}</td>
                  <td>{formatTs(s.created_at_ts)}</td>
                  <td>{formatBytes(s.size_bytes)}</td>
                  <td>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={() => handleRestore(s.name)}
                      disabled={restoring === s.name}
                    >
                      {restoring === s.name ? 'Restoring…' : 'Restore'}
                    </button>
                    <button
                      className="btn btn-danger-outline btn-sm"
                      onClick={() => handleDelete(s.name)}
                      disabled={deleting === s.name}
                    >
                      {deleting === s.name ? '…' : 'Delete'}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

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
