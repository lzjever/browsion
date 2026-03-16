# Local API

Browsion exposes a local HTTP API on `127.0.0.1` for profile management, browser control, recording, and playback.

For the product-facing API boundary, stability levels, and build plan, see [local-api-inventory.md](./local-api-inventory.md).

## Base URL

```text
http://127.0.0.1:38472
```

If an API key is configured, add:

```bash
-H "X-API-Key: <your-key>"
```

## Response shape

Stable endpoints are being normalized to a common envelope.

Success responses include:

```json
{
  "ok": true
}
```

and may also include endpoint-specific fields such as:

```json
{
  "ok": true,
  "url": "http://httpbin.org/html",
  "title": "httpbin.org"
}
```

List endpoints are being normalized to named collections instead of bare arrays:

```json
{
  "ok": true,
  "profiles": []
}
```

```json
{
  "ok": true,
  "browsers": []
}
```

Error responses are being normalized to:

```json
{
  "ok": false,
  "error": {
    "code": "profile_not_found",
    "message": "Profile not found",
    "status": 404
  }
}
```

## Health

```bash
curl http://127.0.0.1:38472/api/health
```

## Profiles

List profiles:

```bash
curl -H "X-API-Key: <your-key>" http://127.0.0.1:38472/api/profiles
```

Create a profile:

```bash
curl -X POST http://127.0.0.1:38472/api/profiles \
  -H "Content-Type: application/json" \
  -H "X-API-Key: <your-key>" \
  -d '{
    "id": "google-demo",
    "name": "Google Demo",
    "description": "",
    "user_data_dir": "/tmp/browsion-google-demo",
    "lang": "en-US",
    "tags": [],
    "custom_args": []
  }'
```

## Browser lifecycle

Launch:

```bash
curl -X POST -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/launch/google-demo
```

Kill:

```bash
curl -X POST -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/kill/google-demo
```

List running browsers:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/running
```

## Browser control

Navigate:

```bash
curl -X POST http://127.0.0.1:38472/api/browser/google-demo/navigate \
  -H "Content-Type: application/json" \
  -H "X-API-Key: <your-key>" \
  -d '{"url":"https://www.google.com"}'
```

Click:

```bash
curl -X POST http://127.0.0.1:38472/api/browser/google-demo/click \
  -H "Content-Type: application/json" \
  -H "X-API-Key: <your-key>" \
  -d '{"selector":"textarea[name=q]"}'
```

Type:

```bash
curl -X POST http://127.0.0.1:38472/api/browser/google-demo/type \
  -H "Content-Type: application/json" \
  -H "X-API-Key: <your-key>" \
  -d '{"selector":"textarea[name=q]","text":"browsion"}'
```

Press Enter:

```bash
curl -X POST http://127.0.0.1:38472/api/browser/google-demo/press_key \
  -H "Content-Type: application/json" \
  -H "X-API-Key: <your-key>" \
  -d '{"key":"Enter"}'
```

List tabs:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/browser/google-demo/tabs
```

## Recording

Start recording:

```bash
curl -X POST -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings/start/google-demo
```

Check recording status:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/profiles/google-demo/recording-status
```

Stop recording:

```bash
curl -X POST -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings/stop/<session-id>
```

List saved recordings:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings
```

Get one recording:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings/<recording-id>
```

Delete a recording:

```bash
curl -X DELETE -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings/<recording-id>
```

Play a recording on a running profile:

```bash
curl -X POST -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings/<recording-id>/play/google-demo
```

## WebSocket events

Browsion also exposes a WebSocket endpoint for real-time status and playback progress:

```text
ws://127.0.0.1:38472/api/ws
```

If an API key is configured, pass it as a query parameter:

```text
ws://127.0.0.1:38472/api/ws?api_key=your-secret
```

Current event types include:

- `BrowserStatusChanged`
- `ActionLogEntry`
- `ProfilesChanged`
- `RecordingPlaybackProgress`
- `Heartbeat`

### Playback progress payload

When a recording is being played, you will receive events shaped like:

```json
{
  "type": "RecordingPlaybackProgress",
  "data": {
    "recording_id": "rec-123",
    "profile_id": "google-demo",
    "action_index": 3,
    "total_actions": 8,
    "action_type": "click",
    "status": "running",
    "error": null
  }
}
```

`status` can be:

- `running`
- `failed`
- `completed`

### Minimal Node listener

Node 22+ includes a built-in WebSocket client. This example listens for playback progress:

```bash
node --input-type=module <<'EOF'
const ws = new WebSocket('ws://127.0.0.1:38472/api/ws');

ws.onopen = () => {
  console.log('connected');
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.type === 'RecordingPlaybackProgress') {
    console.log(message.data);
  }
};

ws.onclose = () => {
  console.log('closed');
};
EOF
```

### HTTP + WebSocket together

Terminal 1, listen for progress:

```bash
node --input-type=module <<'EOF'
const ws = new WebSocket('ws://127.0.0.1:38472/api/ws');
ws.onmessage = (event) => console.log(event.data);
EOF
```

Terminal 2, start playback:

```bash
curl -X POST -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/recordings/<recording-id>/play/google-demo
```

## App settings

Read app settings:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/settings
```

Update app settings:

```bash
curl -X PUT http://127.0.0.1:38472/api/settings \
  -H "Content-Type: application/json" \
  -H "X-API-Key: <your-key>" \
  -d '{"auto_start":false,"minimize_to_tray":true}'
```

Read browser source:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/browser-source
```

Read local API config:

```bash
curl -H "X-API-Key: <your-key>" \
  http://127.0.0.1:38472/api/local-api
```
