---
title: Error Reference
description: Every Alchemist error and decision code, what it means, and how to fix it. Each code links here from the app and the API so you always have one place to look.
slug: /errors
keywords:
  - alchemist error codes
  - encoder open failed
  - videotoolbox session failed
  - transcode failed
  - hardware backend failure
---

# Error reference

Every error Alchemist can report carries a stable **code**. The same code appears in:

- the **job detail** panel in the web UI (with a *Learn more* link to this page),
- the **API** as the `code` and `docs_url` fields of an [RFC 7807](https://www.rfc-editor.org/rfc/rfc7807) `application/problem+json` response,
- the **logs** (`alchemist.log`), tagged on the failing job's span.

Each code below is an anchor: `https://deadsignal.works/alchemist/docs/errors/#<code>`. If the app sent you here, jump to your code.

There are three families:

1. **Encode &amp; job failure codes** — why a transcode job failed. Lower-case, e.g. `encoder_open_failed`.
2. **Internal error codes** — the typed `AlchemistError` surfaced by the engine/API. Prefixed `ERR_`.
3. **API request codes** — returned by HTTP endpoints for bad requests, conflicts, and rate limits. SCREAMING_SNAKE.

---

## Encode &amp; job failure codes

### encoder_open_failed

**What it means:** The selected hardware encoder could not open an encoding session. The most common cause on macOS is VideoToolbox: a session cannot be created when the daemon runs **without a logged-in GUI (WindowServer) session** — for example launched as a `launchd` LaunchDaemon, over SSH, or sandboxed — or when **constant-quality (`-q:v`) mode is unsupported** on the current FFmpeg build/encoder. The FFmpeg log shows `Could not open encoder before EOF` and `Invalid argument`.

**How Alchemist responds:** It automatically retries the job **once on the CPU** (libx265/libx264). If that succeeds, the job completes and is marked as a CPU fallback. This code only surfaces as a failure if the CPU fallback **also** fails or CPU fallback is disabled.

**How to fix:**
- Enable **CPU fallback** in *Settings → Hardware* (lets the one-time fallback recover the job).
- For VideoToolbox specifically: run Alchemist inside a normal logged-in macOS session (a LaunchAgent, not a LaunchDaemon), or switch the encoder rate control to **bitrate** mode.
- If you intended hardware encoding, confirm the GPU is actually usable from the daemon's context (see [Hardware](/hardware)).

### videotoolbox_session_failure

**What it means:** A macOS VideoToolbox session failed or was lost mid-encode (GPU under load, or another process holds the encoder).

**How to fix:** Retry. If it repeats, reduce concurrent jobs, restart Alchemist, or enable CPU fallback.

### hardware_backend_failure

**What it means:** The selected hardware encoding backend (VideoToolbox/VAAPI/QSV/AMF/NVENC) failed during processing.

**How to fix:** Retry first. If it persists, check the hardware probe log and enable CPU fallback in *Settings → Hardware*.

### nvenc_resource_exhausted

**What it means:** The NVIDIA encoder reported a memory/buffer exhaustion error.

**How to fix:** Reduce concurrent jobs, retry under lower GPU load, or enable CPU fallback.

### unsupported_pixel_format

**What it means:** The encoder could not accept the source pixel format or color layout.

**How to fix:** Retry with CPU fallback or a different hardware backend; inspect the source color format in job details.

### encoder_parameter_mismatch

**What it means:** FFmpeg rejected a generated encoder parameter for the selected backend (e.g. `qscale not available for encoder`).

**How to fix:** Read the FFmpeg line that names the rejected option, then retry with a different codec, quality profile, or backend.

### encoder_unavailable

**What it means:** The required encoder is not present in this FFmpeg build or hardware environment (`Unknown encoder`).

**How to fix:** Check *Settings → Hardware*, your FFmpeg encoder support, and fallback settings. In Docker, ensure you are on the bundled jellyfin-ffmpeg image.

### fallback_blocked

**What it means:** A required fallback (e.g. hardware → CPU) was disallowed by the current hardware policy, so the job could not continue.

**How to fix:** Enable CPU fallback in *Settings → Hardware*, or make the preferred encoder available.

### disk_full

**What it means:** FFmpeg could not write output — the temp/output filesystem ran out of space.

**How to fix:** Free space on the temp/output volume or move the output root to a larger filesystem, then retry.

### resource_exhausted

**What it means:** The system ran out of memory or another resource during processing.

**How to fix:** Reduce concurrent jobs or retry on a less loaded machine.

### corrupt_or_unreadable_media

**What it means:** FFmpeg hit a decode/read error (`Invalid data found`, `moov atom not found`). The source is likely corrupt or incomplete.

**How to fix:** Play the file manually or run [Library Doctor](/library-doctor) to confirm whether it is intact.

### empty_output

**What it means:** FFmpeg ran but wrote no packets (`Nothing was written into output file`). Usually the encoder crashed or rejected the stream before real output. Frequently paired with `encoder_open_failed`.

**How to fix:** Inspect the lines around the first FFmpeg error in the job logs for the backend-specific cause.

### source_missing

**What it means:** The source file could not be found (moved, deleted, or unmounted).

**How to fix:** Confirm the source path still exists and is readable by the Alchemist process user.

### permission_denied

**What it means:** Alchemist lacks permission to read the source or write the output.

**How to fix:** Check filesystem permissions for the Alchemist process user on both the source and output paths.

### path_or_permission_failure

**What it means:** A path was missing, read-only, or unreadable while reading the source or writing the output.

**How to fix:** Verify the source exists and the process user can read/write the configured paths.

### analysis_failed

**What it means:** FFprobe could not analyze the input before planning/encoding.

**How to fix:** Inspect job logs and verify the media is readable.

### incomplete_metadata

**What it means:** FFprobe could not determine required metadata, so no defensible transcode decision could be made.

**How to fix:** Run Library Doctor or inspect the file to confirm it is readable.

### planning_failed

**What it means:** An internal error occurred while building the transcode plan.

**How to fix:** Check job logs. If it repeats on a specific input, treat it as a planner bug and report it.

### quality_check_failed

**What it means:** The output failed the configured quality gate (e.g. VMAF), so Alchemist reverted it. **Your original file is preserved.**

**How to fix:** Adjust thresholds in *Settings → Quality* if the gate is stricter than you want.

### finalize_failed

**What it means:** The job encoded/remuxed successfully but final promotion or verification failed.

**How to fix:** Inspect filesystem state and job logs before retrying.

### unknown_ffmpeg_failure

**What it means:** FFmpeg failed but no more specific signature matched.

**How to fix:** Read the raw FFmpeg output in the job logs for the first concrete encoder/media error.

### cancelled

**What it means:** The job was cancelled before completion. The original file is unchanged. Not an error.

### unknown_failure

**What it means:** A failure with no recognized signature. The stored summary is shown verbatim.

**How to fix:** Inspect the job logs for context.

---

## Internal error codes (`ERR_*`)

These are the typed engine/API errors. They appear in API responses and logs.

| Code | Meaning | Typical fix |
| --- | --- | --- |
| `ERR_DATABASE` | SQLite query/connection error | Check disk space and DB path permissions; see logs |
| `ERR_CONFIG` | Invalid configuration | Fix the offending setting in *Settings* or `config.toml` |
| `ERR_HARDWARE` | Hardware detection failed | See [Hardware](/hardware) and the probe log |
| `ERR_FFMPEG` | FFmpeg execution failed | See the job's failure code above and FFmpeg logs |
| `ERR_FFMPEG_NOT_FOUND` | FFmpeg/FFprobe not found | Install FFmpeg or set the binary path |
| `ERR_ENCODER_UNAVAILABLE` | Encoder unavailable | Enable CPU fallback / verify GPU encoder |
| `ERR_QUALITY_CHECK_FAILED` | Quality gate failed | Adjust *Settings → Quality* |
| `ERR_NOTIFICATION` | Notification delivery failed | Check the notification target URL/credentials |
| `ERR_WATCH` | Filesystem watcher error | Verify the watched path is accessible |
| `ERR_IO` | I/O error | Check disk space and path permissions |
| `ERR_ANALYZER` | FFprobe analysis error | Verify the media is readable |
| `ERR_CANCELLED` | Operation cancelled | None — expected |
| `ERR_PAUSED` | Engine paused | Start the engine from the header |
| `ERR_QUERY_TIMEOUT` | DB query timed out | Retry; check DB load/disk health |
| `ERR_UNKNOWN` | Unclassified error | Inspect logs for context |

---

## API request codes

HTTP endpoints return a problem document with a SCREAMING_SNAKE `code` and a `docs_url`
pointing back here. Common ones:

| Code | Status | Meaning |
| --- | --- | --- |
| `AUTH_RATE_LIMITED` | 429 | Too many login attempts for this client |
| `INVALID_STATUS_FILTER` | 400 | A `status` filter contained no recognized values |
| `BATCH_ACTION_CONFLICT` | 409 | Some jobs in a batch could not be modified |
| `HEALTH_SCAN_IN_PROGRESS` | 409 | A library health scan is already running |
| `UPDATE_INSTALL_IN_PROGRESS` | 409 | An update install is already running |
| `PREVIEW_BUSY` | 429 | Another library preview is in flight |
| `PREVIEW_PATH_FORBIDDEN` | 403 | Preview path is outside configured roots |
| `SELFTEST_BUSY` | 429 | A self-test is already running |
| `UPDATE_PREFLIGHT_FAILED` | 507 | Not enough disk space, or the install directory is not writable, to apply the update |
| `UPDATE_BACKUP_FAILED` | 500 | The pre-update database/config backup could not be created |
| `UPDATE_STAGE_FAILED` | 502 | The update could not be downloaded, verified, or extracted |
| `UPDATE_HELPER_FAILED` | 500 | The update apply helper could not be started |
| `LOG_FILE_UNAVAILABLE` | 404 | No log file is available to download yet |

The internal and API codes above are documented on this page; their `docs_url`
resolves here. The most common encode-failure codes (top of the page) additionally
have deep-link anchors. If you received a `code` not listed, the `detail` field of
the response describes the specific problem.
