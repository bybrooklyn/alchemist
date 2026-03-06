import { useEffect, useState } from "react";
import { apiJson, isApiError } from "../lib/api";
import { Activity, Cpu, HardDrive, Clock, Layers } from "lucide-react";
import { motion } from "framer-motion";

interface SystemResources {
    cpu_percent: number;
    memory_used_mb: number;
    memory_total_mb: number;
    memory_percent: number;
    uptime_seconds: number;
    active_jobs: number;
    concurrent_limit: number;
    cpu_count: number;
    gpu_utilization?: number;
    gpu_memory_percent?: number;
}

interface SystemSettings {
    monitoring_poll_interval: number;
}

const MIN_INTERVAL_MS = 500;
const MAX_INTERVAL_MS = 10000;

export default function ResourceMonitor() {
    const [stats, setStats] = useState<SystemResources | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [pollInterval, setPollInterval] = useState<number>(2000);

    useEffect(() => {
        void apiJson<SystemSettings>("/api/settings/system")
            .then((data) => {
                const seconds = Number(data?.monitoring_poll_interval);
                if (!Number.isFinite(seconds)) {
                    return;
                }
                const intervalMs = Math.round(seconds * 1000);
                setPollInterval(Math.min(MAX_INTERVAL_MS, Math.max(MIN_INTERVAL_MS, intervalMs)));
            })
            .catch(() => {
                // Keep default poll interval if settings endpoint is unavailable.
            });
    }, []);

    useEffect(() => {
        let timer: number | null = null;
        let mounted = true;

        const run = async () => {
            if (typeof document !== "undefined" && document.visibilityState === "hidden") {
                schedule(pollInterval * 3);
                return;
            }

            try {
                const data = await apiJson<SystemResources>("/api/system/resources");
                if (!mounted) {
                    return;
                }
                setStats(data);
                setError(null);
            } catch (err) {
                if (!mounted) {
                    return;
                }
                setError(isApiError(err) ? err.message : "Connection error");
            } finally {
                schedule(pollInterval);
            }
        };

        const schedule = (delayMs: number) => {
            if (!mounted) {
                return;
            }
            if (timer !== null) {
                window.clearTimeout(timer);
            }
            timer = window.setTimeout(() => {
                void run();
            }, delayMs);
        };

        const onVisibilityChange = () => {
            if (document.visibilityState === "visible") {
                void run();
            }
        };

        document.addEventListener("visibilitychange", onVisibilityChange);
        void run();

        return () => {
            mounted = false;
            document.removeEventListener("visibilitychange", onVisibilityChange);
            if (timer !== null) {
                window.clearTimeout(timer);
            }
        };
    }, [pollInterval]);

    const formatUptime = (seconds: number) => {
        const d = Math.floor(seconds / (3600 * 24));
        const h = Math.floor((seconds % (3600 * 24)) / 3600);
        const m = Math.floor((seconds % 3600) / 60);

        if (d > 0) return `${d}d ${h}h`;
        if (h > 0) return `${h}h ${m}m`;
        return `${m}m`;
    };

    const getUsageColor = (percent: number) => {
        if (percent > 90) return "text-red-500 bg-red-500/10";
        if (percent > 70) return "text-yellow-500 bg-yellow-500/10";
        return "text-green-500 bg-green-500/10";
    };

    const getBarColor = (percent: number) => {
        if (percent > 90) return "bg-red-500";
        if (percent > 70) return "bg-yellow-500";
        return "bg-green-500";
    };

    if (!stats) {
        return (
            <div className={`p-6 rounded-2xl bg-white/5 border border-white/10 h-48 flex items-center justify-center ${error ? "" : "animate-pulse"}`}>
                <div className="text-center" aria-live="polite">
                    <div className={`text-sm ${error ? "text-red-400" : "text-white/40"}`}>
                        {error ? "Unable to load system stats." : "Loading system stats..."}
                    </div>
                    {error && <div className="text-[10px] text-white/40 mt-2">{error} Retrying automatically...</div>}
                </div>
            </div>
        );
    }

    return (
        <div className="grid grid-cols-2 md:grid-cols-3 2xl:grid-cols-5 gap-3" aria-live="polite">
            <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                className="min-w-0 p-3 rounded-2xl bg-white/5 border border-white/10 backdrop-blur-md"
            >
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 text-white/60 text-sm font-medium">
                        <Cpu size={16} /> CPU Usage
                    </div>
                    <span className={`text-xs font-bold px-2 py-0.5 rounded-full ${getUsageColor(stats.cpu_percent)}`}>
                        {stats.cpu_percent.toFixed(1)}%
                    </span>
                </div>
                <div className="space-y-1">
                    <div className="h-2 w-full bg-white/10 rounded-full overflow-hidden">
                        <div
                            className={`h-full rounded-full transition-all duration-500 ${getBarColor(stats.cpu_percent)}`}
                            style={{ width: `${Math.min(stats.cpu_percent, 100)}%` }}
                        />
                    </div>
                    <div className="flex justify-between text-xs text-white/40">
                        <span>CPU Cores</span>
                        <span>{stats.cpu_count} Logical</span>
                    </div>
                </div>
            </motion.div>

            <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 }}
                className="min-w-0 p-3 rounded-2xl bg-white/5 border border-white/10 backdrop-blur-md"
            >
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 text-white/60 text-sm font-medium">
                        <HardDrive size={16} /> Memory
                    </div>
                    <span className={`text-xs font-bold px-2 py-0.5 rounded-full ${getUsageColor(stats.memory_percent)}`}>
                        {stats.memory_percent.toFixed(1)}%
                    </span>
                </div>
                <div className="space-y-1">
                    <div className="h-2 w-full bg-white/10 rounded-full overflow-hidden">
                        <div
                            className={`h-full rounded-full transition-all duration-500 ${getBarColor(stats.memory_percent)}`}
                            style={{ width: `${Math.min(stats.memory_percent, 100)}%` }}
                        />
                    </div>
                    <div className="flex justify-between text-xs text-white/40">
                        <span>{(stats.memory_used_mb / 1024).toFixed(1)} GB used</span>
                        <span>{(stats.memory_total_mb / 1024).toFixed(0)} GB total</span>
                    </div>
                </div>
            </motion.div>

            <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.2 }}
                className="min-w-0 p-3 rounded-2xl bg-white/5 border border-white/10 backdrop-blur-md"
            >
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 text-white/60 text-sm font-medium">
                        <Layers size={16} /> Active Jobs
                    </div>
                    <span className="text-xs font-bold px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400">
                        {stats.active_jobs} / {stats.concurrent_limit}
                    </span>
                </div>
                <div className="flex items-end gap-1 h-8 mt-2">
                    {Array.from({ length: stats.concurrent_limit }).map((_, i) => (
                        <div
                            key={i}
                            className={`flex-1 rounded-sm transition-all duration-300 ${
                                i < stats.active_jobs ? "bg-blue-500 h-6" : "bg-white/10 h-2"
                            }`}
                        />
                    ))}
                </div>
            </motion.div>

            <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.3 }}
                className="min-w-0 p-3 rounded-2xl bg-white/5 border border-white/10 backdrop-blur-md"
            >
                <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 text-white/60 text-sm font-medium">
                        <Cpu size={16} /> GPU
                    </div>
                    {stats.gpu_utilization != null ? (
                        <span className={`text-xs font-bold px-2 py-0.5 rounded-full ${getUsageColor(stats.gpu_utilization)}`}>
                            {stats.gpu_utilization.toFixed(1)}%
                        </span>
                    ) : (
                        <span className="text-xs font-bold px-2 py-0.5 rounded-full bg-white/10 text-white/40">N/A</span>
                    )}
                </div>
                <div className="space-y-1">
                    <div className="h-2 w-full bg-white/10 rounded-full overflow-hidden">
                        {stats.gpu_utilization != null && (
                            <div
                                className={`h-full rounded-full transition-all duration-500 ${getBarColor(stats.gpu_utilization)}`}
                                style={{ width: `${Math.min(stats.gpu_utilization, 100)}%` }}
                            />
                        )}
                    </div>
                    <div className="flex justify-between text-xs text-white/40">
                        <span>VRAM</span>
                        <span>{stats.gpu_memory_percent?.toFixed(0) || "-"}% used</span>
                    </div>
                </div>
            </motion.div>

            <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.4 }}
                className="min-w-0 p-3 rounded-2xl bg-white/5 border border-white/10 backdrop-blur-md flex flex-col justify-between"
            >
                <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 text-white/60 text-sm font-medium">
                        <Clock size={16} /> Uptime
                    </div>
                    <Activity size={14} className="text-green-500 animate-pulse" />
                </div>
                <div className="text-2xl font-bold text-white/90">{formatUptime(stats.uptime_seconds)}</div>
            </motion.div>
        </div>
    );
}
