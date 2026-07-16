# WhyTho? Architecture Charter v0.2

**Status:** Active planning source of truth  
**Format:** Markdown-only  
**Style note:** use `whytho.` in code-facing contexts; `WhyTho?` may be used as a title/product spelling.  
**Core definition:** `whytho.` is a CLI and library designed to replace FFmpeg in media-server transcoding workflows.

## 1. Executive summary

`whytho.` is a Rust-first CLI and library for modern media-server transcoding workflows. It is intended to replace FFmpeg usage for common server-side media processing tasks by providing a cleaner API, a serious end-user CLI, strong defaults, heavy multithreading, chunked transcoding, strict verification, and focused support for modern codecs.

This version removes the plugin/extension-platform direction. `whytho.` should still be powerful and configurable, but its architecture should be based on normal Rust crates, shared traits, config structs, built-in presets, feature flags, and app/CLI integration rather than dynamic extensions, scripting runtimes, or a plugin sandbox.

## 2. Product shape

| Area | Decision |
|---|---|
| Product type | CLI and Rust library |
| Main use case | Replacing FFmpeg in media-server transcoding workflows |
| Scope size | Large/powerful core, but no plugin platform |
| App relationship | Apps build on top of `whytho.` through APIs, config structs, and traits |
| Alchemist relationship | Alchemist should not dominate the docs; it is just one possible app using `whytho.` |
| CLI role | Serious end-user tool, not just a debug binary |
| Config | `whytho.` should parse config files; CLI should support its own config |
| Presets | Built-in presets are allowed and expected |
| Extensions/plugins | Removed from active architecture |

## 3. Goals

- Replace FFmpeg in common media-server transcoding workflows.
- Provide a clean Rust API and serious CLI.
- Focus on modern codecs rather than every codec ever released.
- Make chunked, multithreaded transcoding a core feature.
- Provide strong built-in presets for common media-server workflows.
- Support config files, CLI flags, and API structs with consistent behavior.
- Provide strict reporting, verification, and quality measurement.
- Use Rust encoders/decoders when possible and fast enough.
- Allow ASM inside Rust codec/backend code where useful.
- Keep future app integrations simple by exposing primitives and policies through traits/config structs.

## 4. Non-goals

- Do not become an FFmpeg clone that handles every historical codec and protocol.
- Do not build a dynamic plugin system in the active architecture.
- Do not ship WASM, Lua, JS, or TS scripting runtimes for v1.
- Do not make Alchemist-specific assumptions inside core `whytho.` docs or APIs.
- Do not make library scanning part of the initial core.
- Do not hide destructive file operations behind defaults.

## 5. Active architecture model

```text
whytho.
  CLI application
  Rust library
  core media engine
  built-in presets
  config parser
  chunking engine
  scheduler
  codec traits
  backend traits
  verification system
  quality system
  file-operation primitives

Apps built on top
  media servers
  Alchemist
  future GUI tools
  custom automation tools
```

The core idea is simple:

```text
WhyTho? performs media work.
Apps decide product behavior.
```

Apps can use `whytho.` directly without any plugin layer. Apps pass config structs, implement traits where needed, choose presets, and decide how to handle outputs, file replacement, storage policy, and UI.

## 6. Crate structure

Preferred structure:

```text
crates/
  whytho-core/
  whytho-cli/
  whytho-codecs/
  whytho-backends/
```

Internal shape:

```text
whytho-core
  media model
  probing interfaces
  job model
  pipeline planner
  chunk planner
  scheduler
  verifier
  quality primitives
  report model
  config model
  file-op primitives

whytho-cli
  user-facing commands
  config file loading
  guided setup
  dry-run planning
  diagnostics
  benchmark UX

whytho-codecs
  shared codec traits
  rav1e adapter
  rav2e implementation
  future encoder/decoder modules

whytho-backends
  CPU backend
  QSV backend
  NVENC/NVDEC backend
  VideoToolbox backend
  VA-API backend
  AMF backend
```

## 7. Codec strategy

| Area | Decision |
|---|---|
| First decode path | H.264 |
| First encode path | AV1 |
| First AV1 encoder | `rav1e` |
| AV2 encoder | `rav2e`, built as part of `whytho.` |
| `rav2e` location | `crates/whytho/codecs/rav2e` or equivalent internal module path |
| `rav2e` branding | Part of WhyTho? |
| `rav2e` target | AV2 encoder |
| ASM | Allowed and encouraged inside Rust codec/backend code where useful |
| Interface style | Shared traits for encoders/decoders |
| Decoders | Eventually in scope |
| Old codecs | Not active core focus; maybe adapters/fallbacks later |

`rav2e` is not an external afterthought. It is part of the WhyTho? vision. `rav1e` should be used for the first real AV1 path while `rav2e` develops.

Codec implementations should sit behind shared traits so the core pipeline can work with different encoders/decoders without special-casing every implementation.

## 8. Backend strategy

| Priority | Backend | Notes |
|---|---|---|
| 1 | CPU | First implementation path so the API and pipeline can prove themselves. |
| 2 | QSV / oneVPL | Very important for media-server and mini-PC use. |
| 3 | NVENC / NVDEC | Important for NVIDIA systems and high-throughput encode/decode. |
| 4 | VideoToolbox | Needed for macOS support. |
| 5 | VA-API | Useful Linux path, but backend variation needs care. |
| 6 | AMF | Important for AMD support. |

Backend details should not leak into high-level app code. Apps should request policies like `prefer hardware`, `require hardware`, `cpu only`, or `backend priority`, and the planner should resolve the actual backend path.

## 9. Presets

Built-in presets are part of `whytho.`. Presets should be useful from the CLI and available through the library API.

Initial preset candidates:

| Preset | Purpose |
|---|---|
| `av1-balanced` | Default modern media-server preset. |
| `av1-storage-saver` | More aggressive compression. |
| `hevc-compatible` | Strong device compatibility with better compression than H.264. |
| `h264-safe` | Maximum compatibility fallback. |
| `remux-clean` | Container/metadata cleanup without video re-encode when possible. |
| `benchmark` | Compare backends/codecs/settings. |
| `strict-verify` | Heavy verification and quality checks. |

Plex/Jellyfin compatibility should be handled through presets, not hardcoded media-server assumptions.

## 10. Config model

`whytho.` should support config files and config structs.

```text
Config file -> parsed by whytho -> config struct -> planner/executor
API caller -> config struct -> planner/executor
CLI flags -> override config struct -> planner/executor
```

CLI config is allowed and expected. Library users should not be forced to use config files, but the parser should exist for CLI users and apps that want it.

Recommended override order:

| Priority | Source |
|---|---|
| 1 | Explicit API/app override |
| 2 | CLI flag |
| 3 | Config file |
| 4 | Built-in preset |
| 5 | Hard default |

Example config shape:

```toml
[defaults]
preset = "av1-balanced"
container = "mkv"

[concurrency]
max_jobs = 3
chunking = true
cpu_workers = "auto"
gpu_jobs.qsv = 2
gpu_jobs.nvenc = 1

[video]
default_codec = "av1"
bitrate_strategy = "half-source-if-h264"

[audio]
default_codec = "opus"
preserve_unless_requested = true

[verification]
mode = "sample"
strict_requires_vmaf = true
fail_if_output_larger = true
max_duration_delta_seconds = 1

[file_ops]
replace_original = false
purge_partial_on_cancel = true
```

## 11. CLI design

The CLI is a serious user-facing product.

| Command | Purpose |
|---|---|
| `whytho plan` | Dry-run/explain command. Shows what would happen before doing it. |
| `whytho run` | Execute a job. |
| `whytho probe` | Inspect media streams, container, metadata, and capabilities. |
| `whytho verify` | Verify an output against an original. |
| `whytho bench` | Compare presets, backends, codecs, and speed/quality results. |
| `whytho doctor` | Diagnose hardware/backend/codec/system support. |
| `whytho setup` | Guided CLI setup for config and defaults. |

`whytho plan` remains the primary dry-run and explanation command.

The CLI should have excellent UX. It should support both hand-holding and power-user behavior.

## 12. File operations

File operations are not app-only, but they must be explicit.

Core `whytho.` should expose primitives for:

- writing temporary outputs
- purging partial chunks
- moving outputs into place
- replacing originals
- preserving originals
- generating file-operation plans

The CLI may expose destructive operations through explicit flags.

Examples:

```text
whytho run input.mkv --output output.mkv
whytho run input.mkv --replace-original
whytho run input.mkv --keep-original
whytho plan input.mkv --replace-original
```

Apps remain responsible for app-specific policy, UX, database state, and user prompts.

## 13. Chunking architecture

Chunking lives in core.

Default behavior:

- chunking enabled by default
- keyframe-aware chunk boundaries
- ability to insert keyframes
- chunk scheduler built into core
- boundary verification required
- global prepass maybe mandatory depending on mode/preset

Pipeline model:

```text
probe
  -> analyze streams
  -> analyze keyframes/GOP
  -> optional/global prepass
  -> plan chunks
  -> encode chunks in parallel
  -> verify chunk boundaries
  -> stitch/mux
  -> verify final output
  -> report
```

Open decision:

> Should global prepass be mandatory for all chunked jobs, or only for quality-focused presets?

Current leaning: mandatory for quality/strict/default media-server presets; optional for fast/debug modes.

## 14. Multithreading model

`whytho.` should be heavily multithreaded by design.

Concurrency layers:

| Layer | Goal |
|---|---|
| Multiple files | Process several jobs at once. |
| Chunks within one file | Split a single file into parallel work units. |
| CPU worker pool | Software encode/decode/filter work. |
| GPU backend queues | Independent limits per hardware backend. |
| IO queue | Reads/writes/muxing/temp movement. |
| Quality queue | VMAF/quality checks without blocking all encoding. |

Default simultaneous jobs: `3`.

CPU and GPU concurrency limits must be separate.

## 15. Verification and quality

Quality checking should be core-oriented.

Modes:

| Mode | Behavior |
|---|---|
| `sample` | Decode/playability samples, stream checks, duration delta, metadata checks. |
| `strict` | Requires VMAF/equivalent metric plus stricter checks. |
| `benchmark` | Compares outputs by speed, size, quality, and backend. |
| `military` | Stress mode for tests and important files. |

VMAF strategy:

- strict mode requires VMAF or equivalent
- start with libvmaf compatibility if needed
- replace/reimplement with Rust-native WhyTho? quality metric over time
- quality system should be in core or core-owned quality module

Open question:

> Should the Rust-native VMAF replacement exactly match VMAF, or become a WhyTho?-specific metric optimized for media-server decisions?

## 16. Library scanning and media-server behavior

Library scanning is not initial scope, but maybe future scope.

Media-server compatibility should be preset-driven:

```text
preset: plex-av1
preset: jellyfin-hevc
preset: universal-h264
```

Do not hardcode Plex/Jellyfin into core planning logic unless a preset/profile explicitly requests it.

## 17. Rejected for active architecture

The following ideas are rejected for the active architecture. They may be revisited later only with a separate architecture decision.

| Rejected idea | Reason |
|---|---|
| Dynamic extension platform | Too much complexity and security/API burden. |
| WASM sandboxing | Not needed for the current direction. |
| Lua/JS/TS scripting | Adds runtime/security complexity. |
| Extension permissions | Only needed if plugins/scripts exist. |
| Extension generator | No plugin system means no generator. |
| Extension marketplace/index | Out of scope. |
| Alchemist-centered docs | WhyTho? should stand on its own. |

## 18. Roadmap

| Milestone | Outcome |
|---|---|
| MVP-0 Architecture | Markdown charter, agent specs, crate boundaries, codec/backend/CLI model. |
| MVP-1 API + CLI foundation | `whytho plan`, config structs, CLI config, report model, fake/early CPU path. |
| MVP-2 Real probe/planning | MKV/H.264 probing, stream metadata, preset resolution, dry-run planning. |
| MVP-3 Real CPU transcode | H.264 input to AV1 output using rav1e, Opus audio, MKV output, sample verification. |
| MVP-4 Chunking | Keyframe analysis, chunk scheduler, boundary verification, stitching. |
| MVP-5 Hardware | QSV then NVENC/NVDEC, backend diagnostics, hardware planning. |
| MVP-6 rav2e | Internal AV2 encoder work begins under WhyTho? codec tree. |
| MVP-7 Strict quality | libvmaf-compatible strict mode, then Rust-native quality metric research. |
| MVP-8 Decoders | Begin decoder work where practical. |

## 19. Research tracks

| Track | Questions |
|---|---|
| Chunking | How to split/stitch with clean timestamps, quality, and boundaries? |
| rav1e | Best integration path for initial AV1 encoding. |
| rav2e | AV2 spec readiness, crate layout, ASM strategy, encoder architecture. |
| QSV/oneVPL | Rust binding strategy, hardware capabilities, zero-copy path. |
| NVENC/NVDEC | Rust binding strategy, CUDA interaction, decode/filter/encode path. |
| VMAF replacement | Compatibility first, faster Rust-native metric later. |
| MKV | Metadata, chapters, attachments, subtitles, stream preservation. |
| Decoders | Which modern decoders are worth implementing in Rust/ASM? |
| CLI UX | How to make `whytho` easier than FFmpeg without hiding power? |

## 20. Decision log

| Decision | Status |
|---|---|
| Markdown is the only important artifact format. | Accepted |
| DOCX should not be maintained going forward. | Accepted |
| No dynamic extension architecture in active design. | Accepted |
| Big/powerful core is acceptable. | Accepted |
| Built-in presets are part of WhyTho?. | Accepted |
| WhyTho? should parse config files. | Accepted |
| CLI should have its own config. | Accepted |
| Alchemist should not dominate WhyTho? docs. | Accepted |
| File ops should be explicit primitives and CLI flags. | Accepted |
| Crate structure preference: core, CLI, codecs, backends. | Accepted |
| `rav2e` is part of WhyTho?. | Accepted |
| `rav2e` targets AV2. | Accepted |
| `rav1e` is the first real AV1 path. | Accepted |
| ASM is encouraged where useful inside Rust codec/backend code. | Accepted |
| Shared codec traits should be used. | Accepted |
| Decoders are eventual scope. | Accepted |
| Chunking lives in core. | Accepted |
| `whytho plan` remains the main dry-run command. | Accepted |
| CLI should have excellent guided UX. | Accepted |

## 21. References and research sources

- NVIDIA Video Codec SDK — https://developer.nvidia.com/video-codec-sdk
- Intel oneVPL / Video Processing Library — https://www.intel.com/content/www/us/en/developer/tools/vpl/overview.html
- VA-API / libva — https://github.com/intel/libva
- Netflix VMAF — https://github.com/Netflix/vmaf
- rav1e — https://github.com/xiph/rav1e
- FFmpeg formats documentation: segmenting/keyframe behavior — https://ffmpeg.org/ffmpeg-formats.html
- AOMedia Film Grain Synthesis 1 — https://aomediacodec.github.io/afgs1-spec/
