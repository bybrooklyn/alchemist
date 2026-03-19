import { useState, useEffect } from "react";
import { FolderOpen, Trash2, Plus, Folder, Play } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";
import ServerDirectoryPicker from "./ui/ServerDirectoryPicker";

interface WatchDir {
    id: number;
    path: string;
    is_recursive: boolean;
}

interface SettingsBundleResponse {
    settings: {
        scanner: {
            directories: string[];
        };
        [key: string]: unknown;
    };
}

export default function WatchFolders() {
    const [dirs, setDirs] = useState<WatchDir[]>([]);
    const [libraryDirs, setLibraryDirs] = useState<string[]>([]);
    const [path, setPath] = useState("");
    const [libraryPath, setLibraryPath] = useState("");
    const [isRecursive, setIsRecursive] = useState(true);
    const [loading, setLoading] = useState(true);
    const [scanning, setScanning] = useState(false);
    const [syncingLibrary, setSyncingLibrary] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [pendingRemoveId, setPendingRemoveId] = useState<number | null>(null);
    const [pickerOpen, setPickerOpen] = useState<null | "library" | "watch">(null);

    const fetchBundle = async () => {
        try {
            const data = await apiJson<SettingsBundleResponse>("/api/settings/bundle");
            setLibraryDirs(data.settings.scanner.directories);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch library directories";
            setError(message);
        }
    };

    const fetchDirs = async () => {
        try {
            const data = await apiJson<WatchDir[]>("/api/settings/watch-dirs");
            setDirs(data);
            setError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch watch directories";
            setError(message);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        void fetchDirs();
        void fetchBundle();
    }, []);

    const triggerScan = async () => {
        setScanning(true);
        setError(null);
        try {
            await apiAction("/api/scan/start", { method: "POST" });
            showToast({ kind: "success", title: "Scan", message: "Library scan started." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to start scan";
            setError(message);
            showToast({ kind: "error", title: "Scan", message });
        } finally {
            window.setTimeout(() => setScanning(false), 1200);
        }
    };

    const addDir = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!path.trim()) return;

        try {
            await apiAction("/api/settings/watch-dirs", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ path: path.trim(), is_recursive: isRecursive }),
            });

            setPath("");
            setIsRecursive(true);
            setError(null);
            await fetchDirs();
            showToast({ kind: "success", title: "Watch Folders", message: "Folder added." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to add directory";
            setError(message);
            showToast({ kind: "error", title: "Watch Folders", message });
        }
    };

    const saveLibraryDirs = async (nextDirectories: string[]) => {
        setSyncingLibrary(true);
        try {
            const bundle = await apiJson<SettingsBundleResponse>("/api/settings/bundle");
            await apiAction("/api/settings/bundle", {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    ...bundle.settings,
                    scanner: {
                        ...bundle.settings.scanner,
                        directories: nextDirectories,
                    },
                }),
            });
            setLibraryDirs(nextDirectories);
            setError(null);
            showToast({ kind: "success", title: "Library", message: "Library directories updated." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to update library directories";
            setError(message);
            showToast({ kind: "error", title: "Library", message });
        } finally {
            setSyncingLibrary(false);
        }
    };

    const addLibraryDir = async () => {
        const nextPath = libraryPath.trim();
        if (!nextPath || libraryDirs.includes(nextPath)) return;
        await saveLibraryDirs([...libraryDirs, nextPath]);
        setLibraryPath("");
    };

    const removeLibraryDir = async (dir: string) => {
        await saveLibraryDirs(libraryDirs.filter(candidate => candidate !== dir));
    };

    const removeDir = async (id: number) => {
        try {
            await apiAction(`/api/settings/watch-dirs/${id}`, {
                method: "DELETE",
            });
            setError(null);
            await fetchDirs();
            showToast({ kind: "success", title: "Watch Folders", message: "Folder removed." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to remove directory";
            setError(message);
            showToast({ kind: "error", title: "Watch Folders", message });
        }
    };

    return (
        <div className="space-y-6" aria-live="polite">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <div className="p-2 bg-helios-solar/10 rounded-lg">
                        <FolderOpen className="text-helios-solar" size={20} />
                    </div>
                    <div>
                        <h2 className="text-lg font-semibold text-helios-ink">Watch Folders</h2>
                        <p className="text-xs text-helios-slate">Manage directories monitored for new media</p>
                    </div>
                </div>
                <button
                    onClick={() => void triggerScan()}
                    disabled={scanning}
                    className="flex items-center gap-2 px-3 py-1.5 bg-helios-solar/10 hover:bg-helios-solar/20 text-helios-solar rounded-lg text-xs font-bold uppercase tracking-wider transition-colors disabled:opacity-50"
                >
                    <Play size={14} className={scanning ? "animate-spin" : ""} />
                    {scanning ? "Scanning..." : "Scan Now"}
                </button>
            </div>

            {error && (
                <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error text-sm">
                    {error}
                </div>
            )}

            <form onSubmit={addDir} className="space-y-3">
                <div className="space-y-3 rounded-2xl border border-helios-line/20 bg-helios-surface-soft/50 p-4">
                    <div>
                        <h3 className="text-sm font-bold text-helios-ink uppercase tracking-wider">Library Directories</h3>
                        <p className="text-[10px] text-helios-slate mt-1">
                            Canonical library roots from setup/TOML. These are stored in the main config file and synchronized into runtime watchers.
                        </p>
                    </div>
                    <div className="flex gap-2">
                        <div className="relative flex-1">
                            <Folder className="absolute left-3 top-1/2 -translate-y-1/2 text-helios-slate/50" size={16} />
                            <input
                                type="text"
                                value={libraryPath}
                                onChange={(e) => setLibraryPath(e.target.value)}
                                placeholder="Add library directory..."
                                className="w-full bg-helios-surface border border-helios-line/20 rounded-xl pl-10 pr-4 py-2.5 text-sm text-helios-ink placeholder:text-helios-slate/40 focus:border-helios-solar focus:ring-1 focus:ring-helios-solar/50 outline-none transition-all"
                            />
                        </div>
                        <button
                            type="button"
                            onClick={() => setPickerOpen("library")}
                            className="rounded-xl border border-helios-line/30 bg-helios-surface px-4 py-2.5 text-sm font-medium text-helios-ink"
                        >
                            Browse
                        </button>
                        <button
                            type="button"
                            onClick={() => void addLibraryDir()}
                            disabled={!libraryPath.trim() || syncingLibrary}
                            className="bg-helios-solar hover:bg-helios-solar-dark text-helios-surface px-5 py-2.5 rounded-xl font-medium text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 shadow-sm shadow-helios-solar/20"
                        >
                            <Plus size={16} /> Add
                        </button>
                    </div>
                    <div className="space-y-2">
                        {libraryDirs.map((dir) => (
                            <div key={dir} className="flex items-center justify-between rounded-xl border border-helios-line/10 bg-helios-surface px-3 py-2">
                                <span className="truncate font-mono text-sm text-helios-ink" title={dir}>{dir}</span>
                                <button
                                    type="button"
                                    onClick={() => void removeLibraryDir(dir)}
                                    disabled={syncingLibrary}
                                    className="rounded-lg p-2 text-helios-slate hover:text-red-500 hover:bg-red-500/10 transition-colors"
                                >
                                    <Trash2 size={16} />
                                </button>
                            </div>
                        ))}
                        {libraryDirs.length === 0 && (
                            <p className="text-xs text-helios-slate">No canonical library directories configured yet.</p>
                        )}
                    </div>
                </div>

                <div className="flex gap-2">
                    <div className="relative flex-1">
                        <Folder className="absolute left-3 top-1/2 -translate-y-1/2 text-helios-slate/50" size={16} />
                        <input
                            type="text"
                            value={path}
                            onChange={(e) => setPath(e.target.value)}
                            placeholder="Enter full directory path..."
                            className="w-full bg-helios-surface border border-helios-line/20 rounded-xl pl-10 pr-4 py-2.5 text-sm text-helios-ink placeholder:text-helios-slate/40 focus:border-helios-solar focus:ring-1 focus:ring-helios-solar/50 outline-none transition-all"
                        />
                    </div>
                    <button
                        type="button"
                        onClick={() => setPickerOpen("watch")}
                        className="rounded-xl border border-helios-line/30 bg-helios-surface px-4 py-2.5 text-sm font-medium text-helios-ink"
                    >
                        Browse
                    </button>
                    <button
                        type="submit"
                        disabled={!path.trim()}
                        className="bg-helios-solar hover:bg-helios-solar-dark text-helios-surface px-5 py-2.5 rounded-xl font-medium text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 shadow-sm shadow-helios-solar/20"
                    >
                        <Plus size={16} /> Add
                    </button>
                </div>
                <label className="inline-flex items-center gap-2 rounded-lg border border-helios-line/20 bg-helios-surface px-3 py-2 text-sm text-helios-ink">
                    <input
                        type="checkbox"
                        checked={isRecursive}
                        onChange={(e) => setIsRecursive(e.target.checked)}
                        className="rounded border-helios-line/30 bg-helios-surface-soft accent-helios-solar"
                    />
                    Watch subdirectories recursively
                </label>
            </form>

            <div className="space-y-2">
                {dirs.map((dir) => (
                    <div key={dir.id} className="flex items-center justify-between p-3 bg-helios-surface border border-helios-line/10 rounded-xl group hover:border-helios-line/30 hover:shadow-sm transition-all">
                        <div className="flex items-center gap-3 overflow-hidden">
                            <div className="p-1.5 bg-helios-slate/5 rounded-lg text-helios-slate">
                                <Folder size={16} />
                            </div>
                            <span className="text-sm font-mono text-helios-ink truncate max-w-[400px]" title={dir.path}>
                                {dir.path}
                            </span>
                            <span className="rounded-full border border-helios-line/20 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-helios-slate">
                                {dir.is_recursive ? "Recursive" : "Top level"}
                            </span>
                        </div>
                        <button
                            onClick={() => setPendingRemoveId(dir.id)}
                            className="p-2 text-helios-slate hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-all opacity-0 group-hover:opacity-100"
                            title="Stop watching"
                        >
                            <Trash2 size={16} />
                        </button>
                    </div>
                ))}

                {!loading && dirs.length === 0 && (
                    <div className="flex flex-col items-center justify-center py-10 text-center border-2 border-dashed border-helios-line/10 rounded-2xl bg-helios-surface/30">
                        <FolderOpen className="text-helios-slate/20 mb-2" size={32} />
                        <p className="text-sm text-helios-slate">No watch folders configured</p>
                        <p className="text-xs text-helios-slate/60 mt-1">Add a directory to start scanning</p>
                    </div>
                )}

                {loading && (
                    <div className="text-center py-8 text-helios-slate animate-pulse text-sm">
                        Loading directories...
                    </div>
                )}
            </div>

            <ConfirmDialog
                open={pendingRemoveId !== null}
                title="Stop watching folder"
                description="Stop watching this folder for new media?"
                confirmLabel="Stop Watching"
                tone="danger"
                onClose={() => setPendingRemoveId(null)}
                onConfirm={async () => {
                    if (pendingRemoveId === null) return;
                    await removeDir(pendingRemoveId);
                }}
            />

            <ServerDirectoryPicker
                open={pickerOpen !== null}
                title={pickerOpen === "library" ? "Select Library Root" : "Select Extra Watch Folder"}
                description={
                    pickerOpen === "library"
                        ? "Choose a canonical server folder that represents a media library root."
                        : "Choose an additional server folder to watch outside the canonical library roots."
                }
                onClose={() => setPickerOpen(null)}
                onSelect={(selectedPath) => {
                    if (pickerOpen === "library") {
                        setLibraryPath(selectedPath);
                    } else {
                        setPath(selectedPath);
                    }
                    setPickerOpen(null);
                }}
            />
        </div>
    );
}
