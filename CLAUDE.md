# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Development (frontend + Tauri backend with hot reload)
npm run tauri dev

# Frontend tests
npm test                 # vitest run
npm run test:watch       # vitest watch

# Rust backend (use scripts/cargo in Cursor due to ARGV0 issue)
cd src-tauri && cargo check --lib           # Fast compile check
cd src-tauri && cargo test --lib            # Unit tests (27 tests)
cd src-tauri && cargo test --test e2e_browser_test -- --test-threads=1  # E2E browser tests (20 tests)

# Build MCP binary
cd src-tauri && cargo build --release --bin browsion-mcp

# Production build
npm run tauri build
```

**Cursor terminal note**: Cursor sets `ARGV0` which breaks rustup's proxy. Use `./scripts/cargo` wrapper or run `unset ARGV0` first.

## Architecture Overview

**Stack**: Tauri 2 (Rust backend + React/TypeScript frontend) + Chrome DevTools Protocol via raw WebSocket (NOT Playwright)

### Three Components
1. **Tauri App** (`src-tauri/src/`) — Desktop app with system tray, profile management, HTTP API server
2. **MCP Server** (`src-tauri/src/bin/browsion-mcp.rs`) — Standalone binary exposing 73 tools for AI agents via stdio
3. **Frontend** (`src/`) — React UI for profile/workflow/recording management

### Key Backend Modules (`src-tauri/src/`)

| Module | Purpose |
|--------|---------|
| `agent/cdp.rs` | CDPClient — browser-level WebSocket, flatten mode, all browser control |
| `agent/session.rs` | SessionManager — per-profile CDP connection pool |
| `agent/types.rs` | Data types: DOMElement, AXNode, PageState, TabInfo, InterceptRule |
| `api/mod.rs` | HTTP API router (70+ endpoints) with action log middleware |
| `api/browser.rs` | Browser control HTTP handlers |
| `config/schema.rs` | BrowserProfile, AppConfig, ProxyPreset, SnapshotInfo |
| `process/launcher.rs` | Chrome launch with flags, proxy, timezone, fingerprint |
| `process/manager.rs` | Process tracking, CDP port allocation, cleanup |
| `workflow/executor.rs` | Workflow step execution engine |
| `recording/session.rs` | Browser-level event recording (clicks, navigation, etc.) |
| `state.rs` | AppState — shared state with config, process_manager, session_manager |

### CDP Flatten Mode Architecture

CDPClient uses a **single browser-level WebSocket** (via `/json/version` endpoint) for all tabs:

- **Tab registry**: `tab_registry: HashMap<target_id, TabState>` tracks per-tab session_id, URL, AX cache
- **Response routing**: `(session_id, msg_id) → oneshot::Sender` for command responses
- **Event routing**: `(session_id, method) → Vec<oneshot::Sender>` for event subscribers
- **Per-tab state**: Each tab has its own `ax_ref_cache` and `inflight_requests` counter
- **Tab switch**: `switch_tab()` saves/restores per-tab state to/from `tab_registry`

**Important patterns**:
- Subscribe to events BEFORE triggering actions (avoid race conditions)
- Use `send_browser_command()` for Target.* domain, `send_command()` for tab-level commands
- AX ref cache is cleared on navigation; `click_ref`/`type_ref` resolve via `DOM.resolveNode(backendNodeId)`

### HTTP API Flow

```
MCP Client → browsion-mcp binary → HTTP API (port 38472) → SessionManager → CDPClient → Chrome
```

API key auth via `X-API-Key` header (optional, from `BROWSION_API_KEY` env).

### Frontend Structure (`src/`)

- `components/ProfileList.tsx` — Main profile list with CRUD, launch/kill
- `components/WorkflowEditor.tsx` — Visual workflow builder
- `components/RecordingPlayer.tsx` — Recording playback UI
- `components/MonitorPage.tsx` — Live screenshots, action log, cookie export
- `api/tauri.ts` — Tauri command wrappers
- `types/profile.ts` — TypeScript types matching Rust schema

## Testing

### E2E Browser Tests (`src-tauri/tests/e2e_browser_test.rs`)
- Launches real Chrome in headless mode
- Uses in-process axum server for test HTML pages
- Requires `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` — WS reader needs separate thread
- Chrome flags: `--headless=new --disable-gpu --disable-dev-shm-usage`
- Tests skip (not fail) if Chrome binary not found

### Known CDP Limitations
- AX-Ref (`click_ref`/`type_ref`/`focus_ref`) may not work on `data:` URLs — CDP `DOM.pushNodesByBackendIdsToFrontend` has limitations
- Storage tools blocked by browser security on `data:` URLs

## Recommended AI Agent Workflow

```
1. list_profiles      → find profile id
2. launch_browser     → start Chrome
3. navigate           → go to URL (waits for load)
4. get_page_state     → URL + title + AX tree with ref_ids
5. click_ref/type_ref → interact via semantic refs (preferred over CSS selectors)
6. screenshot         → visual verification
7. kill_browser       → cleanup
```

## New Tab Workflow (target="_blank" links)

```
1. wait_for_new_tab(timeout_ms=5000)   ← call BEFORE the click
2. click_ref("e5")                     ← click the link
3. switch_tab(target_id=<from step 1>) ← switch to new tab
4. get_page_state()                    ← observe new tab
```
