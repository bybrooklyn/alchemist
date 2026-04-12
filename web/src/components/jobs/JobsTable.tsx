import { RefreshCw, Ban, Trash2, MoreHorizontal } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { RefObject, MutableRefObject } from "react";
import type React from "react";
import type { Job, ConfirmConfig } from "./types";
import { isJobActive, retryCountdown } from "./types";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface JobsTableProps {
    jobs: Job[];
    loading: boolean;
    selected: Set<number>;
    focusedJobId: number | null;
    tick: number;
    encodeStartTimes: MutableRefObject<Map<number, number>>;
    menuJobId: number | null;
    menuRef: RefObject<HTMLDivElement | null>;
    toggleSelect: (id: number) => void;
    toggleSelectAll: () => void;
    fetchJobDetails: (id: number) => Promise<void>;
    setMenuJobId: (id: number | null) => void;
    openConfirm: (config: ConfirmConfig) => void;
    handleAction: (id: number, action: "cancel" | "restart" | "delete") => Promise<void>;
    handlePriority: (job: Job, priority: number, label: string) => Promise<void>;
    getStatusBadge: (status: string) => React.ReactElement;
}

function calcEta(encodeStartTimes: MutableRefObject<Map<number, number>>, jobId: number, progress: number): string | null {
    if (progress <= 0 || progress >= 100) return null;
    const startMs = encodeStartTimes.current.get(jobId);
    if (!startMs) return null;
    const elapsedMs = Date.now() - startMs;
    const totalMs = elapsedMs / (progress / 100);
    const remainingMs = totalMs - elapsedMs;
    const remainingSecs = Math.round(remainingMs / 1000);
    if (remainingSecs < 0) return null;
    if (remainingSecs < 60) return `~${remainingSecs}s remaining`;
    const mins = Math.ceil(remainingSecs / 60);
    return `~${mins} min remaining`;
}

export function JobsTable({
    jobs, loading, selected, focusedJobId, tick, encodeStartTimes,
    menuJobId, menuRef, toggleSelect, toggleSelectAll,
    fetchJobDetails, setMenuJobId, openConfirm, handleAction, handlePriority,
    getStatusBadge,
}: JobsTableProps) {
    return (
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
                                    focusedJobId === job.id && "bg-helios-solar/5"
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
                                                const eta = calcEta(encodeStartTimes, job.id, job.progress);
                                                return eta ? (
                                                    <p className="text-[10px] text-helios-slate mt-0.5 font-mono">{eta}</p>
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
                                    <div className="relative" ref={menuJobId === job.id ? (menuRef as React.RefObject<HTMLDivElement>) : null}>
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
                                                    <button onClick={() => { setMenuJobId(null); void fetchJobDetails(job.id); }} className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft">View details</button>
                                                    <button onClick={() => { setMenuJobId(null); void handlePriority(job, job.priority + 10, "Priority boosted"); }} className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft">Boost priority (+10)</button>
                                                    <button onClick={() => { setMenuJobId(null); void handlePriority(job, job.priority - 10, "Priority lowered"); }} className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft">Lower priority (-10)</button>
                                                    <button onClick={() => { setMenuJobId(null); void handlePriority(job, 0, "Priority reset"); }} className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft">Reset priority</button>
                                                    {(job.status === "failed" || job.status === "cancelled") && (
                                                        <button
                                                            onClick={() => { setMenuJobId(null); openConfirm({ title: "Retry job", body: "Retry this job now?", confirmLabel: "Retry", onConfirm: () => handleAction(job.id, "restart") }); }}
                                                            className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                        >
                                                            Retry
                                                        </button>
                                                    )}
                                                    {["encoding", "analyzing", "remuxing"].includes(job.status) && (
                                                        <button
                                                            onClick={() => { setMenuJobId(null); openConfirm({ title: "Cancel job", body: "Stop this job immediately?", confirmLabel: "Cancel", confirmTone: "danger", onConfirm: () => handleAction(job.id, "cancel") }); }}
                                                            className="w-full px-4 py-2 text-left text-xs font-semibold text-helios-ink hover:bg-helios-surface-soft"
                                                        >
                                                            Stop / Cancel
                                                        </button>
                                                    )}
                                                    {!isJobActive(job) && (
                                                        <button
                                                            onClick={() => { setMenuJobId(null); openConfirm({ title: "Delete job", body: "Delete this job from history?", confirmLabel: "Delete", confirmTone: "danger", onConfirm: () => handleAction(job.id, "delete") }); }}
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
    );
}
