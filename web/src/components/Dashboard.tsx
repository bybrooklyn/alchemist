import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
    Activity,
    CheckCircle2,
    AlertCircle,
    HardDrive,
    Database,
    Zap,
    Terminal,
    type LucideIcon,
} from "lucide-react";
import { apiJson, isApiError } from "../lib/api";
import { useSharedStats } from "../lib/statsStore";
import { showToast } from "../lib/toast";
import ResourceMonitor from "./ResourceMonitor";

interface Job {
    id: number;
    input_path: string;
    status: string;
    created_at: string;
}

interface SettingsBundleResponse {
    settings: {
        scanner: { directories: string[] };
        notifications: { targets: Array<unknown> };
        schedule: { windows: Array<unknown> };
    };
}

interface StatCardProps {
    label: string;
    value: number;
    icon: LucideIcon;
    colorClass: string;
}

interface QuickStartItem {
    title: string;
    body: ReactNode;
    icon: LucideIcon;
    tone: string;
    bg: string;
}

const DEFAULT_STATS = {
    total: 0,
    completed: 0,
    active: 0,
    failed: 0,
    concurrent_limit: 1,
};

function StatCard({ label, value, icon: Icon, colorClass }: StatCardProps) {
    return (
        <div className="p-5 rounded-2xl bg-helios-surface border border-helios-line/40 shadow-sm relative overflow-hidden group hover:bg-helios-surface-soft transition-colors">
            <div className={`absolute -top-2 -right-2 p-3 opacity-10 group-hover:opacity-20 transition-opacity ${colorClass}`}>
                <Icon size={64} />
            </div>
            <div className="relative z-10 flex flex-col gap-1">
                <span className="text-xs font-bold uppercase tracking-wider text-helios-slate">{label}</span>
                <span className={`text-3xl font-bold font-mono tracking-tight ${colorClass}`}>{value}</span>
            </div>
        </div>
    );
}

export default function Dashboard() {
    const [jobs, setJobs] = useState<Job[]>([]);
    const [jobsLoading, setJobsLoading] = useState(true);
    const [bundle, setBundle] = useState<SettingsBundleResponse | null>(null);
    const [engineStatus, setEngineStatus] = useState<"paused" | "running">("paused");
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
                const data = await apiJson<Job[]>(
                    `/api/jobs/table?${new URLSearchParams({
                        limit: "5",
                        sort: "created_at",
                        sort_desc: "true",
                    })}`
                );
                setJobs(data);
            } catch (error) {
                const message = isApiError(error) ? error.message : "Failed to fetch jobs";
                showToast({ kind: "error", title: "Dashboard", message });
            } finally {
                setJobsLoading(false);
            }
        };

        void fetchJobs();
        void apiJson<SettingsBundleResponse>("/api/settings/bundle")
            .then(setBundle)
            .catch(() => undefined);
        void apiJson<{ status: "paused" | "running" }>("/api/engine/status")
            .then((data) => setEngineStatus(data.status))
            .catch(() => undefined);

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

    const quickStartItems = useMemo<QuickStartItem[]>(() => {
        const items: QuickStartItem[] = [];
        const libraryRoots = bundle?.settings.scanner.directories.length ?? 0;
        const notificationTargets = bundle?.settings.notifications.targets.length ?? 0;
        const schedules = bundle?.settings.schedule.windows.length ?? 0;

        if (libraryRoots === 0) {
            items.push({
                title: "Finish Library Setup",
                body: (
                    <>
                        No canonical server library roots are configured yet. Use{" "}
                        <a href="/settings" className="underline hover:text-helios-ink transition-colors">
                            Settings
                        </a>
                        {" "}or re-run setup to point Alchemist at the right server folders.
                    </>
                ),
                icon: HardDrive,
                tone: "text-helios-solar",
                bg: "bg-helios-solar/10",
            });
        }

        if (stats.failed > 0) {
            items.push({
                title: "Review Failures",
                body: (
                    <>
                        {stats.failed} jobs failed recently. Check{" "}
                        <a href="/logs" className="underline hover:text-helios-ink transition-colors">
                            Logs
                        </a>{" "}
                        to diagnose and retry.
                    </>
                ),
                icon: Terminal,
                tone: "text-red-500",
                bg: "bg-red-500/10",
            });
        }

        if (notificationTargets === 0 || schedules === 0) {
            items.push({
                title: "Complete Automation",
                body: (
                    <>
                        {notificationTargets === 0 ? "Notifications" : "Schedule windows"} still need attention if you want a true set-it-and-forget-it workflow.
                    </>
                ),
                icon: Zap,
                tone: "text-amber-500",
                bg: "bg-amber-500/10",
            });
        }

        if (stats.active === 0 && stats.total > 0) {
            items.push({
                title: "Queue Is Idle",
                body: (
                    <>
                        No jobs are active right now. Review the queue in{" "}
                        <a href="/jobs" className="underline hover:text-helios-ink transition-colors">Jobs</a>{" "}
                        or verify that your watched server folders are correct.
                    </>
                ),
                icon: Activity,
                tone: "text-emerald-500",
                bg: "bg-emerald-500/10",
            });
        }

        return items.slice(0, 3);
    }, [bundle, stats.active, stats.failed, stats.total]);

    return (
        <div className="flex flex-col gap-6 flex-1 min-h-0 overflow-hidden">
            {engineStatus === "paused" && (
                <div className="rounded-2xl border border-amber-500/20 bg-amber-500/10 px-5 py-4">
                    <div className="text-[10px] font-bold uppercase tracking-widest text-amber-500">Engine Paused</div>
                    <div className="mt-2 text-sm text-helios-ink">
                        The queue can still fill up, but Alchemist will not start encoding until you click <span className="font-bold">Start</span> in the header.
                    </div>
                </div>
            )}

            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
                <StatCard label="Active Jobs" value={stats.active} icon={Zap} colorClass="text-amber-500" />
                <StatCard label="Completed" value={stats.completed} icon={CheckCircle2} colorClass="text-emerald-500" />
                <StatCard label="Failed" value={stats.failed} icon={AlertCircle} colorClass="text-red-500" />
                <StatCard label="Total Processed" value={stats.total} icon={Database} colorClass="text-helios-solar" />
            </div>

            {bundle && (
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-4">
                        <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Library Roots</div>
                        <div className="mt-2 text-2xl font-bold text-helios-ink">{bundle.settings.scanner.directories.length}</div>
                    </div>
                    <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-4">
                        <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Notification Targets</div>
                        <div className="mt-2 text-2xl font-bold text-helios-ink">{bundle.settings.notifications.targets.length}</div>
                    </div>
                    <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-4">
                        <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Schedule Windows</div>
                        <div className="mt-2 text-2xl font-bold text-helios-ink">{bundle.settings.schedule.windows.length}</div>
                    </div>
                </div>
            )}

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 items-stretch">
                <div className="lg:col-span-2 p-6 rounded-3xl bg-helios-surface border border-helios-line/40 shadow-sm flex flex-col">
                    <div className="flex items-center justify-between mb-6">
                        <h3 className="text-lg font-bold text-helios-ink flex items-center gap-2">
                            <Activity size={20} className="text-helios-solar" />
                            Recent Activity
                        </h3>
                        <a href="/jobs" className="text-xs font-bold text-helios-solar hover:underline uppercase tracking-wide">View All</a>
                    </div>

                    <div className="flex flex-col gap-3">
                        {jobsLoading && jobs.length === 0 ? (
                            <div className="text-center py-8 text-helios-slate animate-pulse">Loading activity...</div>
                        ) : jobs.length === 0 ? (
                            <div className="text-center py-8 text-helios-slate/60 italic">No recent activity found.</div>
                        ) : (
                            jobs.map((job) => {
                                const status = (job.status || "").toLowerCase();
                                return (
                                    <div key={job.id} className="flex items-center justify-between p-3 rounded-xl bg-helios-surface-soft hover:bg-white/5 transition-colors border border-transparent hover:border-helios-line/20">
                                        <div className="flex items-center gap-3 min-w-0">
                                            <div className={`w-2 h-2 rounded-full shrink-0 ${status === "completed"
                                                ? "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.4)]"
                                                : status === "failed"
                                                    ? "bg-red-500"
                                                    : status === "encoding" || status === "analyzing"
                                                        ? "bg-amber-500 animate-pulse"
                                                        : "bg-helios-slate"
                                                }`} />
                                            <div className="flex flex-col min-w-0">
                                                <span className="text-sm font-medium text-helios-ink truncate" title={job.input_path}>
                                                    {job.input_path.split(/[/\\]/).pop()}
                                                </span>
                                                <span className="text-[10px] text-helios-slate uppercase tracking-wide font-bold">
                                                    {job.status} · {formatRelativeTime(job.created_at)}
                                                </span>
                                            </div>
                                        </div>
                                        <span className="text-xs font-mono text-helios-slate/60 whitespace-nowrap ml-4">
                                            #{job.id}
                                        </span>
                                    </div>
                                );
                            })
                        )}
                    </div>
                </div>

                <div className="p-6 rounded-3xl bg-helios-surface border border-helios-line/40 shadow-sm h-full">
                    <h3 className="text-lg font-bold text-helios-ink mb-6 flex items-center gap-2">
                        <Zap size={20} className="text-helios-solar" />
                        Quick Start
                    </h3>
                    <div className="flex flex-col gap-4">
                        {quickStartItems.map(({ title, body, icon: Icon, tone, bg }) => (
                            <div className="flex gap-4 items-start" key={title}>
                                <div className={`p-2.5 rounded-xl ${bg} ${tone} mt-0.5 shadow-inner`}>
                                    <Icon size={18} />
                                </div>
                                <div>
                                    <h4 className="text-sm font-bold text-helios-ink">{title}</h4>
                                    <p className="text-xs text-helios-slate mt-1 leading-relaxed">{body}</p>
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            </div>

            <div className="p-6 rounded-3xl bg-helios-surface border border-helios-line/40 shadow-sm">
                <div className="flex items-center gap-2 mb-6">
                    <Activity size={18} className="text-helios-solar" />
                    <h3 className="text-sm font-bold uppercase tracking-wider text-helios-slate">System Health</h3>
                </div>
                <ResourceMonitor />
            </div>
        </div>
    );
}
