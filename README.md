# Browsion

**Cross-platform Browser Launcher - Manage multiple Chrome profiles with ease**

## Features

- **Cross-platform**: Windows, macOS, Linux
- **System Tray**:
  - Single/double-click to open main window
  - Right-click menu for quick access to recent profiles
  - Auto display running status (● running / ○ stopped)
- **Profile Management**: Add, edit, clone, delete profiles with tags, proxy, timezone, language, fingerprint, headless mode
- **Tags System**: Categorize and filter profiles by tags
- **Smart Forms**:
  - Language field with 100+ locale suggestions (ISO 639-1)
  - Timezone field with all IANA timezones (~400+)
  - Both support manual input or dropdown selection
- **One-click Launch**: Start pre-configured browser instances
- **Process Tracking**: Real-time monitoring of browser status
- **Window Activation**: Quick switch to running browsers
- **Recent Records**: Auto-track last 10 launched profiles
- **Chrome for Testing**: Auto-download and manage CfT versions (Stable/Beta/Dev/Canary)

## Quick Start

### Prerequisites (Linux)

```bash
# Install window management tools (required for activation feature)
sudo pacman -S xdotool wmctrl  # Arch/Manjaro
sudo apt install xdotool wmctrl  # Ubuntu/Debian
```

### Development

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

**Running `cargo` in Cursor's terminal:** Cursor sets the `ARGV0` environment variable, which breaks rustup's proxy. Use the wrapper:

```bash
./scripts/cargo build   # in src-tauri, or from repo root:
make build              # builds the Tauri backend
make test               # runs Tauri tests
```

Or in any shell: `unset ARGV0` then run `cargo` as usual.

## Usage

### Basic Operations

1. **Launch App**: Run `npm run tauri dev`
2. **Set Chrome Source**: Settings → Chrome for Testing (auto-download) or Custom path
3. **Add Profile**: Profiles → Add Profile
4. **Launch Browser**: Click Launch button
5. **Manage Windows**: Use Activate/Kill buttons

### Tray Features

- **Single/Double-click tray icon**: Show main window (auto-restore minimized)
- **Right-click → Recent Profiles**:
  - View last 10 launched profiles
  - `●` = running → click to activate window
  - `○` = stopped → click to launch
- **Close main window**: Auto-minimize to tray (configurable)

## Local API & MCP (Automation)

When `api_port` is set to a non-zero value in config (default **38472**), Browsion runs a local HTTP API on `http://127.0.0.1:{port}` so that external tools (MCP clients, scripts, AI agents) can manage profiles and **control browsers**.

### HTTP API

**Profile CRUD:**
- `GET /api/profiles` — list profiles (includes `is_running` flag)
- `GET /api/profiles/:id` — get one profile
- `POST /api/profiles` — add profile (JSON body)
- `PUT /api/profiles/:id` — update profile (JSON body)
- `DELETE /api/profiles/:id` — delete profile

**Browser Lifecycle:**
- `POST /api/launch/:profile_id` — launch browser, returns `{ pid, cdp_port }`
- `POST /api/kill/:profile_id` — kill running browser
- `GET /api/running` — list all running browsers with PID, CDP port, launch time

**Navigation:**
- `POST /api/browser/:id/navigate` — `{ url, wait_until?, timeout_ms? }`
- `POST /api/browser/:id/navigate_wait` — `{ url, wait_until, timeout_ms }`
- `GET /api/browser/:id/url` — get current URL
- `GET /api/browser/:id/title` — get page title
- `POST /api/browser/:id/back` — go back
- `POST /api/browser/:id/forward` — go forward
- `POST /api/browser/:id/reload` — reload page
- `POST /api/browser/:id/wait_for_navigation` — `{ timeout_ms }`
- `POST /api/browser/:id/wait_for_url` — `{ pattern, timeout_ms }`

**Mouse:**
- `POST /api/browser/:id/click` — `{ selector }`
- `POST /api/browser/:id/hover` — `{ selector }`
- `POST /api/browser/:id/double_click` — `{ selector }`
- `POST /api/browser/:id/right_click` — `{ selector }`
- `POST /api/browser/:id/click_at` — `{ x, y }`
- `POST /api/browser/:id/drag` — `{ from_selector, to_selector }`

**Keyboard:**
- `POST /api/browser/:id/type` — `{ selector, text }`
- `POST /api/browser/:id/slow_type` — `{ selector, text, delay_ms }`
- `POST /api/browser/:id/press_key` — `{ key }`

**Forms:**
- `POST /api/browser/:id/select` — `{ selector, value }`
- `POST /api/browser/:id/upload` — `{ selector, file_path }`

**Scroll/Wait:**
- `POST /api/browser/:id/scroll` — `{ selector?, delta_x?, delta_y?, direction?, amount? }`
- `POST /api/browser/:id/scroll_element` — `{ selector, delta_x, delta_y }`
- `POST /api/browser/:id/scroll_into_view` — `{ selector }`
- `POST /api/browser/:id/wait_for` — `{ selector, timeout_ms }`
- `POST /api/browser/:id/wait_for_text` — `{ text, timeout_ms }`
- `POST /api/browser/:id/wait` — `{ ms }`

**Observe:**
- `GET /api/browser/:id/screenshot` — `?full_page=&format=&quality=` → base64 image
- `POST /api/browser/:id/screenshot_element` — `{ selector, format?, quality? }`
- `GET /api/browser/:id/page_state` — URL + title + AX tree with ref_ids
- `GET /api/browser/:id/ax_tree` — accessibility tree with ref_ids
- `GET /api/browser/:id/dom_context` — structured DOM with elements/forms/links
- `POST /api/browser/:id/extract` — `{ selectors: { key: "css" } }`
- `GET /api/browser/:id/page_text` — full `document.body.innerText`

**AX-Ref (semantic element interaction):**
- `POST /api/browser/:id/click_ref` — `{ ref_id }` — click by AX tree ref
- `POST /api/browser/:id/type_ref` — `{ ref_id, text }` — type by AX tree ref
- `POST /api/browser/:id/focus_ref` — `{ ref_id }` — focus by AX tree ref

**JavaScript:**
- `POST /api/browser/:id/evaluate` — `{ expression }` → JS result (awaits Promises)

**Tabs:**
- `GET /api/browser/:id/tabs` — list tabs
- `POST /api/browser/:id/tabs/new` — `{ url }` → new tab info
- `POST /api/browser/:id/tabs/switch` — `{ target_id }`
- `POST /api/browser/:id/tabs/close` — `{ target_id }`
- `POST /api/browser/:id/wait_for_new_tab` — `{ timeout_ms }` → target_id

**Cookies:**
- `GET /api/browser/:id/cookies` — get cookies
- `POST /api/browser/:id/cookies/set` — `{ name, value, domain, path }`
- `POST /api/browser/:id/cookies/clear` — delete all cookies

**Console:**
- `POST /api/browser/:id/console/enable` — start capturing console output
- `GET /api/browser/:id/console` — get recent console logs
- `POST /api/browser/:id/console/clear` — clear log buffer

**Network:**
- `GET /api/browser/:id/network_log` — request/response log
- `POST /api/browser/:id/network_log/clear` — clear log
- `POST /api/browser/:id/intercept/block` — `{ url_pattern }` — block URLs
- `POST /api/browser/:id/intercept/mock` — `{ url_pattern, status, body, content_type }` — mock response
- `POST /api/browser/:id/intercept/clear` — clear intercept rules

**Dialog:**
- `POST /api/browser/:id/dialog` — `{ accept, prompt_text? }` — handle alert/confirm/prompt

**Emulation:**
- `POST /api/browser/:id/emulate` — viewport, user-agent, geolocation

**Storage:**
- `GET /api/browser/:id/storage/:type` — get localStorage/sessionStorage
- `POST /api/browser/:id/storage/:type` — set item
- `DELETE /api/browser/:id/storage/:type` — clear storage

**Touch:**
- `POST /api/browser/:id/tap` — `{ selector }` — mobile tap
- `POST /api/browser/:id/swipe` — `{ selector, direction, distance }`

**PDF:**
- `GET /api/browser/:id/pdf` — `?landscape=&display_header_footer=…` → base64 PDF

**Frames:**
- `GET /api/browser/:id/frames` — list frames
- `POST /api/browser/:id/frames/switch` — `{ frame_id }`
- `POST /api/browser/:id/frames/main` — switch back to main frame

**Utility:**
- `GET /api/health` — health check

### MCP Server (browsion-mcp)

The **browsion-mcp** binary implements the [Model Context Protocol](https://modelcontextprotocol.io/) over stdio, exposing **73 tools** for AI agents to manage profiles and control browsers.

**Setup:**

1. Start Browsion (with `api_port > 0` in config, default 38472).
2. Build the MCP binary:

```bash
cd src-tauri && cargo build --release --bin browsion-mcp
```

3. Add to your MCP client config:

**Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "browsion": {
      "command": "/path/to/target/release/browsion-mcp"
    }
  }
}
```

**Cursor** (`.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "browsion": {
      "command": "/path/to/target/release/browsion-mcp"
    }
  }
}
```

Set `BROWSION_API_PORT` env var if you use a non-default port.
Set `BROWSION_API_KEY` env var if you configured an API key.

**Tools (73):**

| Category | Tools |
|---|---|
| Profile | `list_profiles`, `get_profile`, `create_profile`, `update_profile`, `delete_profile` |
| Lifecycle | `launch_browser`, `kill_browser`, `get_running_browsers` |
| Navigation | `navigate`, `go_back`, `go_forward`, `reload`, `wait_for_navigation`, `get_current_url`, `get_page_title` |
| Mouse | `click`, `hover`, `double_click`, `right_click`, `click_at`, `drag` |
| Keyboard | `type_text`, `slow_type`, `press_key` |
| Forms | `select_option`, `upload_file` |
| Scroll/Wait | `scroll`, `scroll_element`, `scroll_into_view`, `wait_for_element`, `wait_for_text`, `wait_for_url` |
| Observe | `get_page_state`, `get_ax_tree`, `screenshot`, `screenshot_element`, `get_dom_context`, `extract_data`, `get_page_text` |
| AX-Ref | `click_ref`, `type_ref`, `focus_ref` |
| JavaScript | `evaluate_js` |
| Tabs | `list_tabs`, `new_tab`, `switch_tab`, `close_tab`, `wait_for_new_tab` |
| Cookies | `get_cookies`, `set_cookie`, `delete_cookies` |
| Console | `enable_console_capture`, `get_console_logs`, `clear_console` |
| Network | `get_network_log`, `clear_network_log`, `block_url`, `mock_url`, `clear_intercepts` |
| Dialog | `handle_dialog` |
| Emulation | `emulate` |
| Storage | `get_storage`, `set_storage`, `clear_storage` |
| Touch | `tap`, `swipe` |
| PDF | `print_to_pdf` |
| Frames | `get_frames`, `switch_frame`, `main_frame` |
| Utility | `wait` |

### Recommended AI Agent Workflow

```
1. list_profiles          → find profile id
2. launch_browser         → start Chrome
3. navigate               → go to URL (waits for load)
4. get_page_state         → URL + title + AX tree with ref_ids
5. click_ref / type_ref   → interact via semantic refs (preferred over CSS selectors)
6. screenshot             → visual verification
7. kill_browser           → cleanup
```

### New Tab Workflow (for target="_blank" links)

```
1. wait_for_new_tab(timeout_ms=5000)   ← call BEFORE the click
2. click_ref("e5")                     ← click the link
3. switch_tab(target_id=<from step 1>) ← switch to new tab
4. get_page_state()                    ← observe new tab
```

## ⚙️ Configuration

Configuration file: `~/.config/browsion/config.toml`

```toml
# Browser: Chrome for Testing (auto-download) or custom path
# Configured via Settings in the app

api_port = 38472   # set to 0 to disable HTTP API

[settings]
auto_start = false
minimize_to_tray = true

[[profiles]]
id = "uuid-1234"
name = "US Profile"
description = "US proxy configuration"
user_data_dir = "/home/user/chrome_profiles/us"
proxy_server = "http://192.168.0.220:8889"
lang = "en-US"
timezone = "America/Los_Angeles"
fingerprint = "10000"
color = "#4A90E2"
custom_args = []
tags = ["work", "us-proxy"]
headless = false
```

## Documentation

- [docs/mcp-server-design.md](docs/mcp-server-design.md) - MCP server technical design
- [TRAY_IMPROVEMENTS.md](TRAY_IMPROVEMENTS.md) - Tray functionality details

## Troubleshooting

### App won't start
```bash
export WEBKIT_DISABLE_COMPOSITING_MODE=1
npm run tauri dev
```

### Browser won't launch
Set the correct Chrome path in Settings, or use Chrome for Testing (auto-download):
- Linux: `/usr/bin/google-chrome`
- Windows: `C:\Program Files\Google\Chrome\Application\chrome.exe`
- macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`

### Window activation not working (Linux)
```bash
sudo pacman -S wmctrl xdotool  # Arch/Manjaro
sudo apt install wmctrl xdotool  # Ubuntu/Debian
```

## Tech Stack

- **Backend**: Rust + Tauri 2.0
- **Frontend**: React 18 + TypeScript
- **Build**: Vite 5
- **Config**: TOML
- **CDP**: tokio-tungstenite (flatten mode, multi-session)
- **MCP**: rmcp 0.16 (official SDK, stdio transport)
- **HTTP API**: axum 0.7

## Project Structure

```
browsion/
├── src-tauri/                    # Rust backend
│   ├── src/agent/                # CDP client, session manager, types
│   │   ├── cdp.rs                # Full CDP WebSocket client (flatten mode)
│   │   ├── session.rs            # CDP connection pool (SessionManager)
│   │   └── types.rs              # DOMElement, AXNode, PageState, TabInfo, etc.
│   ├── src/api/mod.rs            # HTTP API (70+ endpoints)
│   ├── src/bin/browsion-mcp.rs   # MCP server binary (73 tools)
│   ├── src/config/               # Configuration schema & storage
│   ├── src/process/              # Process lifecycle + CDP port allocation
│   ├── src/window/               # Window activation
│   ├── src/tray/                 # System tray
│   └── src/state.rs              # AppState (config + ProcessManager + SessionManager)
├── src/                          # React frontend
│   ├── components/               # UI components
│   ├── api/                      # Tauri API wrapper
│   └── types/                    # TypeScript type definitions
├── docs/
│   └── mcp-server-design.md      # MCP server technical design document
└── package.json
```

## License

MIT License

---

**Made with Rust and Tauri**
