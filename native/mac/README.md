# Alchemist Mac

Native SwiftUI companion app for Alchemist.

This package is intentionally isolated under `native/mac` so the Rust backend,
web UI, docs, and native client can evolve without mixing build products or
source ownership.

## Current Scope

- Normal Mac user first.
- Bundled daemon mode first, with remote API mode later.
- Targets the current versioned Alchemist API (`/api/v1/...`) while the backend
  API contract is documented and hardened in parallel.
- Uses `~/Library/Application Support/Alchemist/` for bundled-mode config,
  database, and temp files.
- Requires the newest macOS SDK for the prototype. Older systems should skip
  newer glass effects rather than block the native client design.
- Uses standard SwiftUI containers first. Custom Liquid Glass is limited to
  functional command surfaces; dense content stays readable.

## Just Recipes

From the repo root:

```bash
just mac-build
just mac-test
just mac-check
just mac-run
just mac-run-bundled
```

`just mac-run-bundled` builds/stages the Rust daemon as `native/mac/.artifacts/alchemistd`
and passes it to the Swift app with `ALCHEMIST_DAEMON_PATH`.

See `Docs/LIQUID_GLASS_RESEARCH.md` for current Apple-source design rules.
