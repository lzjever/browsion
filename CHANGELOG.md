# Changelog

All notable changes to this project will be documented in this file.

## [0.9.4] - 2026-03-01

### Testing
- **Testid standardization** — all 47 E2E tests now follow `test_<category>_<operation>_<variant>` pattern
- **Renamed 20 tests** — converted numbered tests (test_01-test_20) to descriptive testid names
- **Form interactions** — test_form_select_dropdown_option for dropdown selection
- **Mouse operations** — test_mouse_click_at_viewport_coordinates for direct coordinate clicks
- **Storage operations** — test_storage_clear_local_storage, test_storage_remove_local_key
- **Cookie operations** — test_cookies_delete_specific_cookie
- **Data extraction** — test_observe_extract_structured_data
- **Geolocation emulation** — test_emulate_set_geolocation
- **Workflow execution** — test_workflow_execute_simple_navigate via HTTP API
- **Recording lifecycle** — test_recording_lifecycle_via_api via HTTP API

### Test Coverage
- **Total test count** — 242 tests (18 frontend + 79 lib + 92 API integration + 6 config + 47 E2E)
- **E2E breakdown** — All tests follow standardized naming covering navigate, mouse, keyboard, form, axref, tabs, cookies, storage, console, network, screenshot, profile, lifecycle, snapshot, emulate, touch, frames, dialog, workflow, recording

### New Features
- **Workflow HTTP API** — REST endpoints for workflow CRUD and execution
- **Recording HTTP API** — REST endpoints for recording lifecycle (start/stop/status)

### Breaking Changes
- Test function names changed — E2E test names now follow standardized pattern

## [0.9.3] - 2026-03-01

### Testing
- **E2E comprehensive expansion** — 20 → 38 tests (90% increase)
- **Browser lifecycle** — `test_lifecycle_launch_and_kill` verifies Chrome launch, CDP connection, and process termination
- **Profile CRUD via HTTP API** — `test_profile_crud_via_api` tests full CREATE/READ/UPDATE/DELETE cycle through API endpoints
- **Mouse operations** — `test_mouse_hover_element`, `test_mouse_drag_element`, `test_mouse_double_and_right_click` cover hover, drag, double-click, and right-click
- **Form handling** — `test_form_upload_file` verifies file upload via DOM.setFileInputFiles
- **Dialog handling** — `test_dialog_handle_alert` tests JavaScript alert/confirm dismissal via Page.handleJavaScriptDialog
- **Device emulation** — `test_emulate_viewport` validates Emulation.setDeviceMetricsOverride and viewport resizing
- **Touch events** — `test_touch_tap_and_swipe` covers touch tap and swipe gestures
- **iframe handling** — `test_frames_switch` tests frame listing via Page.getFrameTree
- **Snapshots** — `test_snapshot_create_restore` verifies snapshot creation and listing via API
- **Cookie portability** — `test_cookie_export_import` tests Network.getAllCookies after setting cookies
- **Action logging** — `test_action_log_records_api_calls` confirms navigate_wait actions are logged
- **Network mocking** — `test_network_mock_url` tests URL pattern interception and custom responses
- **PDF generation** — `test_pdf_generation` validates Page.printToPDF output
- **Wait operations** — `test_wait_for_element` tests element waiting with timeout
- **Scroll operations** — `test_scroll_into_view` verifies scroll-into-view via JS
- **AX reference** — `test_axref_focus` tests focus via accessibility tree reference IDs

### Test Coverage
- **Total test count** — 233 tests (18 frontend + 79 lib + 92 API integration + 6 config + 38 E2E)
- **E2E breakdown** — 20 existing + 6 P0 (lifecycle/profile CRUD/mouse/forms/dialogs) + 6 P1 (emulate/touch/frames/snapshots/cookies/action log) + 6 P2 (network/PDF/mouse variants/scroll/wait/focus)

### Documentation
- Added `src-tauri/tests/README.md` with testid naming standard (`test_<category>_<operation>_<variant>`)
- Documented 20 test categories (navigate, mouse, keyboard, form, axref, tabs, cookies, storage, console, network, screenshot, profile, lifecycle, snapshot, emulate, touch, frames, dialog, workflow, recording)
- Chrome binary discovery order and isolation notes documented

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

## [0.9.1] - 2026-03-01

### Testing
- **Comprehensive API coverage** — expanded integration tests from 20 to 85 test cases, covering all browser control endpoints
- **All browser routes tested** — every API route now has a "not running" error-path test (64 browser routes × HTTP methods)
- **Profile update tests** — PUT `/api/profiles/:id` success and 404 cases
- **Action log endpoint tests** — GET and DELETE `/api/action_log` integration tests plus entry-shape validation
- **Profile snapshots tests** — list endpoint for unknown profile returns 200 with empty array
- **ActionLog unit tests** — push/filter/clear/capacity-limit plus `days_to_ymd` algorithm correctness
- **Path-parsing unit tests** — `parse_path_for_log` covering browser, launch, kill, and CRUD paths
- **Recording-mapping unit tests** — `tool_to_recorded_action` for all mapped and unmapped tools

## [0.9.0] - 2026-03-01

### Added

#### Real-Time Recording UI
- **Live Recording Controls** — start/stop recording directly from profile list
- **Recording Session Manager** — tracks active recording sessions per profile
- **Recording Status Indicator** — shows recording state with action count
- **Recording Save Dialog** — name and describe recordings before saving
- **WebSocket Integration** — real-time recording status updates via `recording-status-changed` event

#### Backend Implementation
- `recording/session.rs` — RecordingSessionManager for active session tracking
- `recording/schema.rs` — RecordingSessionInfo struct with Serialize support
- **5 New Tauri Commands**:
  - `start_recording(profile_id)` — start a recording session
  - `stop_recording(profile_id, name, description)` — stop and save
  - `get_active_recording_sessions()` — list all active sessions
  - `is_recording(profile_id)` — check recording status
  - `get_recording_session_info(profile_id)` — get session details
- **Action Log Integration** — actions automatically recorded to active sessions
- `AppState.recording_session_manager` field

#### Frontend Components
- **ProfileItem Recording Button** — 🔴 Start/⏹ Stop toggle with action count
- **RecordingSaveDialog** — modal for naming recordings with session info
- **Real-time Status** — animated recording indicator in profile status bar
- **TypeScript Types** — RecordingSessionInfo interface added

### Technical
- Fixed RecordingSessionInfo serialization by moving to schema.rs with proper serde imports
- Recording state synchronization via Tauri events
- Action count display updates in real-time

## [0.8.0] - 2026-02-28

### Added

#### Recording & Playback (Phase 3)
- **Recording Engine** — capture browser automation sequences
- **Recording Manager** — persists to `~/.browsion/recordings/{id}.json`
- **15+ Recordable Action Types**:
  - Navigation: navigate, go_back, go_forward, reload
  - Mouse: click, hover, double_click, right_click
  - Keyboard: type, slow_type, press_key
  - Forms: select_option, upload_file
  - Scroll: scroll, scroll_into_view
  - Tabs: new_tab, switch_tab, close_tab
  - Wait: sleep, wait_for_text, wait_for_element
  - Observe: screenshot, get_console_logs, extract
- **Action Metadata** — timestamp (ms from start), optional screenshot
- **Playback Player** — execute recordings on any profile

#### Frontend UI
- **Recordings tab** — new top-level navigation section
- **Recording List** — card grid with action count and duration
- **Recording Player** — modal with:
  - Profile selector
  - Progress bar (current action / total)
  - Action list with status indicators
  - Playback controls with error handling
- **Convert to Workflow** — transform recording into reusable workflow

#### Backend Implementation
- `recording/schema.rs` — Recording, RecordedAction, RecordingSession
- `recording/manager.rs` — persistence to JSON files
- `commands/recording.rs` — Tauri commands + workflow conversion
- `AppState.recording_manager` field
- `From<RecordedActionType> for StepType` conversion

### Technical
- Atomic file writes for recording persistence
- Sequential action execution with error handling
- One-click recording-to-workflow conversion

## [0.7.0] - 2026-02-28

### Added

#### Workflow Engine (Phase 2)
- **Multi-step task automation** — define reusable workflows with sequences of browser actions
- **Workflow Manager** — CRUD operations, persists to `~/.browsion/workflows/{id}.json`
- **20+ Step Types**:
  - Navigation: navigate, go_back, go_forward, reload, wait_for_url
  - Mouse: click, hover, double_click, right_click, drag
  - Keyboard: type, slow_type, press_key
  - Forms: select_option, upload_file
  - Scroll: scroll, scroll_element, scroll_into_view
  - Wait: wait_for_element, wait_for_text, sleep
  - Observe: screenshot, get_page_state, get_page_text, get_cookies
  - Tabs: new_tab, switch_tab, close_tab, wait_for_new_tab
  - Advanced: extract, get_console_logs, set_variable, condition
- **Variable substitution** — use `${varname}` syntax in step parameters
- **Error handling** — `continue_on_error` flag per step
- **Timeout control** — configurable timeout per step (default 30s)

#### Frontend Workflow UI
- **Workflows tab** — new top-level navigation section
- **Workflow List** — card grid showing all workflows with step counts
- **Workflow Editor** — visual editor for creating/editing workflows:
  - Name and description fields
  - Variable editor (name/value pairs)
  - Step list with drag-to-reorder support
  - Parameter inputs based on step type
  - Continue-on-error and timeout settings
- **Run Modal** — select profile and execute workflow:
  - Profile dropdown
  - Step preview
  - Live execution results
  - Per-step status, duration, error messages

#### Backend Implementation
- `workflow/schema.rs` — data structures with serde serialization
- `workflow/manager.rs` — persistence to JSON files
- `workflow/executor.rs` — HTTP API-based execution engine
- `commands/workflow.rs` — Tauri commands for CRUD and execution
- `AppState.workflow_manager` field

### Technical
- Variable resolution via string substitution (`${varname}`)
- HTTP client with API key authentication
- Atomic file writes for workflow persistence
- Display trait for StepType enum

## [0.6.0] - 2026-02-28

### Added

#### WebSocket Real-Time Events
- **WebSocket server** at `/api/ws` for live event streaming
- **Event types**:
  - `BrowserStatusChanged` — browser launched or killed
  - `ActionLogEntry` — new API action logged
  - `ProfilesChanged` — profile added/updated/deleted
  - `Heartbeat` — keep-alive every 30s
- **Auto-reconnect** — clients reconnect after 3s on disconnect
- **Monitor page** now uses WebSocket instead of polling for action log
  - Action log entries appear instantly as they happen
  - Browser status updates in real-time
  - Screenshot polling remains (3s interval, no event needed)
- **Connection indicator** — "● Live" / "○ Offline" status badge

#### Frontend Hook
- `useWebSocket` hook for real-time event subscriptions
- Handles connection, reconnection, and event routing
- Fallback to Tauri events still available

### Changed
- Monitor page action log updates via WebSocket push (no 5s polling)
- Screenshot polling kept at 3s (images too large for WS push)
- API event middleware now broadcasts to WebSocket clients
- `AppState` includes `ws_broadcaster` field
- API modules split: `ws.rs` (WebSocket), `profiles.rs`, `lifecycle.rs`, `browser.rs`, `snapshots.rs` (prepared for future refactoring)

### Technical
- Uses `tokio::sync::broadcast` for efficient fan-out to multiple clients
- Channel capacity: 100 events per client
- WebSocket authenticated via same-origin (localhost only)
- Heartbeat timeout: 35s (auto-disconnect stale connections)

## [0.5.0] - 2026-02-28

### Added

#### Activity Monitor (F2)
- New **Monitor** top-level tab with live observability dashboard
- Per-profile cards showing JPEG thumbnails (polled every 3s), current page URL and title, top-5 recent actions with duration and success/fail badge
- Full **Action Log** table with profile filter dropdown and full-text search (by tool name or profile)
- Pause/Resume toggle — also pauses automatically when the browser tab is hidden

#### Action Log (F1)
- In-memory ring buffer (max 2 000 entries) recording every HTTP API call: timestamp, profile, tool, duration, success/error
- Automatic append to daily JSONL files at `~/.browsion/logs/YYYY-MM-DD.jsonl`
- Tower middleware wired into HTTP API — zero-touch, logs every route transparently
- HTTP routes: `GET /api/action_log?profile_id=&limit=100`, `DELETE /api/action_log?profile_id=`

#### Session Reconnect (F3)
- Running Chrome sessions persisted to `~/.browsion/running_sessions.json` (pid + CDP port per profile)
- On Tauri restart, each saved session is probed (`/json/version`) — live sessions are reconnected without restarting Chrome, dead entries are purged
- `ProcessManager.register_external()` — reattach to an already-running Chrome process

#### Proxy Presets (F6)
- Named proxy presets stored in app config (`ProxyPreset { id, name, url }`)
- New **Proxy Presets** section in Settings: add, delete, and test latency (GET `https://example.com`, 10 s timeout)
- Profile Form proxy field now shows a preset dropdown above the manual text input — choose a preset or type a custom URL
- Tauri commands: `get_proxy_presets`, `add_proxy_preset`, `update_proxy_preset`, `delete_proxy_preset`, `test_proxy`

#### Profile Snapshots (F4)
- Full profile data directory backup/restore at `~/.browsion/snapshots/<profile_id>/<name>/`
- Snapshot manifest (`manifest.json`) tracks name, creation timestamp, and total byte size
- **Snapshots** button on each profile card opens a modal: create, restore, and delete snapshots
- Browser must be stopped to create or restore a snapshot (returns error if running)
- HTTP endpoints: `GET/POST /api/profiles/:id/snapshots`, `POST .../snapshots/:name/restore`, `DELETE .../snapshots/:name`
- MCP tools: `list_profile_snapshots`, `create_profile_snapshot`, `restore_profile_snapshot`
- Tauri commands: `list_snapshots`, `create_snapshot`, `restore_snapshot`, `delete_snapshot`

#### Cookie Import / Export (F5)
- Export all cookies for a running browser as **JSON** or **Netscape** (`.txt`) format — downloaded directly via browser save dialog
- Import cookies from JSON or Netscape file — uploaded via file picker, imported in bulk
- `set_cookie_full` CDP helper preserves `secure`, `httpOnly`, and `expires` on import (existing `set_cookie` omitted these)
- HTTP routes: `GET /api/browser/:id/cookies/export?format=json|netscape`, `POST /api/browser/:id/cookies/import`
- MCP tools: `export_cookies`, `import_cookies`
- Cookie export/import buttons on each Monitor card for quick in-session management

### Changed
- `AppConfig` now includes `proxy_presets` field (backward-compatible default: empty list)
- Monitor page polling skips silently when no browsers are running or when tab is hidden

## [0.4.0] - 2026-02-28

### Added

#### Dedicated MCP Page + Skill Mode
- New **MCP** top-level tab (alongside Profiles and Settings)
- **API Server** section: enable/disable toggle, port input, API key with Generate/Copy, live status badge, Apply button — migrated from Settings and enhanced
- **MCP Binary** section: auto-detects `browsion-mcp` at install path, macOS bundle Resources, and dev `target/release/`; user-editable path with Browse button; collapsible build instructions
- **Client Setup** section: one-click config writer for 7 AI coding tools:
  - **Cursor** — merges into `~/.cursor/mcp.json` (`mcpServers.browsion`)
  - **Claude Code** — merges into `~/.claude.json` (`mcpServers.browsion`)
  - **Codex CLI** — merges into `~/.codex/config.toml` (TOML `[mcp_servers.browsion]`)
  - **Windsurf** — merges into `~/.codeium/windsurf/mcp_config.json`
  - **Zed** — merges into platform `settings.json` (`context_servers.browsion`)
  - **Continue (VS Code)** — creates/merges `.continue/mcpServers/browsion.json` (project-scoped)
  - **OpenClaw** — merges into `openclaw.json` in your project directory (project-scoped)
- Per-tool status dots (green = config found, grey = not yet configured)
- Per-tool config snippet with Copy button (correct format per tool: JSON / TOML / Zed `context_servers` / Continue whole-file)
- **New Tauri commands**: `detect_mcp_tools`, `write_browsion_to_tool`, `find_mcp_binary`

#### Config Write Safety
- All writes are **atomic** (temp file → `rename`) — partial writes never corrupt your config
- All writes **merge** into existing config, preserving every other entry — safe to run on an existing file
- JSONC support for Zed's `settings.json` (strips `//` and `/* */` comments before parse; note: comments are not re-emitted)
- Parse failure on existing file surfaces as an error — original file is never touched

### Changed
- **Settings page**: MCP/API Server section removed; Settings now contains only Browser and Application Settings
- `Write to config` button disabled when API Server config has unsaved changes (prevents writing a stale port/key)
- Port field no longer snaps to `38472` when cleared — uses a string editing buffer committed on blur

## [0.3.0] - 2026-02-27

### Added

#### CDP Flatten Mode (Architecture)
- Single browser-level WebSocket connection via `/json/version` (CDP flatten mode)
- Multi-session support: each tab runs in its own CDP session, all multiplexed over one WS
- Per-tab state: AX ref cache, inflight request counter, session ID, URL — saved and restored on `switch_tab`
- Event routing keyed by `(session_id, method)` to avoid cross-tab interference

#### New MCP Tools (73 total, up from 61)
- `wait_for_new_tab` — subscribe to `Target.targetCreated` before clicking `target="_blank"` links
- `get_page_text` — extract full `document.body.innerText`
- `block_url` — block network requests matching a glob pattern (`*` wildcard support)
- `mock_url` — intercept and mock responses for matched URLs
- `clear_intercepts` — disable all Fetch domain intercept rules
- `print_to_pdf` — render page to PDF with layout options (landscape, headers, margins)
- `tap` — mobile touch tap via `Input.dispatchTouchEvent`
- `swipe` — mobile swipe gesture with direction and distance
- `get_frames` — list all iframes on the page
- `switch_frame` — execute CDP commands inside a specific iframe
- `main_frame` — return focus to the main frame
- `clear_console` — clear the captured console log buffer

#### New HTTP API Routes
- `POST /api/browser/:id/tabs/wait_new` — wait for new tab
- `GET /api/browser/:id/page_text` — get page text
- `POST /api/browser/:id/intercept/block` — block URLs
- `POST /api/browser/:id/intercept/mock` — mock URLs
- `DELETE /api/browser/:id/intercept` — clear intercept rules
- `GET /api/browser/:id/pdf` — print to PDF
- `POST /api/browser/:id/tap` — mobile tap
- `POST /api/browser/:id/swipe` — mobile swipe
- `GET /api/browser/:id/frames` — list frames
- `POST /api/browser/:id/switch_frame` — switch to frame
- `POST /api/browser/:id/main_frame` — switch to main frame
- `POST /api/browser/:id/console/clear` — clear console buffer

#### New Data Types
- `TabInfo.active` — whether a tab is currently focused
- `FrameInfo` — frame ID, URL, parent frame ID
- `ConsoleLogEntry` — level, text, source, timestamp
- `InterceptRule` — url_pattern + action (Block or Mock)

#### UI Improvements
- Toast notifications and `ConfirmDialog` — replaced all `alert()`/`confirm()` in ProfileList
- Profile cards show timezone and user data directory
- Search bar filters by profile name and tags simultaneously
- Loading state on Launch button during browser startup
- Auto-cleanup of dead browser processes every 30s

#### Chrome for Testing (CfT)
- Auto-download Chrome for Testing (Stable/Beta/Dev/Canary channels)
- Platform-aware binary paths (linux64, mac-x64, mac-arm64, win64)
- Progress events streamed to UI during download

#### Test Suite
- 20 E2E browser tests (`src-tauri/tests/e2e_browser_test.rs`) using real Chrome
- Integration tests for API and config (`api_integration_test.rs`, `config_and_cft_test.rs`)
- 27 unit tests (`cargo test --lib`): glob_match, config validation, process, MCP commands

### Fixed

#### Critical Bugs
- **WS reader deadlock**: `tokio::Mutex` self-deadlock in `Network.requestWillBeSent` handler — double-lock in one statement caused all commands to hang. Fixed to single lock acquisition.
- **DOM.pushNodesByBackendIdsToFrontend deprecated** (Chrome 143): replaced with `DOM.resolveNode(backendNodeId)` + `Runtime.callFunctionOn` for AX-ref coordinate resolution
- **slow_type double keypress**: `keyDown` with `text` field caused Chrome to synthesize `keypress` event, doubling typed characters. Removed `text` from `keyDown`/`keyUp` events; only `char` event carries `text`.
- **Tray icon panic**: `app.default_window_icon().unwrap()` replaced with conditional `if let Some(icon)` to avoid panic when no icon is configured
- **Mutex poison panic** in `update_mcp_config`: `.unwrap()` on locked mutex replaced with `.unwrap_or_else(|e| e.into_inner())`
- **CORS header parse panic**: `"X-API-Key".parse().unwrap()` replaced with `HeaderName::from_static("x-api-key")`

#### CDP Fixes
- `new_tab`: changed from GET to PUT for `/json/new` endpoint (Chrome requires PUT)
- `networkidle`: real implementation using `inflight_requests` counter that resets on navigation; polls for 500ms of zero inflight with a single shared deadline
- `upload_file`: shadow DOM traversal via JS `deepQuery` + `DOM.describeNode` + `backendNodeId` (not `DOM.querySelector`)
- `wait_for_text`: use `evaluate_js` with JS try-catch to ignore page-transition errors
- `scroll_element`: fixed mouseWheel events via `Input.dispatchMouseEvent type=mouseWheel`
- Selector encoding: all CSS selectors serialized via `serde_json::to_string()` to prevent injection

#### Process / Config Fixes
- `ProcessManager` now initializes `recent_launches` from persisted config on startup
- Tag filter state preserved across profile refreshes
- User extensions no longer blocked (`--disable-extensions` removed from defaults)
- CDP port counter correctly wraps from 19221 back to 9222
- `Asia/Kolkata` timezone corrected (was `India/Kolkata`, which is invalid IANA)
- Duplicate and deprecated Chrome args removed from presets; headless and window-size presets added
- API key generation uses `crypto.getRandomValues()` (cryptographically secure)
- Silent `save_config` failure now logs `tracing::warn` instead of being silently ignored

#### Network Interception
- `block_url` / `mock_url` patterns now support `*` glob wildcards (e.g. `"*/api/v1/*"`)

### Changed

#### Architecture
- **CDP client**: Migrated from per-tab WebSocket connections to single browser-level WS (flatten mode) for reliable multi-tab support
- **Tab management**: `switch_tab` saves/restores per-tab AX ref cache and inflight counter

#### Documentation
- README completely rewritten: 73 tools, full HTTP API table (70+ endpoints), correct project structure
- `docs/mcp-server-design.md` rewritten from planning doc to accurate technical reference

### Removed

- Legacy AI agent engine (`src-tauri/src/agent/engine.rs`, `action.rs`, `llm.rs`) — replaced by MCP/HTTP API automation
- Legacy agent UI components (`AgentPanel.tsx`, `AISettings.tsx`, `SchedulePanel.tsx`)
- Legacy agent TypeScript types (`src/types/agent.ts`)
- Legacy agent integration tests (superseded by E2E browser tests)

---

## [0.2.3] - 2026-02-19

### Added

#### Profile Dialog UX/UI Improvements
- New two-column layout for Profile Form dialog (wider 1000px modal)
- Large textarea for Description/Notes field (supports JSON, account info, etc.)
- **LaunchArgsSelector component**: Preset checkboxes for common Chromium arguments
  - Performance: `--disable-gpu`, `--disable-dev-shm-usage`, `--disable-software-rasterizer`
  - Security: `--no-sandbox`, `--disable-web-security`, `--ignore-certificate-errors`
  - Window: `--start-maximized`, `--start-fullscreen`
  - Network: `--disable-background-networking`, `--disable-extensions`
  - Automation: `--disable-infobars`, `--disable-blink-features=AutomationControlled`
- Custom Arguments textarea for additional flags

### Changed

#### CDP Launcher
- Removed default `--disable-gpu` argument in headless mode (user can now opt-in via preset)

#### Profile Form Layout
- Left column: Name, User Data Dir, Tags, Language, Color, Proxy, Timezone, Fingerprint
- Right column: Description (large textarea), Launch Arguments Presets, Custom Arguments
- Responsive design: single column on screens < 768px

## [0.2.1] - 2026-02-14

### Fixed

#### CDP Connection
- Fix CDP client connecting to wrong WebSocket (browser vs page target)
- Fix `Page.navigate` and other commands returning "not found" errors
- Fix browser not executing actions despite LLM decisions

#### AI Agent
- Fix stop/pause buttons not working (event listener dependency issue)
- Fix agentId not being set from progress events

### Changed

#### CDP Port
- Use dynamic CDP port allocation (9222+) to support multiple concurrent agents

#### Process Management
- Ensure Chrome process is always closed when agent exits (even on errors)

#### Message History
- Limit LLM message history to 30 messages to avoid token limits and memory growth

## [0.2.0] - 2026-02-14

### Added

#### Profile Tags System
- Add tags field to profiles for categorization and filtering
- Support comma or space separated tag input
- Display tags in profile cards (max 3 visible, overflow shows `+N`)
- Real-time tag filtering with OR logic

#### ProfileForm Improvements
- Three-section modal layout (fixed header/body/footer)
- Cancel/Save buttons always visible without scrolling
- Language field with autocomplete suggestions (ISO 639-1)
- Timezone field with all IANA timezones (~400+)
- Both fields support manual input via datalist

#### Dynamic Data Sources
- Timezones: `Intl.supportedValuesOf('timeZone')` browser API
- Languages: `iso-639-1` npm package + `Intl.DisplayNames`
- No hardcoded lists, always up-to-date

### Fixed

- Fix missing `tags` field in test fixtures (validation.rs, launcher.rs)
- Fix clippy warnings in activation.rs (needless borrows)

### Dependencies

- Added `iso-639-1@3.1.5` for language code data

## [0.1.0] - 2026-02-13

### Added

- Initial release
- Cross-platform browser launcher (Windows, macOS, Linux)
- System tray integration with recent profiles
- Profile management (add, edit, delete, clone)
- Browser launch with custom arguments
- Proxy, timezone, language, fingerprint configuration
- Process tracking and window activation
- TOML-based configuration storage
