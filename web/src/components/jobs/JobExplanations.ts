import type { ExplanationView, LogEntry } from "./types";

function formatReductionPercent(value?: string): string {
    if (!value) return "?";
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? `${(parsed * 100).toFixed(0)}%` : value;
}

export function humanizeSkipReason(reason: string): ExplanationView {
    const pipeIdx = reason.indexOf("|");
    const key = pipeIdx === -1
        ? reason.trim()
        : reason.slice(0, pipeIdx).trim();
    const paramStr = pipeIdx === -1 ? "" : reason.slice(pipeIdx + 1);

    const measured: Record<string, string | number | boolean | null> = {};
    for (const pair of paramStr.split(",")) {
        const [rawKey, ...rawValueParts] = pair.split("=");
        if (!rawKey || rawValueParts.length === 0) continue;
        measured[rawKey.trim()] = rawValueParts.join("=").trim();
    }

    const makeDecision = (
        code: string,
        summary: string,
        detail: string,
        operator_guidance: string | null,
    ): ExplanationView => ({
        category: "decision",
        code,
        summary,
        detail,
        operator_guidance,
        measured,
        legacy_reason: reason,
    });

    switch (key) {
        case "analysis_failed":
            return makeDecision(
                "analysis_failed",
                "File could not be analyzed",
                `FFprobe failed to read this file. It may be corrupt, incomplete, or in an unsupported format. Error: ${measured.error ?? "unknown"}`,
                "Try playing the file in VLC or another media player. If it plays fine, re-run the scan. If not, the file may be damaged.",
            );
        case "planning_failed":
            return makeDecision(
                "planning_failed",
                "Transcoding plan could not be created",
                `An internal error occurred while planning the transcode for this file. This is likely a bug. Error: ${measured.error ?? "unknown"}`,
                "Check the logs below for details. If this happens repeatedly, please report it as a bug.",
            );
        case "already_target_codec":
            return makeDecision(
                "already_target_codec",
                "Already in target format",
                `This file is already encoded as ${measured.codec ?? "the target codec"}${measured.bit_depth ? ` at ${measured.bit_depth}-bit` : ""}. Re-encoding would waste time and could reduce quality.`,
                null,
            );
        case "already_target_codec_wrong_container":
            return makeDecision(
                "already_target_codec_wrong_container",
                "Target codec, wrong container",
                `The video is already in the right codec but wrapped in a ${measured.container ?? "MP4"} container. Alchemist will remux it to ${measured.target_extension ?? "MKV"} - fast and lossless, no quality loss.`,
                null,
            );
        case "bpp_below_threshold":
            return makeDecision(
                "bpp_below_threshold",
                "Already efficiently compressed",
                `Bits-per-pixel (${measured.bpp ?? "?"}) is below the minimum threshold (${measured.threshold ?? "?"}). This file is already well-compressed - transcoding it would spend significant time for minimal space savings.`,
                "If you want to force transcoding, lower the BPP threshold in Settings -> Transcoding.",
            );
        case "below_min_file_size":
            return makeDecision(
                "below_min_file_size",
                "File too small to process",
                `File size (${measured.size_mb ?? "?"}MB) is below the minimum threshold (${measured.threshold_mb ?? "?"}MB). Small files aren't worth the transcoding overhead.`,
                "Lower the minimum file size threshold in Settings -> Transcoding if you want small files processed.",
            );
        case "size_reduction_insufficient":
            return makeDecision(
                "size_reduction_insufficient",
                "Not enough space would be saved",
                `The predicted size reduction (${formatReductionPercent(String(measured.reduction ?? measured.predicted ?? ""))}) is below the required threshold (${formatReductionPercent(String(measured.threshold ?? ""))}). Transcoding this file wouldn't recover meaningful storage.`,
                "Lower the size reduction threshold in Settings -> Transcoding to encode files with smaller savings.",
            );
        case "no_suitable_encoder":
        case "no_available_encoders":
            return makeDecision(
                key,
                "No encoder available",
                `No encoder was found for ${measured.codec ?? measured.requested_codec ?? "the target codec"}. Hardware detection may have failed, or CPU fallback is disabled.`,
                "Check Settings -> Hardware. Enable CPU fallback, or verify your GPU is detected correctly.",
            );
        case "preferred_codec_unavailable_fallback_disabled":
            return makeDecision(
                "preferred_codec_unavailable_fallback_disabled",
                "Preferred encoder unavailable",
                `The preferred codec (${measured.codec ?? "target codec"}) is not available and CPU fallback is disabled in settings.`,
                "Go to Settings -> Hardware and enable CPU fallback, or check that your GPU encoder is working correctly.",
            );
        case "Output path matches input path":
        case "output_path_matches_input":
            return makeDecision(
                "output_path_matches_input",
                "Output would overwrite source",
                "The configured output path is the same as the source file. Alchemist refused to proceed to avoid overwriting your original file.",
                "Go to Settings -> Files and configure a different output suffix or output folder.",
            );
        case "Output already exists":
        case "output_already_exists":
            return makeDecision(
                "output_already_exists",
                "Output file already exists",
                "A transcoded version of this file already exists at the output path. Alchemist skipped it to avoid duplicating work.",
                "If you want to re-transcode it, delete the existing output file first, then retry the job.",
            );
        case "incomplete_metadata":
            return makeDecision(
                "incomplete_metadata",
                "Missing file metadata",
                `FFprobe could not determine the ${measured.missing ?? "required metadata"} for this file. Without reliable metadata Alchemist cannot make a valid transcoding decision.`,
                "Run a Library Doctor scan to check if this file is corrupt. Try playing it in a media player to confirm it is readable.",
            );
        case "already_10bit":
            return makeDecision(
                "already_10bit",
                "Already 10-bit",
                "This file is already encoded in high-quality 10-bit depth. Re-encoding it could reduce quality.",
                null,
            );
        case "remux: mp4_to_mkv_stream_copy":
        case "remux_mp4_to_mkv_stream_copy":
            return makeDecision(
                "remux_mp4_to_mkv_stream_copy",
                "Remuxed (no re-encode)",
                "This file was remuxed from MP4 to MKV using stream copy - fast and lossless. No quality was lost.",
                null,
            );
        case "Low quality (VMAF)":
        case "quality_below_threshold":
            return makeDecision(
                "quality_below_threshold",
                "Quality check failed",
                "The encoded file scored below the minimum VMAF quality threshold. Alchemist rejected the output to protect quality.",
                "The original file has been preserved. You can lower the VMAF threshold in Settings -> Quality, or disable VMAF checking entirely.",
            );
        case "transcode_h264_source":
            return makeDecision(
                "transcode_h264_source",
                "H.264 source prioritized",
                "This file is H.264, which is typically a strong candidate for reclaiming space, so Alchemist prioritized it for transcoding.",
                null,
            );
        case "transcode_recommended":
            return makeDecision(
                "transcode_recommended",
                "Transcode recommended",
                "Alchemist determined this file is a strong candidate for transcoding based on the current codec and measured efficiency.",
                null,
            );
        default:
            return makeDecision("legacy_decision", "Decision recorded", reason, null);
    }
}

export function explainFailureSummary(summary: string): ExplanationView {
    const normalized = summary.toLowerCase();

    const makeFailure = (
        code: string,
        title: string,
        detail: string,
        operator_guidance: string | null,
    ): ExplanationView => ({
        category: "failure",
        code,
        summary: title,
        detail,
        operator_guidance,
        measured: {},
        legacy_reason: summary,
    });

    if (normalized.includes("cancelled")) {
        return makeFailure("cancelled", "Job was cancelled", "This job was cancelled before encoding completed. The original file is untouched.", null);
    }
    if (normalized.includes("no such file or directory")) {
        return makeFailure("source_missing", "Source file missing", "The source file could not be found. It may have been moved or deleted.", "Check that the source file still exists and is readable by Alchemist.");
    }
    if (normalized.includes("invalid data found") || normalized.includes("moov atom not found")) {
        return makeFailure("corrupt_or_unreadable_media", "Media could not be read", "This file appears to be corrupt or incomplete. Try running a Library Doctor scan.", "Verify the source file manually or run Library Doctor to confirm whether it is readable.");
    }
    if (normalized.includes("permission denied")) {
        return makeFailure("permission_denied", "Permission denied", "Alchemist doesn't have permission to read this file. Check the file permissions.", "Check the file and output path permissions for the Alchemist process user.");
    }
    if (normalized.includes("encoder not found") || normalized.includes("unknown encoder")) {
        return makeFailure("encoder_unavailable", "Required encoder unavailable", "The required encoder is not available in your FFmpeg installation.", "Check FFmpeg encoder availability and hardware settings.");
    }
    if (normalized.includes("out of memory") || normalized.includes("cannot allocate memory")) {
        return makeFailure("resource_exhausted", "System ran out of memory", "The system ran out of memory during encoding. Try reducing concurrent jobs.", "Reduce concurrent jobs or rerun under lower system load.");
    }
    if (normalized.includes("transcode_failed") || normalized.includes("ffmpeg exited")) {
        return makeFailure("unknown_ffmpeg_failure", "FFmpeg failed", "FFmpeg failed during encoding. This is often caused by a corrupt source file or an encoder configuration issue. Check the logs below for the specific FFmpeg error.", "Inspect the FFmpeg output in the job logs for the exact failure.");
    }
    if (normalized.includes("probing failed")) {
        return makeFailure("analysis_failed", "Analysis failed", "FFprobe could not read this file. It may be corrupt or in an unsupported format.", "Inspect the source file manually or run Library Doctor to confirm whether it is readable.");
    }
    if (normalized.includes("planning_failed") || normalized.includes("planner")) {
        return makeFailure("planning_failed", "Planner failed", "An error occurred while planning the transcode. Check the logs below for details.", "Treat repeated planner failures as a bug and inspect the logs for the triggering input.");
    }
    if (normalized.includes("output_size=0") || normalized.includes("output was empty")) {
        return makeFailure("unknown_ffmpeg_failure", "Empty output produced", "Encoding produced an empty output file. This usually means FFmpeg crashed silently. Check the logs below for FFmpeg output.", "Inspect the FFmpeg logs before retrying the job.");
    }
    if (normalized.includes("videotoolbox") || normalized.includes("vt_compression") || normalized.includes("err=-12902") || normalized.includes("mediaserverd") || normalized.includes("no capable devices")) {
        return makeFailure("hardware_backend_failure", "Hardware backend failed", "The VideoToolbox hardware encoder failed. This can happen when the GPU is busy, the file uses an unsupported pixel format, or macOS Media Services are unavailable.", "Retry the job. If it keeps failing, check the hardware probe log or enable CPU fallback in Settings -> Hardware.");
    }
    if (normalized.includes("encoder fallback") || normalized.includes("fallback detected")) {
        return makeFailure("fallback_blocked", "Fallback blocked by policy", "The hardware encoder was unavailable and fell back to software encoding, which was not allowed by your settings.", "Enable CPU fallback in Settings -> Hardware, or retry when the GPU is less busy.");
    }
    if (normalized.includes("ffmpeg failed")) {
        return makeFailure("unknown_ffmpeg_failure", "FFmpeg failed", "FFmpeg failed during encoding. Check the logs below for the specific error. Common causes: unsupported pixel format, codec not available, or corrupt source file.", "Inspect the FFmpeg output in the job logs for the exact failure.");
    }

    return makeFailure("legacy_failure", "Failure recorded", summary, "Inspect the job logs for additional context.");
}

export function explainFailureLogs(logs: LogEntry[]): ExplanationView | null {
    const sourceEntries = logs.filter((entry) => entry.message.trim().length > 0);
    if (sourceEntries.length === 0) return null;

    const recentEntries = sourceEntries.slice(-25);
    const prioritizedEntry = [...recentEntries]
        .reverse()
        .find((entry) => ["error", "warn", "warning"].includes(entry.level.toLowerCase()))
        ?? recentEntries[recentEntries.length - 1];
    const combined = recentEntries.map((entry) => entry.message).join("\n");
    const normalized = combined.toLowerCase();
    const primaryMessage = prioritizedEntry.message;

    const makeFailure = (
        code: string,
        summary: string,
        detail: string,
        operator_guidance: string | null,
    ): ExplanationView => ({
        category: "failure",
        code,
        summary,
        detail,
        operator_guidance,
        measured: {},
        legacy_reason: primaryMessage,
    });

    if (normalized.includes("qscale not available for encoder")) {
        return makeFailure("encoder_parameter_mismatch", "Encoder settings rejected", "FFmpeg rejected the selected encoder parameters for this hardware backend. The command was accepted by Alchemist, but the encoder refused to start with the generated rate-control options.", "Check the FFmpeg output below for the rejected flag and compare it with your current codec and hardware settings.");
    }
    if (normalized.includes("videotoolbox") || normalized.includes("vt_compression") || normalized.includes("mediaserverd") || normalized.includes("no capable devices") || normalized.includes("could not open encoder before eof")) {
        return makeFailure("hardware_backend_failure", "Hardware backend failed", "The hardware encoder failed to initialize or produce output. This usually points to an unsupported source format, a backend-specific FFmpeg parameter issue, or temporary media-services instability on the host.", "Retry the job first. If it fails again, inspect the backend-specific FFmpeg lines below and verify hardware fallback settings.");
    }
    if (normalized.includes("nothing was written into output file") || normalized.includes("received no packets") || normalized.includes("output_size=0") || normalized.includes("conversion failed")) {
        return makeFailure("empty_output", "Encoder produced no output", "FFmpeg ran, but no media packets were successfully written to the output file. This usually means the encoder crashed or rejected the stream before real output started.", "Check the lines around the first FFmpeg error below to find the encoder/backend-specific cause.");
    }
    if (normalized.includes("unknown encoder") || normalized.includes("encoder not found")) {
        return makeFailure("encoder_unavailable", "Required encoder unavailable", "The selected encoder is not available in this FFmpeg build.", "Verify FFmpeg encoder support and your hardware settings, then retry the job.");
    }
    if (normalized.includes("invalid data found") || normalized.includes("moov atom not found") || normalized.includes("error while decoding") || normalized.includes("corrupt")) {
        return makeFailure("corrupt_or_unreadable_media", "Media could not be decoded", "FFmpeg hit a decode/read error while processing the source. The file is likely corrupt, incomplete, or not fully readable.", "Try playing the file manually or run Library Doctor to confirm whether the source is intact.");
    }
    if (normalized.includes("permission denied") || normalized.includes("operation not permitted") || normalized.includes("read-only file system") || normalized.includes("no such file or directory")) {
        return makeFailure("path_or_permission_failure", "Path or permission failure", "Alchemist could not read the source or write the output at the required path.", "Check that the source still exists and that the Alchemist process user can read and write the configured paths.");
    }
    if (normalized.includes("ffmpeg failed") || normalized.includes("transcode failed")) {
        return makeFailure("unknown_ffmpeg_failure", "FFmpeg failed", "FFmpeg reported a fatal encoding error, but no more specific structured explanation was stored for this job.", "Inspect the raw FFmpeg output below for the first concrete encoder or media error.");
    }

    return null;
}

export function normalizeDecisionExplanation(
    explanation: ExplanationView | null | undefined,
    legacyReason?: string | null,
): ExplanationView | null {
    if (explanation) return explanation;
    if (legacyReason) return humanizeSkipReason(legacyReason);
    return null;
}

export function normalizeFailureExplanation(
    explanation: ExplanationView | null | undefined,
    legacySummary?: string | null,
    logs?: LogEntry[] | null,
): ExplanationView | null {
    if (explanation) return explanation;
    if (logs && logs.length > 0) {
        const parsedFromLogs = explainFailureLogs(logs);
        if (parsedFromLogs) return parsedFromLogs;
    }
    if (legacySummary) return explainFailureSummary(legacySummary);
    return null;
}
