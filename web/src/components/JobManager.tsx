import { useState, useEffect, useCallback } from "react";
import {
    Search, RefreshCw, Trash2, Ban, Play,
    MoreHorizontal, Check, AlertCircle, Clock, FileVideo,
    X, Info, Activity, Database, Zap, ArrowRight, Maximize2
} from "lucide-react";
import { apiFetch } from "../lib/api";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { motion, AnimatePresence } from "framer-motion";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
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
    bit_depth: number;
    size_bytes: number;
    bit_rate: number;
    fps: number;
    container: string;
    audio_codec?: string;
    audio_channels?: number;
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
    const [search, setSearch] = useState("");
    const [page, setPage] = useState(1);
    const [refreshing, setRefreshing] = useState(false);
    const [focusedJob, setFocusedJob] = useState<JobDetail | null>(null);
    const [detailLoading, setDetailLoading] = useState(false);

    // Filter mapping
    const getStatusFilter = (tab: TabType) => {
        switch (tab) {
            case "active": return "analyzing,encoding";
            case "queued": return "queued";
            case "completed": return "completed";
            case "failed": return "failed,cancelled";
            default: return "";
        }
    };

    const fetchJobs = useCallback(async () => {
        setRefreshing(true);
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
            if (search) {
                params.set("search", search);
            }

            const res = await apiFetch(`/api/jobs/table?${params}`);
            if (res.ok) {
                const data = await res.json();
                setJobs(data);
            }
        } catch (e) {
            console.error("Failed to fetch jobs", e);
        } finally {
            setLoading(false);
            setRefreshing(false);
        }
    }, [activeTab, search, page]);

    useEffect(() => {
        fetchJobs();
        const interval = setInterval(fetchJobs, 5000); // Auto-refresh every 5s
        return () => clearInterval(interval);
    }, [fetchJobs]);

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

    const handleBatch = async (action: "cancel" | "restart" | "delete") => {
        if (selected.size === 0) return;
        if (!confirm(`Are you sure you want to ${action} ${selected.size} jobs?`)) return;

        try {
            const res = await apiFetch("/api/jobs/batch", {
                method: "POST",
                body: JSON.stringify({
                    action,
                    ids: Array.from(selected)
                })
            });

            if (res.ok) {
                setSelected(new Set());
                fetchJobs();
            }
        } catch (e) {
            console.error("Batch action failed", e);
        }
    };

    const clearCompleted = async () => {
        if (!confirm("Clear all completed jobs?")) return;
        await apiFetch("/api/jobs/clear-completed", { method: "POST" });
        fetchJobs();
    };

    const fetchJobDetails = async (id: number) => {
        setDetailLoading(true);
        try {
            const res = await apiFetch(`/api/jobs/${id}/details`);
            if (res.ok) {
                const data = await res.json();
                setFocusedJob(data);
            }
        } catch (e) {
            console.error("Failed to fetch job details", e);
        } finally {
            setDetailLoading(false);
        }
    };

    const handleAction = async (id: number, action: "cancel" | "restart" | "delete") => {
        try {
            const res = await apiFetch(`/api/jobs/${id}/${action}`, { method: "POST" });
            if (res.ok) {
                if (action === "delete") setFocusedJob(null);
                else fetchJobDetails(id);
                fetchJobs();
            }
        } catch (e) {
            console.error(`Action ${action} failed`, e);
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
        };
        return (
            <span className={cn("px-2.5 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider border", styles[status] || styles.queued)}>
                {status}
            </span>
        );
    };

    return (
        <div className="space-y-6 relative">
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
                            value={search}
                            onChange={(e) => setSearch(e.target.value)}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded-lg pl-9 pr-4 py-2 text-sm text-helios-ink focus:border-helios-solar outline-none"
                        />
                    </div>
                    <button
                        onClick={() => fetchJobs()}
                        className={cn("p-2 rounded-lg border border-helios-line/20 hover:bg-helios-surface-soft", refreshing && "animate-spin")}
                    >
                        <RefreshCw size={16} />
                    </button>
                </div>
            </div>

            {/* Batch Actions Bar */}
            {selected.size > 0 && (
                <div className="flex items-center justify-between bg-helios-solar/10 border border-helios-solar/20 px-6 py-3 rounded-xl animate-in fade-in slide-in-from-top-2">
                    <span className="text-sm font-bold text-helios-solar">
                        {selected.size} jobs selected
                    </span>
                    <div className="flex gap-2">
                        <button onClick={() => handleBatch("restart")} className="p-2 hover:bg-helios-solar/20 rounded-lg text-helios-solar" title="Restart">
                            <RefreshCw size={18} />
                        </button>
                        <button onClick={() => handleBatch("cancel")} className="p-2 hover:bg-helios-solar/20 rounded-lg text-helios-solar" title="Cancel">
                            <Ban size={18} />
                        </button>
                        <button onClick={() => handleBatch("delete")} className="p-2 hover:bg-red-500/10 rounded-lg text-red-500" title="Delete">
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
                        </tr>
                    </thead>
                    <tbody className="divide-y divide-helios-line/10">
                        {jobs.length === 0 ? (
                            <tr>
                                <td colSpan={5} className="px-6 py-12 text-center text-helios-slate">
                                    {loading ? "Loading jobs..." : "No jobs found"}
                                </td>
                            </tr>
                        ) : (
                            jobs.map((job) => (
                                <tr
                                    key={job.id}
                                    onClick={() => fetchJobDetails(job.id)}
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
                                            <span className="text-[10px] text-helios-slate truncate max-w-[300px]">
                                                {job.input_path}
                                            </span>
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
                                </tr>
                            ))
                        )}
                    </tbody>
                </table>
            </div>

            {/* Footer Actions */}
            <div className="flex justify-between items-center pt-2">
                <p className="text-xs text-helios-slate font-medium">Showing {jobs.length} jobs (Limit 50)</p>
                <button onClick={clearCompleted} className="text-xs text-red-500 hover:text-red-400 font-bold flex items-center gap-1 transition-colors">
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
                            className="fixed inset-0 bg-helios-ink/40 backdrop-blur-md z-[100]"
                        />
                        <div className="fixed inset-0 flex items-center justify-center pointer-events-none z-[101]">
                            <motion.div
                                key="modal-content"
                                initial={{ opacity: 0, scale: 0.95, y: 10 }}
                                animate={{ opacity: 1, scale: 1, y: 0 }}
                                exit={{ opacity: 0, scale: 0.95, y: 10 }}
                                transition={{ duration: 0.2 }}
                                className="w-full max-w-2xl bg-helios-surface border border-helios-line/20 rounded-2xl shadow-2xl pointer-events-auto overflow-hidden mx-4"
                            >
                                {/* Header */}
                                <div className="p-6 border-b border-helios-line/10 flex justify-between items-start gap-4 bg-helios-surface-soft/50">
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-3 mb-1">
                                            {getStatusBadge(focusedJob.job.status)}
                                            <span className="text-[10px] uppercase font-bold tracking-widest text-helios-slate">Job ID #{focusedJob.job.id}</span>
                                        </div>
                                        <h2 className="text-lg font-bold text-helios-ink truncate" title={focusedJob.job.input_path}>
                                            {focusedJob.job.input_path.split(/[/\\]/).pop()}
                                        </h2>
                                        <p className="text-xs text-helios-slate truncate opacity-60">{focusedJob.job.input_path}</p>
                                    </div>
                                    <button
                                        onClick={() => setFocusedJob(null)}
                                        className="p-2 hover:bg-helios-line/10 rounded-xl transition-colors text-helios-slate"
                                    >
                                        <X size={20} />
                                    </button>
                                </div>

                                <div className="p-6 space-y-8 max-h-[70vh] overflow-y-auto custom-scrollbar">
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
                                                {focusedJob.metadata?.bit_depth}-bit • {focusedJob.metadata?.container.toUpperCase()}
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
                                                        {focusedJob.metadata ? `${(focusedJob.metadata.bit_rate / 1000).toFixed(0)} kbps` : "-"}
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
                                            {(focusedJob.job.status === 'failed' || focusedJob.job.status === 'cancelled') && (
                                                <button
                                                    onClick={() => handleAction(focusedJob.job.id, 'restart')}
                                                    className="px-4 py-2 bg-helios-solar text-white rounded-lg text-sm font-bold flex items-center gap-2 hover:brightness-110 active:scale-95 transition-all shadow-sm"
                                                >
                                                    <RefreshCw size={14} /> Retry Job
                                                </button>
                                            )}
                                            {(focusedJob.job.status === 'encoding' || focusedJob.job.status === 'analyzing') && (
                                                <button
                                                    onClick={() => handleAction(focusedJob.job.id, 'cancel')}
                                                    className="px-4 py-2 border border-helios-line/20 bg-helios-surface text-helios-slate rounded-lg text-sm font-bold flex items-center gap-2 hover:bg-helios-surface-soft active:scale-95 transition-all"
                                                >
                                                    <Ban size={14} /> Stop / Cancel
                                                </button>
                                            )}
                                        </div>
                                        <button
                                            onClick={() => {
                                                if (confirm("Delete this job from history?")) handleAction(focusedJob.job.id, 'delete');
                                            }}
                                            className="px-4 py-2 text-red-500 hover:bg-red-500/5 rounded-lg text-sm font-bold flex items-center gap-2 transition-all"
                                        >
                                            <Trash2 size={14} /> Delete
                                        </button>
                                    </div>
                                </div>
                            </motion.div>
                        </div>
                    </>
                )}
            </AnimatePresence>
        </div>
    );
}
