import { useEffect, useState } from "react";
import {
    Activity,
    CheckCircle2,
    AlertCircle,
    Clock,
    HardDrive,
    Database,
    Zap
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

export default function Dashboard() {
    const [stats, setStats] = useState<Stats>({ total: 0, completed: 0, active: 0, failed: 0 });
    const [jobs, setJobs] = useState<Job[]>([]);
    const [loading, setLoading] = useState(true);

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

    const StatCard = ({ label, value, icon: Icon, colorClass }: any) => (
        <div className="p-5 rounded-2xl bg-helios-surface border border-helios-line/40 shadow-sm relative overflow-hidden group hover:bg-helios-surface-soft transition-colors">
            <div className={`absolute top-0 right-0 p-3 opacity-10 group-hover:opacity-20 transition-opacity ${colorClass}`}>
                <Icon size={64} />
            </div>
            <div className="relative z-10 flex flex-col gap-1">
                <span className="text-xs font-bold uppercase tracking-wider text-helios-slate">{label}</span>
                <span className={`text-3xl font-bold font-mono tracking-tight ${colorClass.replace("text-", "text-")}`}>{value}</span>
            </div>
        </div>
    );

    return (
        <div className="flex flex-col gap-6">
            {/* Stats Grid */}
            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
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

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* Recent Activity */}
                <div className="lg:col-span-2 p-6 rounded-3xl bg-helios-surface border border-helios-line/40 shadow-sm">
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
                                            <span className="text-[10px] text-helios-slate uppercase tracking-wide font-bold">{job.status}</span>
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
                <div className="p-6 rounded-3xl bg-gradient-to-br from-helios-surface to-helios-surface-soft border border-helios-line/40 shadow-sm">
                    <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                        <Clock size={20} className="text-helios-slate" />
                        Quick Tips
                    </h3>
                    <div className="flex flex-col gap-4">
                        <div className="flex gap-3 items-start">
                            <div className="p-2 rounded-lg bg-helios-solar/10 text-helios-solar mt-0.5">
                                <HardDrive size={16} />
                            </div>
                            <div>
                                <h4 className="text-sm font-bold text-helios-ink">Add Media</h4>
                                <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                                    Mount your media volume to <code className="bg-black/10 px-1 py-0.5 rounded font-mono text-[10px]">/data</code> in Docker. Alchemist watches configured folders automatically.
                                </p>
                            </div>
                        </div>

                        <div className="flex gap-3 items-start">
                            <div className="p-2 rounded-lg bg-emerald-500/10 text-emerald-500 mt-0.5">
                                <Zap size={16} />
                            </div>
                            <div>
                                <h4 className="text-sm font-bold text-helios-ink">Performance</h4>
                                <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                                    Toggle <strong>Hardware Acceleration</strong> in Settings if you have a supported GPU (NVIDIA/Intel) for 10x speeds.
                                </p>
                            </div>
                        </div>

                        <div className="flex gap-3 items-start">
                            <div className="p-2 rounded-lg bg-purple-500/10 text-purple-500 mt-0.5">
                                <Activity size={16} />
                            </div>
                            <div>
                                <h4 className="text-sm font-bold text-helios-ink">Monitor Logs</h4>
                                <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                                    Check the <strong>Logs</strong> page for detailed real-time insights into the transcoding pipeline.
                                </p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
