# Changelog

All notable changes to this project will be documented in this file.

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
