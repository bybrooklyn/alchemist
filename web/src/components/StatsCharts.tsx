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
    Timer,
    type LucideIcon,
} from "lucide-react";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";
import { apiJson, isApiError } from "../lib/api";

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

interface ReasonCodeCount {
    code: string;
    count: number;
    last_seen: string | null;
}

interface TopReasonCodesResponse {
    window_days: number;
    skip: ReasonCodeCount[];
    failure: ReasonCodeCount[];
}

type ReasonWindow = "24h" | "7d" | "30d";

export default function StatsCharts() {
    const [stats, setStats] = useState<AggregatedStats | null>(null);
    const [dailyStats, setDailyStats] = useState<DailyStats[]>([]);
    const [detailedStats, setDetailedStats] = useState<DetailedStats[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [reasonWindow, setReasonWindow] = useState<ReasonWindow>("7d");
    const [topReasons, setTopReasons] = useState<TopReasonCodesResponse | null>(null);

    useEffect(() => {
        void fetchAllStats();
    }, []);

    useEffect(() => {
        let cancelled = false;
        const fetchReasons = async () => {
            try {
                const data = await apiJson<TopReasonCodesResponse>(
                    `/api/stats/top-reason-codes?window=${reasonWindow}`,
                );
                if (!cancelled) {
                    setTopReasons(data);
                }
            } catch (_e) {
                if (!cancelled) {
                    setTopReasons({ window_days: 0, skip: [], failure: [] });
                }
            }
        };
        void fetchReasons();
        return () => {
            cancelled = true;
        };
    }, [reasonWindow]);

    const fetchAllStats = async () => {
        try {
            const [aggData, dailyData, detailedData] = await Promise.all([
                apiJson<AggregatedStats>("/api/stats/aggregated"),
                apiJson<DailyStats[]>("/api/stats/daily"),
                apiJson<DetailedStats[]>("/api/stats/detailed")
            ]);
            setStats(aggData);
            setDailyStats(dailyData);
            setDetailedStats(detailedData);
            setError(null);
        } catch (e) {
            setError(isApiError(e) ? e.message : "Failed to fetch statistics");
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

    interface StatCardProps {
        icon: LucideIcon;
        label: string;
        value: string;
        subtext?: string;
        colorClass: string;
    }

    const StatCard = ({ icon: Icon, label, value, subtext, colorClass }: StatCardProps) => (
        <div className="p-6 rounded-lg bg-helios-surface border border-helios-line/40 shadow-sm">
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

    interface MetricCardProps {
        icon: LucideIcon;
        label: string;
        value: string;
        colorClass: string;
    }

    const ReasonTable = ({
        title,
        rows,
        codeParam,
    }: {
        title: string;
        rows: ReasonCodeCount[];
        codeParam: "reason_code" | "failure_code";
    }) => (
        <div>
            <h4 className="text-sm font-bold text-helios-ink mb-3 uppercase tracking-wide">{title}</h4>
            {rows.length === 0 ? (
                <p className="text-sm text-helios-slate">No data in this window.</p>
            ) : (
                <table className="w-full text-sm">
                    <thead>
                        <tr className="border-b border-helios-line/40">
                            <th className="text-left py-2 px-2 text-helios-slate font-medium">Code</th>
                            <th className="text-right py-2 px-2 text-helios-slate font-medium">Count</th>
                            <th className="text-right py-2 px-2 text-helios-slate font-medium">Last Seen</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows.map((row) => (
                            <tr
                                key={row.code}
                                className="border-b border-helios-line/20 hover:bg-helios-surface-soft transition-colors"
                            >
                                <td className="py-2 px-2">
                                    <a
                                        href={`/jobs?${codeParam}=${encodeURIComponent(row.code)}`}
                                        className="font-mono text-helios-ink hover:underline"
                                    >
                                        {row.code}
                                    </a>
                                </td>
                                <td className="py-2 px-2 text-right font-mono text-helios-ink">{row.count}</td>
                                <td className="py-2 px-2 text-right text-helios-slate text-xs">
                                    {row.last_seen ? formatDate(row.last_seen) : "—"}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );

    const MetricCard = ({ icon: Icon, label, value, colorClass }: MetricCardProps) => (
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
            {error && (
                <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error text-sm">
                    {error}
                </div>
            )}
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
                <div className="p-6 rounded-lg bg-helios-surface border border-helios-line/40">
                    <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                        <BarChart3 size={20} className="text-blue-500" />
                        Jobs per Day (Last 30 Days)
                    </h3>
                    {dailyStats.length > 0 ? (
                        <ResponsiveContainer width="100%" height={192}>
                            <BarChart
                                data={dailyStats.map(d => ({
                                    label: formatDate(d.date),
                                    jobs: d.jobs_completed,
                                }))}
                                margin={{ top: 4, right: 4, left: -24, bottom: 4 }}
                            >
                                <XAxis
                                    dataKey="label"
                                    tick={{ fontSize: 10, fill: "rgb(var(--text-muted, 160 160 160))" }}
                                    interval="preserveStartEnd"
                                    tickLine={false}
                                    axisLine={false}
                                />
                                <YAxis
                                    allowDecimals={false}
                                    tick={{ fontSize: 10, fill: "rgb(var(--text-muted, 160 160 160))" }}
                                    tickLine={false}
                                    axisLine={false}
                                />
                                <Tooltip
                                    formatter={(value: number) => [value, "Jobs"]}
                                    contentStyle={{
                                        background: "rgb(var(--bg-panel, 30 30 30))",
                                        border: "1px solid rgb(var(--border, 60 60 60))",
                                        borderRadius: "6px",
                                        fontSize: "12px",
                                    }}
                                />
                                <Bar dataKey="jobs" fill="rgb(59 130 246)" radius={[3, 3, 0, 0]} />
                            </BarChart>
                        </ResponsiveContainer>
                    ) : (
                        <div className="h-48 flex items-center justify-center text-helios-slate">
                            No daily data available
                        </div>
                    )}
                </div>

                {/* Space Efficiency */}
                <div className="p-6 rounded-lg bg-helios-surface border border-helios-line/40">
                    <h3 className="text-lg font-bold text-helios-ink mb-4 flex items-center gap-2">
                        <Zap size={20} className="text-helios-solar" />
                        Space Efficiency
                    </h3>
                    <div className="relative h-10 bg-helios-surface-soft rounded-full overflow-hidden mb-4">
                        <div
                            className="absolute inset-y-0 left-0 bg-gradient-to-r from-emerald-500 to-emerald-400 rounded-full transition-all duration-1000"
                            style={{ width: `${100 - parseFloat(savingsPercent)}%` }}
                        />
                        <div className="absolute inset-0 flex items-center justify-center text-sm font-bold text-helios-main drop-shadow">
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

            {/* Top Reasons (skips + failures) */}
            <div className="p-6 rounded-lg bg-helios-surface border border-helios-line/40">
                <div className="flex items-center justify-between mb-4">
                    <h3 className="text-lg font-bold text-helios-ink flex items-center gap-2">
                        <BarChart3 size={20} className="text-amber-500" />
                        Top Reasons
                    </h3>
                    <div className="inline-flex rounded-lg border border-helios-line/40 overflow-hidden text-xs">
                        {(["24h", "7d", "30d"] as ReasonWindow[]).map((option) => (
                            <button
                                key={option}
                                type="button"
                                onClick={() => setReasonWindow(option)}
                                className={
                                    "px-3 py-1.5 font-medium transition-colors " +
                                    (reasonWindow === option
                                        ? "bg-helios-surface-soft text-helios-ink"
                                        : "text-helios-slate hover:text-helios-ink")
                                }
                            >
                                {option}
                            </button>
                        ))}
                    </div>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <ReasonTable
                        title="Top Skip Reasons"
                        rows={topReasons?.skip ?? []}
                        codeParam="reason_code"
                    />
                    <ReasonTable
                        title="Top Failure Reasons"
                        rows={topReasons?.failure ?? []}
                        codeParam="failure_code"
                    />
                </div>
            </div>

            {/* Recent Jobs Table */}
            {detailedStats.length > 0 && (
                <div className="p-6 rounded-lg bg-helios-surface border border-helios-line/40">
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
                                    const savedPercent = job.input_size_bytes > 0
                                        ? ((saved / job.input_size_bytes) * 100).toFixed(1)
                                        : "0.0";
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
