# Browsion MCP Server — Technical Design

## Overview

The **browsion-mcp** binary implements the [Model Context Protocol](https://modelcontextprotocol.io/) over stdio, exposing **73 tools** across 22 categories. It acts as a bridge between MCP clients (Claude Desktop, Cursor, custom agents) and the Browsion HTTP API.

```
AI Agent (Claude Desktop / Cursor)
    │  MCP stdio (JSON-RPC 2.0)
    ▼
browsion-mcp binary  (src-tauri/src/bin/browsion-mcp.rs)
    │  HTTP REST  http://127.0.0.1:38472
    ▼
Browsion app  (Tauri + Rust)
    │  CDP WebSocket  ws://127.0.0.1:{cdp_port}/json
    ▼
Chrome browser process
```

---

## Architecture

### Components

| Component | File | Role |
|---|---|---|
| **MCP Server** | `src-tauri/src/bin/browsion-mcp.rs` | 73 MCP tools → HTTP API calls |
| **HTTP API** | `src-tauri/src/api/mod.rs` | 70+ REST endpoints, axum 0.7 |
| **CDP Client** | `src-tauri/src/agent/cdp.rs` | Full CDP over WebSocket (flatten mode) |
| **Session Manager** | `src-tauri/src/agent/session.rs` | Per-profile CDP connection pool |
| **Process Manager** | `src-tauri/src/process/` | Chrome launch, PID/CDP port tracking |
| **Config** | `src-tauri/src/config/` | TOML config schema + storage |
| **App State** | `src-tauri/src/state.rs` | `AppState` shared across all subsystems |

### CDP Architecture (Flatten Mode)

CDPClient uses a **single browser-level WebSocket** connection (via `/json/version`) with CDP flatten mode. Each tab gets its own CDP session via `Target.attachToTarget(flatten=true)`.

```
Browser WS (/json/version)
    ├── session="" : Target domain (Target.*, browser-level)
    ├── session="AAA" : Tab 1 commands/events
    └── session="BBB" : Tab 2 commands/events
```

**Per-tab state** (saved/restored on `switch_tab`):
- `session_id` — CDP session for this tab
- `url` — current URL
- `ax_ref_cache` — AX ref_id → backend_node_id mapping
- `inflight_requests` — active network request counter

**Event routing**: `HashMap<(session_id, method), Vec<oneshot::Sender>>` — callers subscribe before triggering actions to avoid races.

---

## Tool Inventory (73 tools)

| Category | Count | Tools |
|---|---|---|
| Profile | 5 | `list_profiles`, `get_profile`, `create_profile`, `update_profile`, `delete_profile` |
| Lifecycle | 3 | `launch_browser`, `kill_browser`, `get_running_browsers` |
| Navigation | 7 | `navigate`, `go_back`, `go_forward`, `reload`, `wait_for_navigation`, `get_current_url`, `get_page_title` |
| Mouse | 6 | `click`, `hover`, `double_click`, `right_click`, `click_at`, `drag` |
| Keyboard | 3 | `type_text`, `slow_type`, `press_key` |
| Forms | 2 | `select_option`, `upload_file` |
| Scroll/Wait | 6 | `scroll`, `scroll_element`, `scroll_into_view`, `wait_for_element`, `wait_for_text`, `wait_for_url` |
| Observe | 7 | `get_page_state`, `get_ax_tree`, `screenshot`, `screenshot_element`, `get_dom_context`, `extract_data`, `get_page_text` |
| AX-Ref | 3 | `click_ref`, `type_ref`, `focus_ref` |
| JavaScript | 1 | `evaluate_js` |
| Tabs | 5 | `list_tabs`, `new_tab`, `switch_tab`, `close_tab`, `wait_for_new_tab` |
| Cookies | 3 | `get_cookies`, `set_cookie`, `delete_cookies` |
| Console | 3 | `enable_console_capture`, `get_console_logs`, `clear_console` |
| Network | 5 | `get_network_log`, `clear_network_log`, `block_url`, `mock_url`, `clear_intercepts` |
| Dialog | 1 | `handle_dialog` |
| Emulation | 1 | `emulate` |
| Storage | 3 | `get_storage`, `set_storage`, `clear_storage` |
| Touch | 2 | `tap`, `swipe` |
| PDF | 1 | `print_to_pdf` |
| Frames | 3 | `get_frames`, `switch_frame`, `main_frame` |
| Utility | 1 | `wait` |

---

## Key Implementation Details

### AX Tree & Semantic Refs

`get_ax_tree()` and `get_page_state()` return an **Accessibility Tree** filtered to ~20–100 nodes:
- Skips: `none`, `generic`, `InlineTextBox`, `StaticText` (unless named), ignored nodes
- Keeps: interactive elements (buttons, inputs, links) + named structural nodes
- Each node gets a `ref_id` like `"e1"`, `"e2"`, stored in `ax_ref_cache`

`click_ref(ref_id)` resolution path:
1. Look up `backend_node_id` from `ax_ref_cache`
2. `DOM.resolveNode(backendNodeId)` → get `objectId`
3. `Runtime.callFunctionOn(objectId, getBoundingClientRect)` → viewport coords
4. `Input.dispatchMouseEvent` (move + click)

`type_ref` / `focus_ref` use `DOM.focus(backendNodeId)` directly.

### Network Interception

`block_url(pattern)` and `mock_url(pattern, ...)` use the **Fetch domain**:
- Patterns support `*` glob wildcard (e.g. `"*/api/v1/*"`)
- `Fetch.requestPaused` events are matched against rules
- Block → `Fetch.failRequest(errorReason=BlockedByClient)`
- Mock → `Fetch.fulfillRequest(status, body, headers)`
- `clear_intercepts()` disables the Fetch domain

### Navigation Waiting

`navigate()` accepts optional `wait_until` + `timeout_ms`:
- `load` — waits for `Page.loadEventFired`
- `domcontentloaded` — waits for `Page.domContentEventFired`
- `networkidle` — polls until `inflight_requests == 0` for 500ms with a shared deadline

### go_back / go_forward

Uses `Page.getNavigationHistory` + `Page.navigateToHistoryEntry` + waits for `Page.frameNavigated`. Does NOT use `history.back()` (which silently no-ops on SPAs without triggering CDP events).

### Screenshot

- **Viewport**: `Page.captureScreenshot(format, quality)`
- **Full page**: `Page.getLayoutMetrics` → set `captureBeyondViewport=true` with clip covering full content height
- **Element**: `deepQuery(selector)` → page-absolute `getBoundingClientRect` → `Page.captureScreenshot(clip=...)`

### Console Capture

Listens to `Runtime.consoleAPICalled` + `Log.entryAdded` events. Buffer is per-CDPClient, max 1000 entries. Requires `enable_console_capture()` before collecting logs.

### New Tab Workflow

```
1. wait_for_new_tab(timeout=5000)   ← subscribe to Target.targetCreated FIRST
2. click_ref("e5")                  ← trigger target="_blank" link
3. switch_tab(target_id=<step 1>)   ← switch to new tab
4. get_page_state()                 ← observe new tab
```

---

## HTTP API

The HTTP API runs on `http://127.0.0.1:{api_port}` (default **38472**). All endpoints require `X-API-Key` header when an API key is configured (except `GET /api/health`).

Route prefixes:
- `/api/profiles` — Profile CRUD
- `/api/launch/:id`, `/api/kill/:id`, `/api/running` — Lifecycle
- `/api/browser/:id/` — CDP browser control (70+ endpoints)
- `/api/health` — Liveness probe

Full endpoint list: see README.md.

---

## Auth & Security

- API key via `X-API-Key` header (env: `BROWSION_API_KEY`)
- `GET /api/health` is always public (used by MCP binary startup probe)
- CORS: not enabled — local-only service

---

## Build & Test

```bash
# Build MCP binary
cd src-tauri && cargo build --release --bin browsion-mcp

# Run all tests (includes 20 E2E browser tests + integration tests)
cargo test --manifest-path src-tauri/Cargo.toml
```

E2E tests (`src-tauri/tests/e2e_browser_test.rs`) require Chrome to be installed and launch real browser instances with `--headless=new --disable-gpu --disable-dev-shm-usage`.

---

## Configuration

Config file: `~/.config/browsion/config.toml`

```toml
api_port = 38472   # 0 = disabled

[settings]
minimize_to_tray = true

[[profiles]]
id = "uuid-1234"
name = "US Profile"
user_data_dir = "/home/user/chrome/us"
proxy_server = "http://proxy:8080"
lang = "en-US"
timezone = "America/Los_Angeles"
headless = false
tags = ["work"]
```

MCP client setup:

```json
{
  "mcpServers": {
    "browsion": {
      "command": "/path/to/browsion-mcp",
      "env": {
        "BROWSION_API_PORT": "38472",
        "BROWSION_API_KEY": "your-key"
      }
    }
  }
}
```
