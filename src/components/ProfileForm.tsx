import React, { useState, useEffect } from 'react';
import { tauriApi } from '../api/tauri';
import type { BrowserProfile, BrowserSource, ProxyPreset } from '../types/profile';
import { v4 as uuidv4 } from 'uuid';
import ISO6391 from 'iso-639-1';
import { LaunchArgsSelector, ARG_CATEGORIES } from './LaunchArgsSelector';

// Common timezones for fingerprint-chromium (--timezone). See https://github.com/adryfish/fingerprint-chromium
const TIMEZONES: { value: string; label: string }[] = [
  { value: '', label: '— Default / system —' },
  { value: 'UTC', label: 'UTC' },
  { value: 'America/New_York', label: 'America/New York' },
  { value: 'America/Los_Angeles', label: 'America/Los Angeles' },
  { value: 'America/Chicago', label: 'America/Chicago' },
  { value: 'America/Denver', label: 'America/Denver' },
  { value: 'Europe/London', label: 'Europe/London' },
  { value: 'Europe/Paris', label: 'Europe/Paris' },
  { value: 'Europe/Berlin', label: 'Europe/Berlin' },
  { value: 'Asia/Shanghai', label: 'Asia/Shanghai' },
  { value: 'Asia/Hong_Kong', label: 'Asia/Hong Kong' },
  { value: 'Asia/Tokyo', label: 'Asia/Tokyo' },
  { value: 'Asia/Singapore', label: 'Asia/Singapore' },
  { value: 'Australia/Sydney', label: 'Australia/Sydney' },
  { value: 'Asia/Kolkata', label: 'Asia/Kolkata' },
];

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
    color: '#3498DB',
    custom_args: [],
    tags: [],
  });
  const [tagsText, setTagsText] = useState('');
  const [customArgsText, setCustomArgsText] = useState('');
  const [browserSource, setBrowserSource] = useState<BrowserSource | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [proxyPresets, setProxyPresets] = useState<ProxyPreset[]>([]);

  const isFingerprintChromium =
    browserSource?.type === 'custom' && browserSource?.fingerprint_chromium;

  // Parse custom_args into preset args (matching our selector) and additional args
  const getPresetArgs = (): string[] => {
    return formData.custom_args.filter((arg) =>
      ARG_CATEGORIES.some((cat) => cat.args.some((a) => a.arg === arg))
    );
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
    tauriApi.getProxyPresets().then(setProxyPresets).catch(() => setProxyPresets([]));
    tauriApi.getBrowserSource().then(setBrowserSource).catch(() => setBrowserSource(null));
  }, []);

  useEffect(() => {
    if (profile) {
      const profileData = profile.id ? profile : { ...profile, id: uuidv4() };
      setFormData(profileData);
      setTagsText((profile.tags || []).join(', '));
      const presetSet = new Set(
        ARG_CATEGORIES.flatMap((cat) => cat.args.map((a) => a.arg))
      );
      const additional = (profileData.custom_args || []).filter(
        (arg) => !presetSet.has(arg)
      );
      setCustomArgsText(additional.join('\n'));
    } else {
      setFormData((prev) => ({ ...prev, id: uuidv4() }));
      setCustomArgsText('');
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

      const additionalArgs = customArgsText
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.length > 0);
      const profileData: BrowserProfile = {
        ...formData,
        proxy_server: formData.proxy_server || undefined,
        color: formData.color || undefined,
        tags,
        custom_args: [...getPresetArgs(), ...additionalArgs],
        timezone: formData.timezone || undefined,
        fingerprint: formData.fingerprint || undefined,
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
                    <label htmlFor="lang">
                      Language{isFingerprintChromium ? ' (--lang)' : ''}
                    </label>
                    {isFingerprintChromium ? (
                      <select
                        id="lang"
                        name="lang"
                        value={formData.lang}
                        onChange={(e) =>
                          setFormData((prev) => ({ ...prev, lang: e.target.value }))
                        }
                      >
                        {LANGUAGES.map((lang) => (
                          <option key={lang.code} value={lang.code}>
                            {lang.name}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <>
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
                      </>
                    )}
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

                {isFingerprintChromium && (
                  <div className="form-row">
                    <div className="form-group">
                      <label htmlFor="fingerprint">Fingerprint (--fingerprint)</label>
                      <input
                        type="text"
                        id="fingerprint"
                        name="fingerprint"
                        value={formData.fingerprint ?? ''}
                        onChange={(e) =>
                          setFormData((prev) => ({
                            ...prev,
                            fingerprint: e.target.value.trim() || undefined,
                          }))
                        }
                        placeholder="e.g. 1000 (32-bit integer seed)"
                      />
                    </div>
                    <div className="form-group">
                      <label htmlFor="timezone">Timezone (--timezone)</label>
                      <select
                        id="timezone"
                        name="timezone"
                        value={formData.timezone ?? ''}
                        onChange={(e) =>
                          setFormData((prev) => ({
                            ...prev,
                            timezone: e.target.value || undefined,
                          }))
                        }
                      >
                        {TIMEZONES.map((tz) => (
                          <option key={tz.value || 'default'} value={tz.value}>
                            {tz.label}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                )}

                <div className="form-group">
                  <label htmlFor="proxy_server">Proxy Server</label>
                  {proxyPresets.length > 0 && (
                    <select
                      className="proxy-preset-select"
                      value=""
                      onChange={(e) => {
                        if (e.target.value) {
                          setFormData((prev) => ({ ...prev, proxy_server: e.target.value }));
                        }
                      }}
                    >
                      <option value="">— Select preset —</option>
                      {proxyPresets.map((p) => (
                        <option key={p.id} value={p.url}>{p.name} ({p.url})</option>
                      ))}
                    </select>
                  )}
                  <input
                    type="text"
                    id="proxy_server"
                    name="proxy_server"
                    value={formData.proxy_server}
                    onChange={handleChange}
                    placeholder="http://192.168.0.220:8889"
                  />
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
                    value={customArgsText}
                    onChange={(e) => setCustomArgsText(e.target.value)}
                    placeholder="--custom-arg=value&#10;--another-flag"
                    rows={3}
                    className="custom-args-textarea"
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
