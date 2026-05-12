# Alchemist Mac API Contract Checklist

The SwiftUI prototype targets the current `/api/v1` router. The legacy `/api`
routes still exist, but native route strings must stay centralized in
`AlchemistAPIClient` so compatibility policy changes do not touch view code.

## Current Routes Used By Prototype

- `POST /api/v1/auth/login`
- `POST /api/v1/auth/logout`
- `GET /api/v1/stats`
- `GET /api/v1/stats/daily`
- `GET /api/v1/stats/savings`
- `GET /api/v1/engine/status`
- `POST /api/v1/engine/pause`
- `POST /api/v1/engine/resume`
- `GET /api/v1/jobs?limit=&page=&sort=&sort_by=&sort_desc=&archived=&status=&search=`
- `GET /api/v1/jobs/:id/details`
- `POST /api/v1/jobs/batch`
- `POST /api/v1/jobs/restart-failed`
- `POST /api/v1/jobs/clear-completed`
- `POST /api/v1/jobs/clear-history`
- `DELETE /api/v1/jobs/:id`
- `POST /api/v1/jobs/:id/cancel`
- `POST /api/v1/jobs/:id/restart`
- `POST /api/v1/jobs/:id/priority`
- `POST /api/v1/jobs/enqueue`
- `GET /api/v1/processor/status`
- `GET /api/v1/profiles`
- `GET /api/v1/profiles/presets`
- `GET /api/v1/settings/bundle`
- `GET /api/v1/settings/preferences/:key`
- `POST /api/v1/settings/preferences`
- `POST /api/v1/settings/watch-dirs`
- `GET /api/v1/system/info`
- `GET /api/v1/system/resources`
- `POST /api/v1/conversion/uploads`
- `GET /api/v1/library/intelligence`
- `GET /api/v1/logs/history?limit=`
- `DELETE /api/v1/logs`
- `GET /api/v1/events`

## Backend Contract Gaps To Close

- Add an API capability/version endpoint independent from app release version.
- Decide whether `/api/v1` becomes the documented canonical surface or remains
  an alias over current `/api` routes.
- Keep structured `{ "error": { "code", "message" } }` on every route the Mac
  app calls.
- Document exact JSON response schemas for jobs, stats, savings, engine status,
  system info, watch folders, conversion upload, and auth.
- Add a local-only bind option for bundled desktop mode. Current Rust server
  defaults are still server-oriented.
- Add daemon readiness endpoint semantics for "process started but setup/login
  not complete".
- Add launch-safe setup status for first-run bundled mode.
- Promote the currently parsed SSE event names and payloads into a documented
  typed contract: `progress`, `status`, `decision`, `log`, `config_updated`,
  `watch_folder_added`, `watch_folder_removed`, `scan_started`,
  `scan_completed`, `engine_idle`, `engine_status_changed`,
  `hardware_state_changed`, and `lagged`.
- Add route or response fields for config path, database path, temp path, log
  path, and WebUI status.
- Decide how upload conversion should stream from native clients without
  requiring full file buffering in the Swift app.
- Define whether native file import should prefer `POST /api/jobs/enqueue` or a
  richer validation/import preview endpoint.
- Define authentication expectations for local bundled mode. Current decision:
  require normal login.
- Add query/filter/search response contracts that let the native queue use
  SwiftUI `.searchable` without client-side table rewrites.

## Swift Client Rules

- Keep all route paths centralized in `AlchemistAPIClient`.
- Do not duplicate queue, transcoding, planning, profile, or validation logic.
- Do not store canonical config in app-local state.
- Use Keychain later only for session/token persistence after the auth contract
  is explicit.
- Treat remote mode as later work; do not let it complicate bundled v1.
