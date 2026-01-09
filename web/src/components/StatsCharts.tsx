import { useEffect, useState } from "react";
import {
    TrendingDown,
    Clock,
    HardDrive,
    Zap,
    BarChart3,
    Activity
} from "lucide-react";

interface AggregatedStats {
    total_input_bytes: number;
    total_output_bytes: number;
    total_savings_bytes: number;
    total_time_seconds: number;
    total_jobs: number;
    avg_vmaf: number;
}

interface DailyStats {
    date: string;
    jobs: number;
    savings_mb: number;
}

export default function StatsCharts() {
    const [stats, setStats] = useState<AggregatedStats | null>(null);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        fetchStats();
    }, []);

    const fetchStats = async () => {
        try {
            const res = await fetch("/api/stats/aggregated");
            if (res.ok) {
                setStats(await res.json());
            }
        } catch (e) {
            console.error("Failed to fetch stats", e);
        } finally {
            setLoading(false);
        }
    };

    const formatBytes = (bytes: number) => {
        if (bytes === 0) return "0 B";
        const k = 1024;
        const sizes = ["B", "KB", "MB", "GB", "TB"];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
    };

    const formatTime = (seconds: number) => {
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        if (hours > 0) {
            return `${hours}h ${minutes}m`;
        }
        return `${minutes}m`;
    };

    if (loading) {
        return (
            <div className="flex items-center justify-center py-20">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-helios-solar"></div>
            </div>
        );
    }

    if (!stats) {
        return (
            <div className="text-center py-20 text-helios-slate">
                <BarChart3 size={48} className="mx-auto mb-4 opacity-50" />
                <p>No statistics available yet.</p>
                <p className="text-sm mt-2">Complete some transcoding jobs to see data here.</p>
            </div>
        );
    }

    const savingsPercent = stats.total_input_bytes > 0
        ? ((stats.total_savings_bytes / stats.total_input_bytes) * 100).toFixed(1)
        : "0";

    const StatCard = ({ icon: Icon, label, value, subtext, colorClass }: any) => (
        <div className="p-6 rounded-2xl bg-helios-surface border border-helios-line/40 shadow-sm">
            <div className="flex items-start justify-between">
                <div>
                    <p className="text-sm font-medium text-helios-slate uppercase tracking-wide mb-1">{label}</p>
                    <p className={`text-3xl font-bold ${colorClass}`}>{value}</p>
                    {subtext && <p className="text-sm text-helios-slate mt-1">{subtext}</p>}
                </div>
                <div className={`p-3 rounded-xl ${colorClass} bg-opacity-10`}>
                    <Icon size={24} className={colorClass} />
                </div>
            </div>
        </div>
    );

    return (
        <div className="space-y-6">
            {/* Main Stats Grid */}
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <StatCard
                    icon={TrendingDown}
                    label="Space Saved"
                    value={formatBytes(stats.total_savings_bytes)}
                    subtext={`${savingsPercent}% reduction`}
                    colorClass="text-emerald-500"
                />
                <StatCard
                    icon={HardDrive}
                    label="Total Processed"
                    value={formatBytes(stats.total_input_bytes)}
                    subtext={`Output: ${formatBytes(stats.total_output_bytes)}`}
                    colorClass="text-blue-500"
                />
                <StatCard
                    icon={Clock}
                    label="Encoding Time"
                    value={formatTime(stats.total_time_seconds)}
                    subtext={`${stats.total_jobs} jobs completed`}
                    colorClass="text-amber-500"
                />
                <StatCard
                    icon={Activity}
                    label="Avg VMAF Score"
                    value={stats.avg_vmaf > 0 ? stats.avg_vmaf.toFixed(1) : "N/A"}
                    subtext={stats.avg_vmaf > 90 ? "Excellent quality" : stats.avg_vmaf > 80 ? "Good quality" : ""}
                    colorClass="text-purple-500"
                />
            </div>

            {/* Visual Bar */}
            <div className="p-6 rounded-2xl bg-helios-surface border border-helios-line/40">
                <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                    <Zap size={20} className="text-helios-solar" />
                    Space Efficiency
                </h3>
                <div className="relative h-8 bg-helios-surface-soft rounded-full overflow-hidden">
                    <div
                        className="absolute inset-y-0 left-0 bg-gradient-to-r from-emerald-500 to-emerald-400 rounded-full transition-all duration-1000"
                        style={{ width: `${100 - parseFloat(savingsPercent)}%` }}
                    />
                    <div className="absolute inset-0 flex items-center justify-center text-sm font-bold text-white drop-shadow">
                        {formatBytes(stats.total_output_bytes)} / {formatBytes(stats.total_input_bytes)}
                    </div>
                </div>
                <div className="flex justify-between text-sm text-helios-slate mt-2">
                    <span>Current Size</span>
                    <span>Original Size</span>
                </div>
            </div>
        </div>
    );
}
