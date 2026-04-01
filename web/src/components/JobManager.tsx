import { useState, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import {
    Search, RefreshCw, Trash2, Ban,
    Clock, X, Info, Activity, Database, Zap, Maximize2, MoreHorizontal, ArrowDown, ArrowUp, AlertCircle
} from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { useDebouncedValue } from "../lib/useDebouncedValue";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { motion, AnimatePresence } from "framer-motion";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

function focusableElements(root: HTMLElement): HTMLElement[] {
    const selector = [
        "a[href]",
        "button:not([disabled])",
        "input:not([disabled])",
        "select:not([disabled])",
        "textarea:not([disabled])",
        "[tabindex]:not([tabindex='-1'])",
    ].join(",");

    return Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(
        (element) => !element.hasAttribute("disabled")
    );
}

export interface SkipDetail {
    summary: string;
    detail: string;
    action: string | null;
    measured: Record<string, string>;
}

function formatReductionPercent(value?: string): string {
    if (!value) {
        return "?";
    }

    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? `${(parsed * 100).toFixed(0)}%` : value;
}

export function humanizeSkipReason(reason: string): SkipDetail {
    const pipeIdx = reason.indexOf("|");
    const key = pipeIdx === -1
        ? reason.trim()
        : reason.slice(0, pipeIdx).trim();
    const paramStr = pipeIdx === -1 ? "" : reason.slice(pipeIdx + 1);

    const measured: Record<string, string> = {};
    for (const pair of paramStr.split(",")) {
        const [rawKey, ...rawValueParts] = pair.split("=");
        if (!rawKey || rawValueParts.length === 0) {
            continue;
        }

        measured[rawKey.trim()] = rawValueParts.join("=").trim();
    }

    const fallbackDisabledMatch = key.match(
        /^Preferred codec\s+(.+?)\s+unavailable and fallback disabled$/i
    );
    if (fallbackDisabledMatch) {
        measured.codec ??= fallbackDisabledMatch[1];
        return {
            summary: "Preferred encoder unavailable",
            detail: `The preferred codec (${measured.codec ?? "target codec"}) is not available and CPU fallback is disabled in settings.`,
            action: "Go to Settings -> Hardware and enable CPU fallback, or check that your GPU encoder is working correctly.",
            measured,
        };
    }

    switch (key) {
        case "analysis_failed":
            return {
                summary: "File could not be analyzed",
                detail: `FFprobe failed to read this file. It may be corrupt, incomplete, or in an unsupported format. Error: ${measured.error ?? "unknown"}`,
                action: "Try playing the file in VLC or another media player. If it plays fine, re-run the scan. If not, the file may be damaged.",
                measured,
            };

        case "planning_failed":
            return {
                summary: "Transcoding plan could not be created",
                detail: `An internal error occurred while planning the transcode for this file. This is likely a bug. Error: ${measured.error ?? "unknown"}`,
                action: "Check the logs below for details. If this happens repeatedly, please report it as a bug.",
                measured,
            };

        case "already_target_codec":
            return {
                summary: "Already in target format",
                detail: `This file is already encoded as ${measured.codec ?? "the target codec"}${measured.bit_depth ? ` at ${measured.bit_depth}-bit` : ""}. Re-encoding would waste time and could reduce quality.`,
                action: null,
                measured,
            };

        case "already_target_codec_wrong_container":
            return {
                summary: "Target codec, wrong container",
                detail: `The video is already in the right codec but wrapped in a ${measured.container ?? "MP4"} container. Alchemist will remux it to ${measured.target_extension ?? "MKV"} - fast and lossless, no quality loss.`,
                action: null,
                measured,
            };

        case "bpp_below_threshold":
            return {
                summary: "Already efficiently compressed",
                detail: `Bits-per-pixel (${measured.bpp ?? "?"}) is below the minimum threshold (${measured.threshold ?? "?"}). This file is already well-compressed - transcoding it would spend significant time for minimal space savings.`,
                action: "If you want to force transcoding, lower the BPP threshold in Settings -> Transcoding.",
                measured,
            };

        case "below_min_file_size":
            return {
                summary: "File too small to process",
                detail: `File size (${measured.size_mb ?? "?"}MB) is below the minimum threshold (${measured.threshold_mb ?? "?"}MB). Small files aren't worth the transcoding overhead.`,
                action: "Lower the minimum file size threshold in Settings -> Transcoding if you want small files processed.",
                measured,
            };

        case "size_reduction_insufficient":
            return {
                summary: "Not enough space would be saved",
                detail: `The predicted size reduction (${formatReductionPercent(measured.predicted)}) is below the required threshold (${formatReductionPercent(measured.threshold)}). Transcoding this file wouldn't recover meaningful storage.`,
                action: "Lower the size reduction threshold in Settings -> Transcoding to encode files with smaller savings.",
                measured,
            };

        case "no_suitable_encoder":
            return {
                summary: "No encoder available",
                detail: `No encoder was found for ${measured.codec ?? "the target codec"}. Hardware detection may have failed, or CPU fallback is disabled.`,
                action: "Check Settings -> Hardware. Enable CPU fallback, or verify your GPU is detected correctly.",
                measured,
            };

        case "Output path matches input path":
            return {
                summary: "Output would overwrite source",
                detail: "The configured output path is the same as the source file. Alchemist refused to proceed to avoid overwriting your original file.",
                action: "Go to Settings -> Files and configure a different output suffix or output folder.",
                measured,
            };

        case "Output already exists":
            return {
                summary: "Output file already exists",
                detail: "A transcoded version of this file already exists at the output path. Alchemist skipped it to avoid duplicating work.",
                action: "If you want to re-transcode it, delete the existing output file first, then retry the job.",
                measured,
            };

        case "incomplete_metadata":
            return {
                summary: "Missing file metadata",
                detail: `FFprobe could not determine the ${measured.missing ?? "required metadata"} for this file. Without reliable metadata Alchemist cannot make a valid transcoding decision.`,
                action: "Run a Library Doctor scan to check if this file is corrupt. Try playing it in a media player to confirm it is readable.",
                measured,
            };

        case "already_10bit":
            return {
                summary: "Already 10-bit",
                detail: "This file is already encoded in high-quality 10-bit depth. Re-encoding it could reduce quality.",
                action: null,
                measured,
            };

        case "remux: mp4_to_mkv_stream_copy":
            return {
                summary: "Remuxed (no re-encode)",
                detail: "This file was remuxed from MP4 to MKV using stream copy - fast and lossless. No quality was lost.",
                action: null,
                measured,
            };

        case "Low quality (VMAF)":
            return {
                summary: "Quality check failed",
                detail: "The encoded file scored below the minimum VMAF quality threshold. Alchemist rejected the output to protect quality.",
                action: "The original file has been preserved. You can lower the VMAF threshold in Settings -> Quality, or disable VMAF checking entirely.",
                measured,
            };

        default:
            return {
                summary: "Decision recorded",
                detail: reason,
                action: null,
                measured,
            };
    }
}

function explainFailureSummary(summary: string): string {
    const normalized = summary.toLowerCase();

    if (normalized.includes("cancelled")) {
        return "This job was cancelled before encoding completed. The original file is untouched.";
    }
    if (normalized.includes("no such file or directory")) {
        return "The source file could not be found. It may have been moved or deleted.";
    }
    if (normalized.includes("invalid data found") || normalized.includes("moov atom not found")) {
        return "This file appears to be corrupt or incomplete. Try running a Library Doctor scan.";
    }
    if (normalized.includes("permission denied")) {
        return "Alchemist doesn't have permission to read this file. Check the file permissions.";
    }
    if (normalized.includes("encoder not found") || normalized.includes("unknown encoder")) {
        return "The required encoder is not available in your FFmpeg installation.";
    }
    if (normalized.includes("out of memory") || normalized.includes("cannot allocate memory")) {
        return "The system ran out of memory during encoding. Try reducing concurrent jobs.";
    }
    if (normalized.includes("transcode_failed") || normalized.includes("ffmpeg exited")) {
        return "FFmpeg failed during encoding. This is often caused by a corrupt source file or an encoder configuration issue. Check the logs below for the specific FFmpeg error.";
    }
    if (normalized.includes("probing failed")) {
        return "FFprobe could not read this file. It may be corrupt or in an unsupported format.";
    }
    if (normalized.includes("planning_failed") || normalized.includes("planner")) {
        return "An error occurred while planning the transcode. Check the logs below for details.";
    }
    if (normalized.includes("output_size=0") || normalized.includes("output was empty")) {
        return "Encoding produced an empty output file. This usually means FFmpeg crashed silently. Check the logs below for FFmpeg output.";
    }

    return summary;
}

function logLevelClass(level: string): string {
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

interface Job {
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
    encoder?: string;
}

function retryCountdown(job: Job): string | null {
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

interface JobMetadata {
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

interface EncodeStats {
    input_size_bytes: number;
    output_size_bytes: number;
    compression_ratio: number;
    encode_time_seconds: number;
    encode_speed: number;
    avg_bitrate_kbps: number;
    vmaf_score?: number;
}

interface LogEntry {
    id: number;
    level: string;
    message: string;
    created_at: string;
}

interface JobDetail {
    job: Job;
    metadata: JobMetadata | null;
    encode_stats: EncodeStats | null;
    job_logs: LogEntry[];
    job_failure_summary: string | null;
}

interface CountMessageResponse {
    count: number;
    message: string;
}

type TabType = "all" | "active" | "queued" | "completed" | "failed" | "skipped" | "archived";
type SortField = "updated_at" | "created_at" | "input_path" | "size";

const SORT_OPTIONS: Array<{ value: SortField; label: string }> = [
    { value: "updated_at", label: "Last Updated" },
    { value: "created_at", label: "Date Added" },
    { value: "input_path", label: "File Name" },
    { value: "size", label: "File Size" },
];

export default function JobManager() {
    const [jobs, setJobs] = useState<Job[]>([]);
    const [loading, setLoading] = useState(true);
    const [selected, setSelected] = useState<Set<number>>(new Set());
    const [activeTab, setActiveTab] = useState<TabType>("all");
    const [searchInput, setSearchInput] = useState("");
    const debouncedSearch = useDebouncedValue(searchInput, 350);
    const [page, setPage] = useState(1);
    const [sortBy, setSortBy] = useState<SortField>("updated_at");
    const [sortDesc, setSortDesc] = useState(true);
    const [refreshing, setRefreshing] = useState(false);
    const [focusedJob, setFocusedJob] = useState<JobDetail | null>(null);
    const [detailLoading, setDetailLoading] = useState(false);
    const [actionError, setActionError] = useState<string | null>(null);
    const [menuJobId, setMenuJobId] = useState<number | null>(null);
    const menuRef = useRef<HTMLDivElement | null>(null);
    const detailDialogRef = useRef<HTMLDivElement | null>(null);
    const detailLastFocusedRef = useRef<HTMLElement | null>(null);
    const confirmOpenRef = useRef(false);
    const encodeStartTimes = useRef<Map<number, number>>(new Map());
    const [confirmState, setConfirmState] = useState<{
        title: string;
        body: string;
        confirmLabel: string;
        confirmTone?: "danger" | "primary";
        onConfirm: () => Promise<void> | void;
    } | null>(null);
    const [tick, setTick] = useState(0);

    useEffect(() => {
        const id = window.setInterval(() => setTick(t => t + 1), 30_000);
        return () => window.clearInterval(id);
    }, []);

    const isJobActive = (job: Job) => ["analyzing", "encoding", "remuxing", "resuming"].includes(job.status);

    const formatJobActionError = (error: unknown, fallback: string) => {
        if (!isApiError(error)) {
            return fallback;
        }

        const blocked = Array.isArray((error.body as { blocked?: unknown } | undefined)?.blocked)
            ? ((error.body as { blocked?: Array<{ id?: number; status?: string }> }).blocked ?? [])
            : [];
        if (blocked.length === 0) {
            return error.message;
        }

        const summary = blocked
            .map((job) => `#${job.id ?? "?"} (${job.status ?? "unknown"})`)
            .join(", ");
        return `${error.message}: ${summary}`;
    };

    // Filter mapping
    const getStatusFilter = (tab: TabType) => {
        switch (tab) {
            case "active": return ["analyzing", "encoding", "remuxing", "resuming"];
            case "queued": return ["queued"];
            case "completed": return ["completed"];
            case "failed": return ["failed", "cancelled"];
            case "skipped": return ["skipped"];
            default: return [];
        }
    };

    const fetchJobs = useCallback(async (silent = false) => {
        if (!silent) {
            setRefreshing(true);
        }
        try {
            const params = new URLSearchParams({
                limit: "50",
                page: page.toString(),
                sort: sortBy,
                sort_desc: String(sortDesc),
                archived: String(activeTab === "archived"),
            });
            params.set("sort_by", sortBy);

            const statusFilter = getStatusFilter(activeTab);
            if (statusFilter.length > 0) {
                params.set("status", statusFilter.join(","));
            }
            if (debouncedSearch) {
                params.set("search", debouncedSearch);
            }

            const data = await apiJson<Job[]>(`/api/jobs/table?${params}`);
            setJobs((prev) =>
                data.map((serverJob) => {
                    const local = prev.find((j) => j.id === serverJob.id);
                    const terminal = ["completed", "skipped", "failed", "cancelled"];
                    if (local && terminal.includes(local.status)) {
                        // Keep the terminal state from SSE to prevent flickering back to a stale poll state.
                        return { ...serverJob, status: local.status };
                    }
                    return serverJob;
                })
            );
            setActionError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch jobs";
            setActionError(message);
            if (!silent) {
                showToast({ kind: "error", title: "Jobs", message });
            }
        } finally {
            setLoading(false);
            if (!silent) {
                setRefreshing(false);
            }
        }
    }, [activeTab, debouncedSearch, page, sortBy, sortDesc]);

    const fetchJobsRef = useRef<() => Promise<void>>(async () => undefined);

    useEffect(() => {
        fetchJobsRef.current = async () => {
            await fetchJobs(true);
        };
    }, [fetchJobs]);

    useEffect(() => {
        void fetchJobs(false);
    }, [fetchJobs]);

    useEffect(() => {
        const pollVisible = () => {
            if (document.visibilityState === "visible") {
                void fetchJobsRef.current();
            }
        };

        const interval = window.setInterval(pollVisible, 5000);
        document.addEventListener("visibilitychange", pollVisible);
        return () => {
            window.clearInterval(interval);
            document.removeEventListener("visibilitychange", pollVisible);
        };
    }, []);

    useEffect(() => {
        let eventSource: EventSource | null = null;
        let cancelled = false;
        let reconnectTimeout: number | null = null;
        let reconnectAttempts = 0;

        const getReconnectDelay = () => {
            // Exponential backoff: 1s, 2s, 4s, 8s, 16s, max 30s
            const baseDelay = 1000;
            const maxDelay = 30000;
            const delay = Math.min(baseDelay * Math.pow(2, reconnectAttempts), maxDelay);
            // Add jitter (±25%) to prevent thundering herd
            const jitter = delay * 0.25 * (Math.random() * 2 - 1);
            return Math.round(delay + jitter);
        };

        const connect = () => {
            if (cancelled) return;
            eventSource?.close();
            eventSource = new EventSource("/api/events");

            eventSource.onopen = () => {
                // Reset reconnect attempts on successful connection
                reconnectAttempts = 0;
            };

            eventSource.addEventListener("status", (e) => {
                try {
                    const { job_id, status } = JSON.parse(e.data) as {
                        job_id: number;
                        status: string;
                    };
                    if (status === "encoding") {
                        encodeStartTimes.current.set(job_id, Date.now());
                    } else {
                        encodeStartTimes.current.delete(job_id);
                    }
                    setJobs((prev) =>
                        prev.map((job) =>
                            job.id === job_id ? { ...job, status } : job
                        )
                    );
                } catch {
                    /* ignore malformed */
                }
            });

            eventSource.addEventListener("progress", (e) => {
                try {
                    const { job_id, percentage } = JSON.parse(e.data) as {
                        job_id: number;
                        percentage: number;
                    };
                    setJobs((prev) =>
                        prev.map((job) =>
                            job.id === job_id ? { ...job, progress: percentage } : job
                        )
                    );
                } catch {
                    /* ignore malformed */
                }
            });

            eventSource.addEventListener("decision", () => {
                // Re-fetch full job list when decisions are made
                void fetchJobsRef.current();
            });

            eventSource.onerror = () => {
                eventSource?.close();
                if (!cancelled) {
                    reconnectAttempts++;
                    const delay = getReconnectDelay();
                    reconnectTimeout = window.setTimeout(connect, delay);
                }
            };
        };

        connect();

        return () => {
            cancelled = true;
            eventSource?.close();
            if (reconnectTimeout !== null) {
                window.clearTimeout(reconnectTimeout);
            }
        };
    }, []);

    useEffect(() => {
        const encodingJobIds = new Set<number>();
        const now = Date.now();

        for (const job of jobs) {
            if (job.status !== "encoding") {
                continue;
            }

            encodingJobIds.add(job.id);
            if (!encodeStartTimes.current.has(job.id)) {
                encodeStartTimes.current.set(job.id, now);
            }
        }

        for (const jobId of Array.from(encodeStartTimes.current.keys())) {
            if (!encodingJobIds.has(jobId)) {
                encodeStartTimes.current.delete(jobId);
            }
        }
    }, [jobs]);

    useEffect(() => {
        if (!menuJobId) return;
        const handleClick = (event: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
                setMenuJobId(null);
            }
        };
        document.addEventListener("mousedown", handleClick);
        return () => document.removeEventListener("mousedown", handleClick);
    }, [menuJobId]);

    useEffect(() => {
        confirmOpenRef.current = confirmState !== null;
    }, [confirmState]);

    useEffect(() => {
        if (!focusedJob) {
            return;
        }

        detailLastFocusedRef.current = document.activeElement as HTMLElement | null;

        const root = detailDialogRef.current;
        if (root) {
            const focusables = focusableElements(root);
            if (focusables.length > 0) {
                focusables[0].focus();
            } else {
                root.focus();
            }
        }

        const onKeyDown = (event: KeyboardEvent) => {
            if (!focusedJob || confirmOpenRef.current) {
                return;
            }

            if (event.key === "Escape") {
                event.preventDefault();
                setFocusedJob(null);
                return;
            }

            if (event.key !== "Tab") {
                return;
            }

            const dialogRoot = detailDialogRef.current;
            if (!dialogRoot) {
                return;
            }

            const focusables = focusableElements(dialogRoot);
            if (focusables.length === 0) {
                event.preventDefault();
                dialogRoot.focus();
                return;
            }

            const first = focusables[0];
            const last = focusables[focusables.length - 1];
            const current = document.activeElement as HTMLElement | null;

            if (event.shiftKey && current === first) {
                event.preventDefault();
                last.focus();
            } else if (!event.shiftKey && current === last) {
                event.preventDefault();
                first.focus();
            }
        };

        document.addEventListener("keydown", onKeyDown);
        return () => {
            document.removeEventListener("keydown", onKeyDown);
            if (detailLastFocusedRef.current) {
                detailLastFocusedRef.current.focus();
            }
        };
    }, [focusedJob]);

    const toggleSelect = (id: number) => {
        const newSet = new Set(selected);
        if (newSet.has(id)) newSet.delete(id);
        else newSet.add(id);
        setSelected(newSet);
    };

    const toggleSelectAll = () => {
        if (selected.size === jobs.length && jobs.length > 0) {
            setSelected(new Set());
        } else {
            setSelected(new Set(jobs.map(j => j.id)));
        }
    };

    const selectedJobs = jobs.filter((job) => selected.has(job.id));
    const hasSelectedActiveJobs = selectedJobs.some(isJobActive);
    const activeCount = jobs.filter((job) => isJobActive(job)).length;
    const failedCount = jobs.filter((job) => ["failed", "cancelled"].includes(job.status)).length;
    const completedCount = jobs.filter((job) => job.status === "completed").length;

    const handleBatch = async (action: "cancel" | "restart" | "delete") => {
        if (selected.size === 0) return;
        setActionError(null);

        try {
            await apiAction("/api/jobs/batch", {
                method: "POST",
                body: JSON.stringify({
                    action,
                    ids: Array.from(selected)
                })
            });
            setSelected(new Set());
            showToast({
                kind: "success",
                title: "Jobs",
                message: `${action[0].toUpperCase()}${action.slice(1)} request sent for selected jobs.`,
            });
            await fetchJobs();
        } catch (e) {
            const message = formatJobActionError(e, "Batch action failed");
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const clearCompleted = async () => {
        setActionError(null);
        try {
            const result = await apiJson<CountMessageResponse>("/api/jobs/clear-completed", {
                method: "POST",
            });
            showToast({ kind: "success", title: "Jobs", message: result.message });
            if (activeTab === "completed" && result.count > 0) {
                showToast({
                    kind: "info",
                    title: "Jobs",
                    message: "Completed jobs archived. View them in the Archived tab.",
                });
            }
            await fetchJobs();
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to clear completed jobs";
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const fetchJobDetails = async (id: number) => {
        setActionError(null);
        setDetailLoading(true);
        try {
            const data = await apiJson<JobDetail>(`/api/jobs/${id}/details`);
            setFocusedJob(data);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch job details";
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        } finally {
            setDetailLoading(false);
        }
    };

    const handleAction = async (id: number, action: "cancel" | "restart" | "delete") => {
        setActionError(null);
        try {
            await apiAction(`/api/jobs/${id}/${action}`, { method: "POST" });
            if (action === "delete") {
                setFocusedJob((current) => (current?.job.id === id ? null : current));
            } else if (focusedJob?.job.id === id) {
                await fetchJobDetails(id);
            }
            await fetchJobs();
            showToast({
                kind: "success",
                title: "Jobs",
                message: `Job ${action} request completed.`,
            });
        } catch (e) {
            const message = formatJobActionError(e, `Job ${action} failed`);
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const handlePriority = async (job: Job, priority: number, label: string) => {
        setActionError(null);
        try {
            await apiAction(`/api/jobs/${job.id}/priority`, {
                method: "POST",
                body: JSON.stringify({ priority }),
            });
            if (focusedJob?.job.id === job.id) {
                setFocusedJob({
                    ...focusedJob,
                    job: {
                        ...focusedJob.job,
                        priority,
                    },
                });
            }
            await fetchJobs();
            showToast({ kind: "success", title: "Jobs", message: `${label} for job #${job.id}.` });
        } catch (e) {
            const message = formatJobActionError(e, "Failed to update priority");
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const formatBytes = (bytes: number) => {
        if (bytes === 0) return "0 B";
        const k = 1024;
        const sizes = ["B", "KB", "MB", "GB", "TB"];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
    };

    const formatDuration = (seconds: number) => {
        const h = Math.floor(seconds / 3600);
        const m = Math.floor((seconds % 3600) / 60);
        const s = Math.floor(seconds % 60);
        return [h, m, s].map(v => v.toString().padStart(2, "0")).join(":");
    };

    const calcEta = (jobId: number, progress: number): string | null => {
        if (progress <= 0 || progress >= 100) {
            return null;
        }

        const startMs = encodeStartTimes.current.get(jobId);
        if (!startMs) {
            return null;
        }

        const elapsedMs = Date.now() - startMs;
        const totalMs = elapsedMs / (progress / 100);
        const remainingMs = totalMs - elapsedMs;
        const remainingSecs = Math.round(remainingMs / 1000);

        if (remainingSecs < 0) {
            return null;
        }
        if (remainingSecs < 60) {
            return `~${remainingSecs}s remaining`;
        }

        const mins = Math.ceil(remainingSecs / 60);
        return `~${mins} min remaining`;
    };

    const getStatusBadge = (status: string) => {
        const styles: Record<string, string> = {
            queued: "bg-helios-slate/10 text-helios-slate border-helios-slate/20",
            analyzing: "bg-blue-500/10 text-blue-500 border-blue-500/20",
            encoding: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
            remuxing: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
            completed: "bg-green-500/10 text-green-500 border-green-500/20",
            failed: "bg-red-500/10 text-red-500 border-red-500/20",
            cancelled: "bg-red-500/10 text-red-500 border-red-500/20",
            skipped: "bg-gray-500/10 text-gray-500 border-gray-500/20",
            archived: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20",
            resuming: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
        };
        return (
            <span className={cn("px-2.5 py-1 rounded-md text-xs font-medium border capitalize", styles[status] || styles.queued)}>
                {status}
            </span>
        );
    };

    const openConfirm = (config: {
        title: string;
        body: string;
        confirmLabel: string;
        confirmTone?: "danger" | "primary";
        onConfirm: () => Promise<void> | void;
    }) => {
        setConfirmState(config);
    };

    const focusedDecision = focusedJob?.job.decision_reason
        ? humanizeSkipReason(focusedJob.job.decision_reason)
        : null;
    const focusedJobLogs = focusedJob?.job_logs ?? [];
    const shouldShowFfmpegOutput = focusedJob
        ? ["failed", "completed", "skipped"].includes(focusedJob.job.status) && focusedJobLogs.length > 0
        : false;
    const completedEncodeStats = focusedJob?.job.status === "completed"
        ? focusedJob.encode_stats
        : null;

    return (
        <div className="space-y-6 relative">
            <div className="flex items-center gap-4 px-1 text-xs text-helios-slate">
                <span>
                    <span className="font-medium text-helios-ink">
                        {activeCount}
                    </span>
                    {" "}active
                </span>
                <span>
                    <span className="font-medium text-red-500">
                        {failedCount}
                    </span>
                    {" "}failed
                </span>
                <span>
                    <span className="font-medium text-emerald-500">
                        {completedCount}
                    </span>
                    {" "}completed
                </span>
            </div>

            {/* Toolbar */}
            <div className="flex flex-col md:flex-row gap-4 justify-between items-center bg-helios-surface/50 p-1 rounded-lg border border-helios-line/10">
                <div className="flex gap-1 p-1 bg-helios-surface border border-helios-line/10 rounded-lg">
                    {(["all", "active", "queued", "completed", "failed", "skipped", "archived"] as TabType[]).map((tab) => (
                        <button
                            key={tab}
                            onClick={() => { setActiveTab(tab); setPage(1); }}
                            className={cn(
                                "px-4 py-1.5 rounded-md text-sm font-medium transition-all capitalize",
                                activeTab === tab
                                    ? "bg-helios-surface-soft text-helios-ink shadow-sm"
                                    : "text-helios-slate hover:text-helios-ink"
                            )}
                        >
                            {tab}
                        </button>
                    ))}
                </div>

                <div className="flex w-full flex-col gap-3 sm:flex-row sm:items-center md:w-auto">
                    <div className="relative flex-1 md:w-64">
                        <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-helios-slate" size={14} />
                        <input
                            type="text"
                            placeholder="Search files..."
                            value={searchInput}
                            onChange={(e) => setSearchInput(e.target.value)}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded-lg pl-9 pr-4 py-2 text-sm text-helios-ink focus:border-helios-solar outline-none"
                        />
                    </div>
                    <div className="flex items-center gap-2">
                        <select
                            value={sortBy}
                            onChange={(e) => {
                                setSortBy(e.target.value as SortField);
                                setPage(1);
                            }}
                            className="h-10 rounded-lg border border-helios-line/20 bg-helios-surface px-3 text-sm text-helios-ink outline-none focus:border-helios-solar"
                        >
                            {SORT_OPTIONS.map((option) => (
                                <option key={option.value} value={option.value}>
                                    {option.label}
                                </option>
                            ))}
                        </select>
                        <button
                            onClick={() => {
                                setSortDesc((current) => !current);
                                setPage(1);
                            }}
                            className="flex h-10 w-10 items-center justify-center rounded-lg border border-helios-line/20 bg-helios-surface text-helios-ink hover:bg-helios-surface-soft"
                            title={sortDesc ? "Sort descending" : "Sort ascending"}
                            aria-label={sortDesc ? "Sort descending" : "Sort ascending"}
                        >
                            {sortDesc ? <ArrowDown size={16} /> : <ArrowUp size={16} />}
                        </button>
                    </div>
                    <button
                        onClick={() => void fetchJobs()}
                        className={cn("p-2 rounded-lg border border-helios-line/20 hover:bg-helios-surface-soft", refreshing && "animate-spin")}
                    >
                        <RefreshCw size={16} />
                    </button>
                </div>
            </div>

            {actionError && (
                <div role="alert" aria-live="polite" className="rounded-lg border border-status-error/30 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {actionError}
                </div>
            )}

            {/* Batch Actions Bar */}
            {selected.size > 0 && (
                <div className="flex items-center justify-between bg-helios-solar/10 border border-helios-solar/20 px-6 py-3 rounded-lg animate-in fade-in slide-in-from-top-2">
                    <div>
                        <span className="text-sm font-bold text-helios-solar">
                            {selected.size} jobs selected
                        </span>
                        {hasSelectedActiveJobs && (
                            <p className="text-xs text-helios-slate mt-1">
                                Active jobs must be cancelled before they can be restarted or deleted.
                            </p>
                        )}
                    </div>
                    <div className="flex gap-2">
                        <button
                            onClick={() =>
                                openConfirm({
                                    title: "Restart jobs",
                                    body: `Restart ${selected.size} selected jobs?`,
                                    confirmLabel: "Restart",
                                    onConfirm: () => handleBatch("restart"),
                                })
                            }
                            disabled={hasSelectedActiveJobs}
                            className="p-2 hover:bg-helios-solar/20 rounded-lg text-helios-solar disabled:opacity-40 disabled:hover:bg-transparent"
                            title="Restart"
                        >
                            <RefreshCw size={18} />
                        </button>
                        <button
                            onClick={() =>
                                openConfirm({
                                    title: "Cancel jobs",
                                    body: `Cancel ${selected.size} selected jobs?`,
                                    confirmLabel: "Cancel",
                                    confirmTone: "danger",
                                    onConfirm: () => handleBatch("cancel"),
                                })
                            }
                            className="p-2 hover:bg-helios-solar/20 rounded-lg text-helios-solar"
                            title="Cancel"
                        >
                            <Ban size={18} />
                        </button>
                        <button
                            onClick={() =>
                                openConfirm({
                                    title: "Delete jobs",
                                    body: `Delete ${selected.size} selected jobs from history?`,
                                    confirmLabel: "Delete",
                                    confirmTone: "danger",
                                    onConfirm: () => handleBatch("delete"),
                                })
                            }
                            disabled={hasSelectedActiveJobs}
                            className="p-2 hover:bg-red-500/10 rounded-lg text-red-500 disabled:opacity-40 disabled:hover:bg-transparent"
                            title="Delete"
                        >
                            <Trash2 size={18} />
                        </button>
                    </div>
                </div>
            )}

            {/* Table */}
            <div className="bg-helios-surface/50 border border-helios-line/20 rounded-lg overflow-hidden shadow-sm">
                <table className="w-full text-left border-collapse">
                    <thead className="bg-helios-surface border-b border-helios-line/20 text-xs font-medium text-helios-slate">
                        <tr>
                            <th className="px-6 py-4 w-10">
                                <input type="checkbox"
                                    checked={jobs.length > 0 && jobs.every(j => selected.has(j.id))}
                                    onChange={toggleSelectAll}
                                    className="rounded border-helios-line/30 bg-helios-surface-soft accent-helios-solar"
                                />
                            </th>
                            <th className="px-6 py-4">File</th>
                            <th className="px-6 py-4">Status</th>
                            <th className="px-6 py-4">Progress</th>
                            <th className="hidden md:table-cell px-6 py-4">Updated</th>
                            <th className="px-6 py-4 w-14"></th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-helios-line/10">
                        {loading && jobs.length === 0 ? (
                            Array.from({ length: 5 }).map((_, index) => (
                                <tr key={`loading-${index}`}>
                                    <td colSpan={6} className="px-6 py-3">
                                        <div className="h-10 w-full rounded-md bg-helios-surface-soft/60 animate-pulse" />
                                    </td>
                                </tr>
                            ))
                        ) : jobs.length === 0 ? (
                            <tr>
                                <td colSpan={6} className="px-6 py-12 text-center text-helios-slate">
                                    No jobs found
                                </td>
                            </tr>
                        ) : (
                            jobs.map((job) => (
                                <tr
                                    key={job.id}
                                    onClick={() => void fetchJobDetails(job.id)}
                                    className={cn(
                                        "group hover:bg-helios-surface/80 transition-all cursor-pointer",
                                        selected.has(job.id) && "bg-helios-surface-soft",
                                        focusedJob?.job.id === job.id && "bg-helios-solar/5"
                                    )}
                                >
                                    <td className="px-6 py-4" onClick={(e) => e.stopPropagation()}>
                                        <input type="checkbox"
                                            checked={selected.has(job.id)}
                                            onChange={() => toggleSelect(job.id)}
                                            className="rounded border-helios-line/30 bg-helios-surface-soft accent-helios-solar"
                                        />
                                    </td>
                                    <td className="px-6 py-4 relative">
                                        <motion.div layoutId={`job-name-${job.id}`} className="flex flex-col">
                                            <span className="font-medium text-helios-ink truncate max-w-[300px]" title={job.input_path}>
                                                {job.input_path.split(/[/\\]/).pop()}
                                            </span>
                                            <div className="flex items-center gap-2">
                                                <span className="text-xs text-helios-slate truncate max-w-[240px]">
                                                    {job.input_path}
                                                </span>
                                                <span className="hidden md:inline rounded-full border border-helios-line/20 px-2 py-0.5 text-xs font-bold text-helios-slate">
                                                    P{job.priority}
                                                </span>
                                            </div>
                                        </motion.div>
                                    </td>
                                    <td className="px-6 py-4">
                                        <motion.div layoutId={`job-status-${job.id}`}>
                                            {getStatusBadge(job.status)}
                                        </motion.div>
                                        {job.status === "failed" && (() => {
                                            // Reference tick so React re-renders countdowns on interval
                                            void tick;
                                            const countdown = retryCountdown(job);
                                            return countdown ? (
                                                <p className="text-[10px] font-mono text-helios-slate mt-0.5">
                                                    {countdown}
                                                </p>
                                            ) : null;
                                        })()}
                                    </td>
                                    <td className="px-6 py-4">
                                        {["encoding", "analyzing", "remuxing"].includes(job.status) ? (
                                            <div className="w-24 space-y-1">
                                                <div className="h-1.5 w-full bg-helios-line/10 rounded-full overflow-hidden">
                                                    <div className="h-full bg-helios-solar rounded-full transition-all duration-500" style={{ width: `${job.progress}%` }} />
                                                </div>
                                                <div className="text-xs text-right font-mono text-helios-slate">
                                                    {job.progress.toFixed(1)}%
                                                </div>
                                                {job.status === "encoding" && (() => {
                                                    const eta = calcEta(job.id, job.progress);
                                                    return eta ? (
                                                        <p className="text-[10px] text-helios-slate mt-0.5 font-mono">
                                                            {eta}
                                                        </p>
                                                    ) : null;
                                                })()}
                                                {job.status === "encoding" && job.encoder && (
                                                    <span className="text-[10px] font-mono text-helios-solar opacity-70">
                                                        {job.encoder}
                                                    </span>
                                                )}
                                            </div>
                                        ) : (
                                            job.vmaf_score ? (
                                                <span className="text-xs font-mono text-helios-slate">
                                                    VMAF: {job.vmaf_score.toFixed(1)}
                                                </span>
                                            ) : (
                                                <span className="text-helios-slate/50">-</span>
                                            )
                                        )}
                                    </td>
                                    <td className="hidden md:table-cell px-6 py-4 text-xs text-helios-slate font-mono">
                                        {new Date(job.updated_at).toLocaleString()}
                                    </td>
                                    <td className="px-6 py-4" onClick={(e) => e.stopPropagation()}>
                                        <div className="relative" ref={menuJobId === job.id ? menuRef : null}>
                                            <button
                                                onClick={() => setMenuJobId(menuJobId === job.id ? null : job.id)}
                                                className="p-2 rounded-lg border border-helios-line/20 hover:bg-helios-surface-soft text-helios-slate"
                                                title="Actions"
                                            >
                                                <MoreHorizontal size={14} />
                                            </button>
                                            <AnimatePresence>
                                                {menuJobId === job.id && (
                                                    <motion.div
                                                        initial={{ opacity: 0, y: 6 }}
                                                        animate={{ opacity: 1, y: 0 }}
                                                        exit={{ opacity: 0, y: 6 }}
                                                        className="absolute right-0 mt-2 w-44 rounded-lg border border-helios-line/20 bg-helios-surface shadow-xl z-20 overflow-hidden"
                                                    >
                                                        <button
                                                            onClick={() => {
                                                                setMenuJobId(null);
                                                                void fetchJobDetails(job.id);
                                                            }}
                                                            className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                        >
                                                            View details
                                                        </button>
                                                        <button
                                                            onClick={() => {
                                                                setMenuJobId(null);
                                                                void handlePriority(job, job.priority + 10, "Priority boosted");
                                                            }}
                                                            className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                        >
                                                            Boost priority (+10)
                                                        </button>
                                                        <button
                                                            onClick={() => {
                                                                setMenuJobId(null);
                                                                void handlePriority(job, job.priority - 10, "Priority lowered");
                                                            }}
                                                            className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                        >
                                                            Lower priority (-10)
                                                        </button>
                                                        <button
                                                            onClick={() => {
                                                                setMenuJobId(null);
                                                                void handlePriority(job, 0, "Priority reset");
                                                            }}
                                                            className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                        >
                                                            Reset priority
                                                        </button>
                                                        {(job.status === "failed" || job.status === "cancelled") && (
                                                            <button
                                                                onClick={() => {
                                                                    setMenuJobId(null);
                                                                    openConfirm({
                                                                        title: "Retry job",
                                                                        body: "Retry this job now?",
                                                                        confirmLabel: "Retry",
                                                                        onConfirm: () => handleAction(job.id, "restart"),
                                                                    });
                                                                }}
                                                                className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                            >
                                                                Retry
                                                            </button>
                                                        )}
                                                        {["encoding", "analyzing", "remuxing"].includes(job.status) && (
                                                            <button
                                                                onClick={() => {
                                                                    setMenuJobId(null);
                                                                    openConfirm({
                                                                        title: "Cancel job",
                                                                        body: "Stop this job immediately?",
                                                                        confirmLabel: "Cancel",
                                                                        confirmTone: "danger",
                                                                        onConfirm: () => handleAction(job.id, "cancel"),
                                                                    });
                                                                }}
                                                                className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                            >
                                                                Stop / Cancel
                                                            </button>
                                                        )}
                                                        {!isJobActive(job) && (
                                                            <button
                                                                onClick={() => {
                                                                    setMenuJobId(null);
                                                                    openConfirm({
                                                                        title: "Delete job",
                                                                        body: "Delete this job from history?",
                                                                        confirmLabel: "Delete",
                                                                        confirmTone: "danger",
                                                                        onConfirm: () => handleAction(job.id, "delete"),
                                                                    });
                                                                }}
                                                                className="w-full px-4 py-2 text-left text-xs font-semibold text-red-500 hover:bg-red-500/5"
                                                            >
                                                                Delete
                                                            </button>
                                                        )}
                                                    </motion.div>
                                                )}
                                            </AnimatePresence>
                                        </div>
                                    </td>
                                </tr>
                            ))
                        )}
                    </tbody>
                </table>
            </div>

            {/* Footer Actions */}
            <div className="flex justify-between items-center pt-2">
                <p className="text-xs text-helios-slate font-medium">Showing {jobs.length} jobs (Limit 50)</p>
                <button
                    onClick={() =>
                        openConfirm({
                            title: "Clear completed jobs",
                            body: "Remove all completed jobs from history?",
                            confirmLabel: "Clear",
                            confirmTone: "danger",
                            onConfirm: () => clearCompleted(),
                        })
                    }
                    className="text-xs text-red-500 hover:text-red-400 font-bold flex items-center gap-1 transition-colors"
                >
                    <Trash2 size={12} /> Clear Completed
                </button>
            </div>

            {/* Detail Overlay - rendered via portal to escape layout constraints */}
            {typeof document !== "undefined" && createPortal(
                <AnimatePresence>
                    {focusedJob && (
                        <>
                            <motion.div
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                exit={{ opacity: 0 }}
                                onClick={() => setFocusedJob(null)}
                                className="fixed inset-0 bg-black/60 backdrop-blur-sm z-[100]"
                            />
                            <div className="fixed inset-0 flex items-center justify-center pointer-events-none z-[101]">
                            <motion.div
                                key="modal-content"
                                initial={{ opacity: 0, scale: 0.95, y: 10 }}
                                animate={{ opacity: 1, scale: 1, y: 0 }}
                                exit={{ opacity: 0, scale: 0.95, y: 10 }}
                                transition={{ duration: 0.2 }}
                                ref={detailDialogRef}
                                role="dialog"
                                aria-modal="true"
                                aria-labelledby="job-details-title"
                                aria-describedby="job-details-path"
                                tabIndex={-1}
                                className="w-full max-w-2xl bg-helios-surface border border-helios-line/20 rounded-lg shadow-2xl pointer-events-auto overflow-hidden mx-4"
                            >
                                {/* Header */}
                                <div className="p-6 border-b border-helios-line/10 flex justify-between items-start gap-4 bg-helios-surface-soft/50">
                                    <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-3 mb-1">
                                            {getStatusBadge(focusedJob.job.status)}
                                            <span className="text-xs font-medium text-helios-slate">Job ID #{focusedJob.job.id}</span>
                                            <span className="text-xs font-medium text-helios-slate">Priority {focusedJob.job.priority}</span>
                                        </div>
                                        <h2 id="job-details-title" className="text-lg font-bold text-helios-ink truncate" title={focusedJob.job.input_path}>
                                            {focusedJob.job.input_path.split(/[/\\]/).pop()}
                                        </h2>
                                        <p id="job-details-path" className="text-xs text-helios-slate truncate opacity-60">{focusedJob.job.input_path}</p>
                                    </div>
                                    <button
                                        onClick={() => setFocusedJob(null)}
                                        className="p-2 hover:bg-helios-line/10 rounded-md transition-colors text-helios-slate"
                                    >
                                        <X size={20} />
                                    </button>
                                </div>

                                <div className="p-6 space-y-8 max-h-[70vh] overflow-y-auto custom-scrollbar">
                                    {detailLoading && (
                                        <p className="text-xs text-helios-slate" aria-live="polite">Loading job details...</p>
                                    )}
                                    {focusedJob.metadata || completedEncodeStats ? (
                                        <>
                                            {focusedJob.metadata && (
                                                <>
                                                    {/* Stats Grid */}
                                                    <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
                                                        <div className="p-4 rounded-lg bg-helios-surface-soft border border-helios-line/20 space-y-1">
                                                            <div className="flex items-center gap-2 text-helios-slate mb-1">
                                                                <Activity size={12} />
                                                                <span className="text-xs font-medium text-helios-slate">Video Codec</span>
                                                            </div>
                                                            <p className="text-sm font-bold text-helios-ink capitalize">
                                                                {focusedJob.metadata.codec_name || "Unknown"}
                                                            </p>
                                                            <p className="text-xs text-helios-slate">
                                                                {(focusedJob.metadata.bit_depth ? `${focusedJob.metadata.bit_depth}-bit` : "Unknown bit depth")} • {focusedJob.metadata.container.toUpperCase()}
                                                            </p>
                                                        </div>

                                                        <div className="p-4 rounded-lg bg-helios-surface-soft border border-helios-line/20 space-y-1">
                                                            <div className="flex items-center gap-2 text-helios-slate mb-1">
                                                                <Maximize2 size={12} />
                                                                <span className="text-xs font-medium text-helios-slate">Resolution</span>
                                                            </div>
                                                            <p className="text-sm font-bold text-helios-ink">
                                                                {`${focusedJob.metadata.width}x${focusedJob.metadata.height}`}
                                                            </p>
                                                            <p className="text-xs text-helios-slate">
                                                                {focusedJob.metadata.fps.toFixed(2)} FPS
                                                            </p>
                                                        </div>

                                                        <div className="p-4 rounded-lg bg-helios-surface-soft border border-helios-line/20 space-y-1">
                                                            <div className="flex items-center gap-2 text-helios-slate mb-1">
                                                                <Clock size={12} />
                                                                <span className="text-xs font-medium text-helios-slate">Duration</span>
                                                            </div>
                                                            <p className="text-sm font-bold text-helios-ink">
                                                                {formatDuration(focusedJob.metadata.duration_secs)}
                                                            </p>
                                                        </div>
                                                    </div>

                                                    {/* Media Details */}
                                                    <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
                                                        <div className="space-y-4">
                                                            <h3 className="text-xs font-medium text-helios-slate/70 flex items-center gap-2">
                                                                <Database size={12} /> Input Details
                                                            </h3>
                                                            <div className="space-y-3">
                                                                <div className="flex justify-between items-center text-xs">
                                                                    <span className="text-helios-slate font-medium">File Size</span>
                                                                    <span className="text-helios-ink font-bold">{formatBytes(focusedJob.metadata.size_bytes)}</span>
                                                                </div>
                                                                <div className="flex justify-between items-center text-xs">
                                                                    <span className="text-helios-slate font-medium">Video Bitrate</span>
                                                                    <span className="text-helios-ink font-bold">
                                                                        {(focusedJob.metadata.video_bitrate_bps ?? focusedJob.metadata.container_bitrate_bps)
                                                                            ? `${(((focusedJob.metadata.video_bitrate_bps ?? focusedJob.metadata.container_bitrate_bps) as number) / 1000).toFixed(0)} kbps`
                                                                            : "-"}
                                                                    </span>
                                                                </div>
                                                                <div className="flex justify-between items-center text-xs">
                                                                    <span className="text-helios-slate font-medium">Audio</span>
                                                                    <span className="text-helios-ink font-bold capitalize">
                                                                        {focusedJob.metadata.audio_codec || "N/A"} ({focusedJob.metadata.audio_channels || 0}ch)
                                                                    </span>
                                                                </div>
                                                            </div>
                                                        </div>

                                                        <div className="space-y-4">
                                                            <h3 className="text-xs font-medium text-helios-solar flex items-center gap-2">
                                                                <Zap size={12} /> Output Details
                                                            </h3>
                                                            {focusedJob.encode_stats ? (
                                                                <div className="space-y-3">
                                                                    <div className="flex justify-between items-center text-xs">
                                                                        <span className="text-helios-slate font-medium">Result Size</span>
                                                                        <span className="text-helios-solar font-bold">{formatBytes(focusedJob.encode_stats.output_size_bytes)}</span>
                                                                    </div>
                                                                    <div className="flex justify-between items-center text-xs">
                                                                        <span className="text-helios-slate font-medium">Reduction</span>
                                                                        <span className="text-green-500 font-bold">
                                                                            {((1 - focusedJob.encode_stats.compression_ratio) * 100).toFixed(1)}% Saved
                                                                        </span>
                                                                    </div>
                                                                    <div className="flex justify-between items-center text-xs">
                                                                        <span className="text-helios-slate font-medium">VMAF Score</span>
                                                                        <div className="flex items-center gap-1.5">
                                                                            <div className="h-1.5 w-16 bg-helios-line/10 rounded-full overflow-hidden">
                                                                                <div className="h-full bg-helios-solar" style={{ width: `${focusedJob.encode_stats.vmaf_score || 0}%` }} />
                                                                            </div>
                                                                            <span className="text-helios-ink font-bold">
                                                                                {focusedJob.encode_stats.vmaf_score?.toFixed(1) || "-"}
                                                                            </span>
                                                                        </div>
                                                                    </div>
                                                                </div>
                                                            ) : (
                                                                <div className="h-[80px] flex items-center justify-center border border-dashed border-helios-line/20 rounded-lg text-xs text-helios-slate italic">
                                                                    {focusedJob.job.status === "encoding"
                                                                        ? "Encoding in progress..."
                                                                        : focusedJob.job.status === "remuxing"
                                                                            ? "Remuxing in progress..."
                                                                            : "No encode data available"}
                                                                </div>
                                                            )}
                                                        </div>
                                                    </div>
                                                </>
                                            )}

                                            {completedEncodeStats && (
                                                <div className="space-y-4">
                                                    <h3 className="text-xs font-medium text-helios-solar flex items-center gap-2">
                                                        <Zap size={12} /> Encode Results
                                                    </h3>
                                                    <div className="p-4 rounded-lg bg-helios-surface-soft border border-helios-line/20 space-y-3">
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">Input size</span>
                                                            <span className="text-helios-ink font-bold">{formatBytes(completedEncodeStats.input_size_bytes)}</span>
                                                        </div>
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">Output size</span>
                                                            <span className="text-helios-ink font-bold">{formatBytes(completedEncodeStats.output_size_bytes)}</span>
                                                        </div>
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">Reduction</span>
                                                            <span className="text-green-500 font-bold">
                                                                {completedEncodeStats.input_size_bytes > 0
                                                                    ? `${((1 - completedEncodeStats.output_size_bytes / completedEncodeStats.input_size_bytes) * 100).toFixed(1)}% saved`
                                                                    : "—"}
                                                            </span>
                                                        </div>
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">Encode time</span>
                                                            <span className="text-helios-ink font-bold">{formatDuration(completedEncodeStats.encode_time_seconds)}</span>
                                                        </div>
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">Speed</span>
                                                            <span className="text-helios-ink font-bold">{`${completedEncodeStats.encode_speed.toFixed(2)}\u00d7 realtime`}</span>
                                                        </div>
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">Avg bitrate</span>
                                                            <span className="text-helios-ink font-bold">{`${completedEncodeStats.avg_bitrate_kbps} kbps`}</span>
                                                        </div>
                                                        <div className="flex justify-between items-center text-xs">
                                                            <span className="text-helios-slate font-medium">VMAF</span>
                                                            <span className="text-helios-ink font-bold">{completedEncodeStats.vmaf_score?.toFixed(1) ?? "—"}</span>
                                                        </div>
                                                    </div>
                                                </div>
                                            )}
                                        </>
                                    ) : (
                                        <div className="flex items-center gap-3 rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-5">
                                            <div className="p-2 rounded-lg bg-helios-surface border border-helios-line/20 text-helios-slate shrink-0">
                                                <Clock size={18} />
                                            </div>
                                            <div>
                                                <p className="text-sm font-medium text-helios-ink">
                                                    Waiting for analysis
                                                </p>
                                                <p className="text-xs text-helios-slate mt-0.5">
                                                    Metadata will appear once this job is picked up by the engine.
                                                </p>
                                            </div>
                                        </div>
                                    )}

                                    {/* Decision Info */}
                                    {focusedJob.job.decision_reason && focusedJob.job.status !== "failed" && focusedJob.job.status !== "skipped" && (
                                        <div className="p-4 rounded-lg bg-helios-solar/5 border border-helios-solar/10">
                                            <div className="flex items-center gap-2 text-helios-solar mb-1">
                                                <Info size={12} />
                                                <span className="text-xs font-medium text-helios-slate">Decision Context</span>
                                            </div>
                                            {focusedDecision && (
                                                <div className="space-y-3">
                                                    <p className="text-sm font-medium text-helios-ink">
                                                        {focusedJob.job.status === "completed"
                                                            ? "Transcoded"
                                                            : focusedDecision.summary}
                                                    </p>
                                                    <p className="text-xs leading-relaxed text-helios-slate">
                                                        {focusedDecision.detail}
                                                    </p>
                                                    {Object.keys(focusedDecision.measured).length > 0 && (
                                                        <div className="space-y-1.5 rounded-lg border border-helios-line/20 bg-helios-surface-soft px-3 py-2.5">
                                                            {Object.entries(focusedDecision.measured).map(([k, v]) => (
                                                                <div key={k} className="flex items-center justify-between text-xs">
                                                                    <span className="font-mono text-helios-slate">{k}</span>
                                                                    <span className="font-mono font-bold text-helios-ink">{v}</span>
                                                                </div>
                                                            ))}
                                                        </div>
                                                    )}
                                                    {focusedDecision.action && (
                                                        <div className="flex items-start gap-2 rounded-lg border border-helios-solar/20 bg-helios-solar/5 px-3 py-2.5">
                                                            <span className="text-xs leading-relaxed text-helios-solar">
                                                                {focusedDecision.action}
                                                            </span>
                                                        </div>
                                                    )}
                                                </div>
                                            )}
                                        </div>
                                    )}

                                    {focusedJob.job.status === "skipped" && focusedJob.job.decision_reason && (
                                        <div className="p-4 rounded-lg bg-helios-surface-soft border border-helios-line/10">
                                            <p className="text-sm text-helios-ink leading-relaxed">
                                                Alchemist analysed this file and decided not to transcode it. Here&apos;s why:
                                            </p>
                                            {focusedDecision && (
                                                <div className="mt-3 space-y-3">
                                                    <p className="text-sm font-medium text-helios-ink">
                                                        {focusedDecision.summary}
                                                    </p>
                                                    <p className="text-xs leading-relaxed text-helios-slate">
                                                        {focusedDecision.detail}
                                                    </p>
                                                    {Object.keys(focusedDecision.measured).length > 0 && (
                                                        <div className="space-y-1.5 rounded-lg border border-helios-line/20 bg-helios-surface px-3 py-2.5">
                                                            {Object.entries(focusedDecision.measured).map(([k, v]) => (
                                                                <div key={k} className="flex items-center justify-between text-xs">
                                                                    <span className="font-mono text-helios-slate">{k}</span>
                                                                    <span className="font-mono font-bold text-helios-ink">{v}</span>
                                                                </div>
                                                            ))}
                                                        </div>
                                                    )}
                                                    {focusedDecision.action && (
                                                        <div className="flex items-start gap-2 rounded-lg border border-helios-solar/20 bg-helios-solar/5 px-3 py-2.5">
                                                            <span className="text-xs leading-relaxed text-helios-solar">
                                                                {focusedDecision.action}
                                                            </span>
                                                        </div>
                                                    )}
                                                </div>
                                            )}
                                        </div>
                                    )}

                                    {focusedJob.job.status === "failed" && (
                                        <div className="rounded-lg border border-status-error/20 bg-status-error/5 px-4 py-4 space-y-2">
                                            <div className="flex items-center gap-2">
                                                <AlertCircle size={14} className="text-status-error shrink-0" />
                                                <span className="text-xs font-semibold text-status-error uppercase tracking-wide">
                                                    Failure Reason
                                                </span>
                                            </div>
                                            {focusedJob.job_failure_summary ? (
                                                <>
                                                    <p className="text-sm font-medium text-helios-ink">
                                                        {explainFailureSummary(focusedJob.job_failure_summary)}
                                                    </p>
                                                    <p className="text-xs font-mono text-helios-slate/70 break-all leading-relaxed">
                                                        {focusedJob.job_failure_summary}
                                                    </p>
                                                </>
                                            ) : (
                                                <p className="text-sm text-helios-slate">
                                                    No error details captured. Check the logs below.
                                                </p>
                                            )}
                                        </div>
                                    )}

                                    {shouldShowFfmpegOutput && (
                                        <details className="rounded-lg border border-helios-line/15 bg-helios-surface-soft/40 p-4">
                                            <summary className="cursor-pointer text-xs text-helios-solar">
                                                Show FFmpeg output ({focusedJobLogs.length} lines)
                                            </summary>
                                            <div className="mt-3 max-h-48 overflow-y-auto rounded-lg bg-helios-main/70 p-3">
                                                {focusedJobLogs.map((entry) => (
                                                    <div
                                                        key={entry.id}
                                                        className={cn(
                                                            "font-mono text-xs leading-relaxed whitespace-pre-wrap break-words",
                                                            logLevelClass(entry.level)
                                                        )}
                                                    >
                                                        {entry.message}
                                                    </div>
                                                ))}
                                            </div>
                                        </details>
                                    )}

                                    {/* Action Toolbar */}
                                    <div className="flex items-center justify-between pt-4 border-t border-helios-line/10">
                                        <div className="flex gap-2">
                                            <button
                                                onClick={() => void handlePriority(focusedJob.job, focusedJob.job.priority + 10, "Priority boosted")}
                                                className="px-3 py-2 border border-helios-line/20 bg-helios-surface text-helios-slate rounded-lg text-sm font-bold hover:bg-helios-surface-soft transition-all"
                                            >
                                                Boost +10
                                            </button>
                                            <button
                                                onClick={() => void handlePriority(focusedJob.job, focusedJob.job.priority - 10, "Priority lowered")}
                                                className="px-3 py-2 border border-helios-line/20 bg-helios-surface text-helios-slate rounded-lg text-sm font-bold hover:bg-helios-surface-soft transition-all"
                                            >
                                                Lower -10
                                            </button>
                                            <button
                                                onClick={() => void handlePriority(focusedJob.job, 0, "Priority reset")}
                                                className="px-3 py-2 border border-helios-line/20 bg-helios-surface text-helios-slate rounded-lg text-sm font-bold hover:bg-helios-surface-soft transition-all"
                                            >
                                                Reset
                                            </button>
                                            {(focusedJob.job.status === 'failed' || focusedJob.job.status === 'cancelled') && (
                                                <button
                                                    onClick={() =>
                                                        openConfirm({
                                                            title: "Retry job",
                                                            body: "Retry this job now?",
                                                            confirmLabel: "Retry",
                                                            onConfirm: () => handleAction(focusedJob.job.id, 'restart'),
                                                        })
                                                    }
                                                    className="px-4 py-2 bg-helios-solar text-helios-main rounded-lg text-sm font-bold flex items-center gap-2 hover:brightness-110 active:scale-95 transition-all shadow-sm"
                                                >
                                                    <RefreshCw size={14} /> Retry Job
                                                </button>
                                            )}
                                            {["encoding", "analyzing", "remuxing"].includes(focusedJob.job.status) && (
                                                <button
                                                    onClick={() =>
                                                        openConfirm({
                                                            title: "Cancel job",
                                                            body: "Stop this job immediately?",
                                                            confirmLabel: "Cancel",
                                                            confirmTone: "danger",
                                                            onConfirm: () => handleAction(focusedJob.job.id, 'cancel'),
                                                        })
                                                    }
                                                    className="px-4 py-2 border border-helios-line/20 bg-helios-surface text-helios-slate rounded-lg text-sm font-bold flex items-center gap-2 hover:bg-helios-surface-soft active:scale-95 transition-all"
                                                >
                                                    <Ban size={14} /> Stop / Cancel
                                                </button>
                                            )}
                                        </div>
                                        {!isJobActive(focusedJob.job) && (
                                            <button
                                                onClick={() =>
                                                    openConfirm({
                                                        title: "Delete job",
                                                        body: "Delete this job from history?",
                                                        confirmLabel: "Delete",
                                                        confirmTone: "danger",
                                                        onConfirm: () => handleAction(focusedJob.job.id, 'delete'),
                                                    })
                                                }
                                                className="px-4 py-2 text-red-500 hover:bg-red-500/5 rounded-lg text-sm font-bold flex items-center gap-2 transition-all"
                                            >
                                                <Trash2 size={14} /> Delete
                                            </button>
                                        )}
                                    </div>
                                </div>
                            </motion.div>
                        </div>
                    </>
                )}
            </AnimatePresence>,
                document.body
            )}

            <ConfirmDialog
                open={confirmState !== null}
                title={confirmState?.title ?? ""}
                description={confirmState?.body ?? ""}
                confirmLabel={confirmState?.confirmLabel ?? "Confirm"}
                tone={confirmState?.confirmTone ?? "primary"}
                onClose={() => setConfirmState(null)}
                onConfirm={async () => {
                    if (!confirmState) {
                        return;
                    }
                    await confirmState.onConfirm();
                }}
            />
        </div>
    );
}
