import { useState, useEffect } from 'react';
import { ProfileList } from './components/ProfileList';
import { ProfileForm } from './components/ProfileForm';
import { Settings } from './components/Settings';
import { AgentPanel } from './components/AgentPanel';
import { SchedulePanel } from './components/SchedulePanel';
import { tauriApi } from './api/tauri';
import type { BrowserProfile } from './types/profile';
import './styles/index.css';

type View = 'profiles' | 'settings' | 'agent' | 'schedule';

function App() {
  const [currentView, setCurrentView] = useState<View>('profiles');
  const [showProfileForm, setShowProfileForm] = useState(false);
  const [editingProfile, setEditingProfile] = useState<BrowserProfile | undefined>();
  const [refreshKey, setRefreshKey] = useState(0);
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);

  // Load profiles for Agent panel
  useEffect(() => {
    tauriApi.getProfiles().then(setProfiles).catch(console.error);
  }, [refreshKey]);

  const handleAddProfile = () => {
    setEditingProfile(undefined);
    setShowProfileForm(true);
  };

  const handleEditProfile = (profile: BrowserProfile) => {
    setEditingProfile(profile);
    setShowProfileForm(true);
  };

  const handleCloneProfile = (profile: BrowserProfile) => {
    // Create a clone with new ID and modified name
    const clonedProfile: BrowserProfile = {
      ...profile,
      id: '', // Will be generated in ProfileForm
      name: `${profile.name} Copy`,
    };
    setEditingProfile(clonedProfile);
    setShowProfileForm(true);
  };

  const handleSaveProfile = () => {
    setShowProfileForm(false);
    setEditingProfile(undefined);
    setRefreshKey((prev) => prev + 1);
  };

  const handleCancelProfile = () => {
    setShowProfileForm(false);
    setEditingProfile(undefined);
  };

  return (
    <div className="app">
      <header className="app-header">
        <h1>Browsion</h1>
        <nav className="app-nav">
          <button
            className={`nav-btn ${currentView === 'profiles' ? 'active' : ''}`}
            onClick={() => setCurrentView('profiles')}
          >
            Profiles
          </button>
          <button
            className={`nav-btn ${currentView === 'agent' ? 'active' : ''}`}
            onClick={() => setCurrentView('agent')}
          >
            AI Agent
          </button>
          <button
            className={`nav-btn ${currentView === 'schedule' ? 'active' : ''}`}
            onClick={() => setCurrentView('schedule')}
          >
            Schedule
          </button>
          <button
            className={`nav-btn ${currentView === 'settings' ? 'active' : ''}`}
            onClick={() => setCurrentView('settings')}
          >
            Settings
          </button>
        </nav>
      </header>

      <main className="app-main">
        {currentView === 'profiles' && (
          <div className="profiles-view">
            <div className="profiles-header">
              <h2>Browser Profiles</h2>
              <button className="btn btn-primary" onClick={handleAddProfile}>
                + Add Profile
              </button>
            </div>
            <ProfileList
              key={refreshKey}
              onEditProfile={handleEditProfile}
              onCloneProfile={handleCloneProfile}
            />
          </div>
        )}

        {currentView === 'settings' && <Settings />}

        {currentView === 'agent' && <AgentPanel profiles={profiles} />}

        {currentView === 'schedule' && <SchedulePanel profiles={profiles} />}
      </main>

      {showProfileForm && (
        <ProfileForm
          profile={editingProfile}
          onSave={handleSaveProfile}
          onCancel={handleCancelProfile}
        />
      )}
    </div>
  );
}

export default App;
