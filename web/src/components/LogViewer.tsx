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

            eventSource.addEventListener("lagged", () => {
                showToast({
                    kind: "warning",
                    title: "Connection interrupted",
                    message: "Refreshing data…",
                });
                void fetchHistory();
                eventSource?.close();
                eventSource = null;
                if (reconnectTimeoutRef.current !== null) {
                    window.clearTimeout(reconnectTimeoutRef.current);
                }
                reconnectTimeoutRef.current = window.setTimeout(connect, 100);
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

    type LogGroup =
        | { kind: "system"; entries: LogEntry[] }
        | { kind: "job"; job_id: number; entries: LogEntry[] };

    const groupedLogs = (() => {
        const groups: LogGroup[] = [];
        const currentJobGroup: Map<number, { entries: LogEntry[]; index: number }> = new Map();

        for (const entry of filteredLogs) {
            if (!entry.job_id) {
                const last = groups[groups.length - 1];
                if (last?.kind === "system") {
                    last.entries.push(entry);
                } else {
                    groups.push({
                        kind: "system",
                        entries: [entry],
                    });
                }
            } else {
                if (!currentJobGroup.has(entry.job_id)) {
                    const group: LogGroup = {
                        kind: "job",
                        job_id: entry.job_id,
                        entries: [],
                    };
                    groups.push(group);
                    currentJobGroup.set(entry.job_id, {
                        entries: (group as Extract<LogGroup, { kind: "job" }>).entries,
                        index: groups.length - 1,
                    });
                }
                currentJobGroup.get(entry.job_id)!.entries.push(entry);
            }
        }
        return groups;
    })();

    const [expandedJobs, setExpandedJobs] = useState<Set<number>>(new Set());

    const toggleJob = (jobId: number) => {
        setExpandedJobs((prev) => {
            const next = new Set(prev);
            if (next.has(jobId)) next.delete(jobId);
            else next.add(jobId);
            return next;
        });
    };

    return (
        <div className="flex flex-col h-full rounded-lg border border-helios-line/40 bg-[#0d1117] overflow-hidden">

            {/* Toolbar */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-helios-line/20 bg-helios-surface/50 shrink-0">
                <div className="flex items-center gap-2 text-helios-slate">
                    <Terminal size={15} />
                    <span className="text-xs font-semibold text-helios-slate">
                        Server Logs
                    </span>
                    {loading && (
                        <span className="text-xs animate-pulse opacity-50 ml-1">
                            Loading…
                        </span>
                    )}
                </div>
                <div className="flex items-center gap-1">
                    <button
                        onClick={() => void fetchHistory()}
                        className="p-1.5 rounded-lg hover:bg-helios-line/10 text-helios-slate transition-colors"
                        title="Reload History"
                    >
                        <RefreshCw size={13} />
                    </button>
                    <button
                        onClick={() => setPaused(!paused)}
                        className="p-1.5 rounded-lg hover:bg-helios-line/10 text-helios-slate transition-colors"
                        title={paused ? "Resume Auto-scroll" : "Pause Auto-scroll"}
                    >
                        {paused ? <Play size={13} /> : <Pause size={13} />}
                    </button>
                    <button
                        onClick={() => setConfirmClear(true)}
                        className="p-1.5 rounded-lg hover:bg-status-error/10 text-helios-slate hover:text-status-error transition-colors"
                        title="Clear Server Logs"
                    >
                        <Trash2 size={13} />
                    </button>
                </div>
            </div>

            {/* Filters */}
            <div className="flex gap-3 px-4 py-2.5 border-b border-helios-line/10 bg-helios-surface/30 shrink-0">
                <div className="relative flex-1">
                    <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 text-helios-slate/50" size={13} />
                    <input
                        type="text"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        placeholder="Filter by text or job id…"
                        className="w-full rounded-lg border border-helios-line/30 bg-helios-surface px-8 py-1.5 text-xs text-helios-ink focus:border-helios-solar outline-none placeholder:text-helios-slate/40"
                    />
                </div>
                <select
                    value={levelFilter}
                    onChange={(e) => setLevelFilter(e.target.value as typeof levelFilter)}
                    className="rounded-lg border border-helios-line/30 bg-helios-surface px-3 py-1.5 text-xs text-helios-ink focus:border-helios-solar outline-none"
                >
                    <option value="all">All levels</option>
                    <option value="info">Info</option>
                    <option value="warn">Warnings</option>
                    <option value="error">Errors</option>
                </select>
            </div>

            {/* Log content */}
            <div
                ref={scrollRef}
                className="flex-1 overflow-y-auto"
                aria-live="polite"
            >
                {streamError && (
                    <div className="text-amber-400 text-center py-4 text-xs px-4">
                        {streamError}
                    </div>
                )}

                {groupedLogs.length === 0 && !loading && !streamError && (
                    <div className="text-helios-slate/30 text-center py-12 text-xs italic">
                        No logs found.
                    </div>
                )}

                {groupedLogs.map((group, gi) => {
                    if (group.kind === "system") {
                        return (
                            <div key={`sys-${gi}`} className="font-mono text-xs">
                                {group.entries.map((log) => (
                                    <div key={log.id} className="flex gap-3 px-4 py-0.5 hover:bg-helios-line/10 group">
                                        <span className="text-helios-slate/40 shrink-0 w-20 text-right select-none tabular-nums">
                                            {formatTime(log.created_at)}
                                        </span>
                                        <span className={cn(
                                            "flex-1 min-w-0 break-all",
                                            log.level.toLowerCase().includes("error")
                                                ? "text-status-error"
                                            : log.level.toLowerCase().includes("warn")
                                                ? "text-amber-400"
                                            : "text-white/90"
                                        )}>
                                            {log.message}
                                        </span>
                                    </div>
                                ))}
                            </div>
                        );
                    }

                    const isExpanded = expandedJobs.has(group.job_id);
                    const hasError = group.entries.some((e) =>
                        e.level.toLowerCase().includes("error")
                    );
                    const firstMsg = group.entries[0]?.message ?? "";

                    return (
                        <div key={`job-${group.job_id}`} className="border-b border-helios-line/10">
                            <button
                                onClick={() => toggleJob(group.job_id)}
                                className="w-full flex items-center gap-3 px-4 py-2 hover:bg-helios-line/10 transition-colors text-left"
                            >
                                <span className={cn(
                                    "text-xs font-mono shrink-0",
                                    "text-helios-slate/50"
                                )}>
                                    {isExpanded ? "▾" : "▸"}
                                </span>
                                <span className="inline-flex items-center px-1.5 py-0.5 rounded bg-helios-line/20 text-helios-slate/80 font-mono text-xs shrink-0">
                                    #{group.job_id}
                                </span>
                                {hasError && (
                                    <span className="w-1.5 h-1.5 rounded-full bg-status-error shrink-0" />
                                )}
                                <span className="text-xs text-helios-slate/70 truncate font-mono flex-1 min-w-0">
                                    {firstMsg}
                                </span>
                                <span className="text-xs text-helios-slate/40 shrink-0">
                                    {group.entries.length} lines
                                </span>
                            </button>

                            {isExpanded && (
                                <div className="font-mono text-xs bg-helios-surface/20 pb-1">
                                    {group.entries.map((log) => (
                                        <div key={log.id} className="flex gap-3 px-10 py-0.5 hover:bg-helios-line/10">
                                            <span className="text-helios-slate/40 shrink-0 w-20 text-right select-none tabular-nums">
                                                {formatTime(log.created_at)}
                                            </span>
                                            <span className={cn(
                                                "flex-1 min-w-0 break-all",
                                                log.level.toLowerCase().includes("error")
                                                    ? "text-status-error"
                                                : log.level.toLowerCase().includes("warn")
                                                    ? "text-amber-400"
                                                : "text-white/90"
                                            )}>
                                                {log.message}
                                            </span>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    );
                })}
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
