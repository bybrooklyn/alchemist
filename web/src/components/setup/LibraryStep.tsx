import { useCallback, useEffect, useState } from "react";
import { motion } from "framer-motion";
import { FolderOpen, FolderSearch, Plus, X } from "lucide-react";
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
            <motion.div
                key="library"
                initial={{ opacity: 0, x: 20 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -20 }}
                className="space-y-6"
            >
                {/* Step heading */}
                <div className="space-y-1">
                    <h2 className="text-xl font-semibold text-helios-ink
                        flex items-center gap-2">
                        <FolderOpen size={20}
                            className="text-helios-solar" />
                        Library Selection
                    </h2>
                    <p className="text-sm text-helios-slate">
                        Choose the server folders Alchemist should scan
                        and keep watching.
                    </p>
                </div>

                {/* Recommendations — shown when server returns any */}
                {recommendations.length > 0 ? (
                    <div className="space-y-2">
                        <p className="text-xs font-medium
                            text-helios-slate">
                            Suggested folders
                        </p>
                        <div className="space-y-2">
                            {recommendations.map((rec) => {
                                const alreadyAdded =
                                    directories.includes(rec.path);
                                return (
                                    <button
                                        key={rec.path}
                                        type="button"
                                        onClick={() =>
                                            !alreadyAdded &&
                                            addDirectory(rec.path)
                                        }
                                        disabled={alreadyAdded}
                                        className={`w-full flex items-center
                                            justify-between gap-4 rounded-lg
                                            border px-4 py-3 text-left
                                            transition-all ${
                                            alreadyAdded
                                                ? "border-helios-solar/30 bg-helios-solar/5 cursor-default"
                                                : "border-helios-line/30 bg-helios-surface hover:border-helios-solar/40 hover:bg-helios-surface-soft"
                                        }`}
                                    >
                                        <div className="min-w-0">
                                            <p className="text-sm font-medium
                                                text-helios-ink truncate">
                                                {rec.label}
                                            </p>
                                            <p className="text-xs font-mono
                                                text-helios-slate/70 truncate
                                                mt-0.5"
                                                title={rec.path}>
                                                {rec.path}
                                            </p>
                                        </div>
                                        {alreadyAdded ? (
                                            <span className="text-xs
                                                text-helios-solar shrink-0
                                                font-medium">
                                                Added
                                            </span>
                                        ) : (
                                            <Plus size={15}
                                                className="text-helios-solar/60
                                                shrink-0" />
                                        )}
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                ) : (
                    /* Empty state — no recommendations */
                    <div className="rounded-lg border border-helios-line/20
                        bg-helios-surface-soft/40 px-5 py-8 text-center
                        space-y-3">
                        <p className="text-sm text-helios-slate">
                            No media folders were auto-detected on this
                            server.
                        </p>
                        <p className="text-xs text-helios-slate/60">
                            Use Browse below to navigate the server
                            filesystem manually.
                        </p>
                    </div>
                )}

                {/* Selected folders as chips */}
                {directories.length > 0 && (
                    <div className="space-y-2">
                        <p className="text-xs font-medium text-helios-slate">
                            Selected ({directories.length})
                        </p>
                        <div className="flex flex-wrap gap-2">
                            {directories.map((dir) => (
                                <div
                                    key={dir}
                                    className="flex items-center gap-2
                                        rounded-lg border border-helios-solar/30
                                        bg-helios-solar/5 pl-3 pr-2 py-1.5"
                                >
                                    <span className="font-mono text-xs
                                        text-helios-ink truncate max-w-[300px]"
                                        title={dir}>
                                        {dir.split("/").pop() || dir}
                                    </span>
                                    <button
                                        type="button"
                                        onClick={() =>
                                            onDirectoriesChange(
                                                directories.filter(
                                                    (d) => d !== dir
                                                )
                                            )
                                        }
                                        className="text-helios-slate/50
                                            hover:text-status-error
                                            transition-colors shrink-0"
                                    >
                                        <X size={13} />
                                    </button>
                                </div>
                            ))}
                        </div>
                    </div>
                )}

                {/* Browse button */}
                <button
                    type="button"
                    onClick={() => setPickerOpen(true)}
                    className="w-full flex items-center justify-center
                        gap-2 rounded-lg border border-helios-line/30
                        bg-helios-surface py-3 text-sm font-medium
                        text-helios-slate hover:border-helios-solar/40
                        hover:text-helios-ink transition-colors"
                >
                    <FolderSearch size={15} />
                    Browse server folders
                </button>

                {/* Manual path input */}
                <div className="space-y-2">
                    <label className="text-xs font-medium
                        text-helios-slate">
                        Or paste a path directly
                    </label>
                    <div className="flex gap-2">
                        <input
                            type="text"
                            value={dirInput}
                            onChange={(e) => onDirInputChange(e.target.value)}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") {
                                    addDirectory(dirInput);
                                }
                            }}
                            placeholder="/path/to/media"
                            className="flex-1 rounded-lg border
                                border-helios-line/40 bg-helios-surface
                                px-4 py-2.5 font-mono text-sm
                                text-helios-ink focus:border-helios-solar
                                outline-none"
                        />
                        <button
                            type="button"
                            onClick={() => addDirectory(dirInput)}
                            className="rounded-lg bg-helios-solar px-4
                                py-2.5 text-sm font-semibold
                                text-helios-main hover:opacity-90
                                transition-opacity"
                        >
                            Add
                        </button>
                    </div>
                </div>

                {previewError && (
                    <div className="rounded-lg border border-status-error/20
                        bg-status-error/10 px-4 py-3 text-sm
                        text-status-error">
                        {previewError}
                    </div>
                )}
            </motion.div>

            <ServerDirectoryPicker
                open={pickerOpen}
                title="Browse Server Folders"
                description="Navigate the server filesystem and choose
                    the folder Alchemist should treat as a media root."
                onClose={() => setPickerOpen(false)}
                onSelect={(path) => {
                    addDirectory(path);
                    setPickerOpen(false);
                }}
            />
        </>
    );
}
