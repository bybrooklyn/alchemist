import { useCallback, useEffect, useState } from "react";
import { motion } from "framer-motion";
import { FolderOpen, FolderSearch, Plus, Search, Sparkles } from "lucide-react";
import { apiJson, isApiError } from "../../lib/api";
import ServerDirectoryPicker from "../ui/ServerDirectoryPicker";
import type { FsPreviewResponse, FsRecommendation, StepValidator } from "./types";

interface LibraryStepProps {
    dirInput: string;
    directories: string[];
    recommendations: FsRecommendation[];
    preview: FsPreviewResponse | null;
    onDirInputChange: (value: string) => void;
    onDirectoriesChange: (value: string[]) => void;
    onPreviewChange: (value: FsPreviewResponse | null) => void;
    registerValidator: (validator: StepValidator) => void;
}

export default function LibraryStep({
    dirInput,
    directories,
    recommendations,
    preview,
    onDirInputChange,
    onDirectoriesChange,
    onPreviewChange,
    registerValidator,
}: LibraryStepProps) {
    const [previewLoading, setPreviewLoading] = useState(false);
    const [previewError, setPreviewError] = useState<string | null>(null);
    const [pickerOpen, setPickerOpen] = useState(false);

    const fetchPreview = useCallback(async (): Promise<FsPreviewResponse | null> => {
        if (directories.length === 0) {
            onPreviewChange(null);
            setPreviewError(null);
            return null;
        }

        setPreviewLoading(true);
        try {
            const data = await apiJson<FsPreviewResponse>("/api/fs/preview", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ directories }),
            });
            onPreviewChange(data);
            setPreviewError(null);
            return data;
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to preview selected folders.";
            setPreviewError(message);
            return null;
        } finally {
            setPreviewLoading(false);
        }
    }, [directories, onPreviewChange]);

    useEffect(() => {
        registerValidator(async () => {
            if (directories.length === 0) {
                return "Select at least one server folder before continuing.";
            }
            const nextPreview = await fetchPreview();
            if (nextPreview && nextPreview.total_media_files === 0) {
                return "Preview did not find any supported media files yet. Double-check the chosen folders.";
            }
            return null;
        });
    }, [directories, fetchPreview, registerValidator]);

    useEffect(() => {
        if (directories.length === 0) {
            return;
        }
        const handle = window.setTimeout(() => void fetchPreview(), 350);
        return () => window.clearTimeout(handle);
    }, [directories, fetchPreview]);

    const addDirectory = (path: string) => {
        const normalized = path.trim();
        if (!normalized || directories.includes(normalized)) {
            return;
        }
        onDirectoriesChange([...directories, normalized]);
        onDirInputChange("");
    };

    return (
        <>
            <motion.div key="library" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
                <div className="space-y-2">
                    <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2"><FolderOpen size={20} className="text-helios-solar" />Library Selection</h2>
                    <p className="text-sm text-helios-slate">Choose the server folders Alchemist should scan and keep watching. Recommendations and preview are here to remove the guesswork.</p>
                </div>

                <div className="grid grid-cols-1 xl:grid-cols-[1.2fr_0.8fr] gap-6">
                    <div className="space-y-5">
                        <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 p-5 space-y-4">
                            <div className="flex items-start justify-between gap-4">
                                <div>
                                    <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><Sparkles size={16} className="text-helios-solar" />Suggested Server Folders</div>
                                    <p className="text-xs text-helios-slate mt-1">Auto-discovered media-like folders from the server filesystem. Review and add what you actually want watched.</p>
                                </div>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                {recommendations.map((recommendation) => {
                                    const hint = recommendation.media_hint;
                                    const badgeLabel =
                                        hint === "medium" ? "Possible media" :
                                        hint === "low"    ? "Few media files" :
                                        null;
                                    return (
                                        <button
                                            key={recommendation.path}
                                            type="button"
                                            onClick={() => addDirectory(recommendation.path)}
                                            className="rounded-md border border-helios-line/20 bg-helios-surface px-4 py-3 text-left hover:border-helios-solar/40 transition-all min-h-[80px] group"
                                        >
                                            <div className="flex items-start justify-between gap-2">
                                                <span className="text-base font-semibold text-helios-ink leading-tight">
                                                    {recommendation.label}
                                                </span>
                                                <div className="flex items-center gap-1.5 shrink-0">
                                                    {badgeLabel && (
                                                        <span className="rounded border border-helios-line/30 px-2 py-0.5 text-[10px] font-medium text-helios-slate/70">
                                                            {badgeLabel}
                                                        </span>
                                                    )}
                                                    <Plus
                                                        size={14}
                                                        className="text-helios-solar/60 group-hover:text-helios-solar transition-colors"
                                                    />
                                                </div>
                                            </div>
                                            <p className="mt-1.5 font-mono text-xs text-helios-slate/70 truncate" title={recommendation.path}>
                                                {recommendation.path}
                                            </p>
                                            <p className="mt-1 text-xs text-helios-slate truncate" title={recommendation.reason}>
                                                {recommendation.reason}
                                            </p>
                                        </button>
                                    );
                                })}
                            </div>
                            <button
                                type="button"
                                onClick={() => setPickerOpen(true)}
                                className="w-full flex items-center justify-center gap-2 rounded-md border border-helios-line/30 py-2.5 text-sm font-medium text-helios-slate hover:border-helios-solar/40 hover:text-helios-ink transition-colors"
                            >
                                <FolderSearch size={15} />
                                Browse Server Folders
                            </button>
                        </div>

                        <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                            <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><FolderOpen size={16} className="text-helios-solar" />Selected Library Roots</div>
                            <div className="flex gap-2">
                                <input type="text" value={dirInput} onChange={(e) => onDirInputChange(e.target.value)} placeholder="Paste a server path or use Browse" className="flex-1 rounded-md border border-helios-line/20 bg-helios-surface-soft px-4 py-3 font-mono text-sm text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none" />
                                <button type="button" onClick={() => addDirectory(dirInput)} className="rounded-md bg-helios-solar px-5 py-3 text-sm font-semibold text-helios-main">Add</button>
                            </div>
                            <div className="space-y-2">
                                {directories.map((dir) => (
                                    <div key={dir} className="flex items-center justify-between rounded-lg border border-helios-line/20 bg-helios-surface-soft/50 px-4 py-3">
                                        <div className="min-w-0">
                                            <p className="font-mono text-sm text-helios-ink truncate" title={dir}>{dir}</p>
                                            <p className="text-[11px] text-helios-slate mt-1">Watched recursively and used as a library root.</p>
                                        </div>
                                        <button type="button" onClick={() => onDirectoriesChange(directories.filter((value) => value !== dir))} className="rounded-md border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-500 hover:bg-red-500/10">Remove</button>
                                    </div>
                                ))}
                                {directories.length === 0 && <p className="text-sm text-helios-slate">No server folders selected yet.</p>}
                            </div>
                        </div>
                    </div>

                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <div className="flex items-center justify-between gap-3">
                            <div>
                                <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><Search size={16} className="text-helios-solar" />Library Preview</div>
                                <p className="text-xs text-helios-slate mt-1">See what Alchemist will likely ingest before you finish setup.</p>
                            </div>
                            <button type="button" onClick={() => void fetchPreview()} disabled={previewLoading || directories.length === 0} className="rounded-md border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink hover:border-helios-solar/30 disabled:opacity-50">{previewLoading ? "Previewing..." : "Refresh Preview"}</button>
                        </div>

                        {previewError && <div className="rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">{previewError}</div>}

                        {preview ? (
                            <div className="space-y-4">
                                <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-4 py-3">
                                    <p className="text-[10px] font-bold uppercase tracking-wider text-emerald-500">Estimated Supported Media</p>
                                    <p className="mt-2 text-2xl font-bold text-helios-ink">{preview.total_media_files}</p>
                                </div>
                                {preview.warnings.length > 0 && <div className="space-y-2">{preview.warnings.map((warning) => <div key={warning} className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-4 py-3 text-xs text-amber-500">{warning}</div>)}</div>}
                                <div className="space-y-3">
                                    {preview.directories.map((directory) => (
                                        <div key={directory.path} className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4">
                                            <div className="flex items-center justify-between gap-3">
                                                <div className="min-w-0">
                                                    <p className="font-mono text-sm text-helios-ink break-all">{directory.path}</p>
                                                    <p className="text-xs text-helios-slate mt-1">{directory.media_files} supported files found</p>
                                                </div>
                                                <span className="rounded-full border border-helios-line/20 px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-helios-slate">{directory.media_hint}</span>
                                            </div>
                                            {directory.sample_files.length > 0 && <div className="mt-3 space-y-1">{directory.sample_files.map((sample) => <p key={sample} className="text-[11px] font-mono text-helios-slate truncate" title={sample}>{sample}</p>)}</div>}
                                        </div>
                                    ))}
                                </div>
                            </div>
                        ) : (
                            <div className="rounded-lg border border-dashed border-helios-line/20 px-4 py-8 text-sm text-helios-slate text-center">Add one or more server folders to preview what Alchemist will scan.</div>
                        )}
                    </div>
                </div>
            </motion.div>

            <ServerDirectoryPicker
                open={pickerOpen}
                title="Browse Server Folders"
                description="Navigate the server filesystem, review guardrails, and choose the folder Alchemist should treat as a media root."
                onClose={() => setPickerOpen(false)}
                onSelect={(path) => {
                    addDirectory(path);
                    setPickerOpen(false);
                }}
            />
        </>
    );
}
