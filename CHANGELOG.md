# Changelog

All notable changes to this project will be documented in this file.

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
