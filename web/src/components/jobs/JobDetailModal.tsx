import { X, Clock, Info, Activity, Database, Zap, Maximize2, AlertCircle, RefreshCw, Ban, Trash2 } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { RefObject } from "react";
import type React from "react";
import type { JobDetail, EncodeStats, ExplanationView, LogEntry, ConfirmConfig, Job } from "./types";
import { formatBytes, formatDuration, logLevelClass, isJobActive } from "./types";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface JobDetailModalProps {
    focusedJob: JobDetail | null;
    detailDialogRef: RefObject<HTMLDivElement | null>;
    detailLoading: boolean;
    onClose: () => void;
    focusedDecision: ExplanationView | null;
    focusedFailure: ExplanationView | null;
    focusedJobLogs: LogEntry[];
    shouldShowFfmpegOutput: boolean;
    completedEncodeStats: EncodeStats | null;
    focusedEmptyState: { title: string; detail: string } | null;
    openConfirm: (config: ConfirmConfig) => void;
    handleAction: (id: number, action: "cancel" | "restart" | "delete") => Promise<void>;
    handlePriority: (job: Job, priority: number, label: string) => Promise<void>;
    getStatusBadge: (status: string) => React.ReactElement;
}

export function JobDetailModal({
    focusedJob, detailDialogRef, detailLoading, onClose,
    focusedDecision, focusedFailure, focusedJobLogs, shouldShowFfmpegOutput,
    completedEncodeStats, focusedEmptyState,
    openConfirm, handleAction, handlePriority, getStatusBadge,
}: JobDetailModalProps) {
    return (
        <AnimatePresence>
            {focusedJob && (
                <>
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        onClick={onClose}
                        className="fixed inset-0 bg-black/60 backdrop-blur-sm z-[100]"
                    />
                    <div className="fixed inset-0 flex items-center justify-center pointer-events-none z-[101]">
                        <motion.div
                            key="modal-content"
                            initial={{ opacity: 0, scale: 0.95, y: 10 }}
                            animate={{ opacity: 1, scale: 1, y: 0 }}
                            exit={{ opacity: 0, scale: 0.95, y: 10 }}
                            transition={{ duration: 0.2 }}
                            ref={detailDialogRef as React.RefObject<HTMLDivElement>}
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
                                    onClick={onClose}
                                    className="p-2 hover:bg-helios-line/10 rounded-md transition-colors text-helios-slate"
                                >
                                    <X size={20} />
                                </button>
                            </div>

                            <div className="p-6 space-y-8 max-h-[70vh] overflow-y-auto custom-scrollbar">
                                {detailLoading && (
                                    <p className="text-xs text-helios-slate" aria-live="polite">Loading job details...</p>
                                )}
                                {/* Active-encode status banner */}
                                {focusedEmptyState && (focusedJob.job.status === "encoding" || focusedJob.job.status === "remuxing") && (
                                    <div className="flex items-center gap-3 rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3">
                                        <div className="p-1.5 rounded-lg bg-helios-surface border border-helios-line/20 text-helios-slate shrink-0">
                                            <Clock size={14} />
                                        </div>
                                        <p className="text-xs font-medium text-helios-ink">{focusedEmptyState.title}</p>
                                    </div>
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
                                ) : focusedEmptyState ? (
                                    <div className="flex items-center gap-3 rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-5">
                                        <div className="p-2 rounded-lg bg-helios-surface border border-helios-line/20 text-helios-slate shrink-0">
                                            <Clock size={18} />
                                        </div>
                                        <div>
                                            <p className="text-sm font-medium text-helios-ink">
                                                {focusedEmptyState.title}
                                            </p>
                                            <p className="text-xs text-helios-slate mt-0.5">
                                                {focusedEmptyState.detail}
                                            </p>
                                        </div>
                                    </div>
                                ) : null}

                                {/* Decision Info */}
                                {focusedDecision && focusedJob.job.status !== "failed" && focusedJob.job.status !== "skipped" && (
                                    <div className="p-4 rounded-lg bg-helios-solar/5 border border-helios-solar/10">
                                        <div className="flex items-center gap-2 text-helios-solar mb-1">
                                            <Info size={12} />
                                            <span className="text-xs font-medium text-helios-slate">Decision Context</span>
                                        </div>
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
                                                            <span className="font-mono font-bold text-helios-ink">{String(v)}</span>
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                            {focusedDecision.operator_guidance && (
                                                <div className="flex items-start gap-2 rounded-lg border border-helios-solar/20 bg-helios-solar/5 px-3 py-2.5">
                                                    <span className="text-xs leading-relaxed text-helios-solar">
                                                        {focusedDecision.operator_guidance}
                                                    </span>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}

                                {focusedJob.job.status === "skipped" && focusedDecision && (
                                    <div className="p-4 rounded-lg bg-helios-surface-soft border border-helios-line/10">
                                        <p className="text-sm text-helios-ink leading-relaxed">
                                            Alchemist analysed this file and decided not to transcode it. Here&apos;s why:
                                        </p>
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
                                                            <span className="font-mono font-bold text-helios-ink">{String(v)}</span>
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                            {focusedDecision.operator_guidance && (
                                                <div className="flex items-start gap-2 rounded-lg border border-helios-solar/20 bg-helios-solar/5 px-3 py-2.5">
                                                    <span className="text-xs leading-relaxed text-helios-solar">
                                                        {focusedDecision.operator_guidance}
                                                    </span>
                                                </div>
                                            )}
                                        </div>
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
                                        {focusedFailure ? (
                                            <>
                                                <p className="text-sm font-medium text-helios-ink">
                                                    {focusedFailure.summary}
                                                </p>
                                                <p className="text-xs leading-relaxed text-helios-slate">
                                                    {focusedFailure.detail}
                                                </p>
                                                {focusedFailure.operator_guidance && (
                                                    <p className="text-xs leading-relaxed text-status-error">
                                                        {focusedFailure.operator_guidance}
                                                    </p>
                                                )}
                                                {focusedFailure.legacy_reason !== focusedFailure.detail && (
                                                    <p className="text-xs font-mono text-helios-slate/70 break-all leading-relaxed">
                                                        {focusedFailure.legacy_reason}
                                                    </p>
                                                )}
                                            </>
                                        ) : (
                                            <p className="text-sm text-helios-slate">
                                                No error details captured. Check the logs below.
                                            </p>
                                        )}
                                    </div>
                                )}

                                {(focusedJob.encode_attempts ?? []).length > 0 && (
                                    <details className="rounded-lg border border-helios-line/15 bg-helios-surface-soft/40 p-4">
                                        <summary className="cursor-pointer text-xs text-helios-solar">
                                            Attempt History ({(focusedJob.encode_attempts ?? []).length})
                                        </summary>
                                        <div className="mt-3 space-y-2">
                                            {(focusedJob.encode_attempts ?? []).map((attempt) => (
                                                <div key={attempt.id} className="flex items-start gap-3 rounded-lg border border-helios-line/10 bg-helios-main/50 px-3 py-2 text-xs">
                                                    <span className={cn(
                                                        "mt-0.5 shrink-0 rounded px-1.5 py-0.5 font-mono font-semibold",
                                                        attempt.outcome === "completed" && "bg-status-success/15 text-status-success",
                                                        attempt.outcome === "failed" && "bg-status-error/15 text-status-error",
                                                        attempt.outcome === "cancelled" && "bg-helios-slate/15 text-helios-slate",
                                                    )}>#{attempt.attempt_number}</span>
                                                    <div className="min-w-0 flex-1">
                                                        <div className="flex items-center gap-2">
                                                            <span className="capitalize font-medium text-helios-ink">{attempt.outcome}</span>
                                                            {attempt.encode_time_seconds != null && (
                                                                <span className="text-helios-slate">{attempt.encode_time_seconds < 60
                                                                    ? `${attempt.encode_time_seconds.toFixed(1)}s`
                                                                    : `${(attempt.encode_time_seconds / 60).toFixed(1)}m`}</span>
                                                            )}
                                                            {attempt.input_size_bytes != null && attempt.output_size_bytes != null && (
                                                                <span className="text-helios-slate">
                                                                    {formatBytes(attempt.input_size_bytes)} → {formatBytes(attempt.output_size_bytes)}
                                                                </span>
                                                            )}
                                                        </div>
                                                        {attempt.failure_summary && (
                                                            <p className="mt-0.5 text-helios-slate/80 truncate">{attempt.failure_summary}</p>
                                                        )}
                                                        <p className="mt-0.5 text-helios-slate/50">{new Date(attempt.finished_at).toLocaleString()}</p>
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    </details>
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
                                        {(focusedJob.job.status === "failed" || focusedJob.job.status === "cancelled") && (
                                            <button
                                                onClick={() =>
                                                    openConfirm({
                                                        title: "Retry job",
                                                        body: "Retry this job now?",
                                                        confirmLabel: "Retry",
                                                        onConfirm: () => handleAction(focusedJob.job.id, "restart"),
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
                                                        onConfirm: () => handleAction(focusedJob.job.id, "cancel"),
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
                                                    onConfirm: () => handleAction(focusedJob.job.id, "delete"),
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
        </AnimatePresence>
    );
}
