import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile } from '../types/profile';
import { v4 as uuidv4 } from 'uuid';

interface ProfileFormProps {
  profile?: BrowserProfile;
  onSave: () => void;
  onCancel: () => void;
}

export const ProfileForm: React.FC<ProfileFormProps> = ({
  profile,
  onSave,
  onCancel,
}) => {
  const [formData, setFormData] = useState<BrowserProfile>({
    id: '',
    name: '',
    description: '',
    user_data_dir: '',
    proxy_server: '',
    lang: 'en-US',
    timezone: '',
    fingerprint: '',
    color: '#3498DB',
    custom_args: [],
  });
  const [customArgsText, setCustomArgsText] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (profile) {
      // If ID is empty, this is a clone operation, generate new ID
      const profileData = profile.id ? profile : { ...profile, id: uuidv4() };
      setFormData(profileData);
      setCustomArgsText(profile.custom_args.join('\n'));
    } else {
      setFormData((prev) => ({ ...prev, id: uuidv4() }));
    }
  }, [profile]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      // Parse custom args
      const custom_args = customArgsText
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.length > 0);

      const profileData: BrowserProfile = {
        ...formData,
        proxy_server: formData.proxy_server || undefined,
        timezone: formData.timezone || undefined,
        fingerprint: formData.fingerprint || undefined,
        color: formData.color || undefined,
        custom_args,
      };

      // If original profile has no ID or is a clone, treat as new
      const isClone = profile && !profile.id;
      if (profile && profile.id && !isClone) {
        await tauriApi.updateProfile(profileData);
      } else {
        await tauriApi.addProfile(profileData);
      }

      onSave();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleChange = (
    e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>
  ) => {
    const { name, value } = e.target;
    setFormData((prev) => ({ ...prev, [name]: value }));
  };

  return (
    <div className="modal-overlay">
      <div className="modal-content">
        <h2>{profile ? 'Edit Profile' : 'Add Profile'}</h2>

        {error && <div className="error-message">{error}</div>}

        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label htmlFor="name">Name *</label>
            <input
              type="text"
              id="name"
              name="name"
              value={formData.name}
              onChange={handleChange}
              required
              placeholder="Profile 10000"
            />
          </div>

          <div className="form-group">
            <label htmlFor="description">Description</label>
            <input
              type="text"
              id="description"
              name="description"
              value={formData.description}
              onChange={handleChange}
              placeholder="US Proxy Profile"
            />
          </div>

          <div className="form-group">
            <label htmlFor="user_data_dir">User Data Directory *</label>
            <input
              type="text"
              id="user_data_dir"
              name="user_data_dir"
              value={formData.user_data_dir}
              onChange={handleChange}
              required
              placeholder="/home/percy/google_profile/10000"
            />
          </div>

          <div className="form-row">
            <div className="form-group">
              <label htmlFor="lang">Language</label>
              <input
                type="text"
                id="lang"
                name="lang"
                value={formData.lang}
                onChange={handleChange}
                placeholder="en-US"
              />
            </div>

            <div className="form-group">
              <label htmlFor="color">Color</label>
              <input
                type="color"
                id="color"
                name="color"
                value={formData.color}
                onChange={handleChange}
              />
            </div>
          </div>

          <div className="form-group">
            <label htmlFor="proxy_server">Proxy Server</label>
            <input
              type="text"
              id="proxy_server"
              name="proxy_server"
              value={formData.proxy_server}
              onChange={handleChange}
              placeholder="http://192.168.0.220:8889"
            />
          </div>

          <div className="form-row">
            <div className="form-group">
              <label htmlFor="timezone">Timezone</label>
              <input
                type="text"
                id="timezone"
                name="timezone"
                value={formData.timezone}
                onChange={handleChange}
                placeholder="America/Los_Angeles"
              />
            </div>

            <div className="form-group">
              <label htmlFor="fingerprint">Fingerprint</label>
              <input
                type="text"
                id="fingerprint"
                name="fingerprint"
                value={formData.fingerprint}
                onChange={handleChange}
                placeholder="10000"
              />
            </div>
          </div>

          <div className="form-group">
            <label htmlFor="custom_args">Custom Arguments (one per line)</label>
            <textarea
              id="custom_args"
              value={customArgsText}
              onChange={(e) => setCustomArgsText(e.target.value)}
              placeholder="--disable-gpu&#10;--no-sandbox"
              rows={4}
            />
          </div>

          <div className="form-actions">
            <button type="button" className="btn btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn btn-primary" disabled={loading}>
              {loading ? 'Saving...' : 'Save'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
