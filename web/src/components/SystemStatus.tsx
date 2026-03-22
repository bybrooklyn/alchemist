import { useEffect, useId, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Activity, X, Zap, CheckCircle2, AlertTriangle, Database } from "lucide-react";
import { useSharedStats } from "../lib/statsStore";

function focusables(root: HTMLElement): HTMLElement[] {
    const selector = [
        "a[href]",
        "button:not([disabled])",
        "input:not([disabled])",
        "select:not([disabled])",
        "textarea:not([disabled])",
        "[tabindex]:not([tabindex='-1'])",
    ].join(",");

    return Array.from(root.querySelectorAll<HTMLElement>(selector));
}

export default function SystemStatus() {
    const { stats, error } = useSharedStats();
    const [isExpanded, setIsExpanded] = useState(false);
    const layoutId = useId();
    const modalRef = useRef<HTMLDivElement | null>(null);
    const closeRef = useRef<HTMLButtonElement | null>(null);
    const lastFocusedRef = useRef<HTMLElement | null>(null);

    useEffect(() => {
        if (!isExpanded) {
            return;
        }

        lastFocusedRef.current = document.activeElement as HTMLElement | null;
        closeRef.current?.focus();

        const onKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.preventDefault();
                setIsExpanded(false);
                return;
            }
            if (event.key !== "Tab") {
                return;
            }

            const root = modalRef.current;
            if (!root) {
                return;
            }

            const list = focusables(root);
            if (list.length === 0) {
                return;
            }

            const first = list[0];
            const last = list[list.length - 1];
            const current = document.activeElement as HTMLElement | null;

            if (event.shiftKey && current === first) {
                event.preventDefault();
                last.focus();
            } else if (!event.shiftKey && current === last) {
                event.preventDefault();
                first.focus();
            }
        };

        document.addEventListener("keydown", onKeyDown);
        return () => {
            document.removeEventListener("keydown", onKeyDown);
            lastFocusedRef.current?.focus();
        };
    }, [isExpanded]);

    if (!stats) {
        return (
            <div className="flex items-center justify-between mb-2" aria-live="polite">
                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">
                    {error ? "Status Unavailable" : "Loading Status..."}
                </span>
                <span className={`w-2 h-2 rounded-full ${error ? "bg-red-500/70" : "bg-helios-slate/50 animate-pulse"}`}></span>
            </div>
        );
    }

    const isActive = stats.active > 0;
    const isFull = stats.active >= stats.concurrent_limit;
    const percentage = Math.min((stats.active / stats.concurrent_limit) * 100, 100);

    return (
        <>
            <motion.div
                layoutId={layoutId}
                onClick={() => setIsExpanded(true)}
                className="flex flex-col gap-3 cursor-pointer group p-4 rounded-xl bg-helios-surface-soft border border-helios-line/40 shadow-sm"
                whileHover={{ scale: 1.02 }}
                transition={{ type: "spring", stiffness: 300, damping: 30 }}
            >
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <span className="relative flex h-2 w-2">
                            <span className={`animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 ${isActive ? "bg-status-success" : "bg-helios-slate"}`}></span>
                            <span className={`relative inline-flex rounded-full h-2 w-2 ${isActive ? "bg-status-success" : "bg-helios-slate"}`}></span>
                        </span>
                        <span className="text-xs font-bold text-helios-slate uppercase tracking-wider group-hover:text-helios-ink transition-colors">Engine Status</span>
                    </div>
                    <div className={`text-[10px] font-bold px-1.5 py-0.5 rounded-md ${isActive ? "bg-status-success/10 text-status-success" : "bg-helios-slate/10 text-helios-slate"}`}>
                        {isActive ? "ONLINE" : "IDLE"}
                    </div>
                </div>

                <div className="space-y-1.5">
                    <div className="flex items-end justify-between text-helios-ink">
                        <span className="text-xs font-medium opacity-80">Active Jobs</span>
                        <div className="flex items-baseline gap-0.5">
                            <span className={`text-lg font-bold ${isFull ? "text-status-warning" : "text-helios-solar"}`}>
                                {stats.active}
                            </span>
                            <span className="text-xs text-helios-slate">/ {stats.concurrent_limit}</span>
                        </div>
                    </div>

                    <div className="h-1.5 w-full bg-helios-line/20 rounded-full overflow-hidden relative">
                        <div
                            className={`h-full transition-all duration-700 ease-out rounded-full ${isFull ? "bg-status-warning" : "bg-helios-solar"}`}
                            style={{ width: `${percentage}%` }}
                        />
                        {Array.from({ length: stats.concurrent_limit }).map((_, i) => (
                            <div key={i} className="absolute top-0 bottom-0 w-[1px] bg-helios-main/20" style={{ left: `${((i + 1) / stats.concurrent_limit) * 100}%` }} />
                        ))}
                    </div>
                </div>
            </motion.div>

            <AnimatePresence>
                {isExpanded && (
                    <>
                        <motion.div
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            exit={{ opacity: 0 }}
                            onClick={() => setIsExpanded(false)}
                            className="fixed inset-0 z-50 bg-black/60 backdrop-blur-md flex items-center justify-center p-4"
                        >
                            <motion.div
                                ref={modalRef}
                                role="dialog"
                                aria-modal="true"
                                aria-labelledby="system-status-title"
                                layoutId={layoutId}
                                className="w-full max-w-lg bg-helios-surface border border-helios-line/30 rounded-xl shadow-2xl overflow-hidden relative outline-none"
                                onClick={(e) => e.stopPropagation()}
                                tabIndex={-1}
                            >
                                <div className="absolute top-0 left-0 w-full h-32 bg-gradient-to-b from-helios-solar/10 to-transparent pointer-events-none" />

                                <div className="p-8 relative">
                                    <div className="flex items-center justify-between mb-8">
                                        <div className="flex items-center gap-3">
                                            <div className="p-2.5 bg-helios-surface-soft rounded-xl border border-helios-line/20 shadow-sm">
                                                <Activity className="text-helios-solar" size={24} />
                                            </div>
                                            <div>
                                                <h2 id="system-status-title" className="text-xl font-bold text-helios-ink tracking-tight">System Status</h2>
                                                <div className="flex items-center gap-2 mt-0.5">
                                                    <span className={`w-1.5 h-1.5 rounded-full ${isActive ? "bg-status-success" : "bg-helios-slate"}`}></span>
                                                    <span className="text-xs font-medium text-helios-slate uppercase tracking-wide">
                                                        {isActive ? "Engine Running" : "Engine Idle"}
                                                    </span>
                                                </div>
                                            </div>
                                        </div>
                                        <button
                                            ref={closeRef}
                                            onClick={() => setIsExpanded(false)}
                                            className="p-2 hover:bg-helios-surface-soft rounded-full text-helios-slate hover:text-helios-ink transition-colors"
                                            aria-label="Close system status dialog"
                                        >
                                            <X size={20} />
                                        </button>
                                    </div>

                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-8">
                                        <div className="bg-helios-surface-soft/50 rounded-lg p-5 border border-helios-line/10 flex flex-col items-center text-center gap-2">
                                            <Zap size={20} className="text-helios-solar opacity-80" />
                                            <div className="flex flex-col">
                                                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">Concurrency</span>
                                                <div className="flex items-baseline justify-center gap-1 mt-1">
                                                    <span className="text-3xl font-bold text-helios-ink">{stats.active}</span>
                                                    <span className="text-sm font-medium text-helios-slate opacity-60">/ {stats.concurrent_limit}</span>
                                                </div>
                                            </div>
                                            <div className="w-full h-2 bg-helios-line/10 rounded-full mt-2 overflow-hidden relative">
                                                <div
                                                    className={`h-full rounded-full ${isFull ? "bg-status-warning" : "bg-helios-solar"}`}
                                                    style={{ width: `${percentage}%` }}
                                                />
                                            </div>
                                        </div>

                                        <div className="bg-helios-surface-soft/50 rounded-lg p-5 border border-helios-line/10 flex flex-col items-center text-center gap-2">
                                            <Database size={20} className="text-blue-400 opacity-80" />
                                            <div className="flex flex-col">
                                                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">Total Jobs</span>
                                                <span className="text-3xl font-bold text-helios-ink mt-1">{stats.total}</span>
                                            </div>
                                            <div className="text-[10px] text-helios-slate mt-1 px-2 py-0.5 bg-helios-line/10 rounded-md">Lifetime</div>
                                        </div>
                                    </div>

                                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                                        <div className="p-3 rounded-xl bg-status-success/5 border border-status-success/10 flex flex-col items-center justify-center text-center">
                                            <CheckCircle2 size={16} className="text-status-success mb-1" />
                                            <span className="text-lg font-bold text-helios-ink">{stats.completed}</span>
                                            <span className="text-[10px] font-bold text-status-success uppercase tracking-wider">Completed</span>
                                        </div>

                                        <div className="p-3 rounded-xl bg-status-error/5 border border-status-error/10 flex flex-col items-center justify-center text-center">
                                            <AlertTriangle size={16} className="text-status-error mb-1" />
                                            <span className="text-lg font-bold text-helios-ink">{stats.failed}</span>
                                            <span className="text-[10px] font-bold text-status-error uppercase tracking-wider">Failed</span>
                                        </div>

                                        <div className="p-3 rounded-xl bg-helios-surface-soft border border-helios-line/10 flex flex-col items-center justify-center text-center opacity-60">
                                            <Activity size={16} className="text-helios-slate mb-1" />
                                            <span className="text-lg font-bold text-helios-ink">--</span>
                                            <span className="text-[10px] font-bold text-helios-slate uppercase tracking-wider">Est. Time</span>
                                        </div>
                                    </div>

                                    <div className="mt-6 pt-6 border-t border-helios-line/10 text-center">
                                        <p className="text-xs text-helios-slate/60">System metrics update automatically while this tab is active.</p>
                                    </div>
                                </div>
                            </motion.div>
                        </motion.div>
                    </>
                )}
            </AnimatePresence>
        </>
    );
}
