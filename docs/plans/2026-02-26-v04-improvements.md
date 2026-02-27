# Browsion V0.4 Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all critical bugs and implement high-priority UX improvements identified in code review.

**Architecture:** Incremental improvements to existing Tauri+React codebase. No new dependencies except a lightweight Toast system built in-house. All changes are backwards compatible.

**Tech Stack:** Rust (Tauri backend), React 18 + TypeScript (frontend), Vite, CSS custom properties.

---

## Phase 1 — P0 Bug Fixes (Quick, No New UI Components)

### Task 1: Fix India/Kolkata Timezone

**Files:**
- Modify: `src/components/ProfileForm.tsx:24`

**Step 1: Make the fix**
Change `'India/Kolkata'` → `'Asia/Kolkata'` in the TIMEZONES array.

```typescript
{ value: 'Asia/Kolkata', label: 'Asia/Kolkata' },
```

**Step 2: Run app and verify**
```bash
npm run tauri dev
```
Open Profile dialog → Timezone dropdown → confirm "Asia/Kolkata" appears correctly.

**Step 3: Commit**
```bash
git add src/components/ProfileForm.tsx
git commit -m "fix: correct India/Kolkata timezone to Asia/Kolkata (valid IANA)"
```

---

### Task 2: Fix CDP Port Wrap-Around Logic

**Files:**
- Modify: `src-tauri/src/process/port.rs`

**Step 1: Read the bug**
Current code stores `9223` but returns `9222` on wrap, causing the counter to skip. Fix: store `9222`, return the result of `fetch_add` (which already returned the pre-increment value).

**Step 2: Fix the implementation**
```rust
pub fn allocate_cdp_port() -> u16 {
    let port = CDP_PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    if port > 65500 {
        // Reset for next caller; this call returns 9222 via the fetch_add above
        // Actually we need to handle this correctly:
        // fetch_add returned the old value (>65500), so reset counter to 9223
        // so the next allocation starts at 9223. This call returns 9222 (the reset base).
        CDP_PORT_COUNTER.store(9223, Ordering::SeqCst);
        return 9222;
    }
    port
}
```

Wait — the existing logic IS correct in intent (returns 9222, next caller gets 9223). The issue is it resets to 9223 when it should reset to 9222 so that the next-next caller gets 9223. Actually the current behavior:
- fetch_add returns >65500, we return 9222 to this caller ✓
- Store 9223 so next caller gets 9223, then 9224...
- But 9222 is only ever used once (at startup), after wrap it jumps to 9223.

The real fix: reset to `9222` so the next caller gets `9222` again (fully correct wraparound):
```rust
pub fn allocate_cdp_port() -> u16 {
    let port = CDP_PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    if port > 65500 {
        CDP_PORT_COUNTER.store(9222, Ordering::SeqCst);
        return 9222;
    }
    port
}
```

**Step 3: Run existing test**
```bash
cd src-tauri && cargo test process::port --lib -- --nocapture
```
Expected: PASS (the existing test just checks incrementing works).

**Step 4: Commit**
```bash
git add src-tauri/src/process/port.rs
git commit -m "fix: correct CDP port wrap-around reset value from 9223 to 9222"
```

---

### Task 3: Use crypto.getRandomValues() for API Key Generation

**Files:**
- Modify: `src/components/Settings.tsx:272-279`

**Step 1: Replace Math.random() with crypto.getRandomValues()**
```typescript
const generateApiKey = () => {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  const array = new Uint8Array(32);
  crypto.getRandomValues(array);
  let key = 'sk-';
  for (let i = 0; i < 32; i++) {
    key += chars[array[i] % chars.length];
  }
  setMcpConfig({ ...mcpConfig, api_key: key });
};
```

**Step 2: Test**
In app Settings → MCP section → click "Generate" multiple times. Each key should be unique and start with "sk-".

**Step 3: Commit**
```bash
git add src/components/Settings.tsx
git commit -m "fix: use crypto.getRandomValues() for secure API key generation"
```

---

### Task 4: Remove Duplicate Args from LaunchArgsSelector

**Files:**
- Modify: `src/components/LaunchArgsSelector.tsx`

**Background:** `launcher.rs` already hardcodes these three args for every Chrome launch:
- `--disable-background-networking`
- `--disable-extensions`
- `--disable-blink-features=AutomationControlled`

Having them in the preset selector is misleading — users think they're toggling these, but they're always on.

**Step 1: Remove the three duplicate entries from ARG_CATEGORIES**

Remove from `Network` category:
- `--disable-background-networking`
- `--disable-extensions`

Remove from `Automation` category:
- `--disable-blink-features=AutomationControlled`

Also remove `--disable-infobars` (deprecated in modern Chrome, does nothing).

The cleaned `ARG_CATEGORIES` should be:
```typescript
export const ARG_CATEGORIES: ArgCategory[] = [
  {
    name: 'Performance',
    args: [
      { arg: '--disable-gpu', description: 'Disable GPU hardware acceleration' },
      { arg: '--disable-dev-shm-usage', description: 'Use /tmp instead of /dev/shm (Docker/CI)' },
      { arg: '--disable-software-rasterizer', description: 'Disable software rasterizer' },
    ],
  },
  {
    name: 'Security',
    args: [
      { arg: '--no-sandbox', description: 'Disable sandbox (Docker/CI required)' },
      { arg: '--disable-web-security', description: 'Disable same-origin policy (testing only)' },
      { arg: '--ignore-certificate-errors', description: 'Ignore SSL certificate errors' },
    ],
  },
  {
    name: 'Window',
    args: [
      { arg: '--start-maximized', description: 'Start browser maximized' },
      { arg: '--start-fullscreen', description: 'Start browser in fullscreen' },
      { arg: '--window-size=1920,1080', description: 'Set fixed window size (headless-like)' },
    ],
  },
  {
    name: 'Automation',
    args: [
      { arg: '--headless', description: 'Run in headless mode (no visible window)' },
      { arg: '--disable-images', description: 'Disable image loading (faster automation)' },
    ],
  },
];
```

**Step 2: Test**
Open any profile form → expand "Presets" → verify no duplicate/deprecated args appear, new useful args are there.

**Step 3: Commit**
```bash
git add src/components/LaunchArgsSelector.tsx
git commit -m "fix: remove duplicate hardcoded args from preset selector, add useful automation presets"
```

---

### Task 5: Remove Hardcoded --disable-extensions from launcher.rs

**Files:**
- Modify: `src-tauri/src/process/launcher.rs`

**Rationale:** `--disable-extensions` prevents users from using any Chrome extensions. Many legitimate use cases (ad blocking, password managers) require extensions. Remove it from the hardcoded defaults.

**Step 1: Remove the line**
```rust
// DELETE this line from build_command():
cmd.arg("--disable-extensions");      // Disable extensions for cleaner profile
```

**Step 2: Run Rust unit tests**
```bash
cd src-tauri && cargo test process::launcher --lib -- --nocapture
```
Update test to NOT assert `--disable-extensions` is present (remove that check if it exists).

**Step 3: Manual test**
Launch a profile. Verify Chrome opens normally. Verify extensions work (if any installed in that profile dir).

**Step 4: Commit**
```bash
git add src-tauri/src/process/launcher.rs
git commit -m "fix: remove hardcoded --disable-extensions to allow user extensions"
```

---

### Task 6: Fix TagFilter State Lost on Profile Refresh

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/ProfileList.tsx`

**Root Cause:** `App.tsx` uses `key={refreshKey}` on `ProfileList`. Changing `key` causes React to unmount+remount the component, destroying local state including the tag filter input.

**Fix:** Move profile data loading into `ProfileList` but remove the `key` prop trick. Instead, expose a `reload()` imperative handle OR pass a `refreshTrigger` prop that triggers a data reload without remounting.

**Step 1: Add `refreshTrigger` prop to ProfileList**
```typescript
// In ProfileList.tsx, add to props interface:
interface ProfileListProps {
  onEditProfile: (profile: BrowserProfile) => void;
  onCloneProfile: (profile: BrowserProfile) => void;
  refreshTrigger: number; // increment to trigger data reload
}

// Replace the existing useEffect that depends on []:
useEffect(() => {
  loadProfiles();
}, [refreshTrigger]); // reload when parent signals refresh
```

**Step 2: Remove the status polling from useEffect and keep it separate**
The polling interval stays in its own separate `useEffect` with `[]` dep — unchanged.

**Step 3: Update App.tsx to remove `key={refreshKey}` and pass prop instead**
```typescript
// Before:
<ProfileList
  key={refreshKey}
  onEditProfile={handleEditProfile}
  onCloneProfile={handleCloneProfile}
/>

// After:
<ProfileList
  refreshTrigger={refreshKey}
  onEditProfile={handleEditProfile}
  onCloneProfile={handleCloneProfile}
/>
```

**Step 4: Test**
1. Type "work" in the tag filter
2. Add a new profile (save)
3. Verify the filter input still shows "work" and is not cleared

**Step 5: Commit**
```bash
git add src/App.tsx src/components/ProfileList.tsx
git commit -m "fix: preserve tag filter state when profiles are refreshed"
```

---

### Task 7: Fix Recent Profiles Persistence Across Restarts

**Files:**
- Modify: `src-tauri/src/process/manager.rs`

**Root Cause:** `ProcessManager.recent_launches` is always initialized as empty Vec. On restart, recent profiles shown in tray come from `config.recent_profiles` (correct) but the `get_recent_profiles` Tauri command reads from `process_manager.recent_launches` (always empty after restart).

**Fix:** Initialize `recent_launches` from config at startup.

**Step 1: Modify ProcessManager to accept initial recent launches**
```rust
impl ProcessManager {
    pub fn new_with_recent(recent: Vec<String>) -> Self {
        Self {
            active_processes: Arc::new(Mutex::new(HashMap::new())),
            system: Arc::new(Mutex::new(System::new_all())),
            recent_launches: Arc::new(Mutex::new(recent)),
        }
    }

    pub fn new() -> Self {
        Self::new_with_recent(Vec::new())
    }
}
```

**Step 2: Modify AppState initialization in lib.rs**
```rust
// In lib.rs where AppState is created:
let process_manager = ProcessManager::new_with_recent(config.recent_profiles.clone());
```

**Step 3: Test**
1. Launch 2-3 profiles
2. Quit and reopen app
3. Right-click tray → Recent Profiles should still show those profiles
4. Call `get_recent_profiles` Tauri command (via Settings or debug) — should return same profiles

**Step 4: Commit**
```bash
git add src-tauri/src/process/manager.rs src-tauri/src/lib.rs
git commit -m "fix: initialize ProcessManager recent_launches from persisted config on startup"
```

---

## Phase 2 — P1 UX Improvements

### Task 8: Toast Notification System (Replace alert/confirm)

**Files:**
- Create: `src/components/Toast.tsx`
- Create: `src/components/ConfirmDialog.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles/index.css`

**Goal:** Replace all `alert()` and `confirm()` calls with styled, non-blocking alternatives.

**Step 1: Create Toast context and component**
```typescript
// src/components/Toast.tsx
import React, { createContext, useContext, useState, useCallback } from 'react';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

interface Toast {
  id: number;
  message: string;
  type: ToastType;
}

interface ToastContextValue {
  showToast: (message: string, type?: ToastType) => void;
}

const ToastContext = createContext<ToastContextValue>({ showToast: () => {} });

export const useToast = () => useContext(ToastContext);

let nextId = 0;

export const ToastProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const showToast = useCallback((message: string, type: ToastType = 'info') => {
    const id = ++nextId;
    setToasts(prev => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 4000);
  }, []);

  return (
    <ToastContext.Provider value={{ showToast }}>
      {children}
      <div className="toast-container">
        {toasts.map(toast => (
          <div key={toast.id} className={`toast toast-${toast.type}`}>
            <span className="toast-message">{toast.message}</span>
            <button className="toast-close" onClick={() => setToasts(prev => prev.filter(t => t.id !== toast.id))}>×</button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
};
```

**Step 2: Create ConfirmDialog component**
```typescript
// src/components/ConfirmDialog.tsx
import React from 'react';

interface ConfirmDialogProps {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
  confirmLabel?: string;
  confirmClassName?: string;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  message,
  onConfirm,
  onCancel,
  confirmLabel = 'Confirm',
  confirmClassName = 'btn btn-danger',
}) => (
  <div className="modal-overlay">
    <div className="confirm-dialog">
      <p className="confirm-message">{message}</p>
      <div className="confirm-actions">
        <button className="btn btn-secondary" onClick={onCancel}>Cancel</button>
        <button className={confirmClassName} onClick={onConfirm}>{confirmLabel}</button>
      </div>
    </div>
  </div>
);
```

**Step 3: Add CSS for Toast and ConfirmDialog**
Add to `src/styles/index.css`:
```css
/* Toast */
.toast-container {
  position: fixed;
  bottom: 1.5rem;
  right: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  z-index: 1000;
  max-width: 360px;
}

.toast {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  border-radius: 6px;
  color: white;
  font-size: 0.875rem;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  animation: toast-in 0.2s ease;
}

@keyframes toast-in {
  from { opacity: 0; transform: translateX(100%); }
  to { opacity: 1; transform: translateX(0); }
}

.toast-success { background: var(--success-color); }
.toast-error { background: var(--danger-color); }
.toast-info { background: var(--primary-color); }
.toast-warning { background: var(--warning-color); }

.toast-message { flex: 1; }
.toast-close {
  background: none;
  border: none;
  color: white;
  cursor: pointer;
  font-size: 1.2rem;
  line-height: 1;
  padding: 0;
  opacity: 0.8;
}
.toast-close:hover { opacity: 1; }

/* ConfirmDialog */
.confirm-dialog {
  background: white;
  border-radius: 8px;
  padding: 1.5rem;
  max-width: 400px;
  width: 90%;
  box-shadow: 0 8px 32px rgba(0,0,0,0.2);
}
.confirm-message {
  margin-bottom: 1.5rem;
  font-size: 1rem;
  line-height: 1.5;
  color: var(--text-color);
}
.confirm-actions {
  display: flex;
  gap: 0.75rem;
  justify-content: flex-end;
}
```

**Step 4: Wrap App in ToastProvider**
```typescript
// src/main.tsx or App.tsx — wrap root in ToastProvider
import { ToastProvider } from './components/Toast';
// ...
<ToastProvider>
  <App />
</ToastProvider>
```

**Step 5: Update ProfileList.tsx to use Toast and ConfirmDialog**

Replace all `alert(...)` with `showToast(...)` and all `confirm(...)` with `ConfirmDialog`.

Key changes needed:
- Add `const { showToast } = useToast();` at top of component
- Add `confirmState` state for pending confirmations: `{ message, onConfirm } | null`
- Replace `alert(`Failed to launch profile: ${err}`)` → `showToast(`Failed to launch: ${err}`, 'error')`
- Replace `if (!confirm(...))` → show ConfirmDialog, move the delete logic into the onConfirm callback

```typescript
// Add state for confirm dialog:
const [confirmState, setConfirmState] = useState<{
  message: string;
  onConfirm: () => void;
  confirmLabel: string;
  confirmClassName: string;
} | null>(null);

// Replace handleDelete:
const handleDelete = (id: string) => {
  setConfirmState({
    message: 'Are you sure you want to delete this profile?',
    confirmLabel: 'Delete',
    confirmClassName: 'btn btn-danger',
    onConfirm: async () => {
      setConfirmState(null);
      try {
        await tauriApi.deleteProfile(id);
        await loadProfiles();
        showToast('Profile deleted', 'success');
      } catch (err) {
        showToast(`Failed to delete profile: ${err}`, 'error');
      }
    },
  });
};

// Add Kill confirm:
const handleKill = (id: string) => {
  setConfirmState({
    message: 'Kill this browser? All unsaved data will be lost.',
    confirmLabel: 'Kill',
    confirmClassName: 'btn btn-danger',
    onConfirm: async () => {
      setConfirmState(null);
      try {
        await tauriApi.killProfile(id);
        const status = await tauriApi.getRunningProfiles();
        setRunningStatus(status);
        showToast('Browser stopped', 'success');
      } catch (err) {
        showToast(`Failed to kill: ${err}`, 'error');
      }
    },
  });
};

// In JSX, render confirm dialog when state is set:
{confirmState && (
  <ConfirmDialog
    message={confirmState.message}
    confirmLabel={confirmState.confirmLabel}
    confirmClassName={confirmState.confirmClassName}
    onConfirm={confirmState.onConfirm}
    onCancel={() => setConfirmState(null)}
  />
)}
```

**Step 6: Test**
- Try deleting a profile → styled confirm dialog appears
- Confirm delete → toast "Profile deleted" appears
- Try launching a non-existent profile → styled error toast appears
- Kill a running browser → confirm dialog appears

**Step 7: Commit**
```bash
git add src/components/Toast.tsx src/components/ConfirmDialog.tsx src/components/ProfileList.tsx src/styles/index.css src/main.tsx
git commit -m "feat: add Toast notification system and ConfirmDialog, replace all alert/confirm"
```

---

### Task 9: Launch Button Loading State

**Files:**
- Modify: `src/components/ProfileList.tsx`
- Modify: `src/components/ProfileItem.tsx`

**Goal:** When user clicks Launch, disable the button and show spinner until Chrome starts or error occurs.

**Step 1: Add launching state to ProfileList**
```typescript
// In ProfileList, add:
const [launchingId, setLaunchingId] = useState<string | null>(null);

// Update handleLaunch:
const handleLaunch = async (id: string) => {
  setLaunchingId(id);
  try {
    await tauriApi.launchProfile(id);
    const status = await tauriApi.getRunningProfiles();
    setRunningStatus(status);
    showToast('Browser launched', 'success');
  } catch (err) {
    showToast(`Failed to launch: ${err}`, 'error');
  } finally {
    setLaunchingId(null);
  }
};
```

**Step 2: Pass launchingId to ProfileItem**
```typescript
// ProfileItem props:
interface ProfileItemProps {
  // ...existing...
  isLaunching?: boolean;
}

// In the Launch button:
{!isRunning ? (
  <button
    className="btn btn-primary"
    onClick={() => onLaunch(profile.id)}
    disabled={isLaunching}
  >
    {isLaunching ? 'Launching…' : 'Launch'}
  </button>
) : ...}
```

**Step 3: Test**
Click Launch on a profile. Button should show "Launching…" and be disabled. After Chrome opens, button disappears (replaced by Activate/Kill).

**Step 4: Commit**
```bash
git add src/components/ProfileList.tsx src/components/ProfileItem.tsx
git commit -m "feat: add loading state to Launch button during browser startup"
```

---

### Task 10: Search by Name + Tags

**Files:**
- Modify: `src/components/ProfileList.tsx`

**Step 1: Update filter logic to include profile name**
```typescript
// Replace the existing filteredProfiles logic:
const filteredProfiles = profiles.filter((profile) => {
  if (!tagFilter.trim()) return true;
  const keywords = tagFilter.trim().toLowerCase().split(/\s+/);
  return keywords.some((kw) =>
    profile.name.toLowerCase().includes(kw) ||
    (profile.tags || []).some((tag) => tag.toLowerCase().includes(kw))
  );
});
```

**Step 2: Update placeholder text**
```typescript
placeholder="Search by name or tags..."
```

**Step 3: Test**
- Type a profile name → should filter to that profile
- Type a tag → should filter by tag
- Type partial name → should match

**Step 4: Commit**
```bash
git add src/components/ProfileList.tsx
git commit -m "feat: extend profile search to match name and tags"
```

---

### Task 11: Profile Card Info Enhancement

**Files:**
- Modify: `src/components/ProfileItem.tsx`

**Goal:** Show timezone and the last segment of data directory path without cluttering the card.

**Step 1: Update ProfileItem to show more info**
```typescript
// After the Lang detail, add timezone and data dir basename:
<span className="detail">Lang: {profile.lang}</span>
{profile.timezone && <span className="detail">TZ: {profile.timezone}</span>}
<span className="detail detail-muted" title={profile.user_data_dir}>
  Dir: …/{profile.user_data_dir.split('/').filter(Boolean).pop() || profile.user_data_dir}
</span>
```

**Step 2: Add CSS for muted detail**
```css
.detail-muted {
  color: var(--text-light);
  font-size: 0.75rem;
}
```

**Step 3: Test**
View profiles in the list. Profiles with timezone should show it. All should show truncated data dir.

**Step 4: Commit**
```bash
git add src/components/ProfileItem.tsx src/styles/index.css
git commit -m "feat: show timezone and data directory basename in profile card"
```

---

## Phase 3 — Background Dead Process Cleanup

### Task 12: Auto-cleanup Dead Processes

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Goal:** Every 30 seconds, clean up processes that have exited naturally (Chrome closed by user).

**Step 1: Add background cleanup task in lib.rs**

After the Tauri app is set up, before `run()`, spawn a background tokio task:

```rust
// In the app setup, after state is created:
{
    let state_clone = Arc::clone(&app_state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            match state_clone.process_manager.cleanup_dead_processes().await {
                Ok(removed) if !removed.is_empty() => {
                    tracing::info!("Cleaned up dead processes: {:?}", removed);
                    // Also remove their CDP sessions
                    for profile_id in &removed {
                        state_clone.session_manager.remove_session(profile_id).await;
                    }
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("Failed to cleanup dead processes: {}", e),
            }
        }
    });
}
```

**Step 2: Check session_manager has remove_session method**
Look at `src-tauri/src/agent/session.rs`. If `remove_session` doesn't exist, add it or use the existing cleanup method.

**Step 3: Test**
1. Launch a profile
2. Manually close the Chrome window
3. Wait 30 seconds
4. Check that the profile now shows "Stopped" in the UI (next poll cycle will confirm)

**Step 4: Commit**
```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add background task to auto-cleanup dead browser processes every 30s"
```

---

## Testing Checklist (run after all tasks complete)

- [ ] All Rust unit tests pass: `cd src-tauri && cargo test`
- [ ] App launches without errors: `npm run tauri dev`
- [ ] Add a profile → saved correctly
- [ ] Edit a profile → tag filter preserved after save
- [ ] Delete a profile → confirm dialog shown → toast on success
- [ ] Launch profile → "Launching…" shown → toast on success → status shows Running
- [ ] Kill profile → confirm dialog shown → toast on success → status shows Stopped
- [ ] Search by name works
- [ ] Search by tag works
- [ ] Timezone shows Asia/Kolkata (not India/Kolkata)
- [ ] API Key "Generate" button produces cryptographically random keys
- [ ] LaunchArgsSelector shows no duplicate/deprecated args
- [ ] Extensions work in launched Chrome profiles (--disable-extensions removed)
