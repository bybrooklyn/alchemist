import { useState, useEffect } from "react";
import { FolderOpen, Trash2, Plus, Folder, Play } from "lucide-react";
import { apiFetch } from "../lib/api";

interface WatchDir {
    id: number;
    path: string;
    is_recursive: boolean;
}

export default function WatchFolders() {
    const [dirs, setDirs] = useState<WatchDir[]>([]);
    const [path, setPath] = useState("");
    const [loading, setLoading] = useState(true);
    const [scanning, setScanning] = useState(false);

    const fetchDirs = async () => {
        try {
            const res = await apiFetch("/api/settings/watch-dirs");
            if (res.ok) {
                const data = await res.json();
                setDirs(data);
            }
        } catch (e) {
            console.error("Failed to fetch watch dirs", e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchDirs();
    }, []);

    const triggerScan = async () => {
        setScanning(true);
        try {
            await apiFetch("/api/scan/start", { method: "POST" });
        } catch (e) {
            console.error("Failed to start scan", e);
        } finally {
            setTimeout(() => setScanning(false), 2000);
        }
    };

    const addDir = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!path.trim()) return;

        try {
            const res = await apiFetch("/api/settings/watch-dirs", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ path: path.trim(), is_recursive: true })
            });

            if (res.ok) {
                setPath("");
                fetchDirs();
            }
        } catch (e) {
            console.error("Failed to add directory", e);
        }
    };

    const removeDir = async (id: number) => {
        if (!confirm("Stop watching this folder?")) return;

        try {
            const res = await apiFetch(`/api/settings/watch-dirs/${id}`, {
                method: "DELETE"
            });

            if (res.ok) {
                fetchDirs();
            }
        } catch (e) {
            console.error("Failed to remove directory", e);
        }
    };

    return (
        <div className="space-y-6">
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
                    onClick={triggerScan}
                    disabled={scanning}
                    className="flex items-center gap-2 px-3 py-1.5 bg-helios-solar/10 hover:bg-helios-solar/20 text-helios-solar rounded-lg text-xs font-bold uppercase tracking-wider transition-colors disabled:opacity-50"
                >
                    <Play size={14} className={scanning ? "animate-spin" : ""} />
                    {scanning ? "Scanning..." : "Scan Now"}
                </button>
            </div>

            <form onSubmit={addDir} className="flex gap-2">
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
                    type="submit"
                    disabled={!path.trim()}
                    className="bg-helios-solar hover:bg-helios-solar-dark text-helios-surface px-5 py-2.5 rounded-xl font-medium text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 shadow-sm shadow-helios-solar/20"
                >
                    <Plus size={16} /> Add
                </button>
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
                        </div>
                        <button
                            onClick={() => removeDir(dir.id)}
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
        </div>
    );
}
