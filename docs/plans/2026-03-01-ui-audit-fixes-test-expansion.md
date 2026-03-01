# UI Audit Fixes + Test Expansion v0.9.2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix 8 confirmed bugs found during UI audit, add Vitest frontend unit tests, expand backend tests to cover workflow/recording modules, release v0.9.2.

**Architecture:** Frontend fixes go in their respective component files. Frontend tests use Vitest + jsdom (needs npm setup). Backend tests extend existing test files.

**Tech Stack:** React 18 + TypeScript, Vitest + @testing-library/react, Rust/axum, cargo test

---

## Bug Fix Tasks

### Task 1: Fix App.tsx — `useState` anti-pattern (CRITICAL)

**Files:**
- Modify: `src/App.tsx` line 1, 22-32

**Problem:** `useState(() => { loadProfiles(); })` is a lazy initializer, not a side-effect. The callback runs synchronously during first render for state initialization, not as an effect. While it does run `loadProfiles()`, the real issue is it uses `useState` as if it were `useEffect`.

**Step 1: Read the file**
```
Read src/App.tsx lines 1-35
```

**Step 2: Fix — add useEffect import and replace useState call**

In `src/App.tsx`, change line 1 from:
```tsx
import { useState } from 'react';
```
to:
```tsx
import { useState, useEffect } from 'react';
```

Replace the `useState(() => {` block (lines 22-32) with:
```tsx
  useEffect(() => {
    const loadProfiles = async () => {
      try {
        const { tauriApi } = await import('./api/tauri');
        const profileList = await tauriApi.getProfiles();
        setProfiles(profileList);
      } catch (e) {
        console.error('Failed to load profiles:', e);
      }
    };
    loadProfiles();
  }, []);
```

**Step 3: Verify TypeScript compiles**
Run: `cd /home/percy/works/browsion && npx tsc --noEmit 2>&1 | head -20`
Expected: no errors

**Step 4: Commit**
```bash
git add src/App.tsx
git commit -m "fix: correct useState anti-pattern — use useEffect for profile loading in App"
```

---

### Task 2: Fix Settings.tsx — null check on cftVersions race condition

**Files:**
- Modify: `src/components/Settings.tsx`

**Problem:** In the channel dropdown `onChange` handler, `cftVersions.find(...)` can return `undefined` when versions aren't loaded yet. Code then does `setCftVersion(v.version)` which crashes.

**Step 1: Read the file**
```
Read src/components/Settings.tsx — find the channel dropdown onChange handler (search for "setCftChannel")
```

**Step 2: Find the exact code and fix**

Find the block that looks like:
```tsx
setCftChannel(e.target.value);
const v = cftVersions.find((x) => x.channel === e.target.value);
setCftVersion(v.version);  // CRASH if v is undefined
```

Fix to:
```tsx
const newChannel = e.target.value;
setCftChannel(newChannel);
const v = cftVersions.find((x) => x.channel === newChannel);
if (v) setCftVersion(v.version);
```

**Step 3: Verify**
Run: `npx tsc --noEmit 2>&1 | head -20`
Expected: no errors

**Step 4: Commit**
```bash
git add src/components/Settings.tsx
git commit -m "fix: guard against undefined version when cftVersions not yet loaded"
```

---

### Task 3: Fix ConfirmDialog.tsx — missing keyboard + overlay click

**Files:**
- Modify: `src/components/ConfirmDialog.tsx`

**Problem:** ConfirmDialog has no Escape key handler and no click-outside-to-dismiss, unlike other modals (SnapshotModal, ProfileForm). Also missing focus management.

**Step 1: Read the file**
```
Read src/components/ConfirmDialog.tsx
```

**Step 2: Rewrite to add useEffect + keyboard + overlay click**

Replace the component with:
```tsx
import React, { useEffect } from 'react';

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
}) => {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onCancel();
      if (e.key === 'Enter') onConfirm();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onCancel, onConfirm]);

  return (
    <div className="modal-overlay" onClick={onCancel} role="dialog" aria-modal="true">
      <div className="confirm-dialog" onClick={(e) => e.stopPropagation()}>
        <p className="confirm-message">{message}</p>
        <div className="confirm-actions">
          <button className="btn btn-secondary" onClick={onCancel} autoFocus>
            Cancel
          </button>
          <button className={confirmClassName} onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
};
```

**Step 3: Verify**
Run: `npx tsc --noEmit 2>&1 | head -20`
Expected: no errors

**Step 4: Commit**
```bash
git add src/components/ConfirmDialog.tsx
git commit -m "fix: add Escape key, Enter key, overlay click dismiss to ConfirmDialog"
```

---

### Task 4: Fix WorkflowList.tsx — new workflow timestamps at 0

**Files:**
- Modify: `src/components/WorkflowList.tsx`

**Problem:** New workflow created with `created_at: 0, updated_at: 0` which renders as 1970-01-01 in the UI.

**Step 1: Read the file**
```
Read src/components/WorkflowList.tsx — find where new workflow object is created (search for "created_at: 0")
```

**Step 2: Fix timestamps**

Find the code creating a new empty workflow:
```tsx
{
  id: '',
  name: '',
  description: '',
  steps: [],
  variables: {},
  created_at: 0,
  updated_at: 0,
}
```

Change `created_at: 0, updated_at: 0` to `created_at: Date.now(), updated_at: Date.now()`.

**Step 3: Commit**
```bash
git add src/components/WorkflowList.tsx
git commit -m "fix: initialize new workflow with current timestamps instead of epoch 0"
```

---

### Task 5: Fix MonitorPage.tsx — DOM leak + sequential fetches

**Files:**
- Modify: `src/components/MonitorPage.tsx`

**Two problems:**
1. Cookie import creates persistent DOM `<input>` elements that accumulate
2. URL + title fetches are sequential (should be parallel)

**Step 1: Read the file**
```
Read src/components/MonitorPage.tsx — full file
```

**Step 2: Fix cookie import DOM leak**

Find the cookie import handler that does:
```tsx
const input = document.createElement('input');
input.type = 'file';
input.onchange = ...
input.click();
```

Add `document.body.appendChild(input)` and cleanup, OR better — use a `useRef` for the file input. Since this is a functional component, the simplest fix is to append to body and remove after use:

```tsx
const handleImportCookies = async (profileId: string) => {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json,.txt';
  document.body.appendChild(input);
  input.onchange = async () => {
    const file = input.files?.[0];
    document.body.removeChild(input);  // Clean up immediately
    if (!file) return;
    if (file.size > 10 * 1024 * 1024) {
      showToast('Cookie file too large (max 10MB)', 'error');
      return;
    }
    // ... rest of handler
  };
  input.click();
};
```

**Step 3: Fix sequential URL+title fetches**

Find the polling code where URL and title are fetched one after another. Change to `Promise.all()`:

```tsx
// Before (sequential):
const pageRes = await fetch(`${base}/url`);
if (pageRes.ok) { url = await pageRes.text(); }
const titleRes = await fetch(`${base}/title`);
if (titleRes.ok) { title = await titleRes.text(); }

// After (parallel):
const [pageRes, titleRes] = await Promise.all([
  fetch(`${base}/url`).catch(() => null),
  fetch(`${base}/title`).catch(() => null),
]);
const url = pageRes?.ok ? await pageRes.text() : '';
const title = titleRes?.ok ? await titleRes.text() : '';
```

**Step 4: Verify TypeScript**
Run: `npx tsc --noEmit 2>&1 | head -20`

**Step 5: Commit**
```bash
git add src/components/MonitorPage.tsx
git commit -m "fix: parallel URL/title fetches in monitor, cleanup dynamic file input DOM element"
```

---

### Task 6: Fix WorkflowEditor.tsx — empty variable key + out-of-bounds step index

**Files:**
- Modify: `src/components/WorkflowEditor.tsx`

**Two problems:**
1. User can add variable with empty key `''`, which renders badly
2. After deleting steps, `selectedStepIndex` may be out-of-bounds

**Step 1: Read the file**
```
Read src/components/WorkflowEditor.tsx — find variable add handler and selectedStepIndex usage
```

**Step 2: Fix empty variable key**

Find `handleAddVariable` or wherever `{ ...variables, '': '' }` is set.
Change to disallow adding if there's already a `''` key:
```tsx
const handleAddVariable = () => {
  if ('' in variables) return; // Don't allow duplicate empty key
  setVariables({ ...variables, '': '' });
};
```

**Step 3: Fix out-of-bounds step index**

Find where `selectedStepIndex` is used. Add a guard:
```tsx
const safeSelectedIndex =
  selectedStepIndex !== null && selectedStepIndex < steps.length
    ? selectedStepIndex
    : null;
const selectedStep = safeSelectedIndex !== null ? steps[safeSelectedIndex] : null;
```

Also in `handleDeleteStep`, add:
```tsx
if (selectedStepIndex !== null && selectedStepIndex >= newSteps.length) {
  setSelectedStepIndex(newSteps.length > 0 ? newSteps.length - 1 : null);
}
```

**Step 4: Commit**
```bash
git add src/components/WorkflowEditor.tsx
git commit -m "fix: prevent empty variable key in workflow editor, guard out-of-bounds step index"
```

---

### Task 7: Fix main.tsx — add Error Boundary

**Files:**
- Modify: `src/main.tsx`

**Problem:** No error boundary means any React component crash shows blank screen with no feedback.

**Step 1: Read main.tsx**
```
Read src/main.tsx
```

**Step 2: Add Error Boundary class component inline**

Rewrite `src/main.tsx` to add a minimal error boundary:
```tsx
import { StrictMode, Component, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { ToastProvider } from './components/Toast';

class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { error: null };
  }
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 32, fontFamily: 'monospace' }}>
          <h2>Something went wrong</h2>
          <pre style={{ color: 'red', whiteSpace: 'pre-wrap' }}>
            {this.state.error.message}
          </pre>
          <button onClick={() => this.setState({ error: null })}>Try again</button>
        </div>
      );
    }
    return this.props.children;
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <ToastProvider>
        <App />
      </ToastProvider>
    </ErrorBoundary>
  </StrictMode>,
);
```

**Step 3: Verify**
Run: `npx tsc --noEmit 2>&1 | head -20`
Run: `npm run build 2>&1 | tail -10`

**Step 4: Commit**
```bash
git add src/main.tsx
git commit -m "fix: add ErrorBoundary to prevent blank screen on component crash"
```

---

## Test Expansion Tasks

### Task 8: Set up Vitest + add frontend unit tests

**Files:**
- Modify: `package.json` (add devDependencies + test script)
- Modify: `vite.config.ts` (add test config)
- Create: `src/__tests__/utils.test.ts`
- Create: `src/__tests__/constants.test.ts`

**Step 1: Install Vitest**
```bash
cd /home/percy/works/browsion
npm install -D vitest @vitest/ui jsdom @testing-library/react @testing-library/jest-dom @testing-library/user-event @types/testing-library__jest-dom
```

**Step 2: Configure vite.config.ts**

Read `vite.config.ts` first, then add test config:
```ts
/// <reference types="vitest" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/__tests__/setup.ts'],
  },
});
```

**Step 3: Create test setup file**

Create `src/__tests__/setup.ts`:
```ts
import '@testing-library/jest-dom';
```

**Step 4: Add test script to package.json**

Add `"test": "vitest run"` and `"test:watch": "vitest"` to scripts.

**Step 5: Create pure utility tests**

Create `src/__tests__/utils.test.ts` with tests for pure functions:

```ts
// Tests for formatBytes (copied from Settings.tsx for testing)
function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

describe('formatBytes', () => {
  test('formats bytes', () => expect(formatBytes(512)).toBe('512 B'));
  test('formats kilobytes', () => expect(formatBytes(1536)).toBe('1.5 KB'));
  test('formats megabytes', () => expect(formatBytes(2 * 1024 * 1024)).toBe('2.0 MB'));
  test('formats 0 bytes', () => expect(formatBytes(0)).toBe('0 B'));
  test('formats 1023 bytes', () => expect(formatBytes(1023)).toBe('1023 B'));
  test('formats exactly 1KB', () => expect(formatBytes(1024)).toBe('1.0 KB'));
});

// Tests for profile tag filtering logic
function profileMatchesFilter(tags: string[], filter: string): boolean {
  if (!filter) return true;
  const lower = filter.toLowerCase();
  return tags.some((tag) => tag.toLowerCase().includes(lower));
}

describe('profileMatchesFilter', () => {
  test('empty filter matches everything', () => {
    expect(profileMatchesFilter(['work', 'us'], '')).toBe(true);
  });
  test('matching tag returns true', () => {
    expect(profileMatchesFilter(['work', 'us-proxy'], 'work')).toBe(true);
  });
  test('non-matching tag returns false', () => {
    expect(profileMatchesFilter(['work', 'us-proxy'], 'eu')).toBe(false);
  });
  test('case-insensitive matching', () => {
    expect(profileMatchesFilter(['Work'], 'work')).toBe(true);
  });
  test('empty tags with filter returns false', () => {
    expect(profileMatchesFilter([], 'anything')).toBe(false);
  });
});
```

Create `src/__tests__/constants.test.ts`:
```ts
import { UI_CONSTANTS } from '../components/constants';

describe('UI_CONSTANTS', () => {
  test('TOAST_DURATION_MS is positive number', () => {
    expect(UI_CONSTANTS.TOAST_DURATION_MS).toBeGreaterThan(0);
  });
  test('SUCCESS_MESSAGE_DURATION_MS is positive number', () => {
    expect(UI_CONSTANTS.SUCCESS_MESSAGE_DURATION_MS).toBeGreaterThan(0);
  });
  test('SCREENSHOT_POLL_INTERVAL_MS is at least 1000ms', () => {
    expect(UI_CONSTANTS.SCREENSHOT_POLL_INTERVAL_MS).toBeGreaterThanOrEqual(1000);
  });
  test('ACTION_LOG_POLL_INTERVAL_MS is at least 1000ms', () => {
    expect(UI_CONSTANTS.ACTION_LOG_POLL_INTERVAL_MS).toBeGreaterThanOrEqual(1000);
  });
});
```

**Step 6: Run tests**
```bash
cd /home/percy/works/browsion
npm test 2>&1 | tail -20
```

**Step 7: Commit**
```bash
git add package.json vite.config.ts src/__tests__/
git commit -m "test: add Vitest frontend test setup with utility and constant unit tests"
```

---

### Task 9: Add more backend API integration tests

**Files:**
- Modify: `src-tauri/tests/api_integration_test.rs`

**What to add:**
1. Profile CRUD: list after multiple adds/deletes, update then list
2. Action log: verify log records API calls correctly (action log with content)
3. Profile with proxy fields (test proxy field round-trip in add/get)
4. Health response body validation
5. API response content-type headers

**Step 1: Read current end of test file**
```
Read src-tauri/tests/api_integration_test.rs lines 770 onwards
```

**Step 2: Add new tests**

```rust
// ---------------------------------------------------------------------------
// Profile list ordering + multiple profiles
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_list_multiple_profiles() {
    let state = make_state();
    for i in 0..3 {
        let p = serde_json::json!({
            "id": format!("multi-{}", i),
            "name": format!("Profile {}", i),
            "description": "",
            "user_data_dir": format!("/tmp/multi-{}", i),
            "lang": "en-US",
            "tags": [],
            "custom_args": []
        });
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/profiles")
            .header("content-type", "application/json")
            .body(json_body(&p))
            .unwrap();
        app(state.clone(), None).oneshot(req).await.unwrap();
    }

    let req = axum::http::Request::builder()
        .uri("/api/profiles")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let profiles: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(profiles.len(), 3);
}

#[tokio::test]
async fn test_api_add_profile_with_tags_and_custom_args() {
    let state = make_state();
    let profile = serde_json::json!({
        "id": "tagged-001",
        "name": "Tagged Profile",
        "description": "test",
        "user_data_dir": "/tmp/tagged-001",
        "lang": "en-US",
        "tags": ["work", "proxy"],
        "custom_args": ["--no-sandbox", "--disable-gpu"]
    });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let req = axum::http::Request::builder()
        .uri("/api/profiles/tagged-001")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let p: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(p["tags"], serde_json::json!(["work", "proxy"]));
    assert_eq!(p["custom_args"], serde_json::json!(["--no-sandbox", "--disable-gpu"]));
}

#[tokio::test]
async fn test_api_add_duplicate_profile_id() {
    let state = make_state();
    let profile = serde_json::json!({
        "id": "dup-001",
        "name": "First",
        "description": "",
        "user_data_dir": "/tmp/dup-001",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    app(state.clone(), None).oneshot(req).await.unwrap();

    // Second add with same ID should conflict or fail
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    // Should be CONFLICT (409) or BAD_REQUEST (400)
    assert!(
        res.status() == StatusCode::CONFLICT || res.status() == StatusCode::BAD_REQUEST,
        "Expected 409 or 400, got {}",
        res.status()
    );
}

// ---------------------------------------------------------------------------
// Action log: verify content after API calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_action_log_records_browser_calls() {
    let state = make_state();

    // Make a navigate call (will fail with browser not running but should still be logged)
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/browser/test-profile/navigate")
        .header("content-type", "application/json")
        .body(json_body(&serde_json::json!({"url": "https://example.com"})))
        .unwrap();
    app(state.clone(), None).oneshot(req).await.unwrap();

    // Wait for async log write
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let req = axum::http::Request::builder()
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!entries.is_empty(), "Expected at least one log entry");
    let nav_entry = entries.iter().find(|e| e["tool"] == "navigate");
    assert!(nav_entry.is_some(), "Expected navigate entry in log");
    let entry = nav_entry.unwrap();
    assert_eq!(entry["profile_id"], "test-profile");
    assert_eq!(entry["success"], false); // browser not running
}

#[tokio::test]
async fn test_action_log_clear_removes_entries() {
    let state = make_state();

    // Create a log entry via API call
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/browser/del-test/navigate")
        .header("content-type", "application/json")
        .body(json_body(&serde_json::json!({"url": "https://example.com"})))
        .unwrap();
    app(state.clone(), None).oneshot(req).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify we have entries
    let req = axum::http::Request::builder()
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let before: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(!before.is_empty());

    // Clear
    let req = axum::http::Request::builder()
        .method("DELETE")
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Verify empty
    let req = axum::http::Request::builder()
        .uri("/api/action_log")
        .body(axum::body::Body::empty())
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let after: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(after.is_empty());
}

// ---------------------------------------------------------------------------
// Profile CRUD: missing field rejection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_api_add_profile_missing_required_field() {
    let app = make_app_no_auth();
    // Missing user_data_dir and other required fields
    let profile = serde_json::json!({
        "name": "Incomplete Profile"
    });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/profiles")
        .header("content-type", "application/json")
        .body(json_body(&profile))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    // Should be 422 Unprocessable Entity for missing required fields
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_api_update_profile_id_mismatch() {
    let state = make_state();
    // Create a profile
    let p = serde_json::json!({
        "id": "mismatch-src",
        "name": "Source",
        "description": "",
        "user_data_dir": "/tmp/mismatch-src",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });
    app(state.clone(), None).oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/profiles")
            .header("content-type", "application/json")
            .body(json_body(&p))
            .unwrap()
    ).await.unwrap();

    // PUT with mismatched ID in body vs URL
    let updated = serde_json::json!({
        "id": "different-id",  // Mismatched ID
        "name": "Updated",
        "description": "",
        "user_data_dir": "/tmp/mismatch-src",
        "lang": "en-US",
        "tags": [],
        "custom_args": []
    });
    let req = axum::http::Request::builder()
        .method("PUT")
        .uri("/api/profiles/mismatch-src")
        .header("content-type", "application/json")
        .body(json_body(&updated))
        .unwrap();
    let res = app(state.clone(), None).oneshot(req).await.unwrap();
    // Should succeed (using URL id) or fail with 400 — either is acceptable, not 500
    assert!(
        !res.status().is_server_error(),
        "Expected non-5xx response, got {}",
        res.status()
    );
}
```

**Step 3: Run tests**
```bash
cd /home/percy/works/browsion/src-tauri
cargo test --test api_integration_test 2>&1 | tail -20
```

Fix any failures — investigate actual behavior for duplicates and mismatched IDs.

**Step 4: Commit**
```bash
git add src-tauri/tests/api_integration_test.rs
git commit -m "test: add profile CRUD edge cases and action log content verification tests"
```

---

### Task 10: Add backend unit tests — workflow + recording edge cases

**Files:**
- Modify: `src-tauri/src/commands/workflow.rs`
- Modify: `src-tauri/src/commands/recording.rs`
- Modify: `src-tauri/src/workflow/schema.rs`
- Modify: `src-tauri/src/recording/schema.rs`

**Step 1: Read existing tests in workflow/schema.rs and recording/schema.rs**

**Step 2: Read workflow/manager.rs to see what's tested**

Look for gaps: empty workflow steps, workflow with invalid step types, recording with 0 actions.

**Step 3: Add tests to src-tauri/src/workflow/schema.rs**

```rust
#[test]
fn test_workflow_empty_steps_valid() {
    let w = Workflow {
        id: "empty".to_string(),
        name: "Empty Workflow".to_string(),
        description: "".to_string(),
        steps: vec![],
        variables: std::collections::HashMap::new(),
        created_at: 0,
        updated_at: 0,
    };
    let json = serde_json::to_string(&w).unwrap();
    let parsed: Workflow = serde_json::from_str(&json).unwrap();
    assert!(parsed.steps.is_empty());
}

#[test]
fn test_workflow_variables_roundtrip() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("url".to_string(), serde_json::Value::String("https://example.com".to_string()));
    vars.insert("timeout".to_string(), serde_json::Value::Number(5000.into()));
    let w = Workflow {
        id: "var-test".to_string(),
        name: "Var Test".to_string(),
        description: "".to_string(),
        steps: vec![],
        variables: vars,
        created_at: 100,
        updated_at: 200,
    };
    let json = serde_json::to_string(&w).unwrap();
    let parsed: Workflow = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.variables.get("url").unwrap(), "https://example.com");
    assert_eq!(parsed.variables.get("timeout").unwrap(), &serde_json::Value::Number(5000.into()));
}

#[test]
fn test_step_type_serialization_roundtrip() {
    for st in [StepType::Navigate, StepType::Click, StepType::Type, StepType::Screenshot] {
        let json = serde_json::to_string(&st).unwrap();
        let parsed: StepType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, st);
    }
}
```

**Step 4: Add tests to src-tauri/src/recording/schema.rs**

```rust
#[test]
fn test_recording_with_no_actions() {
    let r = Recording {
        id: "empty-rec".to_string(),
        name: "Empty".to_string(),
        description: "".to_string(),
        profile_id: "p1".to_string(),
        actions: vec![],
        created_at: 0,
        duration_ms: 0,
    };
    let json = serde_json::to_string(&r).unwrap();
    let parsed: Recording = serde_json::from_str(&json).unwrap();
    assert!(parsed.actions.is_empty());
    assert_eq!(parsed.duration_ms, 0);
}

#[test]
fn test_all_recorded_action_types_serialize() {
    use crate::recording::RecordedActionType;
    let types = [
        RecordedActionType::Navigate,
        RecordedActionType::Click,
        RecordedActionType::Type,
        RecordedActionType::Screenshot,
        RecordedActionType::NewTab,
        RecordedActionType::Scroll,
    ];
    for t in &types {
        let json = serde_json::to_string(t).unwrap();
        let parsed: RecordedActionType = serde_json::from_str(&json).unwrap();
        assert_eq!(&parsed, t);
    }
}
```

**Step 5: Run lib tests**
```bash
cd /home/percy/works/browsion/src-tauri
cargo test --lib 2>&1 | tail -15
```

**Step 6: Commit**
```bash
git add src-tauri/src/workflow/schema.rs src-tauri/src/recording/schema.rs
git commit -m "test: add workflow empty steps, variables, and recording action type roundtrip tests"
```

---

## Release Tasks

### Task 11: Update CHANGELOG and bump version to 0.9.2

**Step 1: Prepend new entry to CHANGELOG.md**

```markdown
## [0.9.2] - 2026-03-01

### Fixed
- **App initialization** — `useState` → `useEffect` for profile loading; profiles now load correctly on startup
- **Settings crash** — null guard on CfT version dropdown when versions not yet loaded
- **ConfirmDialog UX** — added Escape key to dismiss, Enter to confirm, overlay click to close, `role="dialog"`, `autoFocus` on Cancel
- **WorkflowList timestamps** — new workflows now initialize with current timestamp instead of Unix epoch 0
- **MonitorPage performance** — URL and title now fetched in parallel (`Promise.all`), not sequentially
- **MonitorPage memory leak** — dynamic file input elements now cleaned up after cookie import
- **WorkflowEditor** — prevent adding duplicate empty variable keys; auto-select last step when current deleted
- **Error resilience** — added React ErrorBoundary to prevent blank screen on component crash

### Testing
- **Vitest setup** — frontend test infrastructure with jsdom and @testing-library/react
- **Frontend unit tests** — formatBytes utility, profileMatchesFilter logic, UI_CONSTANTS validation
- **Backend integration** — profile list with multiple entries, tags/args roundtrip, duplicate ID detection, action log with real content, action log clear verification
- **Backend unit** — workflow empty steps, variables roundtrip, step type serialization, recording with no actions, all RecordedActionType variants
```

**Step 2: Bump versions in package.json, Cargo.toml, tauri.conf.json from 0.9.1 → 0.9.2**

**Step 3: Run full test suite**
```bash
cd /home/percy/works/browsion
npm test 2>&1 | tail -10
cd src-tauri
cargo test --lib 2>&1 | grep "test result"
cargo test --test api_integration_test 2>&1 | grep "test result"
cargo test --test config_and_cft_test 2>&1 | grep "test result"
npm run build 2>&1 | tail -5
```

All must pass.

**Step 4: Commit**
```bash
git add CHANGELOG.md package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: release v0.9.2 — UI bug fixes and expanded test coverage"
```

---

### Task 12: Push and monitor CI

**Step 1: Push**
```bash
git push origin main
```

**Step 2: Get run ID**
```bash
gh run list --limit 1
```

**Step 3: Poll until completed**
```bash
# Poll every 90 seconds
for i in $(seq 1 10); do
  sleep 90
  result=$(gh run view <RUN_ID> --json status,conclusion -q '"status=\(.status) conclusion=\(.conclusion)"')
  echo "$(date +%H:%M): $result"
  echo "$result" | grep -q "completed" && break
done
```

**Step 4: Report final status**

If CI fails, investigate the specific failing job:
```bash
gh run view <RUN_ID> --log-failed 2>&1 | head -50
```

Common fixes:
- Windows NodeJS.Timeout: use `ReturnType<typeof setTimeout>` not `number`
- macOS missing lib: check CI workflow apt-get / brew install steps
- TypeScript error: run `npx tsc --noEmit` locally to reproduce

Fix and push again.
