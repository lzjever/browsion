import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile } from '../types/profile';
import { v4 as uuidv4 } from 'uuid';
import ISO6391 from 'iso-639-1';
import { LaunchArgsSelector, ARG_CATEGORIES } from './LaunchArgsSelector';

// Get all IANA timezones from browser's Intl API
const getAllTimezones = (): string[] => {
  try {
    // Modern browsers support this (Chrome 99+, Firefox 93+, Safari 15.4+)
    // @ts-ignore
    return Intl.supportedValuesOf('timeZone') as string[];
  } catch {
    // Fallback: common timezones
    return [
      'UTC',
      'America/New_York', 'America/Chicago', 'America/Denver', 'America/Los_Angeles',
      'Europe/London', 'Europe/Paris', 'Europe/Berlin',
      'Asia/Shanghai', 'Asia/Tokyo', 'Asia/Seoul', 'Asia/Singapore',
      'Australia/Sydney',
    ];
  }
};

// Generate locale list from ISO-639-1 + common country codes
const getAllLocales = (): { code: string; name: string }[] => {
  // Common country codes for locale variants
  const countryMap: Record<string, string[]> = {
    'en': ['US', 'GB', 'AU', 'CA', 'NZ', 'IE', 'ZA'],
    'zh': ['CN', 'TW', 'HK', 'SG'],
    'es': ['ES', 'MX', 'AR', 'CO', 'CL'],
    'pt': ['PT', 'BR'],
    'fr': ['FR', 'CA', 'BE', 'CH'],
    'de': ['DE', 'AT', 'CH'],
    'ar': ['SA', 'AE', 'EG', 'MA'],
  };

  // Try to use Intl.DisplayNames for localized names
  let displayNames: Intl.DisplayNames | null = null;
  try {
    displayNames = new Intl.DisplayNames(['en'], { type: 'language' });
  } catch {
    // Not supported, will use ISO6391 names
  }

  const locales: { code: string; name: string }[] = [];

  // Add locale variants (e.g., en-US, zh-CN)
  for (const [lang, countries] of Object.entries(countryMap)) {
    for (const country of countries) {
      const locale = `${lang}-${country}`;
      let name: string;
      try {
        name = displayNames?.of(locale) || ISO6391.getName(lang) || locale;
      } catch {
        name = ISO6391.getName(lang) || locale;
      }
      locales.push({ code: locale, name });
    }
  }

  // Add base language codes (e.g., ja, ko, th)
  const baseLanguages = ['ja', 'ko', 'th', 'vi', 'id', 'ms', 'fil', 'hi', 'tr', 'nl',
    'pl', 'uk', 'cs', 'sv', 'da', 'no', 'fi', 'el', 'he', 'hu', 'ro', 'bg', 'hr',
    'sk', 'sl', 'et', 'lv', 'lt', 'mt', 'cy', 'eu', 'ca', 'gl', 'hy', 'ka',
    'bn', 'ta', 'te', 'ml', 'kn', 'mr', 'gu', 'pa', 'ur', 'fa', 'ps', 'ku'];

  for (const code of baseLanguages) {
    const name = displayNames?.of(code) || ISO6391.getName(code) || code;
    if (ISO6391.getName(code) || displayNames?.of(code)) {
      locales.push({ code, name });
    }
  }

  // Sort by name
  locales.sort((a, b) => a.name.localeCompare(b.name));
  return locales;
};

// Memoized lists
const TIMEZONES = getAllTimezones();
const LANGUAGES = getAllLocales();

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
    tags: [],
  });
  const [tagsText, setTagsText] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Parse custom_args into preset args (matching our selector) and additional args
  const getPresetArgs = (): string[] => {
    return formData.custom_args.filter((arg) =>
      ARG_CATEGORIES.some((cat) => cat.args.some((a) => a.arg === arg))
    );
  };

  const getAdditionalArgs = (): string => {
    const presetArgs = new Set(
      ARG_CATEGORIES.flatMap((cat) => cat.args.map((a) => a.arg))
    );
    return formData.custom_args
      .filter((arg) => !presetArgs.has(arg))
      .join('\n');
  };

  const handlePresetArgsChange = (selectedPresetArgs: string[]) => {
    const presetArgsSet = new Set(
      ARG_CATEGORIES.flatMap((cat) => cat.args.map((a) => a.arg))
    );
    const additionalArgs = formData.custom_args.filter(
      (arg) => !presetArgsSet.has(arg)
    );
    setFormData((prev) => ({
      ...prev,
      custom_args: [...selectedPresetArgs, ...additionalArgs],
    }));
  };

  useEffect(() => {
    if (profile) {
      // If ID is empty, this is a clone operation, generate new ID
      const profileData = profile.id ? profile : { ...profile, id: uuidv4() };
      setFormData(profileData);
      setTagsText((profile.tags || []).join(', '));
    } else {
      setFormData((prev) => ({ ...prev, id: uuidv4() }));
    }
  }, [profile]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      // Parse tags
      const tags = tagsText
        .split(/[,\s]+/)
        .map((t) => t.trim())
        .filter((t) => t.length > 0);

      const profileData: BrowserProfile = {
        ...formData,
        proxy_server: formData.proxy_server || undefined,
        timezone: formData.timezone || undefined,
        fingerprint: formData.fingerprint || undefined,
        color: formData.color || undefined,
        tags,
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
      <div className="modal-content modal-with-footer profile-form-wide">
        <div className="modal-header">
          <h2>{profile ? 'Edit Profile' : 'Add Profile'}</h2>
          {error && <div className="error-message">{error}</div>}
        </div>

        <div className="modal-body">
          <form onSubmit={handleSubmit} id="profile-form">
            <div className="profile-form-grid">
              {/* Left Column - Basic Info */}
              <div className="profile-form-column">
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

                <div className="form-group">
                  <label htmlFor="tags">Tags (comma or space separated)</label>
                  <input
                    type="text"
                    id="tags"
                    value={tagsText}
                    onChange={(e) => setTagsText(e.target.value)}
                    placeholder="work, us-proxy, testing"
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
                      list="lang-list"
                      placeholder="en-US"
                    />
                    <datalist id="lang-list">
                      {LANGUAGES.map((lang) => (
                        <option key={lang.code} value={lang.code}>
                          {lang.name}
                        </option>
                      ))}
                    </datalist>
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
                      value={formData.timezone || ''}
                      onChange={handleChange}
                      list="timezone-list"
                      placeholder="America/Los_Angeles"
                    />
                    <datalist id="timezone-list">
                      {TIMEZONES.map((tz) => (
                        <option key={tz} value={tz} />
                      ))}
                    </datalist>
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
              </div>

              {/* Right Column - Notes & Args */}
              <div className="profile-form-column">
                <div className="form-group">
                  <label htmlFor="description">Description / Notes</label>
                  <textarea
                    id="description"
                    name="description"
                    value={formData.description}
                    onChange={handleChange}
                    placeholder="Store JSON configs, account info, notes..."
                    rows={10}
                    className="description-textarea"
                  />
                </div>

                <div className="form-group">
                  <label>Launch Arguments Presets</label>
                  <LaunchArgsSelector
                    selectedArgs={getPresetArgs()}
                    onArgsChange={handlePresetArgsChange}
                  />
                </div>

                <div className="form-group">
                  <label htmlFor="custom_args">Custom Arguments (one per line)</label>
                  <textarea
                    id="custom_args"
                    value={getAdditionalArgs()}
                    onChange={(e) => {
                      const additionalArgs = e.target.value
                        .split('\n')
                        .map((line) => line.trim())
                        .filter((line) => line.length > 0);
                      const presetArgs = getPresetArgs();
                      setFormData((prev) => ({
                        ...prev,
                        custom_args: [...presetArgs, ...additionalArgs],
                      }));
                    }}
                    placeholder="--custom-arg=value&#10;--another-flag"
                    rows={3}
                  />
                </div>
              </div>
            </div>
          </form>
        </div>

        <div className="modal-footer">
          <button type="button" className="btn btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="submit"
            form="profile-form"
            className="btn btn-primary"
            disabled={loading}
          >
            {loading ? 'Saving...' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
};
