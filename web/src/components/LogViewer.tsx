import { useEffect, useRef, useState } from "react";
import { Terminal, Pause, Play, Trash2, RefreshCw, Search } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface LogEntry {
    id: number;
    level: string;
    job_id?: number;
    message: string;
    created_at: string;
}

export default function LogViewer() {
    const [logs, setLogs] = useState<LogEntry[]>([]);
    const [paused, setPaused] = useState(false);
    const [loading, setLoading] = useState(true);
    const [streamError, setStreamError] = useState<string | null>(null);
    const [confirmClear, setConfirmClear] = useState(false);
    const [query, setQuery] = useState("");
    const [levelFilter, setLevelFilter] = useState<"all" | "info" | "warn" | "error">("all");

    const scrollRef = useRef<HTMLDivElement>(null);
    const pausedRef = useRef(paused);
    const reconnectTimeoutRef = useRef<number | null>(null);
    const maxLogs = 1000;

    useEffect(() => {
        pausedRef.current = paused;
    }, [paused]);

    const fetchHistory = async () => {
        setLoading(true);
        try {
            const history = await apiJson<LogEntry[]>("/api/logs/history?limit=200");
            setLogs(history.reverse());
            setStreamError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch logs";
            setStreamError(message);
        } finally {
            setLoading(false);
        }
    };

    const clearLogs = async () => {
        try {
            await apiAction("/api/logs", { method: "DELETE" });
            setLogs([]);
            showToast({ kind: "success", title: "Logs", message: "Server logs cleared." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to clear logs";
            showToast({ kind: "error", title: "Logs", message });
        }
    };

    useEffect(() => {
        void fetchHistory();

        let eventSource: EventSource | null = null;
        let cancelled = false;

        const connect = () => {
            if (cancelled) return;

            setStreamError(null);
            eventSource?.close();
            eventSource = new EventSource("/api/events");

            const appendLog = (message: string, level: string, jobId?: number) => {
                if (pausedRef.current) {
                    return;
                }

                const entry: LogEntry = {
                    id: Date.now() + Math.random(),
                    level,
                    message,
                    job_id: jobId,
                    created_at: new Date().toISOString(),
                };

                setLogs((prev) => {
                    const next = [...prev, entry];
                    if (next.length > maxLogs) {
                        return next.slice(next.length - maxLogs);
                    }
                    return next;
                });
            };

            eventSource.addEventListener("log", (event) => {
                const data = event.data;
                try {
                    const parsed = JSON.parse(data) as { message?: string; level?: string; job_id?: number };
                    if (parsed.message) {
                        appendLog(parsed.message, parsed.level ?? "info", parsed.job_id);
                        return;
                    }
                } catch {
                    // Fall back to plain text handling.
                }

                appendLog(data, data.toLowerCase().includes("error") ? "error" : "info");
            });

            eventSource.addEventListener("decision", (event) => {
                try {
                    const data = JSON.parse(event.data) as { action: string; reason: string; job_id?: number };
                    appendLog(`Decision: ${data.action.toUpperCase()} - ${data.reason}`, "info", data.job_id);
                } catch {
                    // Ignore malformed SSE payload.
                }
            });

            eventSource.addEventListener("status", (event) => {
                try {
                    const data = JSON.parse(event.data) as { status: string; job_id?: number };
                    appendLog(`Status changed to ${data.status}`, "info", data.job_id);
                } catch {
                    // Ignore malformed SSE payload.
                }
            });

            eventSource.onerror = () => {
                eventSource?.close();
                eventSource = null;
                setStreamError("Log stream unavailable. Reconnecting…");

                if (reconnectTimeoutRef.current !== null) {
                    window.clearTimeout(reconnectTimeoutRef.current);
                }
                reconnectTimeoutRef.current = window.setTimeout(connect, 3000);
            };
        };

        connect();
        return () => {
            cancelled = true;
            if (reconnectTimeoutRef.current !== null) {
                window.clearTimeout(reconnectTimeoutRef.current);
                reconnectTimeoutRef.current = null;
            }
            eventSource?.close();
        };
    }, []);

    useEffect(() => {
        if (!paused && scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [logs, paused]);

    const formatTime = (iso: string) => {
        try {
            return new Date(iso).toLocaleTimeString();
        } catch {
            return iso;
        }
    };

    const filteredLogs = logs.filter((log) => {
        const level = log.level.toLowerCase();
        if (levelFilter !== "all" && !level.includes(levelFilter)) {
            return false;
        }
        if (!query.trim()) {
            return true;
        }
        const haystack = `${log.message} ${log.job_id ?? ""}`.toLowerCase();
        return haystack.includes(query.trim().toLowerCase());
    });

    return (
        <div className="flex flex-col h-full rounded-2xl border border-helios-line/40 bg-[#0d1117] overflow-hidden shadow-2xl">
            <div className="flex items-center justify-between px-4 py-3 border-b border-helios-line/20 bg-helios-surface/50 backdrop-blur">
                <div className="flex items-center gap-2 text-helios-slate" aria-live="polite">
                    <Terminal size={16} />
                    <span className="text-xs font-bold uppercase tracking-widest">Server Logs</span>
                    {loading && <span className="text-xs animate-pulse opacity-50 ml-2">Loading history...</span>}
                </div>
                <div className="flex items-center gap-2">
                    <button
                        onClick={() => void fetchHistory()}
                        className="p-1.5 rounded-lg hover:bg-helios-line/10 text-helios-slate transition-colors"
                        title="Reload History"
                    >
                        <RefreshCw size={14} />
                    </button>
                    <button
                        onClick={() => setPaused(!paused)}
                        className="p-1.5 rounded-lg hover:bg-helios-line/10 text-helios-slate transition-colors"
                        title={paused ? "Resume Auto-scroll" : "Pause Auto-scroll"}
                    >
                        {paused ? <Play size={14} /> : <Pause size={14} />}
                    </button>
                    <button
                        onClick={() => setConfirmClear(true)}
                        className="p-1.5 rounded-lg hover:bg-red-500/10 text-helios-slate hover:text-red-400 transition-colors"
                        title="Clear Server Logs"
                    >
                        <Trash2 size={14} />
                    </button>
                </div>
            </div>

            <div className="border-b border-helios-line/10 bg-helios-surface/30 px-4 py-3 flex flex-col md:flex-row gap-3">
                <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-helios-slate" size={14} />
                    <input
                        type="text"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder="Filter logs by text or job id…"
                        className="w-full rounded-lg border border-helios-line/20 bg-helios-surface px-9 py-2 text-sm text-helios-ink focus:border-helios-solar outline-none"
                    />
                </div>
                <select
                    value={levelFilter}
                    onChange={(e) => setLevelFilter(e.target.value as "all" | "info" | "warn" | "error")}
                    className="rounded-lg border border-helios-line/20 bg-helios-surface px-4 py-2 text-sm text-helios-ink focus:border-helios-solar outline-none"
                >
                    <option value="all">All Levels</option>
                    <option value="info">Info</option>
                    <option value="warn">Warnings</option>
                    <option value="error">Errors</option>
                </select>
            </div>

            <div
                ref={scrollRef}
                className="flex-1 overflow-y-auto p-4 font-mono text-xs space-y-1 scrollbar-thin scrollbar-thumb-helios-line/20 scrollbar-track-transparent"
                aria-live="polite"
            >
                {streamError && <div className="text-amber-400 text-center py-4 text-[11px] font-semibold">{streamError}</div>}
                {filteredLogs.length === 0 && !loading && !streamError && (
                    <div className="text-helios-slate/30 text-center py-10 italic">No logs found.</div>
                )}
                {filteredLogs.map((log) => (
                    <div key={log.id} className="flex gap-3 hover:bg-white/5 px-2 py-0.5 rounded -mx-2 group">
                        <span className="text-helios-slate/50 shrink-0 select-none w-20 text-right">{formatTime(log.created_at)}</span>

                        <div className="flex-1 min-w-0 break-all">
                            {log.job_id && (
                                <span className="inline-block px-1.5 py-0.5 rounded bg-white/5 text-helios-slate/80 mr-2 text-[10px]">
                                    #{log.job_id}
                                </span>
                            )}
                            <span
                                className={cn(
                                    log.level.toLowerCase().includes("error")
                                        ? "text-red-400 font-bold"
                                        : log.level.toLowerCase().includes("warn")
                                            ? "text-amber-400"
                                            : "text-white/90"
                                )}
                            >
                                {log.message}
                            </span>
                        </div>
                    </div>
                ))}
            </div>

            <ConfirmDialog
                open={confirmClear}
                title="Clear server logs"
                description="Delete all stored server logs?"
                confirmLabel="Clear Logs"
                tone="danger"
                onClose={() => setConfirmClear(false)}
                onConfirm={clearLogs}
            />
        </div>
    );
}
