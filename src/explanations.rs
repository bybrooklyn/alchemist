use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationCategory {
    Decision,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Explanation {
    pub category: ExplanationCategory,
    pub code: String,
    pub summary: String,
    pub detail: String,
    pub operator_guidance: Option<String>,
    pub measured: BTreeMap<String, Value>,
    pub legacy_reason: String,
}

impl Explanation {
    pub fn new(
        category: ExplanationCategory,
        code: impl Into<String>,
        summary: impl Into<String>,
        detail: impl Into<String>,
        operator_guidance: Option<String>,
        legacy_reason: impl Into<String>,
    ) -> Self {
        Self {
            category,
            code: code.into(),
            summary: summary.into(),
            detail: detail.into(),
            operator_guidance,
            measured: BTreeMap::new(),
            legacy_reason: legacy_reason.into(),
        }
    }

    pub fn with_measured(mut self, key: impl Into<String>, value: Value) -> Self {
        self.measured.insert(key.into(), value);
        self
    }
}

fn split_legacy_reason(reason: &str) -> (String, BTreeMap<String, Value>) {
    let trimmed = reason.trim();
    if let Some((code, raw_params)) = trimmed.split_once('|') {
        let mut measured = BTreeMap::new();
        for pair in raw_params.split(',') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            if let Some((key, raw_value)) = pair.split_once('=') {
                measured.insert(key.trim().to_string(), parse_primitive(raw_value.trim()));
            }
        }
        (code.trim().to_string(), measured)
    } else {
        (trimmed.to_string(), BTreeMap::new())
    }
}

fn parse_primitive(value: &str) -> Value {
    if value.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if value.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if value.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if let Ok(parsed) = value.parse::<i64>() {
        return json!(parsed);
    }
    if let Ok(parsed) = value.parse::<f64>() {
        return json!(parsed);
    }
    Value::String(value.to_string())
}

fn measured_string(measured: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    measured.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        _ => None,
    })
}

fn measured_f64(measured: &BTreeMap<String, Value>, key: &str) -> Option<f64> {
    measured.get(key).and_then(|value| match value {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    })
}

fn measured_i64(measured: &BTreeMap<String, Value>, key: &str) -> Option<i64> {
    measured.get(key).and_then(|value| match value {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.parse::<i64>().ok(),
        _ => None,
    })
}

fn action_verb(action: &str) -> &'static str {
    match action {
        "remux" => "remux",
        "reject" => "reject",
        "encode" | "transcode" => "transcode",
        _ => "decision",
    }
}

pub fn explanation_to_json(explanation: &Explanation) -> String {
    serde_json::to_string(explanation).unwrap_or_else(|_| "{}".to_string())
}

pub fn explanation_from_json(payload: &str) -> Option<Explanation> {
    serde_json::from_str(payload).ok()
}

pub fn decision_from_legacy(action: &str, legacy_reason: &str) -> Explanation {
    let (legacy_code, measured) = split_legacy_reason(legacy_reason);

    if legacy_reason == "Output path matches input path" {
        return Explanation::new(
            ExplanationCategory::Decision,
            "output_path_matches_input",
            "Output would overwrite source",
            "The configured output path is the same as the source file. Alchemist refused to proceed to avoid overwriting the original file.",
            Some(
                "Go to Settings -> Files and configure a different output suffix or output folder."
                    .to_string(),
            ),
            legacy_reason,
        );
    }

    if legacy_reason == "Output already exists" {
        return Explanation::new(
            ExplanationCategory::Decision,
            "output_already_exists",
            "Output file already exists",
            "A transcoded version of this file already exists at the planned output path, so Alchemist skipped it to avoid duplicating work.",
            Some("Delete the existing output file if you want to run the job again.".to_string()),
            legacy_reason,
        );
    }

    if legacy_reason == "H.264 source prioritized for transcode" {
        return Explanation::new(
            ExplanationCategory::Decision,
            "transcode_h264_source",
            "H.264 source prioritized",
            "This file is H.264, so Alchemist prioritized it for transcoding because H.264 sources are often the easiest place to reclaim storage.",
            None,
            legacy_reason,
        )
        .with_measured("current_codec", json!("h264"));
    }

    if legacy_reason.starts_with("Ready for ") && legacy_reason.contains(" transcode") {
        return Explanation::new(
            ExplanationCategory::Decision,
            "transcode_recommended",
            "Transcode recommended",
            "Alchemist determined this file is a strong candidate for transcoding based on the current codec and measured efficiency.",
            None,
            legacy_reason,
        );
    }

    if legacy_reason == "No suitable encoder available" {
        return Explanation::new(
            ExplanationCategory::Decision,
            "no_suitable_encoder",
            "No suitable encoder available",
            "No encoder was available for the requested output codec under the current hardware and fallback policy.",
            Some("Check Settings -> Hardware, enable CPU fallback, or verify that the expected GPU encoder is available.".to_string()),
            legacy_reason,
        );
    }

    if legacy_reason == "No available encoders for current hardware policy" {
        return Explanation::new(
            ExplanationCategory::Decision,
            "no_available_encoders",
            "No encoders available",
            "The current hardware policy left Alchemist with no available encoders for this job.",
            Some(
                "Check Settings -> Hardware and verify CPU encoding or fallback policy."
                    .to_string(),
            ),
            legacy_reason,
        );
    }

    if legacy_reason.starts_with("Preferred codec ")
        && legacy_reason.ends_with(" unavailable and fallback disabled")
    {
        let codec = legacy_reason
            .trim_start_matches("Preferred codec ")
            .trim_end_matches(" unavailable and fallback disabled");
        return Explanation::new(
            ExplanationCategory::Decision,
            "preferred_codec_unavailable_fallback_disabled",
            "Preferred encoder unavailable",
            format!(
                "The preferred codec ({codec}) is not available and CPU fallback is disabled, so Alchemist did not proceed."
            ),
            Some("Go to Settings -> Hardware and enable CPU fallback, or verify your preferred GPU encoder is available.".to_string()),
            legacy_reason,
        )
        .with_measured("codec", json!(codec));
    }

    match legacy_code.as_str() {
        "analysis_failed" => {
            Explanation::new(
                ExplanationCategory::Decision,
                "analysis_failed",
                "File could not be analyzed",
                format!(
                    "FFprobe failed to read this file. It may be corrupt, incomplete, or in an unsupported format. Error: {}",
                    measured_string(&measured, "error").unwrap_or_else(|| "unknown".to_string())
                ),
                Some("Try playing the file in a media player or run Library Doctor to check for corruption.".to_string()),
                legacy_reason,
            )
            .with_measured(
                "error",
                measured
                    .get("error")
                    .cloned()
                    .unwrap_or_else(|| json!("unknown")),
            )
        }
        "planning_failed" => Explanation::new(
            ExplanationCategory::Decision,
            "planning_failed",
            "Transcode plan could not be created",
            format!(
                "An internal planning error occurred while preparing this job. Error: {}",
                measured_string(&measured, "error").unwrap_or_else(|| "unknown".to_string())
            ),
            Some("Check the logs for details. If this repeats, treat it as a planner bug.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "error",
            measured
                .get("error")
                .cloned()
                .unwrap_or_else(|| json!("unknown")),
        ),
        "already_target_codec" => {
            let codec = measured_string(&measured, "codec").unwrap_or_else(|| "target codec".to_string());
            let bit_depth = measured_i64(&measured, "bit_depth");
            let detail = if let Some(bit_depth) = bit_depth {
                format!("This file is already encoded as {codec} at {bit_depth}-bit depth. Re-encoding it would waste time and could reduce quality.")
            } else {
                format!("This file is already encoded as {codec}. Re-encoding it would waste time and could reduce quality.")
            };

            Explanation::new(
                ExplanationCategory::Decision,
                "already_target_codec",
                "Already in target format",
                detail,
                None,
                legacy_reason,
            )
            .with_measured("codec", json!(codec))
            .with_measured(
                "bit_depth",
                bit_depth.map_or(Value::Null, |value| json!(value)),
            )
        }
        "already_target_codec_wrong_container" => {
            let container =
                measured_string(&measured, "container").unwrap_or_else(|| "mp4".to_string());
            let target_extension = measured_string(&measured, "target_extension")
                .unwrap_or_else(|| "mkv".to_string());
            Explanation::new(
                ExplanationCategory::Decision,
                "already_target_codec_wrong_container",
                "Target codec, wrong container",
                format!(
                    "The file is already in the target codec but wrapped in a {container} container. Alchemist will remux it to {target_extension} without re-encoding."
                ),
                None,
                legacy_reason,
            )
            .with_measured("container", json!(container))
            .with_measured("target_extension", json!(target_extension))
        }
        "bpp_below_threshold" => Explanation::new(
            ExplanationCategory::Decision,
            "bpp_below_threshold",
            "Already efficiently compressed",
            format!(
                "Bits-per-pixel ({:.3}) is below the configured threshold ({:.3}). This file is already efficiently compressed, so transcoding would likely save very little space.",
                measured_f64(&measured, "bpp").unwrap_or_default(),
                measured_f64(&measured, "threshold").unwrap_or_default()
            ),
            Some("Lower the BPP threshold in Settings -> Transcoding if you want more aggressive re-encoding.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "bpp",
            measured.get("bpp").cloned().unwrap_or_else(|| json!(0.0)),
        )
        .with_measured(
            "threshold",
            measured
                .get("threshold")
                .cloned()
                .unwrap_or_else(|| json!(0.0)),
        ),
        "below_min_file_size" => Explanation::new(
            ExplanationCategory::Decision,
            "below_min_file_size",
            "File too small to process",
            format!(
                "File size ({} MB) is below the minimum threshold ({} MB), so the transcoding overhead is not worth it.",
                measured_i64(&measured, "size_mb").unwrap_or_default(),
                measured_i64(&measured, "threshold_mb").unwrap_or_default()
            ),
            Some("Lower the minimum file size threshold in Settings -> Transcoding if you want smaller files processed.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "size_mb",
            measured
                .get("size_mb")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        )
        .with_measured(
            "threshold_mb",
            measured
                .get("threshold_mb")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        ),
        "size_reduction_insufficient" => Explanation::new(
            ExplanationCategory::Decision,
            "size_reduction_insufficient",
            "Not enough space would be saved",
            format!(
                "The predicted or measured size reduction ({:.3}) is below the required threshold ({:.3}), so Alchemist rejected the output as not worthwhile.",
                measured_f64(&measured, "reduction")
                    .or_else(|| measured_f64(&measured, "predicted"))
                    .unwrap_or_default(),
                measured_f64(&measured, "threshold").unwrap_or_default(),
            ),
            Some("Lower the size reduction threshold in Settings -> Transcoding if you want to keep smaller wins.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "reduction",
            measured
                .get("reduction")
                .or_else(|| measured.get("predicted"))
                .cloned()
                .unwrap_or_else(|| json!(0.0)),
        )
        .with_measured(
            "threshold",
            measured
                .get("threshold")
                .cloned()
                .unwrap_or_else(|| json!(0.0)),
        )
        .with_measured(
            "output_size",
            measured
                .get("output_size")
                .cloned()
                .unwrap_or(Value::Null),
        ),
        "no_available_encoders" => Explanation::new(
            ExplanationCategory::Decision,
            "no_available_encoders",
            "No encoders available",
            "The current hardware policy left Alchemist with no available encoders for this job.",
            Some(
                "Check Settings -> Hardware and verify CPU encoding or fallback policy."
                    .to_string(),
            ),
            legacy_reason,
        )
        .with_measured(
            "requested_codec",
            measured
                .get("requested_codec")
                .cloned()
                .unwrap_or(Value::Null),
        )
        .with_measured(
            "allow_cpu_fallback",
            measured
                .get("allow_cpu_fallback")
                .cloned()
                .unwrap_or(Value::Null),
        )
        .with_measured(
            "allow_cpu_encoding",
            measured
                .get("allow_cpu_encoding")
                .cloned()
                .unwrap_or(Value::Null),
        ),
        "preferred_codec_unavailable_fallback_disabled" => Explanation::new(
            ExplanationCategory::Decision,
            "preferred_codec_unavailable_fallback_disabled",
            "Preferred encoder unavailable",
            format!(
                "The preferred codec ({}) is not available and CPU fallback is disabled in settings.",
                measured_string(&measured, "codec").unwrap_or_else(|| "target codec".to_string())
            ),
            Some("Go to Settings -> Hardware and enable CPU fallback, or check that your GPU encoder is working correctly.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "codec",
            measured.get("codec").cloned().unwrap_or(Value::Null),
        ),
        "no_suitable_encoder" => Explanation::new(
            ExplanationCategory::Decision,
            "no_suitable_encoder",
            "No suitable encoder available",
            "No encoder was found for the requested output codec under the current hardware and fallback policy.".to_string(),
            Some("Check Settings -> Hardware. Enable CPU fallback, or verify the expected GPU encoder is available.".to_string()),
            legacy_reason,
        ),
        "incomplete_metadata" => Explanation::new(
            ExplanationCategory::Decision,
            "incomplete_metadata",
            "Missing file metadata",
            format!(
                "FFprobe could not determine the required {} metadata, so Alchemist cannot make a defensible transcode decision.",
                measured_string(&measured, "missing").unwrap_or_else(|| "file".to_string())
            ),
            Some("Run Library Doctor or inspect the file manually to confirm it is readable.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "missing",
            measured
                .get("missing")
                .cloned()
                .unwrap_or_else(|| json!("metadata")),
        ),
        "quality_below_threshold" => Explanation::new(
            ExplanationCategory::Decision,
            "quality_below_threshold",
            "Quality check failed",
            "The output failed the configured quality gate, so Alchemist reverted it instead of promoting a lower-quality file.".to_string(),
            Some("Adjust the quality thresholds in Settings -> Quality if this is stricter than you want.".to_string()),
            legacy_reason,
        )
        .with_measured(
            "metric",
            measured
                .get("metric")
                .cloned()
                .unwrap_or_else(|| json!("vmaf")),
        )
        .with_measured(
            "score",
            measured.get("score").cloned().unwrap_or(Value::Null),
        )
        .with_measured(
            "threshold",
            measured.get("threshold").cloned().unwrap_or(Value::Null),
        ),
        "transcode_h264_source" => Explanation::new(
            ExplanationCategory::Decision,
            "transcode_h264_source",
            "H.264 source prioritized",
            "The file is H.264, which is typically a strong candidate for reclaiming space, so Alchemist prioritized it for transcoding.".to_string(),
            None,
            legacy_reason,
        )
        .with_measured(
            "current_codec",
            measured
                .get("current_codec")
                .cloned()
                .unwrap_or_else(|| json!("h264")),
        ),
        "transcode_recommended" => Explanation::new(
            ExplanationCategory::Decision,
            "transcode_recommended",
            "Transcode recommended",
            "Alchemist determined the file should be transcoded based on the target codec, current codec, and measured efficiency.".to_string(),
            None,
            legacy_reason,
        )
        .with_measured(
            "target_codec",
            measured
                .get("target_codec")
                .cloned()
                .unwrap_or(Value::Null),
        )
        .with_measured(
            "current_codec",
            measured
                .get("current_codec")
                .cloned()
                .unwrap_or(Value::Null),
        )
        .with_measured("bpp", measured.get("bpp").cloned().unwrap_or(Value::Null)),
        "remux_mp4_to_mkv_stream_copy" => Explanation::new(
            ExplanationCategory::Decision,
            "remux_mp4_to_mkv_stream_copy",
            "Remux only",
            "The file can be moved into the target container with stream copy, so Alchemist will remux it without re-encoding.".to_string(),
            None,
            legacy_reason,
        ),
        _ => Explanation::new(
            ExplanationCategory::Decision,
            format!("{}_{}", action_verb(action), legacy_code.to_ascii_lowercase().replace([' ', ':', '(', ')', '.'], "_")),
            "Decision recorded",
            legacy_reason.to_string(),
            None,
            legacy_reason,
        ),
    }
}

pub fn failure_from_summary(summary: &str) -> Explanation {
    let normalized = summary.to_ascii_lowercase();

    if normalized.contains("cancelled") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "cancelled",
            "Job was cancelled",
            "The job was cancelled before processing completed. The original file is unchanged.",
            None,
            summary,
        );
    }

    if normalized.contains("no such file or directory") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "source_missing",
            "Source file missing",
            "The source file could not be found. It may have been moved, deleted, or become unavailable.",
            Some(
                "Check that the source path still exists and is readable by Alchemist.".to_string(),
            ),
            summary,
        );
    }

    if normalized.contains("invalid data found")
        || normalized.contains("moov atom not found")
        || normalized.contains("probing failed")
    {
        return Explanation::new(
            ExplanationCategory::Failure,
            "corrupt_or_unreadable_media",
            "Media could not be read",
            "FFmpeg or FFprobe could not read the media successfully. The file may be corrupt, incomplete, or in an unsupported format.",
            Some("Run Library Doctor or try opening the file in a media player to confirm it is readable.".to_string()),
            summary,
        );
    }

    if normalized.contains("permission denied") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "permission_denied",
            "Permission denied",
            "Alchemist does not have permission to read from or write to a required path.",
            Some("Check filesystem permissions and ensure the process user can access the source and output paths.".to_string()),
            summary,
        );
    }

    if normalized.contains("unknown encoder") || normalized.contains("encoder not found") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "encoder_unavailable",
            "Required encoder unavailable",
            "The required encoder is not available in the current FFmpeg build or hardware environment.",
            Some(
                "Check Settings -> Hardware, FFmpeg encoder availability, and fallback settings."
                    .to_string(),
            ),
            summary,
        );
    }

    if normalized.contains("videotoolbox")
        || normalized.contains("vt_compression")
        || normalized.contains("mediaserverd")
        || normalized.contains("no capable devices")
        || normalized.contains("vaapi")
        || normalized.contains("qsv")
        || normalized.contains("amf")
        || normalized.contains("nvenc")
    {
        return Explanation::new(
            ExplanationCategory::Failure,
            "hardware_backend_failure",
            "Hardware backend failed",
            "The selected hardware encoding backend failed during processing.",
            Some("Retry the job, check the hardware probe log, or enable CPU fallback if appropriate.".to_string()),
            summary,
        );
    }

    if normalized.contains("fallback detected")
        || normalized.contains("fallback disabled")
        || normalized.contains("cpu fallback")
    {
        return Explanation::new(
            ExplanationCategory::Failure,
            "fallback_blocked",
            "Fallback blocked by policy",
            "The job could not continue because the required fallback path was disallowed by the current hardware policy.",
            Some("Enable CPU fallback in Settings -> Hardware or make the preferred encoder available.".to_string()),
            summary,
        );
    }

    if normalized.contains("out of memory") || normalized.contains("cannot allocate memory") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "resource_exhausted",
            "System ran out of memory",
            "The system ran out of memory or another required resource during processing.",
            Some("Reduce concurrent jobs, lower workload pressure, or retry on a less loaded machine.".to_string()),
            summary,
        );
    }

    if normalized.contains("planner failed") || normalized.contains("planning_failed") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "planning_failed",
            "Planner failed",
            "An internal error occurred while building the transcode plan.",
            Some(
                "Check the job logs for details. If this repeats, treat it as a planner bug."
                    .to_string(),
            ),
            summary,
        );
    }

    if normalized.contains("analysis_failed") || normalized.contains("ffprobe failed") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "analysis_failed",
            "Analysis failed",
            "An error occurred while analyzing the input media before planning or encoding.",
            Some("Inspect the job logs and verify the media file is readable.".to_string()),
            summary,
        );
    }

    if normalized.contains("finalization failed") || normalized.contains("finalize_failed") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "finalize_failed",
            "Finalization failed",
            "The job encoded or remuxed successfully, but final promotion or verification failed.",
            Some("Inspect filesystem state and job logs before retrying.".to_string()),
            summary,
        );
    }

    if normalized.contains("vmaf")
        || normalized.contains("quality gate failed")
        || normalized.contains("quality check failed")
    {
        return Explanation::new(
            ExplanationCategory::Failure,
            "quality_check_failed",
            "Quality check failed",
            "The output did not pass the configured quality guard, so Alchemist refused to keep it.",
            Some("Adjust the quality thresholds in Settings -> Quality if this is stricter than intended.".to_string()),
            summary,
        );
    }

    if normalized.contains("ffmpeg failed") || normalized.contains("transcode failed") {
        return Explanation::new(
            ExplanationCategory::Failure,
            "unknown_ffmpeg_failure",
            "FFmpeg failed",
            "FFmpeg failed during processing. The logs contain the most specific error details available.",
            Some("Inspect the FFmpeg output in the job logs for the root cause.".to_string()),
            summary,
        );
    }

    Explanation::new(
        ExplanationCategory::Failure,
        "unknown_failure",
        "Failure recorded",
        summary.to_string(),
        Some("Inspect the job logs for additional context.".to_string()),
        summary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_decision_payloads() {
        let explanation =
            decision_from_legacy("skip", "bpp_below_threshold|bpp=0.043,threshold=0.050");
        assert_eq!(explanation.code, "bpp_below_threshold");
        assert_eq!(explanation.category, ExplanationCategory::Decision);
        assert_eq!(measured_f64(&explanation.measured, "bpp"), Some(0.043));
    }

    #[test]
    fn parses_failure_summaries() {
        let explanation = failure_from_summary("Transcode failed: Unknown encoder 'missing'");
        assert_eq!(explanation.code, "encoder_unavailable");
        assert_eq!(explanation.category, ExplanationCategory::Failure);
    }

    #[test]
    fn round_trips_json_payload() {
        let explanation = decision_from_legacy(
            "transcode",
            "transcode_recommended|target_codec=av1,current_codec=hevc,bpp=0.120",
        );
        let payload = explanation_to_json(&explanation);
        assert_eq!(explanation_from_json(&payload), Some(explanation));
    }
}
