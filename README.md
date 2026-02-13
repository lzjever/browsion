# Browsion

**Cross-platform Browser Launcher - Manage multiple Chrome profiles with ease**

## Features

- **Cross-platform**: Windows, macOS, Linux
- **System Tray**:
  - Single/double-click to open main window
  - Right-click menu for quick access to recent profiles
  - Auto display running status (● running / ○ stopped)
- **Profile Management**: Add, edit, delete, clone profiles
- **Tags System**: Categorize and filter profiles by tags
- **Smart Forms**:
  - Language field with 100+ locale suggestions (ISO 639-1)
  - Timezone field with all IANA timezones (~400+)
  - Both support manual input or dropdown selection
- **One-click Launch**: Start pre-configured browser instances
- **Process Tracking**: Real-time monitoring of browser status
- **Window Activation**: Quick switch to running browsers
- **Flexible Config**: Proxy, timezone, language, fingerprint settings
- **Recent Records**: Auto-track last 10 launched profiles

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

## Usage

### Basic Operations

1. **Launch App**: Run `npm run tauri dev`
2. **Set Chrome Path**: Settings → Set Chrome path
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

## ⚙️ 配置示例

配置文件: `~/.config/browsion/config.toml`

```toml
chrome_path = "/usr/bin/google-chrome"

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
```

## Documentation

- [CHANGELOG.md](CHANGELOG.md) - Version history
- [TRAY_IMPROVEMENTS.md](TRAY_IMPROVEMENTS.md) - Tray functionality details

## Troubleshooting

### App won't start
```bash
export WEBKIT_DISABLE_COMPOSITING_MODE=1
npm run tauri dev
```

### Browser won't launch
Set the correct Chrome path in Settings:
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

## Project Structure

```
browsion/
├── src-tauri/          # Rust backend
│   ├── src/config/     # Configuration
│   ├── src/process/    # Process management
│   ├── src/window/     # Window activation
│   └── src/tray/       # System tray
├── src/                # React frontend
│   ├── components/     # UI components
│   ├── api/            # API wrapper
│   └── types/          # Type definitions
└── package.json
```

## License

MIT License

---

**Made with Rust and Tauri**
