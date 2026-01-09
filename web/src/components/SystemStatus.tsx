import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Activity, X, Zap, CheckCircle2, AlertTriangle, Database } from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface Stats {
    active: number;
    concurrent_limit: number;
    completed: number;
    failed: number;
    total: number;
}

export default function SystemStatus() {
    const [stats, setStats] = useState<Stats | null>(null);
    const [isExpanded, setIsExpanded] = useState(false);

    useEffect(() => {
        const fetchStats = async () => {
            try {
                const res = await fetch("/api/stats");
                if (res.ok) {
                    const data = await res.json();
                    setStats({
                        active: data.active || 0,
                        concurrent_limit: data.concurrent_limit || 1,
                        completed: data.completed || 0,
                        failed: data.failed || 0,
                        total: data.total || 0,
                    });
                }
            } catch (e) {
                console.error("Failed to fetch system status", e);
            }
        };

        fetchStats();
        const interval = setInterval(fetchStats, 5000);
        return () => clearInterval(interval);
    }, []);

    if (!stats) {
        return (
            <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">Loading Status...</span>
                <span className="w-2 h-2 rounded-full bg-helios-slate/50 animate-pulse"></span>
            </div>
        );
    }

    const isActive = stats.active > 0;
    const isFull = stats.active >= stats.concurrent_limit;
    const percentage = Math.min((stats.active / stats.concurrent_limit) * 100, 100);

    return (
        <>
            {/* Compact Sidebar View */}
            <motion.div
                layoutId="status-container"
                onClick={() => setIsExpanded(true)}
                className="flex flex-col gap-3 cursor-pointer group p-4 rounded-xl bg-helios-surface-soft border border-helios-line/40 shadow-sm"
                whileHover={{ scale: 1.02 }}
                transition={{ type: "spring", stiffness: 300, damping: 25 }}
            >
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <span className="relative flex h-2 w-2">
                            <span className={`animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 ${isActive ? 'bg-status-success' : 'bg-helios-slate'}`}></span>
                            <span className={`relative inline-flex rounded-full h-2 w-2 ${isActive ? 'bg-status-success' : 'bg-helios-slate'}`}></span>
                        </span>
                        <span className="text-xs font-bold text-helios-slate uppercase tracking-wider group-hover:text-helios-inc transition-colors">Engine Status</span>
                    </div>
                    <motion.div layoutId="status-badge" className={`text-[10px] font-bold px-1.5 py-0.5 rounded-md ${isActive ? 'bg-status-success/10 text-status-success' : 'bg-helios-slate/10 text-helios-slate'}`}>
                        {isActive ? 'ONLINE' : 'IDLE'}
                    </motion.div>
                </div>

                <div className="space-y-1.5">
                    <div className="flex items-end justify-between text-helios-ink">
                        <span className="text-xs font-medium opacity-80">Active Jobs</span>
                        <div className="flex items-baseline gap-0.5">
                            <motion.span layoutId="active-count" className={`text-lg font-bold ${isFull ? 'text-status-warning' : 'text-helios-solar'}`}>
                                {stats.active}
                            </motion.span>
                            <span className="text-xs text-helios-slate">
                                / {stats.concurrent_limit}
                            </span>
                        </div>
                    </div>

                    <div className="h-1.5 w-full bg-helios-line/20 rounded-full overflow-hidden relative">
                        <motion.div
                            layoutId="progress-bar"
                            className={`h-full transition-all duration-700 ease-out rounded-full ${isFull ? 'bg-status-warning' : 'bg-helios-solar'}`}
                            style={{ width: `${percentage}%` }}
                        />
                        {Array.from({ length: stats.concurrent_limit }).map((_, i) => (
                            <div key={i} className="absolute top-0 bottom-0 w-[1px] bg-helios-main/20" style={{ left: `${((i + 1) / stats.concurrent_limit) * 100}%` }} />
                        ))}
                    </div>
                </div>
            </motion.div>

            {/* Expanded Modal View */}
            <AnimatePresence>
                {isExpanded && (
                    <>
                        {/* Backdrop */}
                        <motion.div
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            exit={{ opacity: 0 }}
                            onClick={() => setIsExpanded(false)}
                            className="fixed inset-0 z-50 bg-black/60 backdrop-blur-md flex items-center justify-center p-4"
                        >
                            {/* Modal Card */}
                            <motion.div
                                layoutId="status-container"
                                className="w-full max-w-lg bg-helios-surface border border-helios-line/30 rounded-3xl shadow-2xl overflow-hidden relative"
                                onClick={(e) => e.stopPropagation()}
                            >
                                {/* Header Background Effect */}
                                <div className="absolute top-0 left-0 w-full h-32 bg-gradient-to-b from-helios-solar/10 to-transparent pointer-events-none" />

                                <div className="p-8 relative">
                                    <div className="flex items-center justify-between mb-8">
                                        <div className="flex items-center gap-3">
                                            <div className="p-2.5 bg-helios-surface-soft rounded-xl border border-helios-line/20 shadow-sm">
                                                <Activity className="text-helios-solar" size={24} />
                                            </div>
                                            <div>
                                                <h2 className="text-xl font-bold text-helios-ink tracking-tight">System Status</h2>
                                                <div className="flex items-center gap-2 mt-0.5">
                                                    <span className={`w-1.5 h-1.5 rounded-full ${isActive ? 'bg-status-success' : 'bg-helios-slate'}`}></span>
                                                    <span className="text-xs font-medium text-helios-slate uppercase tracking-wide">
                                                        {isActive ? 'Engine Running' : 'Engine Idle'}
                                                    </span>
                                                </div>
                                            </div>
                                        </div>
                                        <button
                                            onClick={() => setIsExpanded(false)}
                                            className="p-2 hover:bg-helios-surface-soft rounded-full text-helios-slate hover:text-helios-ink transition-colors"
                                        >
                                            <X size={20} />
                                        </button>
                                    </div>

                                    {/* Main Metrics Grid */}
                                    <div className="grid grid-cols-2 gap-4 mb-8">
                                        <div className="bg-helios-surface-soft/50 rounded-2xl p-5 border border-helios-line/10 flex flex-col items-center text-center gap-2">
                                            <Zap size={20} className="text-helios-solar opacity-80" />
                                            <div className="flex flex-col">
                                                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">Concurrency</span>
                                                <div className="flex items-baseline justify-center gap-1 mt-1">
                                                    <motion.span layoutId="active-count" className="text-3xl font-bold text-helios-ink">
                                                        {stats.active}
                                                    </motion.span>
                                                    <span className="text-sm font-medium text-helios-slate opacity-60">
                                                        / {stats.concurrent_limit}
                                                    </span>
                                                </div>
                                            </div>
                                            {/* Big Progress Bar */}
                                            <div className="w-full h-2 bg-helios-line/10 rounded-full mt-2 overflow-hidden relative">
                                                <motion.div
                                                    layoutId="progress-bar"
                                                    className={`h-full rounded-full ${isFull ? 'bg-status-warning' : 'bg-helios-solar'}`}
                                                    style={{ width: `${percentage}%` }}
                                                />
                                            </div>
                                        </div>

                                        <div className="bg-helios-surface-soft/50 rounded-2xl p-5 border border-helios-line/10 flex flex-col items-center text-center gap-2">
                                            <Database size={20} className="text-blue-400 opacity-80" />
                                            <div className="flex flex-col">
                                                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">Total Jobs</span>
                                                <span className="text-3xl font-bold text-helios-ink mt-1">
                                                    {stats.total}
                                                </span>
                                            </div>
                                            <div className="text-[10px] text-helios-slate mt-1 px-2 py-0.5 bg-helios-line/10 rounded-md">
                                                Lifetime
                                            </div>
                                        </div>
                                    </div>

                                    {/* Secondary Metrics Row */}
                                    <div className="grid grid-cols-3 gap-3">
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
                                        <p className="text-xs text-helios-slate/60">
                                            System metrics update automatically every 5 seconds.
                                        </p>
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
