import { useEffect, useState } from "react";
import { AlertTriangle, Copy, Sparkles } from "lucide-react";
import { apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface DuplicatePath {
    id: number;
    path: string;
    status: string;
}

interface DuplicateGroup {
    stem: string;
    count: number;
    paths: DuplicatePath[];
}

interface RecommendationCounts {
    duplicates: number;
    remux_only_candidate: number;
    wasteful_audio_layout: number;
    commentary_cleanup_candidate: number;
}

interface IntelligenceRecommendation {
    type: string;
    title: string;
    summary: string;
    path: string;
    suggested_action: string;
}

interface IntelligenceResponse {
    duplicate_groups: DuplicateGroup[];
    total_duplicates: number;
    recommendation_counts: RecommendationCounts;
    recommendations: IntelligenceRecommendation[];
}

const STATUS_DOT: Record<string, string> = {
    analyzing: "bg-helios-cyan animate-pulse",
    completed: "bg-status-success",
    failed: "bg-status-error",
    remuxing: "bg-helios-solar animate-pulse",
    resuming: "bg-helios-solar animate-pulse",
    skipped: "bg-helios-slate/40",
    encoding: "bg-helios-solar animate-pulse",
    queued: "bg-helios-slate/30",
};

const TYPE_LABELS: Record<string, string> = {
    remux_only_candidate: "Remux Opportunities",
    wasteful_audio_layout: "Wasteful Audio Layouts",
    commentary_cleanup_candidate: "Commentary Cleanup",
};

export default function LibraryIntelligence() {
    const [data, setData] = useState<IntelligenceResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        const fetch = async () => {
            try {
                const result = await apiJson<IntelligenceResponse>("/api/library/intelligence");
                setData(result);
            } catch (e) {
                const message = isApiError(e) ? e.message : "Failed to load intelligence data.";
                setError(message);
                showToast({
                    kind: "error",
                    title: "Intelligence",
                    message,
                });
            } finally {
                setLoading(false);
            }
        };

        void fetch();
    }, []);

    const groupedRecommendations = data?.recommendations.reduce<Record<string, IntelligenceRecommendation[]>>(
        (groups, recommendation) => {
            groups[recommendation.type] ??= [];
            groups[recommendation.type].push(recommendation);
            return groups;
        },
        {},
    ) ?? {};

    return (
        <div className="flex flex-col gap-6">
            <div>
                <h1 className="text-xl font-bold text-helios-ink">Library Intelligence</h1>
                <p className="mt-1 text-sm text-helios-slate">
                    Deterministic storage-focused recommendations based on duplicate detection, planner output, and stream metadata.
                </p>
            </div>

            {loading && (
                <div className="animate-pulse rounded-lg border border-helios-line/30 bg-helios-surface p-8 text-center text-sm text-helios-slate">
                    Scanning library...
                </div>
            )}

            {error && (
                <div className="rounded-lg border border-status-error/20 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {error}
                </div>
            )}

            {data && (
                <>
                    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
                        <StatCard label="Duplicate groups" value={String(data.duplicate_groups.length)} accent="text-helios-ink" />
                        <StatCard label="Extra copies" value={String(data.total_duplicates)} accent="text-helios-solar" />
                        <StatCard label="Remux opportunities" value={String(data.recommendation_counts.remux_only_candidate)} accent="text-helios-cyan" />
                        <StatCard label="Audio / commentary" value={String(data.recommendation_counts.wasteful_audio_layout + data.recommendation_counts.commentary_cleanup_candidate)} accent="text-helios-ink" />
                    </div>

                    {Object.keys(groupedRecommendations).length > 0 && (
                        <div className="space-y-4">
                            {Object.entries(groupedRecommendations).map(([type, recommendations]) => (
                                <section key={type} className="rounded-lg border border-helios-line/30 bg-helios-surface overflow-hidden">
                                    <div className="flex items-center gap-2 border-b border-helios-line/20 bg-helios-surface-soft/40 px-5 py-3">
                                        <Sparkles size={14} className="text-helios-solar" />
                                        <h2 className="text-sm font-semibold text-helios-ink">
                                            {TYPE_LABELS[type] ?? type}
                                        </h2>
                                    </div>
                                    <div className="divide-y divide-helios-line/10">
                                        {recommendations.map((recommendation, index) => (
                                            <div key={`${recommendation.path}-${index}`} className="px-5 py-4">
                                                <div className="flex items-center justify-between gap-4">
                                                    <div>
                                                        <h3 className="text-sm font-semibold text-helios-ink">{recommendation.title}</h3>
                                                        <p className="mt-1 text-sm text-helios-slate">{recommendation.summary}</p>
                                                    </div>
                                                </div>
                                                <p className="mt-3 break-all font-mono text-xs text-helios-slate">{recommendation.path}</p>
                                                <div className="mt-3 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-3 py-2 text-xs text-helios-ink">
                                                    <span className="font-semibold text-helios-solar">Suggested action:</span> {recommendation.suggested_action}
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                </section>
                            ))}
                        </div>
                    )}

                    {data.duplicate_groups.length === 0 ? (
                        <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-helios-line/30 bg-helios-surface p-10 text-center">
                            <AlertTriangle size={28} className="text-helios-slate/40" />
                            <p className="text-sm font-medium text-helios-ink">
                                No duplicate groups found
                            </p>
                            <p className="max-w-xs text-xs text-helios-slate">
                                Every tracked basename in your library appears to be unique.
                            </p>
                        </div>
                    ) : (
                        <div className="flex flex-col gap-3">
                            {data.duplicate_groups.map((group) => (
                                <div
                                    key={group.stem}
                                    className="overflow-hidden rounded-lg border border-helios-line/30 bg-helios-surface"
                                >
                                    <div className="flex items-center justify-between border-b border-helios-line/20 bg-helios-surface-soft/40 px-5 py-3">
                                        <div className="flex min-w-0 items-center gap-3">
                                            <Copy
                                                size={14}
                                                className="shrink-0 text-helios-solar"
                                            />
                                            <span className="truncate font-mono text-sm font-semibold text-helios-ink">
                                                {group.stem}
                                            </span>
                                        </div>
                                        <span className="ml-4 shrink-0 font-mono text-xs font-bold text-helios-solar">
                                            {group.count}x
                                        </span>
                                    </div>

                                    <div className="divide-y divide-helios-line/10">
                                        {group.paths.map((path) => (
                                            <div
                                                key={path.id}
                                                className="flex items-center gap-3 px-5 py-3"
                                            >
                                                <div
                                                    className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                                                        STATUS_DOT[path.status] ??
                                                        "bg-helios-slate/30"
                                                    }`}
                                                />
                                                <span className="break-all font-mono text-xs text-helios-slate">
                                                    {path.path}
                                                </span>
                                                <span className="ml-auto shrink-0 text-xs capitalize text-helios-slate/50">
                                                    {path.status}
                                                </span>
                                            </div>
                                        ))}
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </>
            )}
        </div>
    );
}

function StatCard({
    label,
    value,
    accent,
}: {
    label: string;
    value: string;
    accent: string;
}) {
    return (
        <div className="rounded-lg border border-helios-line/30 bg-helios-surface px-5 py-4">
            <p className="text-xs font-medium text-helios-slate">{label}</p>
            <p className={`mt-1 font-mono text-2xl font-bold ${accent}`}>{value}</p>
        </div>
    );
}
