# Browsion - Quick Start Guide

## Getting Started

### Prerequisites

Install the required tools:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js (via nvm recommended)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 20
nvm use 20

# Linux only: Install wmctrl for window activation
sudo apt install wmctrl  # Ubuntu/Debian
sudo pacman -S wmctrl    # Arch Linux
```

### Installation

1. **Install dependencies**:
```bash
npm install
```

2. **Run in development mode**:
```bash
npm run tauri dev
```

This will:
- Start the Vite dev server (frontend)
- Compile the Rust backend
- Launch the application

### First Time Setup

1. When the app launches, go to **Settings**
2. Set the **Chrome Executable Path**:
   - Linux: `/usr/bin/google-chrome` or `/usr/bin/chromium`
   - macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
   - Windows: `C:\Program Files\Google\Chrome\Application\chrome.exe`
3. Click **Save Chrome Path**

### Creating Your First Profile

1. Click **Add Profile**
2. Fill in the required fields:
   - **Name**: A friendly name (e.g., "Work Profile")
   - **User Data Directory**: Where Chrome stores profile data
     - Example: `/home/percy/chrome-profiles/work`
3. Optional fields:
   - **Proxy Server**: `http://proxy.example.com:8080`
   - **Language**: `en-US`
   - **Timezone**: `America/New_York`
   - **Fingerprint**: Custom fingerprint ID
   - **Color**: Visual color tag for the profile
4. Click **Save**

### Using Profiles

- **Launch**: Start a new browser instance with this profile
- **Activate**: Focus an already running browser window
- **Kill**: Terminate the running browser instance
- **Edit**: Modify profile settings
- **Delete**: Remove the profile (only when not running)

### System Tray

The app runs in the system tray:
- **Click tray icon**: Show/hide the main window
- **Right-click**: Access menu (Show Window, Quit)
- **Close window**: Minimizes to tray (configurable in Settings)

## Building for Production

```bash
npm run tauri build
```

Output will be in:
- **Linux**: `src-tauri/target/release/bundle/deb/` or `src-tauri/target/release/bundle/appimage/`
- **macOS**: `src-tauri/target/release/bundle/dmg/`
- **Windows**: `src-tauri/target/release/bundle/msi/`

## Common Issues

### "Chrome executable not found"
- Verify the path in Settings is correct
- Make sure Chrome is actually installed
- On Linux, try `which google-chrome` to find the path

### Window activation doesn't work (Linux)
- Install wmctrl: `sudo apt install wmctrl`
- Or install xdotool: `sudo apt install xdotool`

### Build errors
```bash
# Clean and rebuild
cd src-tauri
cargo clean
cd ..
npm run tauri build
```

### Port 5173 already in use
```bash
# Kill the process using the port
lsof -ti:5173 | xargs kill -9
```

## Development Tips

### Hot Reload

- **Frontend changes**: Auto-reload (Vite HMR)
- **Rust changes**: Requires recompile (automatic in dev mode)

### Debugging

**Rust backend**:
```bash
RUST_LOG=debug npm run tauri dev
```

**Frontend**:
- Open DevTools in the app window (Ctrl+Shift+I / Cmd+Option+I)

### Configuration File

Manually edit the config file for advanced settings:
```bash
# Linux
nano ~/.config/browsion/config.toml

# macOS
nano ~/Library/Application\ Support/com.browsion.app/config.toml

# Windows
notepad %APPDATA%\browsion\config.toml
```

## Example TOML Configuration

```toml
chrome_path = "/usr/bin/google-chrome"

[settings]
auto_start = false
minimize_to_tray = true

[[profiles]]
id = "550e8400-e29b-41d4-a716-446655440000"
name = "Development"
description = "Local development profile"
user_data_dir = "/home/percy/chrome-dev"
lang = "en-US"
custom_args = ["--disable-web-security"]

[[profiles]]
id = "550e8400-e29b-41d4-a716-446655440001"
name = "Testing"
description = "Testing with US proxy"
user_data_dir = "/home/percy/chrome-test"
proxy_server = "http://192.168.1.100:8080"
lang = "en-US"
timezone = "America/New_York"
fingerprint = "test-001"
color = "#3498db"
custom_args = []
```

## Next Steps

- Read the full [README.md](README.md) for detailed information
- Check the implementation plan in your project documentation
- Customize the UI styling in `src/styles/index.css`
- Add custom Chrome arguments as needed for your use case

## Support

If you encounter issues:
1. Check the application logs (console output)
2. Verify your configuration file is valid TOML
3. Ensure Chrome is properly installed and accessible
4. For window activation issues on Linux, verify wmctrl/xdotool is installed
