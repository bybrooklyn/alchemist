import { useEffect, useRef, useState } from "react";
import { Terminal, Pause, Play, Trash2 } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface LogEntry {
    id: number;
    timestamp: string;
    message: string;
    level: "info" | "warn" | "error" | "debug";
}

export default function LogViewer() {
    const [logs, setLogs] = useState<LogEntry[]>([]);
    const [paused, setPaused] = useState(false);
    const scrollRef = useRef<HTMLDivElement>(null);
    const maxLogs = 1000;

    useEffect(() => {
        let eventSource: EventSource | null = null;

        const connect = () => {
            eventSource = new EventSource("/api/events");

            eventSource.addEventListener("log", (e) => {
                if (paused) return; // Note: This drops logs while paused. Alternatively we could buffer.
                // But usually "pause" implies "stop scrolling me".
                // For now, let's keep accumulating but not auto-scroll? 
                // Or truly ignore updates. Let's buffer/append always effectively, but control scroll?
                // Simpler: Just append functionality is standard. "Pause" usually means "Pause updates".

                // Actually, if we are paused, we shouldn't update state to avoid re-renders/shifting.
                if (paused) return;

                const message = e.data;
                const level = message.toLowerCase().includes("error")
                    ? "error"
                    : message.toLowerCase().includes("warn")
                        ? "warn"
                        : "info";

                addLog({
                    id: Date.now() + Math.random(),
                    timestamp: new Date().toLocaleTimeString(),
                    message,
                    level
                });
            });

            // Also listen for other events to show interesting activity
            eventSource.addEventListener("decision", (e) => {
                if (paused) return;
                try {
                    const data = JSON.parse(e.data);
                    addLog({
                        id: Date.now() + Math.random(),
                        timestamp: new Date().toLocaleTimeString(),
                        message: `Decision: ${data.action.toUpperCase()} Job #${data.job_id} - ${data.reason}`,
                        level: "info"
                    });
                } catch { }
            });

            eventSource.addEventListener("job_status", (e) => {
                if (paused) return;
                try {
                    const data = JSON.parse(e.data);
                    addLog({
                        id: Date.now() + Math.random(),
                        timestamp: new Date().toLocaleTimeString(),
                        message: `Job #${data.job_id} status changed to ${data.status}`,
                        level: "info"
                    });
                } catch { }
            });

            eventSource.onerror = (e) => {
                console.error("SSE Error", e);
                eventSource?.close();
                // Reconnect after delay
                setTimeout(connect, 5000);
            };
        };

        connect();

        return () => {
            eventSource?.close();
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [paused]); // Re-connecting on pause toggle is inefficient. Better to use ref for paused state inside callback.

    // Correction: Use ref for paused state to avoid reconnecting.
    const pausedRef = useRef(paused);
    useEffect(() => { pausedRef.current = paused; }, [paused]);

    // Re-implement effect with ref
    useEffect(() => {
        let eventSource: EventSource | null = null;
        const connect = () => {
            const token = localStorage.getItem('alchemist_token') || '';
            eventSource = new EventSource(`/api/events?token=${token}`);
            const handleMsg = (msg: string, level: "info" | "warn" | "error" = "info") => {
                if (pausedRef.current) return;
                setLogs(prev => {
                    const newLogs = [...prev, {
                        id: Date.now() + Math.random(),
                        timestamp: new Date().toLocaleTimeString(),
                        message: msg,
                        level
                    }];
                    if (newLogs.length > maxLogs) return newLogs.slice(newLogs.length - maxLogs);
                    return newLogs;
                });
            };

            eventSource.addEventListener("log", (e) => handleMsg(e.data, e.data.toLowerCase().includes("warn") ? "warn" : e.data.toLowerCase().includes("error") ? "error" : "info"));
            eventSource.addEventListener("decision", (e) => {
                try { const d = JSON.parse(e.data); handleMsg(`Decision: ${d.action.toUpperCase()} Job #${d.job_id} - ${d.reason}`); } catch { }
            });
            eventSource.addEventListener("status", (e) => { // NOTE: "status" event name in server.rs is "status", not "job_status" in one place? server.rs:376 says "status"
                try { const d = JSON.parse(e.data); handleMsg(`Job #${d.job_id} is now ${d.status}`); } catch { }
            });

            eventSource.onerror = () => { eventSource?.close(); setTimeout(connect, 3000); };
        };
        connect();
        return () => eventSource?.close();
    }, []);

    // Auto-scroll
    useEffect(() => {
        if (!paused && scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [logs, paused]);


    const addLog = (log: LogEntry) => {
        setLogs(prev => [...prev.slice(-999), log]);
    };

    return (
        <div className="flex flex-col h-full rounded-2xl border border-helios-line/40 bg-[#0d1117] overflow-hidden shadow-2xl">
            <div className="flex items-center justify-between px-4 py-3 border-b border-helios-line/20 bg-helios-surface/50 backdrop-blur">
                <div className="flex items-center gap-2 text-helios-slate">
                    <Terminal size={16} />
                    <span className="text-xs font-bold uppercase tracking-widest">System Logs</span>
                </div>
                <div className="flex items-center gap-2">
                    <button
                        onClick={() => setPaused(!paused)}
                        className="p-1.5 rounded-lg hover:bg-helios-line/10 text-helios-slate transition-colors"
                        title={paused ? "Resume Auto-scroll" : "Pause Auto-scroll"}
                    >
                        {paused ? <Play size={14} /> : <Pause size={14} />}
                    </button>
                    <button
                        onClick={() => setLogs([])}
                        className="p-1.5 rounded-lg hover:bg-red-500/10 text-helios-slate hover:text-red-400 transition-colors"
                        title="Clear Logs"
                    >
                        <Trash2 size={14} />
                    </button>
                </div>
            </div>

            <div
                ref={scrollRef}
                className="flex-1 overflow-y-auto p-4 font-mono text-xs space-y-1 scrollbar-thin scrollbar-thumb-helios-line/20 scrollbar-track-transparent"
            >
                {logs.length === 0 && (
                    <div className="text-helios-slate/30 text-center py-10 italic">Waiting for events...</div>
                )}
                {logs.map((log) => (
                    <div key={log.id} className="flex gap-3 hover:bg-white/5 px-2 py-0.5 rounded -mx-2">
                        <span className="text-helios-slate/50 shrink-0 select-none">{log.timestamp}</span>
                        <span className={cn(
                            "break-all",
                            log.level === "error" ? "text-red-400 font-bold" :
                                log.level === "warn" ? "text-amber-400" :
                                    "text-helios-mist/80"
                        )}>
                            {log.message}
                        </span>
                    </div>
                ))}
            </div>
        </div>
    );
}
