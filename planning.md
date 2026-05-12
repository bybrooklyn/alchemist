# Planning

Last updated: 2026-05-12

This file tracks current coordination notes only. Detailed historical execution logs belong in git history, `CHANGELOG.md`, and release notes.

## Current Baseline

- Version: `0.3.2-rc.2`
- Release line: 0.3.2 release-candidate hardening
- Required orientation files: `CHANGELOG.md`, `VERSION`, `CLAUDE.md`
- Living planning sources:
  - `backlog.md` for active and future product work
  - `audit.md` for verified bugs/security/correctness findings
  - `ideas.md` for optional future ideas
  - `native/mac/Docs/swift.md` for the native macOS client specification

## Current Cleanup / Integration Pass

Scope accepted on 2026-05-12:

- Remove approved local/generated junk and ignore it going forward.
- Remove the existing placeholder browser icon file and every reference to it.
- Keep MCP read-only and protocol-correct for v1.
- Turn the Jellyfin skeleton into a compileable plugin with a library-event hook and completed-job refresh path.
- Consolidate root markdown without disrupting the clean native/mac separation in `native/mac/Docs/swift.md`.

Validation targets:

- The placeholder icon asset/reference grep returns no matches.
- `cargo test mcp --lib` passes.
- `dotnet build integrations/jellyfin/Alchemist.Jellyfin/Alchemist.Jellyfin.csproj` passes.
- `dotnet test integrations/jellyfin/Alchemist.Jellyfin.Tests/Alchemist.Jellyfin.Tests.csproj` passes.
- Broader Rust/web checks should run before release handoff because this checkout already contains large unrelated WIP.

## Recently Shipped Queue Snapshot

These items are implemented and should be treated as product surface needing maintenance, tests, and docs sync rather than future planning:

- `MIG-3`: v1 structured API error envelope across high-traffic server paths.
- `INT-3`: ntfy notification target support.
- `AUTO-2`: notification quiet hours v1.
- `IMPR-1`: Astro content collection foundation.
- `PERF-1`: ffprobe result cache foundation.
- `INT-1`: Sonarr/Radarr ARR webhook ingress with scoped token and path translations.

## Near-Term Candidates

- Finish current WIP verification and release-readiness cleanup.
- Keep native macOS work isolated under `native/mac` and preserve `native/mac/Docs/swift.md` as the product spec.
- Harden the new MCP surface and validate the Jellyfin plugin against a live Jellyfin install before packaging release artifacts.
- Continue AMD AV1 validation only with real hardware evidence.
- Promote backlog items only when they support the automation-first transcoding mission.
