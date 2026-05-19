# Planning

Last updated: 2026-05-19

This file tracks current coordination notes only. Detailed historical execution logs belong in git history, `CHANGELOG.md`, and release notes.

## Current Baseline

- Version: `0.3.3-rc.4`
- Release line: 0.3.3 release-candidate hardening
- Required orientation files: `CHANGELOG.md`, `VERSION`, `CLAUDE.md`
- Living planning sources:
  - `backlog.md` for active and future product work
  - `audit.md` for verified bugs/security/correctness findings
  - `ideas.md` for optional future ideas
  - `native/mac/Docs/swift.md` for the native macOS client specification

## Package Refresh / Release Check Pass

Status: dependency refresh validated on 2026-05-19. Rust packages are updated to the latest versions resolvable under the repository `rust-version = "1.85"` floor; web, docs, e2e, and Jellyfin package manifests/locks have been refreshed.

Breaking-change fixes landed in this pass:

- Axum route parameters now use the 0.8 `{id}` / `{*file}` syntax, and the API-contract checker accepts the canonical v1 job-delete path.
- Reqwest 0.13 uses the `rustls` and `query` features instead of the removed `rustls-tls` feature.
- Rand 0.10 imports and OS RNG usage were updated for the new traits/types.
- SHA-2 0.11 digest output is hex-encoded explicitly where lower-hex formatting was removed.
- Docs TypeScript 6 config suppresses the known `baseUrl` deprecation, matching the web config.
- Bun audit overrides were raised for `devalue` and `webpack-dev-server`.
- Docs now force transitive `ws` resolution to `8.20.1` so both Docusaurus' webpack-dev-server and bundle-analyzer paths clear the current audit advisory.

Latest validation:

- `cargo upgrade --incompatible allow --recursive true --dry-run` - no further manifest changes.
- `bun outdated` in `web`, `docs`, and `web-e2e` - no outdated packages reported.
- `dotnet list ... package --outdated` for both Jellyfin projects - no updates reported from NuGet.
- `bunx playwright install chromium` refreshed the browser cache for `@playwright/test` 1.60.0.
- `just release-check` - passed on 2026-05-19 after the docs `ws` override, covering Rust fmt/clippy/check/tests, actionlint, API contract, web verify/audit, docs build/audit, E2E reliability, and Jellyfin build/tests.

## Roadmap Execution Pass

Status: in progress from the broad referenced-docs plan. Work should land as small vertical slices with docs and targeted tests before moving to the next item.

Completed in this pass:

- `F-2` library plan preview follow-through: Watch Folders has an accessible dry-run Preview action backed by `/api/v1/library/preview`, with e2e coverage and roadmap docs updated from future to shipped first cut.
- `UX-2` jobs keyboard shortcuts first cut: `/` focuses jobs search, `?` opens the jobs-page shortcut reference, and `Esc` closes it. The handler ignores text-entry fields so normal typing is not intercepted.
- `UX-3` jobs explanation search first cut: the existing jobs table search now matches file paths, decision explanations, reason codes, and failure explanations.
- `UX-6` mobile dashboard first cut: narrow screens now show an Active Now panel with active job progress and hide the desktop stats strip.
- `UX-9` file-settings impact first cut: File Settings now summarizes staged output naming, output root, replace-policy, delete-source, and future-job effects before save.
- `UX-10` storage reclaimed first cut: Statistics now shows the raw saved storage plus an approximate 4 GB movie equivalent.
- `UX-12` aggregate queue ETA first cut: Dashboard now shows remaining queue work and a recent-throughput ETA from `/api/v1/stats/queue-eta`.
- `POL-6` timestamp first cut: shared `TimeDisplay` now renders relative dense timestamps with exact local and UTC values on hover/focus in the jobs table and attempt history.
- `POL-7` FFmpeg stderr explanation first cut: a table-driven classifier now recognizes common disk-full, NVENC resource, pixel-format, corrupt-input, and encoder-parameter signatures.
- `POL-2` jobs context-menu first cut: right-click opens the row action menu at the cursor and supports Copy input path.
- `OP-3` restore validation first cut: `/api/v1/system/backup/validate-restore` checks uploaded `.db.gz` snapshots and returns schema/job metadata without touching the live database.
- `OP-4` self-install first cut: `--install` and `--install-directory <PATH>` copy the current executable into a target binary directory, with focused CLI coverage.
- `OP-6` config validation first cut: `/api/v1/settings/config/validate` parses candidate TOML without persistence and the Config editor shows a redacted validation preview.
- `ENC-2` chapter preservation first cut: analyzer metadata records source chapter counts, and finalization logs a non-fatal job warning when an output loses chapters.

Latest validation:

- `cd web && bun run typecheck` - passed.
- `cargo test get_jobs_filtered_search_matches_paths_decisions_and_failures --lib` - passed.
- `cd web-e2e && bunx playwright test tests/job-tabs.spec.ts` - 6 passed.
- `cd web-e2e && bunx playwright test tests/job-tabs.spec.ts tests/jobs-success.spec.ts` - 16 passed.
- `cd web-e2e && bunx playwright test tests/dashboard-ui.spec.ts` - 6 passed.
- `cd web-e2e && bunx playwright test tests/savings-overview.spec.ts` - 2 passed.
- `cd web-e2e && bunx playwright test tests/jobs-success.spec.ts` - 11 passed.
- `cargo test queue_eta_uses_recent_encode_samples_and_concurrency --lib` - passed.
- `python3 scripts/check_api_contract.py` - passed, 88 v1 routes documented.
- `cd web-e2e && bunx playwright test tests/dashboard-ui.spec.ts` - 7 passed.
- `cargo test restore_validation --lib` - 2 passed.
- `python3 scripts/check_api_contract.py` - passed, 89 v1 routes documented.
- `cd docs && bun run build` - passed.
- `cd web && bun run typecheck` - passed.
- `cd web-e2e && bunx playwright test tests/settings-success.spec.ts` - 6 passed.
- `cargo test --lib media::analyzer --quiet` - 13 passed.
- `cargo test chapter_preservation_warning_only_when_output_loses_chapters --lib` - passed.
- `cd docs && bun run build` - passed.
- `cargo test explanations --lib` - 7 passed.
- `cd web && bun run typecheck` - passed.
- `cd docs && bun run build` - passed.
- `cargo test install_binary_copies_current_executable_to_custom_directory --bin alchemist` - passed.
- `cd docs && bun run build` - passed.
- `cargo test raw_config_validate_returns_summary_without_persisting --lib` - passed.
- `python3 scripts/check_api_contract.py` - passed, 90 v1 routes documented.
- `cd web && bun run typecheck` - passed.
- `cd web-e2e && bunx playwright test tests/settings-success.spec.ts` - 7 passed.
- `cd docs && bun run build` - passed.
- `cd web && bun run typecheck` - passed.
- `cd web-e2e && bunx playwright test tests/library-intake.spec.ts` - 5 passed.
- `cd docs && bun run build` - passed.
- `just check-rust` - passed fmt, strict clippy, and cargo check.
- `just check-web` - passed web install/typecheck/build and 92 Playwright tests.
- `cd docs && bun run build` - passed.
- `just check-web` - passed web typecheck/build and 88 Playwright tests.

## Analyzer Metadata Improvement Pass

Status: analyzer-only implementation validated locally. Serialization compatibility, label/metric coverage, probe-cache behavior, and strict Rust checks are green. Library Intelligence/UI consumption remains intentionally deferred because current job persistence stores `MediaMetadata`, not full `MediaAnalysis.analysis_report`.

Scope accepted on 2026-05-16:

- Quiet internal improvement only. Do not introduce a branded "Smart Detection" product surface or rename existing Analyzer / Planner / Library Intelligence concepts.
- Keep the first pass analyzer-only: improve FFprobe-derived facts, deterministic factual labels, warnings, and metrics that later planner or intelligence work can consume.
- Store the first pass in `MediaAnalysis` and its serialized cache footprint only. No new database table, migration, endpoint, or UI surface for this slice.
- Preserve current planner behavior. Do not change skip/remux/transcode decisions, enqueue behavior, execution, finalization, replacement policy, or Library Intelligence UI in this pass.
- Defer sampled/expensive probes: cropdetect, complexity/grain probes, VMAF pre-flight, decode spot checks, OCR, and Jellyfin/client prediction.

Implementation shape:

- Add a typed analyzer report to `MediaAnalysis`:
  - `analysis_report: AnalyzerReport` with `#[serde(default)]`.
  - `AnalyzerReport { labels: Vec<AnalyzerLabel>, metrics: AnalyzerMetrics }`.
  - `AnalyzerReport`, `AnalyzerMetrics`, and all new fields must implement `Default`.
- Define `AnalyzerLabel` as a `snake_case` enum of factual classifications only:
  - Density and container facts: `high_bpp_density`, `low_bpp_density`, `remux_like_density`.
  - Audio facts: `heavy_audio`, `lossless_audio`.
  - Subtitle facts: `image_subtitle`, `styled_subtitle`.
  - HDR/color facts: `hdr_metadata`, `bt2020_without_transfer`, `dolby_vision_metadata`.
  - Structure/playback metadata facts: `interlaced_metadata`, `variable_frame_rate_hint`.
  - Existing warning mirrors: `missing_video_bitrate`, `missing_container_bitrate`, `missing_duration`, `missing_fps`, `missing_bit_depth`, `unrecognized_pixel_format`.
- Keep product-policy conclusions out of analyzer labels:
  - Do not add `manual_review`, `not_worth_processing`, `direct_play_friendly`, `browser_audio_incompatible`, `storage_win`, or numeric risk/waste/compatibility scores in this pass.
  - Those decisions belong in a later planner or Library Intelligence pass after the analyzer facts are stable.
- Define `AnalyzerMetrics` with optional measured fields:
  - `raw_bpp`, `normalized_bpp`, `estimated_container_bitrate_bps`, `audio_bitrate_share`.
  - `video_stream_count`, `audio_stream_count`, `subtitle_stream_count`, `image_subtitle_count`, `text_subtitle_count`.
  - `hdr_metadata_present`, `has_bt2020_metadata`, `has_missing_color_transfer`.
  - `fps_from_average_rate`, `fps_from_frame_count`.
- Populate the report in `FfmpegAnalyzer::analyze` after `MediaMetadata`, `warnings`, and `confidence` are known.
- Use pure helper functions for report construction so tests can cover labels and metrics without spawning FFprobe.
- Extend the existing FFprobe `-show_entries` list only for cheap metadata needed by this report. If a field is not available from the normal probe path, leave the label or metric absent rather than spawning a new subprocess.
- Keep all labels explainable from measured metadata. A future UI should be able to show "why this label exists" without reverse-engineering planner logic.

Validation targets:

- Unit tests for pure classification helpers:
  - high and low BPP density
  - remux-like density
  - estimated container bitrate fallback, including the no-video-bitrate case where density labels stay absent
  - heavy and lossless audio labels across all audio streams
  - image, text, and styled subtitle counts
  - stream counts and aggregate audio bitrate share
  - HDR and BT.2020 metadata labels
  - Dolby Vision side-data label using `stream_side_data`
  - legacy-compatible FPS selection so planner behavior does not drift
  - missing metadata labels mirrored from every `AnalysisWarning`
  - legacy `MediaAnalysis` JSON without `analysis_report`
  - partial `AnalyzerReport` JSON with missing nested fields
- Targeted Rust checks after implementation: analyzer tests, probe-cache tests, and `just check-rust`.
- No migration test is required unless the scope changes to add schema.

Latest validation in this resumed pass, 2026-05-18:

- `cargo test --lib media::analyzer --quiet` - 12 passed
- `cargo test --lib probe_cache --quiet` - 4 passed
- `cargo test --lib --quiet` - 258 passed
- `cargo clippy --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used` - passed
- `cargo test --test integration_ffmpeg_minimal --quiet` - 7 passed
- `just check-rust` - passed fmt, strict clippy, and cargo check

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

Latest validation in this cleanup/integration pass, 2026-05-18:

- Placeholder icon/reference grep returned no matches.
- `cargo test mcp --lib --quiet` - 7 passed
- `dotnet build integrations/jellyfin/Alchemist.Jellyfin/Alchemist.Jellyfin.csproj` - passed with 0 warnings and 0 errors
- `dotnet test integrations/jellyfin/Alchemist.Jellyfin.Tests/Alchemist.Jellyfin.Tests.csproj` - 6 passed
- `just check-web` - passed typecheck, Astro build, and 85 Playwright tests
- `just check` - passed Rust fmt/clippy/check, API contract, frontend typecheck/build, and native macOS checks

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
