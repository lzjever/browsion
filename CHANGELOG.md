# Changelog

All notable changes to this project will be documented in this file.

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
