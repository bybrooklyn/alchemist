import { useState, useEffect, useCallback, useRef } from "react";
import {
    Search, RefreshCw, Trash2, Ban,
    Clock, X, Info, Activity, Database, Zap, Maximize2, MoreHorizontal
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

interface Job {
    id: number;
    input_path: string;
    output_path: string;
    status: string;
    priority: number;
    progress: number;
    created_at: string;
    updated_at: string;
    vmaf_score?: number;
    decision_reason?: string;
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

interface JobDetail {
    job: Job;
    metadata?: JobMetadata;
    encode_stats?: EncodeStats;
}

type TabType = "all" | "active" | "queued" | "completed" | "failed";

export default function JobManager() {
    const [jobs, setJobs] = useState<Job[]>([]);
    const [loading, setLoading] = useState(true);
    const [selected, setSelected] = useState<Set<number>>(new Set());
    const [activeTab, setActiveTab] = useState<TabType>("all");
    const [searchInput, setSearchInput] = useState("");
    const debouncedSearch = useDebouncedValue(searchInput, 350);
    const [page, setPage] = useState(1);
    const [refreshing, setRefreshing] = useState(false);
    const [focusedJob, setFocusedJob] = useState<JobDetail | null>(null);
    const [detailLoading, setDetailLoading] = useState(false);
    const [actionError, setActionError] = useState<string | null>(null);
    const [menuJobId, setMenuJobId] = useState<number | null>(null);
    const menuRef = useRef<HTMLDivElement | null>(null);
    const detailDialogRef = useRef<HTMLDivElement | null>(null);
    const detailLastFocusedRef = useRef<HTMLElement | null>(null);
    const confirmOpenRef = useRef(false);
    const [confirmState, setConfirmState] = useState<{
        title: string;
        body: string;
        confirmLabel: string;
        confirmTone?: "danger" | "primary";
        onConfirm: () => Promise<void> | void;
    } | null>(null);

    const isJobActive = (job: Job) => ["analyzing", "encoding", "resuming"].includes(job.status);

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
            case "active": return "analyzing,encoding,resuming";
            case "queued": return "queued";
            case "completed": return "completed";
            case "failed": return "failed,cancelled";
            default: return "";
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
                sort: "updated_at",
                sort_desc: "true"
            });

            if (activeTab !== "all") {
                params.set("status", getStatusFilter(activeTab));
            }
            if (debouncedSearch) {
                params.set("search", debouncedSearch);
            }

            const data = await apiJson<Job[]>(`/api/jobs/table?${params}`);
            setJobs(data);
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
    }, [activeTab, debouncedSearch, page]);

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
            await apiAction("/api/jobs/clear-completed", { method: "POST" });
            showToast({ kind: "success", title: "Jobs", message: "Completed jobs cleared." });
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

    const getStatusBadge = (status: string) => {
        const styles: Record<string, string> = {
            queued: "bg-helios-slate/10 text-helios-slate border-helios-slate/20",
            analyzing: "bg-blue-500/10 text-blue-500 border-blue-500/20",
            encoding: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
            completed: "bg-green-500/10 text-green-500 border-green-500/20",
            failed: "bg-red-500/10 text-red-500 border-red-500/20",
            cancelled: "bg-red-500/10 text-red-500 border-red-500/20",
            skipped: "bg-gray-500/10 text-gray-500 border-gray-500/20",
            resuming: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
        };
        return (
            <span className={cn("px-2.5 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider border", styles[status] || styles.queued)}>
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

    return (
        <div className="space-y-6 relative">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-4">
                    <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Visible Active</div>
                    <div className="mt-2 text-2xl font-bold text-helios-ink">{activeCount}</div>
                </div>
                <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-4">
                    <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Visible Failed</div>
                    <div className="mt-2 text-2xl font-bold text-red-500">{failedCount}</div>
                </div>
                <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-4">
                    <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Visible Completed</div>
                    <div className="mt-2 text-2xl font-bold text-emerald-500">{completedCount}</div>
                </div>
            </div>

            {/* Toolbar */}
            <div className="flex flex-col md:flex-row gap-4 justify-between items-center bg-helios-surface/50 p-1 rounded-xl border border-helios-line/10">
                <div className="flex gap-1 p-1 bg-helios-surface border border-helios-line/10 rounded-lg">
                    {(["all", "active", "queued", "completed", "failed"] as TabType[]).map((tab) => (
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

                <div className="flex items-center gap-3 w-full md:w-auto">
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
                    <button
                        onClick={() => void fetchJobs()}
                        className={cn("p-2 rounded-lg border border-helios-line/20 hover:bg-helios-surface-soft", refreshing && "animate-spin")}
                    >
                        <RefreshCw size={16} />
                    </button>
                </div>
            </div>

            {actionError && (
                <div role="alert" aria-live="polite" className="rounded-xl border border-status-error/30 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {actionError}
                </div>
            )}

            {/* Batch Actions Bar */}
            {selected.size > 0 && (
                <div className="flex items-center justify-between bg-helios-solar/10 border border-helios-solar/20 px-6 py-3 rounded-xl animate-in fade-in slide-in-from-top-2">
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
            <div className="bg-helios-surface/50 border border-helios-line/20 rounded-2xl overflow-hidden shadow-sm">
                <table className="w-full text-left border-collapse">
                    <thead className="bg-helios-surface border-b border-helios-line/20 text-xs font-bold text-helios-slate uppercase tracking-wider">
                        <tr>
                            <th className="px-6 py-4 w-10">
                                <input type="checkbox"
                                    checked={selected.size === jobs.length && jobs.length > 0}
                                    onChange={toggleSelectAll}
                                    className="rounded border-helios-line/30 bg-helios-surface-soft accent-helios-solar"
                                />
                            </th>
                            <th className="px-6 py-4">File</th>
                            <th className="px-6 py-4">Status</th>
                            <th className="px-6 py-4">Progress</th>
                            <th className="px-6 py-4">Updated</th>
                            <th className="px-6 py-4 w-14"></th>
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-helios-line/10">
                        {jobs.length === 0 ? (
                            <tr>
                                <td colSpan={6} className="px-6 py-12 text-center text-helios-slate">
                                    {loading ? "Loading jobs..." : "No jobs found"}
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
                                                <span className="text-[10px] text-helios-slate truncate max-w-[240px]">
                                                    {job.input_path}
                                                </span>
                                                <span className="rounded-full border border-helios-line/20 px-2 py-0.5 text-[10px] font-bold text-helios-slate">
                                                    P{job.priority}
                                                </span>
                                            </div>
                                        </motion.div>
                                    </td>
                                    <td className="px-6 py-4">
                                        <motion.div layoutId={`job-status-${job.id}`}>
                                            {getStatusBadge(job.status)}
                                        </motion.div>
                                    </td>
                                    <td className="px-6 py-4">
                                        {job.status === 'encoding' || job.status === 'analyzing' ? (
                                            <div className="w-24 space-y-1">
                                                <div className="h-1.5 w-full bg-helios-line/10 rounded-full overflow-hidden">
                                                    <div className="h-full bg-helios-solar rounded-full transition-all duration-500" style={{ width: `${job.progress}%` }} />
                                                </div>
                                                <div className="text-[10px] text-right font-mono text-helios-slate">
                                                    {job.progress.toFixed(1)}%
                                                </div>
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
                                    <td className="px-6 py-4 text-xs text-helios-slate font-mono">
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
                                                        className="absolute right-0 mt-2 w-44 rounded-xl border border-helios-line/20 bg-helios-surface shadow-xl z-20 overflow-hidden"
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
                                                        {(job.status === "encoding" || job.status === "analyzing") && (
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

            {/* Detail Overlay */}
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
                                className="w-full max-w-2xl bg-helios-surface border border-helios-line/20 rounded-2xl shadow-2xl pointer-events-auto overflow-hidden mx-4"
                            >
                                {/* Header */}
                                <div className="p-6 border-b border-helios-line/10 flex justify-between items-start gap-4 bg-helios-surface-soft/50">
                                    <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-3 mb-1">
                                            {getStatusBadge(focusedJob.job.status)}
                                            <span className="text-[10px] uppercase font-bold tracking-widest text-helios-slate">Job ID #{focusedJob.job.id}</span>
                                            <span className="text-[10px] uppercase font-bold tracking-widest text-helios-slate">Priority {focusedJob.job.priority}</span>
                                        </div>
                                        <h2 id="job-details-title" className="text-lg font-bold text-helios-ink truncate" title={focusedJob.job.input_path}>
                                            {focusedJob.job.input_path.split(/[/\\]/).pop()}
                                        </h2>
                                        <p id="job-details-path" className="text-xs text-helios-slate truncate opacity-60">{focusedJob.job.input_path}</p>
                                    </div>
                                    <button
                                        onClick={() => setFocusedJob(null)}
                                        className="p-2 hover:bg-helios-line/10 rounded-xl transition-colors text-helios-slate"
                                    >
                                        <X size={20} />
                                    </button>
                                </div>

                                <div className="p-6 space-y-8 max-h-[70vh] overflow-y-auto custom-scrollbar">
                                    {detailLoading && (
                                        <p className="text-xs text-helios-slate" aria-live="polite">Loading job details...</p>
                                    )}
                                    {/* Stats Grid */}
                                    <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
                                        <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/10 space-y-1">
                                            <div className="flex items-center gap-2 text-helios-slate mb-1">
                                                <Activity size={12} />
                                                <span className="text-[10px] font-bold uppercase tracking-wider">Video Codec</span>
                                            </div>
                                            <p className="text-sm font-bold text-helios-ink capitalize">
                                                {focusedJob.metadata?.codec_name || "Unknown"}
                                            </p>
                                            <p className="text-[10px] text-helios-slate">
                                                {(focusedJob.metadata?.bit_depth ? `${focusedJob.metadata.bit_depth}-bit` : "Unknown bit depth")} • {focusedJob.metadata?.container.toUpperCase()}
                                            </p>
                                        </div>

                                        <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/10 space-y-1">
                                            <div className="flex items-center gap-2 text-helios-slate mb-1">
                                                <Maximize2 size={12} />
                                                <span className="text-[10px] font-bold uppercase tracking-wider">Resolution</span>
                                            </div>
                                            <p className="text-sm font-bold text-helios-ink">
                                                {focusedJob.metadata ? `${focusedJob.metadata.width}x${focusedJob.metadata.height}` : "-"}
                                            </p>
                                            <p className="text-[10px] text-helios-slate">
                                                {focusedJob.metadata?.fps.toFixed(2)} FPS
                                            </p>
                                        </div>

                                        <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/10 space-y-1">
                                            <div className="flex items-center gap-2 text-helios-slate mb-1">
                                                <Clock size={12} />
                                                <span className="text-[10px] font-bold uppercase tracking-wider">Duration</span>
                                            </div>
                                            <p className="text-sm font-bold text-helios-ink">
                                                {focusedJob.metadata ? formatDuration(focusedJob.metadata.duration_secs) : "-"}
                                            </p>
                                        </div>
                                    </div>

                                    {/* Media Details */}
                                    <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
                                        <div className="space-y-4">
                                            <h3 className="text-[10px] font-black uppercase tracking-[0.2em] text-helios-slate/60 flex items-center gap-2">
                                                <Database size={12} /> Input Details
                                            </h3>
                                            <div className="space-y-3">
                                                <div className="flex justify-between items-center text-xs">
                                                    <span className="text-helios-slate font-medium">File Size</span>
                                                    <span className="text-helios-ink font-bold">{focusedJob.metadata ? formatBytes(focusedJob.metadata.size_bytes) : "-"}</span>
                                                </div>
                                                <div className="flex justify-between items-center text-xs">
                                                    <span className="text-helios-slate font-medium">Video Bitrate</span>
                                                    <span className="text-helios-ink font-bold">
                                                        {focusedJob.metadata && (focusedJob.metadata.video_bitrate_bps ?? focusedJob.metadata.container_bitrate_bps)
                                                            ? `${(((focusedJob.metadata.video_bitrate_bps ?? focusedJob.metadata.container_bitrate_bps) as number) / 1000).toFixed(0)} kbps`
                                                            : "-"}
                                                    </span>
                                                </div>
                                                <div className="flex justify-between items-center text-xs">
                                                    <span className="text-helios-slate font-medium">Audio</span>
                                                    <span className="text-helios-ink font-bold capitalize">
                                                        {focusedJob.metadata?.audio_codec || "N/A"} ({focusedJob.metadata?.audio_channels || 0}ch)
                                                    </span>
                                                </div>
                                            </div>
                                        </div>

                                        <div className="space-y-4">
                                            <h3 className="text-[10px] font-black uppercase tracking-[0.2em] text-helios-solar flex items-center gap-2">
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
                                                <div className="h-[80px] flex items-center justify-center border border-dashed border-helios-line/20 rounded-xl text-[10px] text-helios-slate italic">
                                                    {focusedJob.job.status === 'encoding' ? "Encoding in progress..." : "No encode data available"}
                                                </div>
                                            )}
                                        </div>
                                    </div>

                                    {/* Decision Info */}
                                    {focusedJob.job.decision_reason && (
                                        <div className="p-4 rounded-xl bg-amber-500/5 border border-amber-500/10">
                                            <div className="flex items-center gap-2 text-amber-600 mb-1">
                                                <Info size={12} />
                                                <span className="text-[10px] font-bold uppercase tracking-wider">Decision Context</span>
                                            </div>
                                            <p className="text-xs text-amber-700/80 leading-relaxed italic">
                                                "{focusedJob.job.decision_reason}"
                                            </p>
                                        </div>
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
                                            {(focusedJob.job.status === 'encoding' || focusedJob.job.status === 'analyzing') && (
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
            </AnimatePresence>

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
