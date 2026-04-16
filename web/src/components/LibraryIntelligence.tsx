import { useCallback, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { AlertTriangle, Copy, Sparkles, Zap, Search } from "lucide-react";
import { apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";
import { JobDetailModal } from "./jobs/JobDetailModal";
import { getStatusBadge } from "./jobs/jobStatusBadge";
import { useJobDetailController } from "./jobs/useJobDetailController";

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
    const [queueingRemux, setQueueingRemux] = useState(false);

    const fetchIntelligence = useCallback(async () => {
        try {
            const result = await apiJson<IntelligenceResponse>("/api/library/intelligence");
            setData(result);
            setError(null);
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
    }, []);

    const {
        focusedJob,
        detailLoading,
        confirmState,
        detailDialogRef,
        openJobDetails,
        handleAction,
        handlePriority,
        openConfirm,
        setConfirmState,
        closeJobDetails,
        focusedDecision,
        focusedFailure,
        focusedJobLogs,
        shouldShowFfmpegOutput,
        completedEncodeStats,
        focusedEmptyState,
    } = useJobDetailController({
        onRefresh: fetchIntelligence,
    });

    useEffect(() => {
        void fetchIntelligence();
    }, [fetchIntelligence]);

    const groupedRecommendations = useMemo(
        () => data?.recommendations.reduce<Record<string, IntelligenceRecommendation[]>>(
            (groups, recommendation) => {
                groups[recommendation.type] ??= [];
                groups[recommendation.type].push(recommendation);
                return groups;
            },
            {},
        ) ?? {},
        [data],
    );

    const handleQueueAllRemux = async () => {
        const remuxPaths = groupedRecommendations.remux_only_candidate ?? [];
        if (remuxPaths.length === 0) {
            return;
        }

        setQueueingRemux(true);
        let enqueued = 0;
        let skipped = 0;
        let failed = 0;

        for (const recommendation of remuxPaths) {
            try {
                const result = await apiJson<{ enqueued: boolean; message: string }>("/api/jobs/enqueue", {
                    method: "POST",
                    body: JSON.stringify({ path: recommendation.path }),
                });
                if (result.enqueued) {
                    enqueued += 1;
                } else {
                    skipped += 1;
                }
            } catch {
                failed += 1;
            }
        }

        setQueueingRemux(false);
        await fetchIntelligence();
        showToast({
            kind: failed > 0 ? "error" : "success",
            title: "Intelligence",
            message: `Queue all finished: ${enqueued} enqueued, ${skipped} skipped, ${failed} failed.`,
        });
    };

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
                                        {type === "remux_only_candidate" && recommendations.length > 0 && (
                                            <button
                                                onClick={() => void handleQueueAllRemux()}
                                                disabled={queueingRemux}
                                                className="ml-auto inline-flex items-center gap-2 rounded-lg border border-helios-solar/20 bg-helios-solar/10 px-3 py-1.5 text-xs font-semibold text-helios-solar transition-colors hover:bg-helios-solar/20 disabled:opacity-60"
                                            >
                                                <Zap size={12} />
                                                {queueingRemux ? "Queueing..." : "Queue all"}
                                            </button>
                                        )}
                                    </div>
                                    <div className="divide-y divide-helios-line/10">
                                        {recommendations.map((recommendation, index) => (
                                            <div key={`${recommendation.path}-${index}`} className="px-5 py-4">
                                                <div className="flex items-center justify-between gap-4">
                                                    <div>
                                                        <h3 className="text-sm font-semibold text-helios-ink">{recommendation.title}</h3>
                                                        <p className="mt-1 text-sm text-helios-slate">{recommendation.summary}</p>
                                                    </div>
                                                    {type === "remux_only_candidate" && (
                                                        <button
                                                            onClick={() => void apiJson<{ enqueued: boolean; message: string }>("/api/jobs/enqueue", {
                                                                method: "POST",
                                                                body: JSON.stringify({ path: recommendation.path }),
                                                            }).then((result) => {
                                                                showToast({
                                                                    kind: result.enqueued ? "success" : "info",
                                                                    title: "Intelligence",
                                                                    message: result.message,
                                                                });
                                                                return fetchIntelligence();
                                                            }).catch((err) => {
                                                                const message = isApiError(err) ? err.message : "Failed to enqueue remux opportunity.";
                                                                showToast({ kind: "error", title: "Intelligence", message });
                                                            })}
                                                            className="inline-flex items-center gap-2 rounded-lg border border-helios-line/20 bg-helios-surface px-3 py-2 text-xs font-semibold text-helios-ink transition-colors hover:bg-helios-surface-soft"
                                                        >
                                                            <Zap size={12} />
                                                            Queue
                                                        </button>
                                                    )}
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
                                                <button
                                                    onClick={() => void openJobDetails(path.id)}
                                                    className="inline-flex items-center gap-1 rounded-lg border border-helios-line/20 bg-helios-surface px-2.5 py-1.5 text-[11px] font-semibold text-helios-ink transition-colors hover:bg-helios-surface-soft"
                                                >
                                                    <Search size={12} />
                                                    Review
                                                </button>
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

            {typeof document !== "undefined" && createPortal(
                <JobDetailModal
                    focusedJob={focusedJob}
                    detailDialogRef={detailDialogRef}
                    detailLoading={detailLoading}
                    onClose={closeJobDetails}
                    focusedDecision={focusedDecision}
                    focusedFailure={focusedFailure}
                    focusedJobLogs={focusedJobLogs}
                    shouldShowFfmpegOutput={shouldShowFfmpegOutput}
                    completedEncodeStats={completedEncodeStats}
                    focusedEmptyState={focusedEmptyState}
                    openConfirm={openConfirm}
                    handleAction={handleAction}
                    handlePriority={handlePriority}
                    getStatusBadge={getStatusBadge}
                />,
                document.body,
            )}

            <ConfirmDialog
                open={confirmState !== null}
                title={confirmState?.title ?? ""}
                description={confirmState?.body ?? ""}
                confirmLabel={confirmState?.confirmLabel ?? "Confirm"}
                tone={confirmState?.confirmTone ?? "primary"}
                onClose={() => setConfirmState(null)}
                onConfirm={async () => {
                    if (!confirmState) {
                        return;
                    }
                    await confirmState.onConfirm();
                }}
            />
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
