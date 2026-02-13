# Getting Started with Browsion

## ✅ Pre-Flight Checklist

Follow these steps to get Browsion up and running:

### 1. Verify Prerequisites

```bash
# Check Rust installation
rustc --version
# Should show: rustc 1.70+ or higher

# Check Node.js installation
node --version
# Should show: v18.0.0 or higher

# Check npm installation
npm --version
# Should show: 9.0.0 or higher
```

If any are missing:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js (via nvm)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 20
nvm use 20
```

### 2. Linux-Specific Setup

For window activation to work on Linux, install wmctrl:

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install wmctrl

# Arch Linux
sudo pacman -S wmctrl

# Fedora
sudo dnf install wmctrl
```

### 3. Install Project Dependencies

```bash
cd /home/percy/works/browsion
npm install
```

This will install:
- React and React DOM
- Tauri API packages
- TypeScript and type definitions
- Vite build tool
- UUID library

### 4. First Run (Development Mode)

```bash
npm run tauri dev
```

**What happens:**
1. Vite starts the development server (port 5173)
2. Rust code compiles (first time takes 5-10 minutes)
3. Application window opens
4. System tray icon appears

**First Launch Configuration:**
1. Click **Settings** tab
2. Set Chrome executable path:
   - Linux: `/usr/bin/google-chrome` or `/usr/bin/chromium`
   - macOS: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
   - Windows: `C:\Program Files\Google\Chrome\Application\chrome.exe`
3. Click **Save Chrome Path**
4. Optionally toggle:
   - "Auto-start on system boot" (not yet implemented)
   - "Minimize to tray when closing window"

### 5. Create Your First Profile

1. Click **Profiles** tab
2. Click **+ Add Profile** button
3. Fill in the form:

**Required Fields:**
- **Name**: Give it a descriptive name (e.g., "Work Profile")
- **User Data Directory**: Where Chrome stores this profile's data
  - Example: `/home/percy/chrome-profiles/work`
  - This directory will be created automatically by Chrome

**Optional Fields:**
- **Description**: Notes about this profile
- **Language**: Default language code (e.g., `en-US`, `zh-CN`)
- **Color**: Pick a color for easy identification
- **Proxy Server**: Full proxy URL
  - HTTP: `http://192.168.1.100:8080`
  - HTTPS: `https://proxy.example.com:3128`
  - SOCKS5: `socks5://localhost:1080`
- **Timezone**: IANA timezone (e.g., `America/New_York`, `Europe/London`)
- **Fingerprint**: Custom fingerprint identifier
- **Custom Arguments**: One Chrome flag per line
  - Example: `--disable-gpu`
  - Example: `--window-size=1920,1080`

4. Click **Save**

### 6. Test the Profile

1. Find your newly created profile in the list
2. Click **Launch** button
   - Chrome should start with your configured settings
   - Status changes to "● Running"
3. Click **Activate** button
   - Chrome window should come to foreground
4. Click **Kill** button
   - Chrome should close
   - Status returns to "○ Stopped"

### 7. System Tray Interaction

- **Left-click tray icon**: Show/hide main window
- **Right-click tray icon**: Menu options
  - Show Window
  - Quit
- **Close main window**: Minimizes to tray (if enabled in Settings)

## 🎨 Before Building for Production

### Replace Placeholder Icons

The current icons are just placeholder "B" letters. For production:

1. **Create or obtain a proper icon** (512x512 PNG recommended)

2. **Use an icon generator service:**
   - https://icon.kitchen/
   - https://iconverticons.com/
   - https://www.img2go.com/convert-to-ico

3. **Replace files in `src-tauri/icons/`:**
   - `icon.png` (512x512 source)
   - `32x32.png`
   - `128x128.png`
   - `128x128@2x.png` (256x256)
   - `icon.icns` (macOS)
   - `icon.ico` (Windows)

### Customize Branding

Edit these files to match your brand:
- `src-tauri/tauri.conf.json` - Update productName, identifier
- `src/styles/index.css` - Customize colors in `:root` variables
- `README.md` - Update description and author info

## 🏗️ Building for Production

### Linux Build

```bash
npm run tauri build
```

Output locations:
- **DEB package**: `src-tauri/target/release/bundle/deb/browsion_0.1.0_amd64.deb`
- **AppImage**: `src-tauri/target/release/bundle/appimage/browsion_0.1.0_amd64.AppImage`
- **Binary**: `src-tauri/target/release/browsion`

### macOS Build (requires macOS)

```bash
npm run tauri build
```

Output:
- **DMG**: `src-tauri/target/release/bundle/dmg/Browsion_0.1.0_x64.dmg`
- **App**: `src-tauri/target/release/bundle/macos/Browsion.app`

### Windows Build (requires Windows)

```bash
npm run tauri build
```

Output:
- **MSI**: `src-tauri/target/release/bundle/msi/Browsion_0.1.0_x64_en-US.msi`
- **EXE**: `src-tauri/target/release/browsion.exe`

## 🧪 Testing Scenarios

### Scenario 1: Basic Profile
```
Name: Basic Test
User Data Dir: /tmp/browsion-test
Language: en-US
```
Expected: Chrome launches with English UI

### Scenario 2: Proxy Profile
```
Name: Proxy Test
User Data Dir: /tmp/browsion-proxy
Proxy Server: http://localhost:8080
```
Expected: Chrome routes traffic through proxy

### Scenario 3: Multi-Profile
```
1. Create 3 different profiles
2. Launch all 3
3. Verify all show as "Running"
4. Activate each one
5. Kill all
```
Expected: All operations work independently

## 📁 Configuration File Location

The configuration is automatically saved to:

```bash
# Linux
~/.config/browsion/config.toml

# macOS
~/Library/Application Support/com.browsion.app/config.toml

# Windows
%APPDATA%\browsion\config.toml
```

You can manually edit this file. Format:
```toml
chrome_path = "/usr/bin/google-chrome"

[settings]
auto_start = false
minimize_to_tray = true

[[profiles]]
id = "unique-uuid-here"
name = "My Profile"
description = "Description here"
user_data_dir = "/path/to/profile"
proxy_server = "http://proxy:8080"
lang = "en-US"
timezone = "America/New_York"
fingerprint = "12345"
color = "#3498db"
custom_args = ["--disable-gpu", "--no-sandbox"]
```

## 🐛 Common Issues & Solutions

### Issue: "Chrome executable not found"
**Solution:**
1. Verify Chrome is installed: `which google-chrome`
2. Update path in Settings
3. Restart the app

### Issue: "Window activation doesn't work" (Linux)
**Solution:**
```bash
sudo apt install wmctrl
# or
sudo apt install xdotool
```

### Issue: Port 5173 already in use
**Solution:**
```bash
# Kill process on port 5173
lsof -ti:5173 | xargs kill -9
# Or change port in vite.config.ts
```

### Issue: Rust compilation fails
**Solution:**
```bash
# Update Rust
rustup update

# Clean and rebuild
cd src-tauri
cargo clean
cd ..
npm run tauri dev
```

### Issue: TypeScript errors
**Solution:**
```bash
# Reinstall dependencies
rm -rf node_modules package-lock.json
npm install
```

## 📚 Further Reading

- **Full Documentation**: See [README.md](README.md)
- **Implementation Details**: See [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md)
- **Tauri Docs**: https://tauri.app/
- **React Docs**: https://react.dev/

## 🎯 Quick Command Reference

```bash
# Development
npm run tauri dev        # Run in dev mode
npm run dev              # Frontend only (no Tauri)

# Building
npm run build            # Build frontend
npm run tauri build      # Build entire app

# Utilities
cargo test              # Run Rust tests (in src-tauri/)
npm run preview         # Preview production build

# Debugging
RUST_LOG=debug npm run tauri dev    # Verbose Rust logs
```

## 🚀 You're Ready!

If you've completed all the steps above, you now have:
- ✅ A working Browsion installation
- ✅ Chrome path configured
- ✅ At least one test profile created
- ✅ Understanding of how to launch and manage browsers
- ✅ Knowledge of where config is stored
- ✅ Ability to build for production

**Next:** Create profiles for your actual use cases and start managing your Chrome instances!

## 💡 Tips

1. **User Data Directory Naming**: Use descriptive paths like:
   - `/home/user/chrome-profiles/work`
   - `/home/user/chrome-profiles/personal`
   - `/home/user/chrome-profiles/testing`

2. **Color Coding**: Use colors consistently:
   - Blue for development
   - Green for production
   - Red for testing
   - Yellow for staging

3. **Custom Arguments**: Useful flags:
   - `--incognito` - Private browsing
   - `--disable-gpu` - Software rendering
   - `--window-size=1920,1080` - Set window size
   - `--disable-web-security` - CORS bypass (dev only!)
   - `--user-agent="..."` - Custom user agent

4. **Backup Config**: Periodically backup your config file:
   ```bash
   cp ~/.config/browsion/config.toml ~/browsion-backup-$(date +%Y%m%d).toml
   ```

Happy browsing! 🎉
