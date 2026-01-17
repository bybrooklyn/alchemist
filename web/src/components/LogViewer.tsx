import { useEffect, useRef, useState } from "react";
import { Terminal, Pause, Play, Trash2, RefreshCw } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { apiFetch } from "../lib/api";

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
    const scrollRef = useRef<HTMLDivElement>(null);
    const pausedRef = useRef(paused);
    const reconnectTimeoutRef = useRef<number | null>(null);
    const maxLogs = 1000;

    // Sync ref
    useEffect(() => { pausedRef.current = paused; }, [paused]);

    const fetchHistory = async () => {
        setLoading(true);
        try {
            const res = await apiFetch("/api/logs/history?limit=200");
            if (res.ok) {
                const history = await res.json();
                // Logs come newest first (DESC), reverse for display
                setLogs(history.reverse());
            }
        } catch (e) {
            console.error("Failed to fetch logs", e);
        } finally {
            setLoading(false);
        }
    };

    const clearLogs = async () => {
        if (!confirm("Are you sure you want to clear all server logs?")) return;
        try {
            await apiFetch("/api/logs", { method: "DELETE" });
            setLogs([]);
        } catch (e) {
            console.error("Failed to clear logs", e);
        }
    };

    useEffect(() => {
        fetchHistory();

        let eventSource: EventSource | null = null;
        let cancelled = false;
        const connect = () => {
            if (cancelled) return;
            setStreamError(null);
            if (eventSource) {
                eventSource.close();
                eventSource = null;
            }
            eventSource = new EventSource('/api/events');

            const handleMsg = (msg: string, level: string, job_id?: number) => {
                if (pausedRef.current) return;

                const entry: LogEntry = {
                    id: Date.now() + Math.random(),
                    level,
                    message: msg,
                    job_id,
                    created_at: new Date().toISOString()
                };

                setLogs(prev => {
                    const newLogs = [...prev, entry];
                    if (newLogs.length > maxLogs) return newLogs.slice(newLogs.length - maxLogs);
                    return newLogs;
                });
            };

            eventSource.addEventListener("log", (e) => {
                try {
                    // Expecting simple text or JSON? 
                    // Backend sends AlchemistEvent::Log { level, job_id, message }
                    // But SSE serializer matches structure.
                    // Wait, existing SSE in server.rs sends plain text or JSON?
                    // Let's check server.rs sse_handler or Event impl.
                    // Assuming existing impl sends `data: message` for "log" event.
                    // But I added structured event in backend: AlchemistEvent::Log
                    // If server.rs uses `sse::Event::default().event("log").data(...)`

                    // Actually, I need to check `sse_handler` in `server.rs` to see what it sends.
                    // Assuming it sends JSON for structured events or adapts.
                    // If it used to send string, I should support string.
                    const data = e.data;
                    // Try parsing JSON first
                    try {
                        const json = JSON.parse(data);
                        if (json.message) {
                            handleMsg(json.message, json.level || "info", json.job_id);
                            return;
                        }
                    } catch { }

                    // Fallback to text
                    handleMsg(data, data.toLowerCase().includes("error") ? "error" : "info");
                } catch { }
            });

            eventSource.addEventListener("decision", (e) => {
                try { const d = JSON.parse(e.data); handleMsg(`Decision: ${d.action.toUpperCase()} - ${d.reason}`, "info", d.job_id); } catch { }
            });
            eventSource.addEventListener("status", (e) => {
                try { const d = JSON.parse(e.data); handleMsg(`Status changed to ${d.status}`, "info", d.job_id); } catch { }
            });

            eventSource.onerror = () => {
                eventSource?.close();
                eventSource = null;
                setStreamError("Log stream unavailable. Please check authentication.");
                if (reconnectTimeoutRef.current) {
                    window.clearTimeout(reconnectTimeoutRef.current);
                }
                reconnectTimeoutRef.current = window.setTimeout(connect, 3000);
            };
        };

        connect();
        return () => {
            cancelled = true;
            if (reconnectTimeoutRef.current) {
                window.clearTimeout(reconnectTimeoutRef.current);
                reconnectTimeoutRef.current = null;
            }
            eventSource?.close();
        };
    }, []);

    // Auto-scroll
    useEffect(() => {
        if (!paused && scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [logs, paused]);

    const formatTime = (iso: string) => {
        try {
            return new Date(iso).toLocaleTimeString();
        } catch { return iso; }
    };

    return (
        <div className="flex flex-col h-full rounded-2xl border border-helios-line/40 bg-[#0d1117] overflow-hidden shadow-2xl">
            <div className="flex items-center justify-between px-4 py-3 border-b border-helios-line/20 bg-helios-surface/50 backdrop-blur">
                <div className="flex items-center gap-2 text-helios-slate">
                    <Terminal size={16} />
                    <span className="text-xs font-bold uppercase tracking-widest">Server Logs</span>
                    {loading && <span className="text-xs animate-pulse opacity-50 ml-2">Loading history...</span>}
                </div>
                <div className="flex items-center gap-2">
                    <button
                        onClick={fetchHistory}
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
                        onClick={clearLogs}
                        className="p-1.5 rounded-lg hover:bg-red-500/10 text-helios-slate hover:text-red-400 transition-colors"
                        title="Clear Server Logs"
                    >
                        <Trash2 size={14} />
                    </button>
                </div>
            </div>

            <div
                ref={scrollRef}
                className="flex-1 overflow-y-auto p-4 font-mono text-xs space-y-1 scrollbar-thin scrollbar-thumb-helios-line/20 scrollbar-track-transparent"
            >
                {streamError && (
                    <div className="text-amber-400 text-center py-4 text-[11px] font-semibold">
                        {streamError}
                    </div>
                )}
                {logs.length === 0 && !loading && !streamError && (
                    <div className="text-helios-slate/30 text-center py-10 italic">No logs found.</div>
                )}
                {logs.map((log) => (
                    <div key={log.id} className="flex gap-3 hover:bg-white/5 px-2 py-0.5 rounded -mx-2 group">
                        <span className="text-helios-slate/50 shrink-0 select-none w-20 text-right">{formatTime(log.created_at)}</span>

                        <div className="flex-1 min-w-0 break-all">
                            {log.job_id && (
                                <span className="inline-block px-1.5 py-0.5 rounded bg-white/5 text-helios-slate/80 mr-2 text-[10px]">#{log.job_id}</span>
                            )}
                            <span className={cn(
                                log.level.toLowerCase().includes("error") ? "text-red-400 font-bold" :
                                    log.level.toLowerCase().includes("warn") ? "text-amber-400" :
                                        "text-white/90"
                            )}>
                                {log.message}
                            </span>
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
