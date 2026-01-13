import { useEffect, useMemo, useState } from "react";
import {
    Activity,
    CheckCircle2,
    AlertCircle,
    Clock,
    HardDrive,
    Database,
    Zap,
    Terminal
} from "lucide-react";
import { apiFetch } from "../lib/api";

interface Stats {
    total: number;
    completed: number;
    active: number;
    failed: number;
}

interface Job {
    id: number;
    input_path: string;
    status: string;
    created_at: string;
}

import ResourceMonitor from "./ResourceMonitor";

export default function Dashboard() {
    const [stats, setStats] = useState<Stats>({ total: 0, completed: 0, active: 0, failed: 0 });
    const [jobs, setJobs] = useState<Job[]>([]);
    const [loading, setLoading] = useState(true);

    const lastJob = jobs[0];

    useEffect(() => {
        const fetchData = async () => {
            try {
                const [statsRes, jobsRes] = await Promise.all([
                    apiFetch("/api/stats"),
                    apiFetch("/api/jobs/table")
                ]);

                if (statsRes.ok) {
                    setStats(await statsRes.json());
                }
                if (jobsRes.ok) {
                    const allJobs = await jobsRes.json();
                    // Get 5 most recent
                    setJobs(allJobs.slice(0, 5));
                }
            } catch (e) {
                console.error("Dashboard fetch error", e);
            } finally {
                setLoading(false);
            }
        };

        fetchData();
        const interval = setInterval(fetchData, 5000);
        return () => clearInterval(interval);
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

    const quickStartItems = useMemo(() => {
        const items = [];
        if (stats.total === 0) {
            items.push({
                title: "Connect Your Library",
                body: (
                    <>
                        Map your media to{" "}
                        <code className="bg-black/20 px-1.5 py-0.5 rounded font-mono text-[10px] text-helios-solar">
                            /data
                        </code>{" "}
                        and set Watch Folders in{" "}
                        <a href="/settings" className="underline hover:text-helios-ink transition-colors">
                            Settings
                        </a>
                        .
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
        if (stats.active === 0 && stats.total > 0) {
            items.push({
                title: "Queue New Work",
                body: (
                    <>
                        No active jobs right now. Add items in{" "}
                        <a href="/jobs" className="underline hover:text-helios-ink transition-colors">
                            Jobs
                        </a>{" "}
                        or drop files into your watched folders.
                    </>
                ),
                icon: Activity,
                tone: "text-emerald-500",
                bg: "bg-emerald-500/10",
            });
        }
        if (items.length === 0) {
            items.push({
                title: "Optimize Throughput",
                body: (
                    <>
                        Tune Hardware Acceleration and Thread Allocation in{" "}
                        <a href="/settings" className="underline hover:text-helios-ink transition-colors">
                            Settings
                        </a>{" "}
                        to squeeze out more FPS.
                    </>
                ),
                icon: Zap,
                tone: "text-amber-500",
                bg: "bg-amber-500/10",
            });
        }
        return items.slice(0, 3);
    }, [stats.active, stats.failed, stats.total]);

    const StatCard = ({ label, value, icon: Icon, colorClass }: any) => (
        <div className="p-5 rounded-2xl bg-helios-surface border border-helios-line/40 shadow-sm relative overflow-hidden group hover:bg-helios-surface-soft transition-colors">
            <div className={`absolute -top-2 -right-2 p-3 opacity-10 group-hover:opacity-20 transition-opacity ${colorClass}`}>
                <Icon size={64} />
            </div>
            <div className="relative z-10 flex flex-col gap-1">
                <span className="text-xs font-bold uppercase tracking-wider text-helios-slate">{label}</span>
                <span className={`text-3xl font-bold font-mono tracking-tight ${colorClass.replace("text-", "text-")}`}>{value}</span>
            </div>
        </div>
    );

    return (
        <div className="flex flex-col gap-6 flex-1 min-h-0 overflow-hidden">
            {/* Stats Grid */}
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
                <StatCard
                    label="Active Jobs"
                    value={stats.active}
                    icon={Zap}
                    colorClass="text-amber-500"
                />
                <StatCard
                    label="Completed"
                    value={stats.completed}
                    icon={CheckCircle2}
                    colorClass="text-emerald-500"
                />
                <StatCard
                    label="Failed"
                    value={stats.failed}
                    icon={AlertCircle}
                    colorClass="text-red-500"
                />
                <StatCard
                    label="Total Processed"
                    value={stats.total}
                    icon={Database}
                    colorClass="text-helios-solar"
                />
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 items-stretch">
                {/* Recent Activity */}
                <div className="lg:col-span-2 p-6 rounded-3xl bg-helios-surface border border-helios-line/40 shadow-sm flex flex-col">
                    <div className="flex items-center justify-between mb-6">
                        <h3 className="text-lg font-bold text-helios-ink flex items-center gap-2">
                            <Activity size={20} className="text-helios-solar" />
                            Recent Activity
                        </h3>
                        <a href="/jobs" className="text-xs font-bold text-helios-solar hover:underline uppercase tracking-wide">View All</a>
                    </div>

                    <div className="flex flex-col gap-3">
                        {loading && jobs.length === 0 ? (
                            <div className="text-center py-8 text-helios-slate animate-pulse">Loading activity...</div>
                        ) : jobs.length === 0 ? (
                            <div className="text-center py-8 text-helios-slate/60 italic">No recent activity found.</div>
                        ) : (
                            jobs.map(job => (
                                <div key={job.id} className="flex items-center justify-between p-3 rounded-xl bg-helios-surface-soft hover:bg-white/5 transition-colors border border-transparent hover:border-helios-line/20">
                                    <div className="flex items-center gap-3 min-w-0">
                                        <div className={`w-2 h-2 rounded-full shrink-0 ${job.status === 'Completed' ? 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.4)]' :
                                            job.status === 'Failed' ? 'bg-red-500' :
                                                job.status === 'Encoding' ? 'bg-amber-500 animate-pulse' :
                                                    'bg-helios-slate'
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
                            ))
                        )}
                    </div>
                </div>

                {/* Getting Started Tips */}
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
                                    <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                                        {body}
                                    </p>
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
