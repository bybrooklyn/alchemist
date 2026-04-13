// Shared types for job management components

export interface ExplanationView {
    category: "decision" | "failure";
    code: string;
    summary: string;
    detail: string;
    operator_guidance: string | null;
    measured: Record<string, string | number | boolean | null>;
    legacy_reason: string;
}

export interface ExplanationPayload {
    category: "decision" | "failure";
    code: string;
    summary: string;
    detail: string;
    operator_guidance: string | null;
    measured: Record<string, string | number | boolean | null>;
    legacy_reason: string;
}

export interface Job {
    id: number;
    input_path: string;
    output_path: string;
    status: string;
    priority: number;
    progress: number;
    created_at: string;
    updated_at: string;
    attempt_count: number;
    vmaf_score?: number;
    decision_reason?: string;
    decision_explanation?: ExplanationPayload | null;
    encoder?: string;
}

export interface JobMetadata {
    duration_secs: number;
    codec_name: string;
    width: number;
    height: number;
    bit_depth?: number;
    size_bytes: number;
    video_bitrate_bps?: number;
    container_bitrate_bps?: number;
    fps: number;
    container: string;
    audio_codec?: string;
    audio_channels?: number;
    dynamic_range?: string;
}

export interface EncodeStats {
    input_size_bytes: number;
    output_size_bytes: number;
    compression_ratio: number;
    encode_time_seconds: number;
    encode_speed: number;
    avg_bitrate_kbps: number;
    vmaf_score?: number;
}

export interface EncodeAttempt {
    id: number;
    attempt_number: number;
    started_at: string | null;
    finished_at: string;
    outcome: "completed" | "failed" | "cancelled";
    failure_code: string | null;
    failure_summary: string | null;
    input_size_bytes: number | null;
    output_size_bytes: number | null;
    encode_time_seconds: number | null;
}

export interface LogEntry {
    id: number;
    level: string;
    message: string;
    created_at: string;
}

export interface JobDetail {
    job: Job;
    metadata: JobMetadata | null;
    encode_stats: EncodeStats | null;
    encode_attempts: EncodeAttempt[] | null;
    job_logs: LogEntry[];
    job_failure_summary: string | null;
    decision_explanation: ExplanationPayload | null;
    failure_explanation: ExplanationPayload | null;
    queue_position: number | null;
}

export interface CountMessageResponse {
    count: number;
    message: string;
}

export interface ConfirmConfig {
    title: string;
    body: string;
    confirmLabel: string;
    confirmTone?: "danger" | "primary";
    onConfirm: () => Promise<void> | void;
}

export type TabType = "all" | "active" | "queued" | "completed" | "failed" | "skipped" | "archived";
export type SortField = "updated_at" | "created_at" | "input_path" | "size";

export const SORT_OPTIONS: Array<{ value: SortField; label: string }> = [
    { value: "updated_at", label: "Last Updated" },
    { value: "created_at", label: "Date Added" },
    { value: "input_path", label: "File Name" },
    { value: "size", label: "File Size" },
];

// Pure data utilities

export function isJobActive(job: Job): boolean {
    return ["analyzing", "encoding", "remuxing", "resuming"].includes(job.status);
}

export function retryCountdown(job: Job): string | null {
    if (job.status !== "failed") return null;
    if (!job.attempt_count || job.attempt_count === 0) return null;

    const backoffMins =
        job.attempt_count === 1 ? 5
        : job.attempt_count === 2 ? 15
        : job.attempt_count === 3 ? 60
        : 360;

    const updatedMs = new Date(job.updated_at).getTime();
    const retryAtMs = updatedMs + backoffMins * 60 * 1000;
    const remainingMs = retryAtMs - Date.now();

    if (remainingMs <= 0) return "Retrying soon";

    const remainingMins = Math.ceil(remainingMs / 60_000);
    if (remainingMins < 60) return `Retrying in ${remainingMins}m`;
    const hrs = Math.floor(remainingMins / 60);
    const mins = remainingMins % 60;
    return mins > 0 ? `Retrying in ${hrs}h ${mins}m` : `Retrying in ${hrs}h`;
}

export function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

export function formatDuration(seconds: number): string {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    return [h, m, s].map(v => v.toString().padStart(2, "0")).join(":");
}

export function logLevelClass(level: string): string {
    switch (level.toLowerCase()) {
        case "error":
            return "text-status-error";
        case "warn":
        case "warning":
            return "text-helios-solar";
        default:
            return "text-helios-slate";
    }
}

export function jobDetailEmptyState(status: string): { title: string; detail: string } {
    switch (status) {
        case "queued":
            return {
                title: "Waiting in queue",
                detail: "This job is queued and waiting for an available worker slot.",
            };
        case "analyzing":
            return {
                title: "Analyzing media",
                detail: "Alchemist is reading the file metadata and planning the next action.",
            };
        case "encoding":
            return {
                title: "Encoding in progress",
                detail: "The transcode is running now. Detailed input metadata may appear once analysis data is fully persisted.",
            };
        case "remuxing":
            return {
                title: "Remuxing in progress",
                detail: "The job is copying compatible streams into the target container without re-encoding video.",
            };
        case "resuming":
            return {
                title: "Resuming job",
                detail: "The job is being re-queued and prepared to continue processing.",
            };
        case "failed":
            return {
                title: "No metadata captured",
                detail: "This job failed before Alchemist could persist complete media metadata.",
            };
        case "skipped":
            return {
                title: "No metadata captured",
                detail: "This file was skipped before full media metadata was stored in the job detail view.",
            };
        default:
            return {
                title: "No encode data available",
                detail: "Detailed metadata is not available for this job yet.",
            };
    }
}
