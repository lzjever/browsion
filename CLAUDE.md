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
cd src-tauri && cargo test --lib            # Unit tests (26 tests)

# Production build
npm run tauri build
```

**Cursor terminal note**: Cursor sets `ARGV0` which breaks rustup's proxy. Use `./scripts/cargo` wrapper or run `unset ARGV0` first.

## Architecture Overview

**Stack**: Tauri 2 (Rust backend + React/TypeScript frontend)

**Browsion is a Browser Profile Manager + CDP Port Exposer**

### Two Components
1. **Tauri App** (`src-tauri/src/`) — Desktop app with system tray, profile management, HTTP API server
2. **Frontend** (`src/`) — React UI for profile management and settings

### Key Backend Modules (`src-tauri/src/`)

| Module | Purpose |
|--------|---------|
| `api/mod.rs` | HTTP API router (20 endpoints) |
| `api/lifecycle.rs` | Browser launch/kill handlers, returns CDP port |
| `api/ws.rs` | WebSocket for real-time browser status events |
| `config/schema.rs` | BrowserProfile, AppConfig, ProxyPreset, SnapshotInfo |
| `process/launcher.rs` | Chrome launch with flags, proxy, timezone, fingerprint |
| `process/manager.rs` | Process tracking, CDP port allocation, cleanup |
| `state.rs` | AppState — shared state with config, process_manager |

### HTTP API (20 endpoints)

| Category | Endpoints |
|----------|-----------|
| Profiles | `GET/POST/PUT/DELETE /api/profiles`, `GET/POST /api/profiles/:id/snapshots`, `POST .../snapshots/:name/restore`, `DELETE .../snapshots/:name` |
| Lifecycle | `POST /api/launch/:profile_id` (returns `{"pid": 123, "cdp_port": 9222}`), `POST /api/kill/:profile_id`, `POST /api/register-external`, `GET /api/running` |
| Settings | `GET/PUT /api/settings`, `GET/PUT /api/browser-source`, `GET/PUT /api/local-api` |
| WebSocket | `GET /api/ws` — real-time `browser-status-changed`, `profiles-changed` events |
| Health | `GET /api/health` |

### Frontend Structure (`src/`)

- `components/ProfileList.tsx` — Main profile list with CRUD, launch/kill, real-time status
- `components/ProfileForm.tsx` — Profile edit dialog with proxy presets dropdown
- `components/Settings.tsx` — Browser source, proxy presets, local API configuration
- `api/tauri.ts` — Tauri command wrappers
- `types/profile.ts` — TypeScript types matching Rust schema

## Using Browsion

1. Create a profile with a user data directory
2. Launch the profile — Browsion starts Chrome with `--remote-debugging-port=XXXX`
3. Connect to Chrome via CDP at `http://127.0.0.1:XXXX` (returned by launch API)
4. Use any CDP client (Puppeteer, Playwright, custom WebSocket) to control the browser
5. Kill the browser when done

## Browser Status Tracking

The `ProcessManager.is_running()` function uses:
- **5-second grace period** for newly launched processes (avoids sysinfo timing issues)
- **Full process refresh** (`ProcessesToUpdate::All`) to ensure newly launched processes are visible
- **Process verification** — checks PID exists, process name contains "chrome", and not a zombie

## Testing

- Unit tests: `cargo test --lib` (26 tests)
- Tests cover config validation, process management, proxy presets, launcher args
