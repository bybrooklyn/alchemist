import { useEffect, useMemo, useState } from "react";
import { Folder, FolderOpen, AlertTriangle, Sparkles } from "lucide-react";
import { apiJson, isApiError } from "../../lib/api";

interface FsBreadcrumb {
    label: string;
    path: string;
}

interface FsDirEntry {
    name: string;
    path: string;
    readable: boolean;
    hidden: boolean;
    media_hint: "high" | "medium" | "low" | "unknown";
    warning?: string | null;
}

interface FsBrowseResponse {
    path: string;
    readable: boolean;
    breadcrumbs: FsBreadcrumb[];
    warnings: string[];
    entries: FsDirEntry[];
}

interface FsRecommendation {
    path: string;
    label: string;
    reason: string;
    media_hint: "high" | "medium" | "low" | "unknown";
}

interface FsRecommendationsResponse {
    recommendations: FsRecommendation[];
}

interface ServerDirectoryPickerProps {
    open: boolean;
    title: string;
    description: string;
    onClose: () => void;
    onSelect: (path: string) => void;
}

function mediaBadgeTone(hint: FsDirEntry["media_hint"] | FsRecommendation["media_hint"]) {
    switch (hint) {
        case "high":
            return "border-emerald-500/20 bg-emerald-500/10 text-emerald-500";
        case "medium":
            return "border-amber-500/20 bg-amber-500/10 text-amber-500";
        case "low":
            return "border-helios-line/30 bg-helios-surface text-helios-slate";
        default:
            return "border-helios-line/20 bg-helios-surface text-helios-slate/70";
    }
}

export default function ServerDirectoryPicker({
    open,
    title,
    description,
    onClose,
    onSelect,
}: ServerDirectoryPickerProps) {
    const [browse, setBrowse] = useState<FsBrowseResponse | null>(null);
    const [recommendations, setRecommendations] = useState<FsRecommendation[]>([]);
    const [error, setError] = useState("");
    const [loading, setLoading] = useState(false);
    const [manualPath, setManualPath] = useState("");

    const loadBrowse = async (path?: string) => {
        setLoading(true);
        try {
            const query = path ? `?path=${encodeURIComponent(path)}` : "";
            const data = await apiJson<FsBrowseResponse>(`/api/fs/browse${query}`);
            setBrowse(data);
            setManualPath(data.path);
            setError("");
        } catch (err) {
            setError(isApiError(err) ? err.message : "Failed to browse server folders.");
        } finally {
            setLoading(false);
        }
    };

    const loadRecommendations = async () => {
        try {
            const data = await apiJson<FsRecommendationsResponse>("/api/fs/recommendations");
            setRecommendations(data.recommendations);
        } catch (err) {
            setError(isApiError(err) ? err.message : "Failed to load folder recommendations.");
        }
    };

    useEffect(() => {
        if (!open) return;
        void Promise.all([loadBrowse(), loadRecommendations()]);
    }, [open]);

    const visibleRecommendations = useMemo(
        () => recommendations.slice(0, 8),
        [recommendations]
    );

    if (!open) {
        return null;
    }

    return (
        <div className="fixed inset-0 z-[220]">
            <button
                type="button"
                aria-label="Close directory picker"
                onClick={onClose}
                className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            />

            <div className="absolute inset-0 flex items-center justify-center px-4 py-6">
                <div className="w-full max-w-5xl rounded-xl border border-helios-line/30 bg-helios-surface shadow-2xl overflow-hidden">
                    <div className="border-b border-helios-line/20 px-6 py-5 flex items-start justify-between gap-4">
                        <div>
                            <div className="flex items-center gap-3">
                                <div className="rounded-xl bg-helios-solar/10 p-2 text-helios-solar">
                                    <FolderOpen size={20} />
                                </div>
                                <div>
                                    <h3 className="text-lg font-bold text-helios-ink">{title}</h3>
                                    <p className="text-sm text-helios-slate">{description}</p>
                                </div>
                            </div>
                            <p className="mt-3 text-[11px] text-helios-slate">
                                You are browsing the <span className="font-bold text-helios-ink">server filesystem</span>, not your browser’s local machine.
                            </p>
                        </div>
                        <button
                            type="button"
                            onClick={onClose}
                            className="rounded-xl border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-slate hover:bg-helios-surface-soft"
                        >
                            Close
                        </button>
                    </div>

                    <div className="grid grid-cols-1 lg:grid-cols-[320px_1fr] min-h-[620px]">
                        <aside className="border-r border-helios-line/20 bg-helios-surface-soft/40 px-5 py-5 space-y-5">
                            <div className="space-y-2">
                                <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                                    Jump To Server Path
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        value={manualPath}
                                        onChange={(e) => setManualPath(e.target.value)}
                                        placeholder="/media/movies"
                                        className="flex-1 rounded-xl border border-helios-line/20 bg-helios-surface px-3 py-2 font-mono text-sm text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
                                    />
                                    <button
                                        type="button"
                                        onClick={() => void loadBrowse(manualPath)}
                                        className="rounded-xl bg-helios-solar px-4 py-2 text-sm font-semibold text-helios-main"
                                    >
                                        Open
                                    </button>
                                </div>
                            </div>

                            <div className="space-y-3">
                                <div className="flex items-center gap-2 text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                                    <Sparkles size={12} />
                                    Recommended Media Roots
                                </div>
                                <div className="space-y-2">
                                    {visibleRecommendations.map((recommendation) => (
                                        <button
                                            key={recommendation.path}
                                            type="button"
                                            onClick={() => void loadBrowse(recommendation.path)}
                                            className="w-full rounded-lg border border-helios-line/20 bg-helios-surface px-3 py-3 text-left hover:border-helios-solar/30 hover:bg-helios-surface-soft transition-all"
                                        >
                                            <div className="flex items-start justify-between gap-3">
                                                <div>
                                                    <div className="text-sm font-semibold text-helios-ink">{recommendation.label}</div>
                                                    <div className="text-[10px] text-helios-slate mt-1 break-all">{recommendation.path}</div>
                                                </div>
                                                <span className={`rounded-full border px-2 py-1 text-[10px] font-bold uppercase tracking-wider ${mediaBadgeTone(recommendation.media_hint)}`}>
                                                    {recommendation.media_hint}
                                                </span>
                                            </div>
                                            <p className="mt-2 text-[11px] text-helios-slate">{recommendation.reason}</p>
                                        </button>
                                    ))}
                                </div>
                            </div>
                        </aside>

                        <section className="px-6 py-5 flex flex-col">
                            {error && (
                                <div className="mb-4 rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                                    {error}
                                </div>
                            )}

                            {browse && (
                                <>
                                    <div className="flex flex-wrap gap-2 mb-4">
                                        {browse.breadcrumbs.map((crumb) => (
                                            <button
                                                key={crumb.path}
                                                type="button"
                                                onClick={() => void loadBrowse(crumb.path)}
                                                className="rounded-full border border-helios-line/20 bg-helios-surface px-3 py-1 text-xs font-semibold text-helios-ink hover:border-helios-solar/30"
                                            >
                                                {crumb.label}
                                            </button>
                                        ))}
                                    </div>

                                    <div className="mb-4 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4">
                                        <div className="flex items-start justify-between gap-4">
                                            <div>
                                                <p className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                                                    Current Server Folder
                                                </p>
                                                <p className="mt-2 font-mono text-sm text-helios-ink break-all">
                                                    {browse.path}
                                                </p>
                                            </div>
                                            <button
                                                type="button"
                                                onClick={() => onSelect(browse.path)}
                                                className="shrink-0 rounded-xl bg-helios-solar px-4 py-2 text-sm font-semibold text-helios-main"
                                            >
                                                Use This Folder
                                            </button>
                                        </div>
                                        {browse.warnings.length > 0 && (
                                            <div className="mt-4 space-y-2">
                                                {browse.warnings.map((warning) => (
                                                    <div
                                                        key={warning}
                                                        className="flex items-start gap-2 rounded-xl border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-500"
                                                    >
                                                        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                                        <span>{warning}</span>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>

                                    <div className="flex items-center justify-between mb-3">
                                        <div className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                                            Child Folders
                                        </div>
                                        {loading && <div className="text-xs text-helios-slate animate-pulse">Loading…</div>}
                                    </div>

                                    <div className="flex-1 overflow-y-auto rounded-lg border border-helios-line/20 bg-helios-surface-soft/30">
                                        {browse.entries.length === 0 ? (
                                            <div className="flex h-full min-h-[260px] items-center justify-center px-6 text-sm text-helios-slate">
                                                No child directories were found here.
                                            </div>
                                        ) : (
                                            <div className="divide-y divide-helios-line/10">
                                                {browse.entries.map((entry) => (
                                                    <div
                                                        key={entry.path}
                                                        className="flex items-start justify-between gap-4 px-4 py-4 hover:bg-helios-surface/60 transition-colors"
                                                    >
                                                        <button
                                                            type="button"
                                                            onClick={() => void loadBrowse(entry.path)}
                                                            className="min-w-0 flex-1 text-left"
                                                        >
                                                            <div className="flex items-center gap-3">
                                                                <div className="rounded-xl bg-helios-solar/10 p-2 text-helios-solar">
                                                                    <Folder size={16} />
                                                                </div>
                                                                <div className="min-w-0">
                                                                    <div className="flex items-center gap-2">
                                                                        <span className="truncate text-sm font-semibold text-helios-ink">{entry.name}</span>
                                                                        <span className={`rounded-full border px-2 py-1 text-[10px] font-bold uppercase tracking-wider ${mediaBadgeTone(entry.media_hint)}`}>
                                                                            {entry.media_hint}
                                                                        </span>
                                                                        {entry.hidden && (
                                                                            <span className="rounded-full border border-helios-line/20 px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-helios-slate">
                                                                                hidden
                                                                            </span>
                                                                        )}
                                                                    </div>
                                                                    <div className="mt-1 break-all font-mono text-[11px] text-helios-slate">
                                                                        {entry.path}
                                                                    </div>
                                                                    {entry.warning && (
                                                                        <div className="mt-2 text-[11px] text-amber-500">{entry.warning}</div>
                                                                    )}
                                                                </div>
                                                            </div>
                                                        </button>
                                                        <button
                                                            type="button"
                                                            onClick={() => onSelect(entry.path)}
                                                            className="shrink-0 rounded-xl border border-helios-line/20 px-3 py-2 text-xs font-semibold text-helios-ink hover:border-helios-solar/30"
                                                        >
                                                            Select
                                                        </button>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                </>
                            )}

                            {!browse && loading && (
                                <div className="flex flex-1 items-center justify-center text-helios-slate animate-pulse">
                                    Loading server folders…
                                </div>
                            )}
                        </section>
                    </div>
                </div>
            </div>
        </div>
    );
}
