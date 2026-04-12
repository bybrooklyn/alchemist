import { useEffect, useMemo, useState } from "react";
import {
    AreaChart,
    Area,
    XAxis,
    YAxis,
    Tooltip,
    ResponsiveContainer,
    BarChart,
    Bar,
    Cell,
} from "recharts";
import { apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface CodecSavings {
    bytes_saved: number;
    codec: string;
    job_count: number;
}

interface DailySavings {
    bytes_saved: number;
    date: string;
}

interface SavingsSummary {
    job_count: number;
    savings_by_codec: CodecSavings[];
    savings_over_time: DailySavings[];
    savings_percent: number;
    total_bytes_saved: number;
    total_input_bytes: number;
    total_output_bytes: number;
}

const GIB = 1_073_741_824;
const TIB = 1_099_511_627_776;

function formatHeroStorage(bytes: number): string {
    if (bytes >= TIB) {
        return `${(bytes / TIB).toFixed(1)} TB`;
    }
    return `${(bytes / GIB).toFixed(1)} GB`;
}


function formatChartDate(date: string): string {
    const parsed = new Date(date);
    if (Number.isNaN(parsed.getTime())) {
        return date;
    }
    return parsed.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export default function SavingsOverview() {
    const [summary, setSummary] = useState<SavingsSummary | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        const fetchSummary = async () => {
            try {
                const data = await apiJson<SavingsSummary>("/api/stats/savings");
                setSummary(data);
                setError(null);
            } catch (err) {
                const message = isApiError(err) ? err.message : "Failed to load storage savings.";
                setError(message);
                showToast({ kind: "error", title: "Savings", message });
            } finally {
                setLoading(false);
            }
        };

        void fetchSummary();
    }, []);

    const chartData = useMemo(
        () =>
            (summary?.savings_over_time ?? []).map((entry) => ({
                date: entry.date,
                label: formatChartDate(entry.date),
                gb_saved: Number((entry.bytes_saved / GIB).toFixed(1)),
            })),
        [summary]
    );

    if (loading) {
        return (
            <div className="space-y-4">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                    <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                        <div className="h-4 w-28 animate-pulse rounded bg-helios-surface-soft/60" />
                        <div className="mt-4 h-10 w-40 animate-pulse rounded bg-helios-surface-soft/60" />
                        <div className="mt-3 h-3 w-32 animate-pulse rounded bg-helios-surface-soft/60" />
                    </div>
                    <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                        <div className="h-4 w-28 animate-pulse rounded bg-helios-surface-soft/60" />
                        <div className="mt-4 h-10 w-40 animate-pulse rounded bg-helios-surface-soft/60" />
                        <div className="mt-3 h-3 w-32 animate-pulse rounded bg-helios-surface-soft/60" />
                    </div>
                </div>
                <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                    <div className="h-4 w-40 animate-pulse rounded bg-helios-surface-soft/60" />
                    <div className="mt-4 h-[200px] animate-pulse rounded bg-helios-surface-soft/40" />
                </div>
            </div>
        );
    }

    if (error || !summary) {
        return (
            <div className="rounded-lg border border-status-error/30 bg-status-error/10 px-4 py-6 text-center text-sm text-status-error">
                {error ?? "Unable to load storage savings."}
            </div>
        );
    }

    if (summary.job_count === 0) {
        return (
            <div className="rounded-lg border border-helios-line/40 bg-helios-surface px-6 py-8 text-center text-sm text-helios-slate">
                No transcoding data yet — savings will appear here once jobs complete.
            </div>
        );
    }

    const codecChartData = (summary?.savings_by_codec ?? []).map((entry) => ({
        codec: entry.codec.toUpperCase(),
        gb_saved: Number((entry.bytes_saved / GIB).toFixed(2)),
        job_count: entry.job_count,
    }));

    return (
        <div className="space-y-6">
            {/* Total Library Reduction */}
            <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                <div className="text-sm font-medium text-helios-slate mb-4">Total Library Reduction</div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <div className="text-xs text-helios-slate/70">Original size</div>
                        <div className="mt-1 font-mono text-2xl font-bold text-helios-ink">
                            {formatHeroStorage(summary.total_input_bytes)}
                        </div>
                    </div>
                    <div>
                        <div className="text-xs text-helios-slate/70">Current size</div>
                        <div className="mt-1 font-mono text-2xl font-bold text-helios-ink">
                            {formatHeroStorage(summary.total_output_bytes)}
                        </div>
                    </div>
                    <div>
                        <div className="text-xs text-helios-slate/70">Space recovered</div>
                        <div className="mt-1 font-mono text-2xl font-bold text-helios-solar">
                            {formatHeroStorage(summary.total_bytes_saved)}
                            <span className="ml-2 text-base font-semibold text-helios-slate">
                                ({summary.savings_percent.toFixed(1)}%)
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                    <div className="text-sm font-medium text-helios-slate">Total saved</div>
                    <div className="mt-3 font-mono text-4xl font-bold text-helios-solar">
                        {formatHeroStorage(summary.total_bytes_saved)}
                    </div>
                    <div className="mt-2 text-sm text-helios-slate">
                        saved across {summary.job_count} files
                    </div>
                </div>
                <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                    <div className="text-sm font-medium text-helios-slate">Average reduction</div>
                    <div className="mt-3 font-mono text-4xl font-bold text-helios-solar">
                        {summary.savings_percent.toFixed(1)}%
                    </div>
                    <div className="mt-2 text-sm text-helios-slate">smaller on average</div>
                </div>
            </div>

            <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                <div className="text-sm font-medium text-helios-slate">
                    Savings over the last 30 days
                </div>
                {chartData.length === 0 ? (
                    <div className="py-10 text-center text-sm text-helios-slate">No data yet</div>
                ) : (
                    <div className="mt-4">
                        <ResponsiveContainer width="100%" height={220}>
                            <AreaChart
                                data={chartData}
                                margin={{ top: 8, right: 8, left: 0, bottom: 0 }}
                            >
                                <defs>
                                    <linearGradient
                                        id="savingsGradient"
                                        x1="0" y1="0" x2="0" y2="1"
                                    >
                                        <stop
                                            offset="5%"
                                            stopColor="rgb(var(--accent-primary))"
                                            stopOpacity={0.3}
                                        />
                                        <stop
                                            offset="95%"
                                            stopColor="rgb(var(--accent-primary))"
                                            stopOpacity={0}
                                        />
                                    </linearGradient>
                                </defs>
                                <XAxis
                                    dataKey="label"
                                    tick={{ fontSize: 11, fill: "rgb(var(--text-muted))" }}
                                    tickLine={false}
                                    axisLine={false}
                                    interval="preserveStartEnd"
                                />
                                <YAxis
                                    tick={{ fontSize: 11, fill: "rgb(var(--text-muted))" }}
                                    tickLine={false}
                                    axisLine={false}
                                    tickFormatter={(v: number) =>
                                        v >= 1 ? `${v.toFixed(1)}GB` : `${(v * 1024).toFixed(0)}MB`
                                    }
                                    width={52}
                                />
                                <Tooltip
                                    formatter={(value: number) => [
                                        value >= 1
                                            ? `${value.toFixed(2)} GB`
                                            : `${(value * 1024).toFixed(0)} MB`,
                                        "Saved",
                                    ]}
                                    labelStyle={{
                                        color: "rgb(var(--text-primary))",
                                        fontSize: 12,
                                    }}
                                    contentStyle={{
                                        background: "rgb(var(--bg-panel))",
                                        border: "1px solid rgb(var(--border-subtle))",
                                        borderRadius: 8,
                                        fontSize: 12,
                                    }}
                                />
                                <Area
                                    type="monotone"
                                    dataKey="gb_saved"
                                    stroke="rgb(var(--accent-primary))"
                                    strokeWidth={2}
                                    fill="url(#savingsGradient)"
                                    dot={false}
                                    activeDot={{ r: 4 }}
                                />
                            </AreaChart>
                        </ResponsiveContainer>
                    </div>
                )}
            </div>

            <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                <div className="text-sm font-medium text-helios-slate">Savings by codec</div>
                {codecChartData.length === 0 ? (
                    <div className="py-8 text-center text-sm text-helios-slate">
                        No transcoding data yet — savings will appear here once jobs complete.
                    </div>
                ) : (
                    <div className="mt-4">
                        <ResponsiveContainer width="100%" height={180}>
                            <BarChart
                                data={codecChartData}
                                margin={{ top: 8, right: 8, left: 0, bottom: 0 }}
                            >
                                <XAxis
                                    dataKey="codec"
                                    tick={{
                                        fontSize: 12,
                                        fill: "rgb(var(--text-primary))",
                                        fontWeight: 600,
                                    }}
                                    tickLine={false}
                                    axisLine={false}
                                />
                                <YAxis
                                    tick={{
                                        fontSize: 11,
                                        fill: "rgb(var(--text-muted))",
                                    }}
                                    tickLine={false}
                                    axisLine={false}
                                    tickFormatter={(v: number) =>
                                        v >= 1
                                            ? `${v.toFixed(1)}GB`
                                            : `${(v * 1024).toFixed(0)}MB`
                                    }
                                    width={52}
                                />
                                <Tooltip
                                    formatter={(value: number, _: string, props: {
                                        payload?: { job_count?: number };
                                    }) => [
                                        value >= 1
                                            ? `${value.toFixed(2)} GB`
                                            : `${(value * 1024).toFixed(0)} MB`,
                                        `Saved (${props.payload?.job_count ?? 0} jobs)`,
                                    ]}
                                    contentStyle={{
                                        background: "rgb(var(--bg-panel))",
                                        border: "1px solid rgb(var(--border-subtle))",
                                        borderRadius: 8,
                                        fontSize: 12,
                                    }}
                                />
                                <Bar
                                    dataKey="gb_saved"
                                    radius={[4, 4, 0, 0]}
                                    maxBarSize={80}
                                >
                                    {codecChartData.map((_, index) => (
                                        <Cell
                                            key={index}
                                            fill="rgb(var(--accent-primary))"
                                            fillOpacity={0.8}
                                        />
                                    ))}
                                </Bar>
                            </BarChart>
                        </ResponsiveContainer>
                    </div>
                )}
            </div>
        </div>
    );
}
