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

## Local API

Browsion exposes a local HTTP API on `http://127.0.0.1:{port}` for profile management, browser control, recording, and playback.

Common endpoints:
- `GET /api/profiles`
- `POST /api/launch/:profile_id`
- `POST /api/browser/:id/navigate`
- `POST /api/browser/:id/click`
- `POST /api/browser/:id/type`
- `POST /api/recordings/start/:profile_id`
- `POST /api/recordings/:id/play/:profile_id`
- `GET /api/settings`
- `GET /api/browser-source`
- `GET /api/local-api`

Full curl examples and endpoint notes live in [docs/local-api.md](docs/local-api.md).
Real-time WebSocket examples for playback progress are also documented there.
The product-facing API boundary and stability classification live in [docs/local-api-inventory.md](docs/local-api-inventory.md).

### Recommended automation flow

```text
1. GET /api/profiles
2. POST /api/launch/:profile_id
3. POST /api/browser/:id/navigate
4. GET /api/browser/:id/page_state
5. POST /api/browser/:id/click or /type
6. POST /api/recordings/start/:profile_id
7. POST /api/recordings/:id/play/:profile_id
```

## ⚙️ Configuration

Configuration file: `~/.config/browsion/config.toml`

```toml
# Browser: Chrome for Testing (auto-download) or custom path
# Configured via Settings in the app

[mcp]
enabled = true
api_port = 38472
# api_key = "optional-secret"

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

- [docs/local-api.md](docs/local-api.md) - Local API curl usage
- [docs/local-api-inventory.md](docs/local-api-inventory.md) - Local API product contract and stability levels
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
- **HTTP API**: axum 0.7

## Project Structure

```
browsion/
├── src-tauri/                    # Rust backend
│   ├── src/agent/                # CDP client, session manager, types
│   │   ├── cdp.rs                # Full CDP WebSocket client (flatten mode)
│   │   ├── session.rs            # CDP connection pool (SessionManager)
│   │   └── types.rs              # DOMElement, AXNode, PageState, TabInfo, etc.
│   ├── src/api/mod.rs            # HTTP API
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
│   ├── local-api.md              # curl-based local API guide
│   ├── local-api-inventory.md    # API contract, stability levels, build plan
└── package.json
```

## License

MIT License

---

**Made with Rust and Tauri**
