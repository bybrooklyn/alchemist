import { useEffect, useMemo, useState } from "react";
import {
    Area,
    AreaChart,
    CartesianGrid,
    ResponsiveContainer,
    Tooltip,
    XAxis,
    YAxis,
} from "recharts";
import { apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface CodecSavings {
    codec: string;
    bytes_saved: number;
}

interface DailySavings {
    date: string;
    bytes_saved: number;
}

interface SavingsSummary {
    total_input_bytes: number;
    total_output_bytes: number;
    total_bytes_saved: number;
    savings_percent: number;
    job_count: number;
    savings_by_codec: CodecSavings[];
    savings_over_time: DailySavings[];
}

const GIB = 1_073_741_824;
const TIB = 1_099_511_627_776;

function formatHeroStorage(bytes: number): string {
    if (bytes >= TIB) {
        return `${(bytes / TIB).toFixed(1)} TB`;
    }
    return `${(bytes / GIB).toFixed(1)} GB`;
}

function formatCompactStorage(bytes: number): string {
    if (bytes >= GIB) {
        return `${(bytes / GIB).toFixed(1)} GB`;
    }
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
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

    const maxCodecSavings = Math.max(
        ...summary.savings_by_codec.map((entry) => entry.bytes_saved),
        1
    );

    return (
        <div className="space-y-6">
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
                    <div className="mt-4 h-[200px]">
                        <ResponsiveContainer width="100%" height="100%">
                            <AreaChart data={chartData}>
                                <CartesianGrid
                                    stroke="rgb(var(--border-subtle) / 0.25)"
                                    vertical={false}
                                />
                                <XAxis
                                    dataKey="label"
                                    tick={{ fill: "rgb(var(--text-muted))", fontSize: 12 }}
                                    tickLine={false}
                                    axisLine={false}
                                />
                                <YAxis
                                    tick={{ fill: "rgb(var(--text-muted))", fontSize: 12 }}
                                    tickLine={false}
                                    axisLine={false}
                                    tickFormatter={(value: number) => `${value.toFixed(1)} GB`}
                                />
                                <Tooltip
                                    contentStyle={{
                                        backgroundColor: "rgb(var(--bg-panel))",
                                        border: "1px solid rgb(var(--border-subtle) / 0.4)",
                                        borderRadius: "12px",
                                        color: "rgb(var(--text-primary))",
                                    }}
                                    formatter={(value: number) => [`${value.toFixed(1)} GB`, "Saved"]}
                                    labelFormatter={(label: string) => label}
                                />
                                <Area
                                    type="monotone"
                                    dataKey="gb_saved"
                                    stroke="rgb(var(--accent-primary))"
                                    fill="rgba(var(--accent-primary), 0.2)"
                                    strokeWidth={2}
                                />
                            </AreaChart>
                        </ResponsiveContainer>
                    </div>
                )}
            </div>

            <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                <div className="text-sm font-medium text-helios-slate">Savings by codec</div>
                {summary.savings_by_codec.length === 0 ? (
                    <div className="py-8 text-center text-sm text-helios-slate">
                        No transcoding data yet — savings will appear here once jobs complete.
                    </div>
                ) : (
                    <div className="mt-4 flex flex-col gap-2">
                        {summary.savings_by_codec.map((entry) => (
                            <div
                                key={entry.codec}
                                className="grid grid-cols-[120px_minmax(0,1fr)_110px] items-center gap-3"
                            >
                                <div className="text-sm font-medium text-helios-ink">
                                    {entry.codec}
                                </div>
                                <div className="h-3 rounded bg-helios-surface-soft">
                                    <div
                                        className="h-full rounded bg-helios-solar/70"
                                        style={{
                                            width: `${Math.max(
                                                (entry.bytes_saved / maxCodecSavings) * 100,
                                                4
                                            )}%`,
                                        }}
                                    />
                                </div>
                                <div className="text-right text-sm text-helios-slate">
                                    {formatCompactStorage(entry.bytes_saved)}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}
