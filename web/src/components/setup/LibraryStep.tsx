import { useCallback, useEffect, useState } from "react";
import { motion } from "framer-motion";
import { ChevronLeft, ChevronRight, Folder, FolderOpen, X } from "lucide-react";
import { apiJson, isApiError } from "../../lib/api";
import type { FsPreviewResponse, FsRecommendation, StepValidator } from "./types";

interface LibraryStepProps {
    dirInput: string;
    directories: string[];
    recommendations: FsRecommendation[];
    onDirInputChange: (value: string) => void;
    onDirectoriesChange: (value: string[]) => void;
    onPreviewChange: (value: FsPreviewResponse | null) => void;
    registerValidator: (validator: StepValidator) => void;
}

interface FsBreadcrumb {
    name: string;
    path: string;
}

interface FsDirEntry {
    name: string;
    path: string;
    readable: boolean;
}

interface FsBrowseResponse {
    path: string;
    readable: boolean;
    breadcrumbs: FsBreadcrumb[];
    warnings: string[];
    entries: FsDirEntry[];
}

export default function LibraryStep({
    dirInput,
    directories,
    recommendations: _recommendations,
    onDirInputChange,
    onDirectoriesChange,
    onPreviewChange,
    registerValidator,
}: LibraryStepProps) {
    const [pickerOpen, setPickerOpen] = useState(false);
    const [browse, setBrowse] = useState<FsBrowseResponse | null>(null);
    const [browseError, setBrowseError] = useState("");
    const [browseLoading, setBrowseLoading] = useState(false);

    const previewFailureMessage = (err: unknown) =>
        isApiError(err)
            ? err.message
            : "Failed to preview the selected folders. Double-check the path and that the Alchemist server can read it.";

    const fetchPreview = useCallback(async (): Promise<FsPreviewResponse | null> => {
        if (directories.length === 0) {
            onPreviewChange(null);
            return null;
        }

        try {
            const data = await apiJson<FsPreviewResponse>("/api/fs/preview", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ directories }),
            });
            onPreviewChange(data);
            return data;
        } catch (err) {
            onPreviewChange(null);
            throw new Error(previewFailureMessage(err));
        }
    }, [directories, onPreviewChange]);

    registerValidator(async () => {
        if (directories.length === 0) {
            return "Select at least one server folder before continuing.";
        }
        return null;
    });

    useEffect(() => {
        if (directories.length === 0) {
            return;
        }
        const handle = window.setTimeout(() => {
            void fetchPreview().catch(() => undefined);
        }, 350);
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

    const loadBrowse = useCallback(async (path?: string) => {
        setBrowseLoading(true);
        setBrowseError("");
        try {
            const query = path ? `?path=${encodeURIComponent(path)}` : "";
            const data = await apiJson<FsBrowseResponse>(`/api/fs/browse${query}`);
            setBrowse(data);
        } catch (err) {
            setBrowse(null);
            setBrowseError(isApiError(err) ? err.message : "Failed to browse server folders.");
        } finally {
            setBrowseLoading(false);
        }
    }, []);

    useEffect(() => {
        if (!pickerOpen) {
            return;
        }
        void loadBrowse();
    }, [pickerOpen, loadBrowse]);

    const removeDirectory = (path: string) => {
        onDirectoriesChange(directories.filter((directory) => directory !== path));
    };

    const handleBrowseOpen = () => {
        setBrowse(null);
        setBrowseError("");
        setPickerOpen(true);
    };

    const handleBrowseClose = () => {
        setPickerOpen(false);
        setBrowse(null);
        setBrowseError("");
        setBrowseLoading(false);
    };

    const currentBrowsePath = browse?.path ?? "";
    const currentBrowseName =
        currentBrowsePath.split("/").filter(Boolean).pop() || currentBrowsePath || "root";
    const breadcrumbs = browse?.breadcrumbs ?? [];
    const parentBreadcrumb =
        breadcrumbs.length > 1 ? breadcrumbs[breadcrumbs.length - 2] : null;
    const visibleEntries = browse?.entries?.filter((entry) => entry.readable) ?? [];

    return (
        <motion.div
            key="library"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="space-y-6"
        >
            <div className="space-y-1">
                <h2 className="flex items-center gap-2 text-xl font-semibold text-helios-ink">
                    <FolderOpen size={20} className="text-helios-solar" />
                    Library Selection
                </h2>
                <p className="text-sm text-helios-slate">
                    Choose folders Alchemist should scan and watch for new media.
                </p>
            </div>

            <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
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
                    className="flex-1 rounded-lg border border-helios-line/40 bg-helios-surface px-4 py-2.5 font-mono text-sm text-helios-ink outline-none transition-colors focus:border-helios-solar"
                />
                <button
                    type="button"
                    onClick={handleBrowseOpen}
                    className="rounded-lg border border-helios-line/30 bg-helios-surface px-4 py-2.5 text-sm font-medium text-helios-slate transition-colors hover:border-helios-solar/40 hover:text-helios-ink"
                >
                    Browse
                </button>
                <button
                    type="button"
                    onClick={() => addDirectory(dirInput)}
                    className="rounded-lg bg-helios-solar px-4 py-2.5 text-sm font-semibold text-helios-main transition-opacity hover:opacity-90"
                >
                    Add
                </button>
            </div>

            {pickerOpen ? (
                <div className="flex h-[420px] flex-col gap-4 overflow-hidden rounded-lg border border-helios-line/30 bg-helios-surface p-4">
                    <div className="shrink-0 flex items-start justify-between gap-4">
                        <div className="min-w-0 space-y-3">
                            <div className="space-y-1">
                                <p className="text-xs font-medium uppercase tracking-[0.12em] text-helios-slate/70">
                                    Server Filesystem
                                </p>
                                <div className="flex items-center gap-2">
                                    <Folder size={16} className="shrink-0 text-helios-solar" />
                                    <p className="truncate text-sm font-medium text-helios-ink">
                                        {currentBrowseName}
                                    </p>
                                </div>
                            </div>

                            <div className="flex flex-wrap items-center gap-2">
                                <button
                                    type="button"
                                    onClick={() =>
                                        parentBreadcrumb
                                            ? void loadBrowse(parentBreadcrumb.path)
                                            : void loadBrowse()
                                    }
                                    disabled={browseLoading || !browse || !parentBreadcrumb}
                                    className="inline-flex items-center gap-1.5 rounded-lg border border-helios-line/30 px-3 py-1.5 text-sm text-helios-slate transition-colors hover:border-helios-solar/40 hover:text-helios-ink disabled:cursor-not-allowed disabled:opacity-40"
                                >
                                    <ChevronLeft size={15} />
                                    Up
                                </button>

                                <div className="min-w-0 flex-1 overflow-x-auto">
                                    <div className="flex min-w-max items-center gap-1.5 text-sm text-helios-slate">
                                        {breadcrumbs.length > 0 ? (
                                            breadcrumbs.map((crumb, index) => {
                                                const isCurrent = crumb.path === currentBrowsePath;
                                                return (
                                                    <div
                                                        key={crumb.path}
                                                        className="flex items-center gap-1.5"
                                                    >
                                                        {index > 0 && (
                                                            <span className="text-helios-slate/50">/</span>
                                                        )}
                                                        <button
                                                            type="button"
                                                            onClick={() => void loadBrowse(crumb.path)}
                                                            className={
                                                                isCurrent
                                                                    ? "rounded-lg bg-helios-solar/10 px-2 py-1 font-medium text-helios-ink"
                                                                    : "rounded-lg px-2 py-1 transition-colors hover:bg-helios-surface-soft hover:text-helios-ink"
                                                            }
                                                        >
                                                            {crumb.name.replace(/^\//, "")}
                                                        </button>
                                                    </div>
                                                );
                                            })
                                        ) : (
                                            <span className="rounded-lg bg-helios-solar/10 px-2 py-1 font-medium text-helios-ink">
                                                /
                                            </span>
                                        )}
                                    </div>
                                </div>
                            </div>
                        </div>
                        <button
                            type="button"
                            onClick={handleBrowseClose}
                            className="shrink-0 rounded-lg border border-helios-line/30 px-3 py-1.5 text-sm text-helios-slate transition-colors hover:border-helios-solar/40 hover:text-helios-ink"
                            aria-label="Close folder browser"
                        >
                            <X size={16} />
                        </button>
                    </div>

                    <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain rounded-lg border border-helios-line/20 bg-helios-surface-soft/30">
                        {browse?.warnings.length ? (
                            <div className="border-b border-helios-line/10 px-4 py-3">
                                {browse.warnings.map((warning) => (
                                    <p
                                        key={warning}
                                        className="text-xs text-helios-slate"
                                    >
                                        {warning}
                                    </p>
                                ))}
                            </div>
                        ) : null}

                        {browseLoading ? (
                            <div className="animate-pulse space-y-3 p-4">
                                {Array.from({ length: 5 }).map((_, index) => (
                                    <div
                                        key={index}
                                        className="flex items-center gap-3 rounded-lg border border-helios-line/10 bg-helios-surface px-4 py-3"
                                    >
                                        <div className="h-4 w-4 rounded bg-helios-line/20" />
                                        <div className="h-3 flex-1 rounded bg-helios-line/20" />
                                        <div className="h-3 w-3 rounded bg-helios-line/20" />
                                    </div>
                                ))}
                            </div>
                        ) : browseError ? (
                            <div className="px-4 py-6 text-sm text-status-error">{browseError}</div>
                        ) : visibleEntries.length === 0 ? (
                            <div className="px-4 py-6 text-sm text-helios-slate">
                                No readable child folders were found here.
                            </div>
                        ) : (
                            <div className="divide-y divide-helios-line/10">
                                {parentBreadcrumb ? (
                                    <button
                                        type="button"
                                        onClick={() => void loadBrowse(parentBreadcrumb.path)}
                                        className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-helios-surface/70"
                                    >
                                        <ChevronLeft size={16} className="shrink-0 text-helios-slate" />
                                        <div className="min-w-0 flex-1">
                                            <span className="block truncate text-sm font-medium text-helios-ink">
                                                ..
                                            </span>
                                            <span className="block truncate text-xs text-helios-slate">
                                                Go up to {parentBreadcrumb.name.replace(/^\//, "")}
                                            </span>
                                        </div>
                                    </button>
                                ) : null}

                                {visibleEntries.map((entry) => (
                                    <button
                                        key={entry.path}
                                        type="button"
                                        onClick={() => void loadBrowse(entry.path)}
                                        className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-helios-solar/5"
                                    >
                                        <Folder size={16} className="shrink-0 text-helios-slate" />
                                        <div className="min-w-0 flex-1">
                                            <span className="block truncate text-sm text-helios-ink">
                                                {entry.name}
                                            </span>
                                            <span className="block truncate font-mono text-xs text-helios-slate/80">
                                                {entry.path}
                                            </span>
                                        </div>
                                        <ChevronRight
                                            size={16}
                                            className="shrink-0 text-helios-slate"
                                        />
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    <div className="shrink-0 flex flex-col gap-3 rounded-lg border border-helios-line/20 bg-helios-surface-soft/30 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                        <div className="min-w-0">
                            <p className="text-xs font-medium text-helios-slate/80">
                                Current folder
                            </p>
                            <p className="min-w-0 break-all font-mono text-xs text-helios-slate">
                                {currentBrowsePath || "/"}
                            </p>
                        </div>
                        <button
                            type="button"
                            onClick={() => {
                                if (!currentBrowsePath) {
                                    return;
                                }
                                addDirectory(currentBrowsePath);
                                handleBrowseClose();
                            }}
                            disabled={!currentBrowsePath}
                            className="shrink-0 rounded-lg bg-helios-solar px-4 py-2 text-sm font-semibold text-helios-main transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            Add this folder
                        </button>
                    </div>
                </div>
            ) : directories.length > 0 ? (
                <div className="overflow-hidden rounded-lg border border-helios-line/30 bg-helios-surface">
                    {directories.map((dir, index) => (
                        <div
                            key={dir}
                            className={`flex items-start gap-4 px-4 py-3 ${
                                index < directories.length - 1 ? "border-b border-helios-line/10" : ""
                            }`}
                        >
                            <p
                                className="min-w-0 flex-1 break-all font-mono text-sm text-helios-slate"
                                title={dir}
                            >
                                {dir}
                            </p>
                            <button
                                type="button"
                                onClick={() => removeDirectory(dir)}
                                className="shrink-0 rounded-lg p-1.5 text-helios-slate transition-colors hover:text-status-error"
                                aria-label={`Remove ${dir}`}
                            >
                                <X size={15} />
                            </button>
                        </div>
                    ))}
                </div>
            ) : (
                <div className="py-8 text-center">
                    <p className="text-sm text-helios-slate/60">No folders added yet</p>
                    <p className="mt-1 text-sm text-helios-slate/60">
                        Add a folder above or browse the server filesystem
                    </p>
                </div>
            )}
        </motion.div>
    );
}
