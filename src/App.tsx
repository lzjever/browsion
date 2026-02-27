import { useState } from 'react';
import { ProfileList } from './components/ProfileList';
import { ProfileForm } from './components/ProfileForm';
import { Settings } from './components/Settings';
import { McpPage } from './components/McpPage';
import type { BrowserProfile } from './types/profile';
import './styles/index.css';

type View = 'profiles' | 'settings' | 'mcp';

function App() {
  const [currentView, setCurrentView] = useState<View>('profiles');
  const [showProfileForm, setShowProfileForm] = useState(false);
  const [editingProfile, setEditingProfile] = useState<BrowserProfile | undefined>();
  const [refreshKey, setRefreshKey] = useState(0);

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
            className={`nav-btn ${currentView === 'settings' ? 'active' : ''}`}
            onClick={() => setCurrentView('settings')}
          >
            Settings
          </button>
          <button
            className={`nav-btn ${currentView === 'mcp' ? 'active' : ''}`}
            onClick={() => setCurrentView('mcp')}
          >
            MCP
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
              refreshTrigger={refreshKey}
              onEditProfile={handleEditProfile}
              onCloneProfile={handleCloneProfile}
            />
          </div>
        )}

        {currentView === 'settings' && <Settings />}

        {currentView === 'mcp' && <McpPage />}
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
