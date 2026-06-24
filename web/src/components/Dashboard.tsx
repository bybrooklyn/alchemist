import { useEffect, useState } from "react";
import {
    Activity,
    CheckCircle2,
    AlertCircle,
    HardDrive,
    Database,
    Zap,
    Clock3,
    type LucideIcon,
} from "lucide-react";
import { apiJson, isApiError } from "../lib/api";
import { useSharedStats } from "../lib/statsStore";
import { showToast } from "../lib/toast";
import ResourceMonitor from "./ResourceMonitor";
import { withErrorBoundary } from "./ErrorBoundary";

interface Job {
    id: number;
    input_path: string;
    status: string;
    progress?: number;
    created_at: string;
    updated_at?: string;
}

interface SettingsBundleResponse {
    settings: {
        scanner: { directories: string[] };
        notifications: { targets: Array<unknown> };
        schedule: { windows: Array<unknown> };
    };
}

interface PreferenceResponse {
    key: string;
    value: string;
}

interface DailyStat {
    date: string;
    jobs_completed: number;
    bytes_saved: number;
}

interface QueueEtaResponse {
    remaining_jobs: number;
    est_seconds_remaining: number | null;
    sample_size: number;
}

interface StatCardProps {
    label: string;
    value: number;
    icon: LucideIcon;
    colorClass: string;
}

const DEFAULT_STATS = {
    total: 0,
    completed: 0,
    active: 0,
    failed: 0,
    concurrent_limit: 1,
};

function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function StatCard({ label, value, icon: Icon, colorClass }: StatCardProps) {
    return (
        <div className="px-4 py-3 rounded-lg bg-helios-surface border border-helios-line/30 hover:bg-helios-surface-soft transition-colors">
            <div className="flex items-center justify-between gap-3">
                <span className="text-xs font-medium text-helios-slate flex items-center gap-1.5">
                    <Icon size={14} className={`${colorClass} opacity-70`} />
                    {label}
                </span>
                <span className={`text-xl font-bold font-mono ${colorClass}`}>{value}</span>
            </div>
        </div>
    );
}

function isActiveStatus(status: string): boolean {
    return ["analyzing", "encoding", "remuxing", "resuming"].includes(status.toLowerCase());
}

function formatDuration(seconds: number): string {
    const totalMinutes = Math.max(1, Math.round(seconds / 60));
    if (totalMinutes < 60) return `${totalMinutes}m`;

    const hours = Math.floor(totalMinutes / 60);
    const minutes = totalMinutes % 60;
    if (hours < 24) return `${hours}h ${minutes}m`;

    const days = Math.floor(hours / 24);
    const remainingHours = hours % 24;
    return `${days}d ${remainingHours}h`;
}

function Dashboard() {
    const [jobs, setJobs] = useState<Job[]>([]);
    const [activeJobs, setActiveJobs] = useState<Job[]>([]);
    const [jobsLoading, setJobsLoading] = useState(true);
    const [activeJobsLoading, setActiveJobsLoading] = useState(true);
    const [bundle, setBundle] = useState<SettingsBundleResponse | null>(null);
    const [weekStats, setWeekStats] = useState<{
        bytesSaved: number;
        jobsCompleted: number;
        avgCompression: number;
    } | null>(null);
    const [queueEta, setQueueEta] = useState<QueueEtaResponse | null>(null);
    const [queueEtaLoading, setQueueEtaLoading] = useState(true);
    const [engineStatus, setEngineStatus] = useState<"paused" | "running" | "draining">("paused");
    const { stats: sharedStats, error: statsError } = useSharedStats();
    const stats = sharedStats ?? DEFAULT_STATS;

    useEffect(() => {
        if (!statsError) {
            return;
        }
        showToast({
            kind: "error",
            title: "Stats",
            message: statsError,
        });
    }, [statsError]);

    useEffect(() => {
        const fetchJobs = async () => {
            try {
                const [recent, active] = await Promise.all([
                    apiJson<Job[]>(
                        `/api/jobs/table?${new URLSearchParams({
                            limit: "5",
                            sort: "created_at",
                            sort_desc: "true",
                        })}`
                    ),
                    apiJson<Job[]>(
                        `/api/jobs/table?${new URLSearchParams({
                            limit: "8",
                            status: "analyzing,encoding,remuxing,resuming",
                            sort: "updated_at",
                            sort_desc: "true",
                        })}`
                    ),
                ]);
                setJobs(recent);
                setActiveJobs(active.filter((job) => isActiveStatus(job.status)));
            } catch (error) {
                const message = isApiError(error) ? error.message : "Failed to fetch jobs";
                showToast({ kind: "error", title: "Dashboard", message });
            } finally {
                setJobsLoading(false);
                setActiveJobsLoading(false);
            }
        };

        void fetchJobs();
        void (async () => {
            try {
                const bundleResponse = await apiJson<SettingsBundleResponse>("/api/settings/bundle");
                setBundle(bundleResponse);

                if (
                    bundleResponse.settings.scanner.directories.length === 0
                    && typeof window !== "undefined"
                    && window.location.pathname !== "/setup"
                ) {
                    let setupComplete: string | null = null;
                    try {
                        const preference = await apiJson<PreferenceResponse>(
                            "/api/settings/preferences/setup_complete"
                        );
                        setupComplete = preference.value;
                    } catch (error) {
                        if (!(isApiError(error) && error.status === 404)) {
                            throw error;
                        }
                    }

                    if (setupComplete !== "true") {
                        window.location.href = "/setup";
                    }
                }
            } catch {
                // Ignore setup redirect lookup failures here; dashboard data fetches handle their own UX.
            }
        })();
        void apiJson<{ status: "paused" | "running" | "draining" }>("/api/engine/status")
            .then((data) => setEngineStatus(data.status))
            .catch((e) => { console.debug("Dashboard: engine status fetch failed", e); });

        const pollVisible = () => {
            if (document.visibilityState === "visible") {
                void fetchJobs();
            }
        };

        const intervalId = window.setInterval(pollVisible, 5000);
        document.addEventListener("visibilitychange", pollVisible);

        return () => {
            window.clearInterval(intervalId);
            document.removeEventListener("visibilitychange", pollVisible);
        };
    }, []);

    useEffect(() => {
        const fetchWeekStats = async () => {
            try {
                const data = await apiJson<DailyStat[]>("/api/stats/daily");
                const last7 = data.slice(-7);
                const bytesSaved = last7.reduce((sum, d) => sum + d.bytes_saved, 0);
                const jobsCompleted = last7.reduce((sum, d) => sum + d.jobs_completed, 0);
                setWeekStats({ bytesSaved, jobsCompleted, avgCompression: 0 });
            } catch {
                // not critical — panel just won't show
            }
        };
        const fetchQueueEta = async () => {
            try {
                setQueueEta(await apiJson<QueueEtaResponse>("/api/stats/queue-eta"));
            } catch {
                // Non-critical estimate; dashboard still renders without it.
            } finally {
                setQueueEtaLoading(false);
            }
        };
        void fetchWeekStats();
        void fetchQueueEta();
    }, []);

    const formatRelativeTime = (iso?: string) => {
        if (!iso) return "Just now";
        const then = new Date(iso).getTime();
        if (Number.isNaN(then)) return "Just now";
        const diff = Math.max(0, Date.now() - then);
        const minutes = Math.floor(diff / 60000);
        if (minutes < 1) return "Just now";
        if (minutes < 60) return `${minutes}m ago`;
        const hours = Math.floor(minutes / 60);
        if (hours < 24) return `${hours}h ago`;
        const days = Math.floor(hours / 24);
        return `${days}d ago`;
    };

    return (
        <div className="flex flex-col gap-5 flex-1 min-h-0 overflow-hidden">

            {/* Engine paused banner */}
            {engineStatus === "paused" && (
                <div className="rounded-lg border border-helios-solar/20 bg-helios-solar/10 px-4 py-3 flex items-center gap-3">
                    <span className="text-helios-solar shrink-0 text-xs font-semibold">ENGINE PAUSED</span>
                    <span className="text-sm text-helios-ink">
                        Analysis runs automatically. Click{" "}
                        <span className="font-bold">Start</span>
                        {" "}in the header to begin encoding.
                    </span>
                </div>
            )}

            <section
                aria-labelledby="dashboard-active-now-title"
                className="md:hidden rounded-lg bg-helios-surface border border-helios-line/30 overflow-hidden"
            >
                <div className="flex items-center justify-between px-4 py-3 border-b border-helios-line/20">
                    <h3 id="dashboard-active-now-title" className="text-sm font-semibold text-helios-ink flex items-center gap-2">
                        <Zap size={16} className="text-helios-solar" />
                        Active Now
                    </h3>
                    <a href="/jobs" className="text-xs font-medium text-helios-solar hover:underline">
                        Jobs
                    </a>
                </div>
                <div className="p-3">
                    {activeJobsLoading ? (
                        <div className="space-y-2">
                            {Array.from({ length: 3 }).map((_, i) => (
                                <div key={i} className="h-14 w-full rounded-lg bg-helios-surface-soft/60 animate-pulse" />
                            ))}
                        </div>
                    ) : activeJobs.length === 0 ? (
                        <div className="py-8 text-center text-sm text-helios-slate/70">
                            No active jobs
                        </div>
                    ) : (
                        <div className="space-y-2">
                            {activeJobs.map((job) => {
                                const progress = Math.max(0, Math.min(100, job.progress ?? 0));
                                return (
                                    <div key={job.id} className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 p-3">
                                        <div className="flex items-start justify-between gap-3">
                                            <div className="min-w-0">
                                                <p className="truncate text-sm font-semibold text-helios-ink" title={job.input_path}>
                                                    {job.input_path.split(/[/\\]/).pop()}
                                                </p>
                                                <p className="mt-0.5 text-xs capitalize text-helios-slate">
                                                    {job.status}
                                                </p>
                                            </div>
                                            <span className="font-mono text-xs text-helios-solar">
                                                {progress.toFixed(0)}%
                                            </span>
                                        </div>
                                        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-helios-line/20">
                                            <div
                                                className="h-full rounded-full bg-helios-solar transition-all duration-500"
                                                style={{ width: `${progress}%` }}
                                            />
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>
            </section>

            {/* Stat row — compact horizontal strip */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                <StatCard label="Active Jobs" value={stats.active} icon={Zap} colorClass="text-helios-solar" />
                <StatCard label="Completed" value={stats.completed} icon={CheckCircle2} colorClass="text-helios-solar" />
                <StatCard label="Failed" value={stats.failed} icon={AlertCircle} colorClass="text-status-error" />
                <StatCard label="Total Processed" value={stats.total} icon={Database} colorClass="text-helios-solar" />
            </div>

            {/* Main content row */}
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-5 flex-1 min-h-0">

                {/* Recent Activity — takes 2/3 */}
                <div className="lg:col-span-2 rounded-lg bg-helios-surface border border-helios-line/30 flex flex-col overflow-hidden">
                    <div className="flex items-center justify-between px-5 py-4 border-b border-helios-line/20">
                        <h3 className="text-sm font-semibold text-helios-ink flex items-center gap-2">
                            <Activity size={16} className="text-helios-solar" />
                            Recent Activity
                        </h3>
                        <a href="/jobs" className="text-xs font-medium text-helios-solar hover:underline">
                            View all
                        </a>
                    </div>
                    <div className="flex flex-col gap-1 p-3 overflow-y-auto flex-1">
                        {jobsLoading && jobs.length === 0 ? (
                            <div className="py-2">
                                {Array.from({ length: 5 }).map((_, i) => (
                                    <div key={i} className="h-9 w-full rounded-lg bg-helios-surface-soft/60 animate-pulse mb-1.5" />
                                ))}
                            </div>
                        ) : jobs.length === 0 ? (
                            <div className="flex flex-col items-center justify-center py-10 gap-2">
                                <span className="text-sm text-helios-slate/60">
                                    No recent activity.
                                </span>
                                <a href="/settings" className="text-xs text-helios-solar hover:underline">
                                    Add a library folder
                                </a>
                            </div>
                        ) : (
                            jobs.map((job) => {
                                const s = (job.status || "").toLowerCase();
                                return (
                                    <div key={job.id} className="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-helios-surface-soft/60 transition-colors group">
                                        <div className="flex items-center gap-3 min-w-0">
                                            <div className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                                                s === "completed"
                                                    ? "bg-helios-solar"
                                                : s === "failed"
                                                    ? "bg-status-error"
                                                : s === "encoding" || s === "analyzing"
                                                    ? "bg-helios-solar animate-pulse"
                                                : "bg-helios-slate/40"
                                            }`} />
                                            <div className="flex flex-col min-w-0">
                                                <span className="text-sm font-medium text-helios-ink truncate" title={job.input_path}>
                                                    {job.input_path.split(/[/\\]/).pop()}
                                                </span>
                                                <span className="text-xs text-helios-slate/70">
                                                    {job.status} ·{" "}
                                                    {formatRelativeTime(job.created_at)}
                                                </span>
                                            </div>
                                        </div>
                                        <span className="text-xs font-mono text-helios-slate/50 whitespace-nowrap ml-4">
                                            #{job.id}
                                        </span>
                                    </div>
                                );
                            })
                        )}
                    </div>
                </div>

                {/* Right column: weekly savings + bundle stats */}
                <div className="flex flex-col gap-4 h-full">
                    <div className="rounded-lg bg-helios-surface border border-helios-line/30 p-5 flex-1 flex flex-col">
                        <h3 className="text-sm font-semibold text-helios-ink mb-4 flex items-center gap-2">
                            <HardDrive size={15} className="text-helios-solar" />
                            Last 7 Days
                        </h3>
                        {weekStats ? (
                            <div className="space-y-3">
                                <div className="flex items-center justify-between text-xs">
                                    <span className="text-helios-slate">
                                        Space recovered
                                    </span>
                                    <span className="font-bold font-mono text-helios-solar">
                                        {formatBytes(weekStats.bytesSaved)}
                                    </span>
                                </div>
                                <div className="flex items-center justify-between text-xs">
                                    <span className="text-helios-slate">
                                        Jobs completed
                                    </span>
                                    <span className="font-bold font-mono text-helios-ink">
                                        {weekStats.jobsCompleted}
                                    </span>
                                </div>
                            </div>
                        ) : (
                            <p className="text-xs text-helios-slate/60 italic">
                                No data yet
                            </p>
                        )}
                    </div>

                    <div className="rounded-lg bg-helios-surface border border-helios-line/30 p-5 space-y-3">
                        <h3 className="text-sm font-semibold text-helios-ink flex items-center gap-2">
                            <Clock3 size={15} className="text-helios-solar" />
                            Queue ETA
                        </h3>
                        {queueEtaLoading ? (
                            <div className="space-y-2">
                                <div className="h-3 w-28 rounded bg-helios-surface-soft animate-pulse" />
                                <div className="h-4 w-36 rounded bg-helios-surface-soft animate-pulse" />
                            </div>
                        ) : queueEta ? (
                            <div className="space-y-2">
                                <div className="flex items-center justify-between text-xs">
                                    <span className="text-helios-slate">Remaining</span>
                                    <span className="font-bold font-mono text-helios-ink">
                                        {queueEta.remaining_jobs} jobs remaining
                                    </span>
                                </div>
                                <div className="flex items-center justify-between gap-3 text-xs">
                                    <span className="text-helios-slate">Estimate</span>
                                    <span className="text-right font-bold font-mono text-helios-solar">
                                        {queueEta.remaining_jobs === 0
                                            ? "Queue is clear"
                                            : queueEta.est_seconds_remaining !== null
                                                ? `About ${formatDuration(queueEta.est_seconds_remaining)} left`
                                                : "Unavailable"}
                                    </span>
                                </div>
                                <p className="text-[11px] leading-4 text-helios-slate/70">
                                    Based on {queueEta.sample_size} recent completed jobs.
                                </p>
                            </div>
                        ) : (
                            <p className="text-xs text-helios-slate/60 italic">
                                ETA unavailable until completed samples exist.
                            </p>
                        )}
                    </div>

                    {/* Config summary */}
                    {bundle && (
                        <div className="rounded-lg bg-helios-surface border border-helios-line/30 p-5 space-y-3">
                            <h3 className="text-sm font-semibold text-helios-ink">Configuration</h3>
                            <div className="space-y-2">
                                <div className="flex items-center justify-between text-xs">
                                    <span className="text-helios-slate">
                                        Library roots
                                    </span>
                                    <span className="font-medium text-helios-ink font-mono">
                                        {bundle.settings.scanner.directories.length}
                                    </span>
                                </div>
                                <div className="flex items-center justify-between text-xs">
                                    <span className="text-helios-slate">
                                        Notification targets
                                    </span>
                                    <span className="font-medium text-helios-ink font-mono">
                                        {bundle.settings.notifications.targets.length}
                                    </span>
                                </div>
                                <div className="flex items-center justify-between text-xs">
                                    <span className="text-helios-slate">
                                        Schedule windows
                                    </span>
                                    <span className="font-medium text-helios-ink font-mono">
                                        {bundle.settings.schedule.windows.length}
                                    </span>
                                </div>
                            </div>
                        </div>
                    )}
                </div>
            </div>

            {/* Resource Monitor */}
            <div className="rounded-lg bg-helios-surface border border-helios-line/30 p-5">
                <h3 className="text-sm font-semibold text-helios-slate mb-5 flex items-center gap-2">
                    <Activity size={15} className="text-helios-solar" />
                    System Health
                </h3>
                <ResourceMonitor />
            </div>

        </div>
    );
}

export default withErrorBoundary(Dashboard, "Dashboard");
