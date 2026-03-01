# E2E Browser Tests

This directory contains end-to-end browser tests that launch a real Chrome browser in headless mode and exercise the full CDP functionality.

## Testid Naming Standard

All tests should follow the pattern: `test_<category>_<operation>_<variant>`

### Categories

- `navigate` - Navigation operations (navigate, go_back, go_forward, reload, wait_for_url)
- `mouse` - Mouse operations (click, hover, double_click, right_click, click_at, drag)
- `keyboard` - Keyboard input (type_text, slow_type, press_key)
- `form` - Form interactions (select_option, upload_file)
- `axref` - AX-tree reference operations (click_ref, type_ref, focus_ref)
- `tabs` - Tab management (list_tabs, new_tab, switch_tab, close_tab, wait_for_new_tab)
- `cookies` - Cookie CRUD (set_cookie, get_cookies, delete_cookies, export_cookies, import_cookies)
- `storage` - Web storage (set_storage, get_storage, remove_storage, clear_storage)
- `console` - Console capture (enable_console_capture, get_console_logs, clear_console)
- `network` - Network operations (get_network_log, clear_network_log, block_url, mock_url, clear_intercepts)
- `screenshot` - Captures (screenshot, screenshot_element)
- `profile` - Profile management (create_profile, update_profile, delete_profile, list_profiles)
- `lifecycle` - Browser lifecycle (launch_browser, kill_browser, get_running_browsers)
- `snapshot` - Profile snapshots (create_snapshot, restore_snapshot, delete_snapshot, list_snapshots)
- `emulate` - Device emulation (emulate_viewport, emulate_mobile)
- `touch` - Touch operations (tap, swipe)
- `frames` - iframe handling (get_frames, switch_frame, main_frame)
- `dialog` - Dialog handling (handle_dialog)
- `workflow` - Workflow automation (create_workflow, run_workflow)
- `recording` - Recording sessions (start_recording, stop_recording, recording_to_workflow)

## Running Tests

```bash
# Run all E2E tests
cargo test --test e2e_browser_test -- --test-threads=1

# Run specific test
cargo test --test e2e_browser_test -- test test_mouse_hover_element -- --test-threads=1
```

## Test Organization

Tests are numbered sequentially as they are added (test_01, test_02, etc.) but should use descriptive testid naming for clarity.

## Chrome Binary

Tests require Chrome to be available. Discovery order:
1. `CHROME_PATH` environment variable
2. Common platform paths (Linux/macOS/Windows)
3. PATH lookup

## Isolation

Tests run with `--test-threads=1` to avoid port conflicts.

## Current Coverage

- **Total tests**: 47
- All tests follow standardized `test_<category>_<operation>_<variant>` naming
- Coverage includes: navigate, mouse, keyboard, form, axref, tabs, cookies, storage, console, network, screenshot, profile, lifecycle, snapshot, emulate, touch, frames, dialog, workflow, recording
