# Local API Inventory

This document defines the intended product-facing contract for Browsion's local API.

Design rule:

- Browsion Local API is the primary control plane.
- CDP is a low-level escape hatch, not the default integration surface.

Stability levels:

- `stable`: recommended for agent use, curl use, docs, and long-lived integrations
- `experimental`: available but semantics may change as the product model is tightened
- `internal`: keep for internal UI/testing/debug use only; do not recommend externally

## Control Plane Boundary

Use the Local API for:

- profile lifecycle
- browser lifecycle
- common browser actions
- tabs
- recording / playback
- settings
- observability and progress events

Use raw CDP only for:

- debugging Local API bugs
- accessing Chrome features that are not yet wrapped
- temporary advanced automation during API gaps

## Stable Surface

### Health and configuration

- `GET /api/health`
- `GET /api/settings`
- `PUT /api/settings`
- `GET /api/browser-source`
- `PUT /api/browser-source`
- `GET /api/local-api`
- `PUT /api/local-api`

### Profiles

- `GET /api/profiles`
- `POST /api/profiles`
- `GET /api/profiles/:id`
- `PUT /api/profiles/:id`
- `DELETE /api/profiles/:id`
- `GET /api/running`

### Browser lifecycle

- `POST /api/launch/:profile_id`
- `POST /api/kill/:profile_id`
- `POST /api/register-external`

### Core browser actions

- `POST /api/browser/:id/navigate`
- `POST /api/browser/:id/navigate_wait`
- `GET /api/browser/:id/url`
- `GET /api/browser/:id/title`
- `POST /api/browser/:id/back`
- `POST /api/browser/:id/forward`
- `POST /api/browser/:id/reload`
- `POST /api/browser/:id/click`
- `POST /api/browser/:id/hover`
- `POST /api/browser/:id/double_click`
- `POST /api/browser/:id/right_click`
- `POST /api/browser/:id/type`
- `POST /api/browser/:id/slow_type`
- `POST /api/browser/:id/press_key`
- `POST /api/browser/:id/select_option`
- `POST /api/browser/:id/upload_file`
- `POST /api/browser/:id/scroll`
- `POST /api/browser/:id/scroll_into_view`
- `POST /api/browser/:id/wait_for`
- `POST /api/browser/:id/wait_for_text`
- `POST /api/browser/:id/wait_for_url`
- `GET /api/browser/:id/page_text`
- `POST /api/browser/:id/evaluate`
- `POST /api/browser/:id/extract`
- `GET /api/browser/:id/screenshot`
- `GET /api/browser/:id/screenshot_element`

### Tabs

- `GET /api/browser/:id/tabs`
- `POST /api/browser/:id/tabs/new`
- `POST /api/browser/:id/tabs/switch`
- `POST /api/browser/:id/tabs/close`
- `POST /api/browser/:id/tabs/wait_new`

### Recording and playback

- `GET /api/recordings`
- `POST /api/recordings`
- `GET /api/recordings/:id`
- `DELETE /api/recordings/:id`
- `POST /api/recordings/start/:profile_id`
- `POST /api/recordings/stop/:session_id`
- `GET /api/profiles/:id/recording-status`
- `POST /api/recordings/:id/play/:profile_id`

### Observability

- `GET /api/browser/:id/console`
- `POST /api/browser/:id/console/enable`
- `POST /api/browser/:id/console/clear`
- `GET /api/browser/:id/network_log`
- `POST /api/browser/:id/network_log/clear`
- `GET /api/action_log`
- `DELETE /api/action_log`
- `GET /api/ws`

### Storage and cookies

- `GET /api/browser/:id/storage`
- `POST /api/browser/:id/storage`
- `DELETE /api/browser/:id/storage`
- `GET /api/browser/:id/cookies`
- `POST /api/browser/:id/cookies/set`
- `POST /api/browser/:id/cookies/clear`
- `GET /api/browser/:id/cookies/export`
- `POST /api/browser/:id/cookies/import`

## Experimental Surface

These are useful, but their long-term API shape is not settled yet.

### AI-oriented page models

- `GET /api/browser/:id/dom_context`
- `GET /api/browser/:id/ax_tree`
- `GET /api/browser/:id/page_state`
- `POST /api/browser/:id/click_ref`
- `POST /api/browser/:id/type_ref`
- `POST /api/browser/:id/focus_ref`

Reason:

- useful for agent context gathering
- likely needs a more explicit schema contract
- may evolve into a separate "page model" surface

### Device and advanced interaction controls

- `POST /api/browser/:id/emulate`
- `POST /api/browser/:id/click_at`
- `POST /api/browser/:id/drag`
- `POST /api/browser/:id/tap`
- `POST /api/browser/:id/swipe`
- `POST /api/browser/:id/scroll_element`

Reason:

- powerful but less commonly needed
- needs more real-world compatibility testing

### Frames, intercepts, PDF, dialogs

- `GET /api/browser/:id/frames`
- `POST /api/browser/:id/switch_frame`
- `POST /api/browser/:id/main_frame`
- `POST /api/browser/:id/intercept/block`
- `POST /api/browser/:id/intercept/mock`
- `DELETE /api/browser/:id/intercept`
- `GET /api/browser/:id/pdf`
- `POST /api/browser/:id/handle_dialog`
- `POST /api/browser/:id/wait_for_nav`

Reason:

- these work as advanced controls
- semantics and product framing are not yet fully normalized

### Snapshots

- `GET /api/profiles/:id/snapshots`
- `POST /api/profiles/:id/snapshots`
- `POST /api/profiles/:id/snapshots/:name/restore`
- `DELETE /api/profiles/:id/snapshots/:name`

Reason:

- useful but not yet central to the simplified product direction

## Internal Surface

These should not be documented as first-class external controls until their role is re-validated.

- legacy command aliases retained for compatibility
- any UI-only orchestration still happening outside the backend
- direct CDP assumptions leaking through test-only or debug-only flows

## Product Gaps

The API is already strong enough to act as the main control plane, but it is not yet product-complete.

### Gap 1: result model normalization

Current issue:

- different endpoints return different shapes (`ok`, `url`, `title`, raw arrays, wrapped objects)

Target:

- define a standard success envelope for action endpoints
- define a standard error envelope with machine-usable codes

### Gap 2: waiting semantics

Current issue:

- some actions encode waiting internally
- others require separate follow-up calls

Target:

- support a consistent wait contract on major action endpoints
- document recommended wait patterns for agent callers

### Gap 3: observability depth

Current issue:

- playback progress exists
- console/network exist
- step trace and failure diagnostics are still thin

Target:

- add structured action trace events
- add failure snapshots and richer playback diagnostics

### Gap 4: real-site robustness

Current issue:

- behavior is strong on deterministic fixtures
- live-site behavior still depends on redirects, timing, protocol quirks, and page-specific differences

Target:

- keep growing manual external smoke coverage
- harden screenshots, waits, tab activation, and redirect handling

### Gap 5: contract clarity

Current issue:

- Local API and CDP both exist, but the product contract is only implicit

Target:

- explicitly document Local API as the official contract
- explicitly document CDP as advanced and best-effort

## Immediate Build Plan

### Phase A: API contract cleanup

1. Inventory every endpoint and assign `stable`, `experimental`, or `internal`
2. Align docs and UI wording to this classification
3. Mark CDP as low-level and non-primary

### Phase B: response model cleanup

1. Normalize action endpoint responses
2. Normalize error codes and messages
3. Add consistency tests for response shape

### Phase C: waiting and execution model

1. Add a unified wait contract for major actions
2. Reduce the need for client-side choreography
3. Make playback and direct browser actions share more execution primitives

### Phase D: observability

1. Extend playback trace data
2. Add richer action-level diagnostics
3. Expose a clearer operator story through WebSocket and HTTP

### Phase E: real-world hardening

1. Expand manual external smoke tests
2. Expand deterministic e2e around redirects, tabs, and history
3. Clean up process/session residue handling

## Delivery Standard

Treat the Local API as product-grade only when all of the following are true:

- common agent tasks do not require direct CDP
- errors can be diagnosed from Local API outputs alone
- major browser flows pass deterministic e2e
- major browser flows pass manual external smoke
- documentation matches real behavior
