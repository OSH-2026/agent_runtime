# Android ActionFlow Workflows

These workflows use action names and input schemas registered by the Kotlin Android
Action Runtime. Paste a YAML file into the Tauri Workflow Android app and point it
at the runtime's gRPC endpoint (normally `127.0.0.1:8080` on the same device).

## Included workflows

- `device-health-report.yaml`: collects device, system, network, battery, and
  storage data in parallel, then creates and copies a combined report.
- `travel-preflight.yaml`: checks travel-related permissions, location,
  connectivity, battery, and calendar data before opening a map, note, and email.
- `communication-digest.yaml`: collects recent messages, calls, notifications,
  and matching contacts, then builds a note, clipboard copy, and email draft.
- `incident-evidence-capture.yaml`: captures a photo, short audio recording, and
  screenshot together with device context, then creates an evidence manifest.

## Runtime notes

- Android permissions and notification-listener/MediaProjection grants must be
  enabled before actions that require them can complete.
- Replace placeholder email addresses, contact queries, destinations, and
  calendar timestamps before a real demonstration.
- `requiresConfirmation: true` is intentionally absent because the current
  dispatcher can move a node to `WaitingHuman`, but the demo app has no resume
  control yet.
- References currently create dependencies and substitute the entire upstream
  JSON payload. A reference such as `${location.latitude}` still resolves to the
  whole `location` output, so these examples use references in text reports.
