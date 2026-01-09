import { useEffect, useState } from "react";
import {
    TrendingDown,
    Clock,
    HardDrive,
    Zap,
    BarChart3,
    Activity,
    Gauge,
    FileVideo,
    Timer
} from "lucide-react";
import { apiFetch } from "../lib/api";

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
    jobs_completed: number;
    bytes_saved: number;
    total_input_bytes: number;
    total_output_bytes: number;
}

interface DetailedStats {
    job_id: number;
    input_path: string;
    input_size_bytes: number;
    output_size_bytes: number;
    compression_ratio: number;
    encode_time_seconds: number;
    encode_speed: number;
    avg_bitrate_kbps: number;
    vmaf_score: number | null;
    created_at: string;
}

export default function StatsCharts() {
    const [stats, setStats] = useState<AggregatedStats | null>(null);
    const [dailyStats, setDailyStats] = useState<DailyStats[]>([]);
    const [detailedStats, setDetailedStats] = useState<DetailedStats[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        fetchAllStats();
    }, []);

    const fetchAllStats = async () => {
        try {
            const [aggRes, dailyRes, detailedRes] = await Promise.all([
                apiFetch("/api/stats/aggregated"),
                apiFetch("/api/stats/daily"),
                apiFetch("/api/stats/detailed")
            ]);

            if (aggRes.ok) setStats(await aggRes.json());
            if (dailyRes.ok) setDailyStats(await dailyRes.json());
            if (detailedRes.ok) setDetailedStats(await detailedRes.json());
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
        const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(k));
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

    const formatDate = (dateStr: string) => {
        const date = new Date(dateStr);
        return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    };

    if (loading) {
        return (
            <div className="flex items-center justify-center py-20">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-helios-solar"></div>
            </div>
        );
    }

    if (!stats || stats.total_jobs === 0) {
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

    // Calculate averages from detailed stats
    const avgCompression = detailedStats.length > 0
        ? (detailedStats.reduce((sum, s) => sum + s.compression_ratio, 0) / detailedStats.length).toFixed(2)
        : "N/A";
    const avgSpeed = detailedStats.length > 0
        ? (detailedStats.reduce((sum, s) => sum + s.encode_speed, 0) / detailedStats.length).toFixed(1)
        : "N/A";
    const avgBitrate = detailedStats.length > 0
        ? (detailedStats.reduce((sum, s) => sum + s.avg_bitrate_kbps, 0) / detailedStats.length).toFixed(0)
        : "N/A";

    // Find max for bar chart scaling
    const maxDailyJobs = Math.max(...dailyStats.map(d => d.jobs_completed), 1);

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

    const MetricCard = ({ icon: Icon, label, value, colorClass }: any) => (
        <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/20 flex items-center gap-3">
            <div className={`p-2 rounded-lg ${colorClass} bg-opacity-10`}>
                <Icon size={18} className={colorClass} />
            </div>
            <div>
                <p className="text-xs text-helios-slate uppercase tracking-wide">{label}</p>
                <p className={`text-lg font-bold ${colorClass}`}>{value}</p>
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

            {/* Charts Row */}
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* Daily Activity Chart */}
                <div className="p-6 rounded-2xl bg-helios-surface border border-helios-line/40">
                    <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                        <BarChart3 size={20} className="text-blue-500" />
                        Daily Activity (Last 30 Days)
                    </h3>
                    {dailyStats.length > 0 ? (
                        <div className="h-48 flex items-end gap-1">
                            {dailyStats.map((day, i) => {
                                const height = (day.jobs_completed / maxDailyJobs) * 100;
                                return (
                                    <div
                                        key={i}
                                        className="flex-1 flex flex-col items-center group"
                                    >
                                        <div
                                            className="w-full bg-gradient-to-t from-blue-500 to-blue-400 rounded-t transition-all hover:from-blue-600 hover:to-blue-500 cursor-pointer relative"
                                            style={{ height: `${Math.max(height, 4)}%` }}
                                            title={`${formatDate(day.date)}: ${day.jobs_completed} jobs, ${formatBytes(day.bytes_saved)} saved`}
                                        >
                                            <div className="absolute -top-8 left-1/2 -translate-x-1/2 bg-helios-ink text-white text-xs px-2 py-1 rounded opacity-0 group-hover:opacity-100 whitespace-nowrap pointer-events-none z-10">
                                                {day.jobs_completed} jobs
                                            </div>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    ) : (
                        <div className="h-48 flex items-center justify-center text-helios-slate">
                            No daily data available
                        </div>
                    )}
                    {dailyStats.length > 0 && (
                        <div className="flex justify-between text-xs text-helios-slate mt-2">
                            <span>{formatDate(dailyStats[0]?.date)}</span>
                            <span>{formatDate(dailyStats[dailyStats.length - 1]?.date)}</span>
                        </div>
                    )}
                </div>

                {/* Space Efficiency */}
                <div className="p-6 rounded-2xl bg-helios-surface border border-helios-line/40">
                    <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                        <Zap size={20} className="text-helios-solar" />
                        Space Efficiency
                    </h3>
                    <div className="relative h-10 bg-helios-surface-soft rounded-full overflow-hidden mb-4">
                        <div
                            className="absolute inset-y-0 left-0 bg-gradient-to-r from-emerald-500 to-emerald-400 rounded-full transition-all duration-1000"
                            style={{ width: `${100 - parseFloat(savingsPercent)}%` }}
                        />
                        <div className="absolute inset-0 flex items-center justify-center text-sm font-bold text-white drop-shadow">
                            {formatBytes(stats.total_output_bytes)} / {formatBytes(stats.total_input_bytes)}
                        </div>
                    </div>
                    <div className="flex justify-between text-sm text-helios-slate mb-6">
                        <span>Current Size</span>
                        <span>Original Size</span>
                    </div>

                    {/* Performance Metrics */}
                    <h4 className="text-sm font-bold text-helios-ink mb-3 uppercase tracking-wide">Performance Metrics</h4>
                    <div className="grid grid-cols-3 gap-3">
                        <MetricCard icon={Gauge} label="Avg Ratio" value={`${avgCompression}x`} colorClass="text-cyan-500" />
                        <MetricCard icon={Timer} label="Avg Speed" value={`${avgSpeed} fps`} colorClass="text-orange-500" />
                        <MetricCard icon={FileVideo} label="Avg Bitrate" value={`${avgBitrate} kbps`} colorClass="text-pink-500" />
                    </div>
                </div>
            </div>

            {/* Recent Jobs Table */}
            {detailedStats.length > 0 && (
                <div className="p-6 rounded-2xl bg-helios-surface border border-helios-line/40">
                    <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                        <FileVideo size={20} className="text-amber-500" />
                        Recent Completed Jobs
                    </h3>
                    <div className="overflow-x-auto">
                        <table className="w-full text-sm">
                            <thead>
                                <tr className="border-b border-helios-line/40">
                                    <th className="text-left py-3 px-2 text-helios-slate font-medium">File</th>
                                    <th className="text-right py-3 px-2 text-helios-slate font-medium">Input</th>
                                    <th className="text-right py-3 px-2 text-helios-slate font-medium">Output</th>
                                    <th className="text-right py-3 px-2 text-helios-slate font-medium">Saved</th>
                                    <th className="text-right py-3 px-2 text-helios-slate font-medium">Ratio</th>
                                    <th className="text-right py-3 px-2 text-helios-slate font-medium">VMAF</th>
                                    <th className="text-right py-3 px-2 text-helios-slate font-medium">Time</th>
                                </tr>
                            </thead>
                            <tbody>
                                {detailedStats.slice(0, 10).map((job) => {
                                    const saved = job.input_size_bytes - job.output_size_bytes;
                                    const savedPercent = ((saved / job.input_size_bytes) * 100).toFixed(1);
                                    const filename = job.input_path.split(/[/\\]/).pop() || job.input_path;
                                    return (
                                        <tr key={job.job_id} className="border-b border-helios-line/20 hover:bg-helios-surface-soft transition-colors">
                                            <td className="py-3 px-2 truncate max-w-[200px]" title={job.input_path}>
                                                <span className="font-medium text-helios-ink">{filename}</span>
                                            </td>
                                            <td className="py-3 px-2 text-right text-helios-slate font-mono text-xs">
                                                {formatBytes(job.input_size_bytes)}
                                            </td>
                                            <td className="py-3 px-2 text-right text-helios-slate font-mono text-xs">
                                                {formatBytes(job.output_size_bytes)}
                                            </td>
                                            <td className="py-3 px-2 text-right">
                                                <span className="text-emerald-500 font-bold text-xs">{savedPercent}%</span>
                                            </td>
                                            <td className="py-3 px-2 text-right text-helios-slate font-mono text-xs">
                                                {job.compression_ratio.toFixed(2)}x
                                            </td>
                                            <td className="py-3 px-2 text-right">
                                                <span className={`font-bold text-xs ${job.vmaf_score && job.vmaf_score > 90 ? 'text-emerald-500' :
                                                    job.vmaf_score && job.vmaf_score > 80 ? 'text-amber-500' : 'text-helios-slate'
                                                    }`}>
                                                    {job.vmaf_score ? job.vmaf_score.toFixed(1) : 'N/A'}
                                                </span>
                                            </td>
                                            <td className="py-3 px-2 text-right text-helios-slate font-mono text-xs">
                                                {formatTime(job.encode_time_seconds)}
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    </div>
                </div>
            )}
        </div>
    );
}
