import { useEffect, useState } from "react";
import clsx from "clsx";
import { Info, LogOut, Pause, Play, Square, X } from "lucide-react";
import { motion } from "framer-motion";
import AboutDialog from "./AboutDialog";
import { apiAction, apiJson } from "../lib/api";
import { showToast } from "../lib/toast";

interface EngineStatus {
    status: "running" | "paused" | "draining";
    manual_paused: boolean;
    scheduler_paused: boolean;
    draining: boolean;
    mode: "background" | "balanced" | "throughput";
    concurrent_limit: number;
    is_manual_override: boolean;
}

interface EngineMode {
    mode: "background" | "balanced" | "throughput";
    is_manual_override: boolean;
    concurrent_limit: number;
    cpu_count: number;
    computed_limits: {
        background: number;
        balanced: number;
        throughput: number;
    };
}

export default function HeaderActions() {
    const [engineStatus, setEngineStatus] = useState<EngineStatus | null>(null);
    const [engineMode, setEngineMode] = useState<EngineMode | null>(null);
    const [engineLoading, setEngineLoading] = useState(false);
    const [showAdvanced, setShowAdvanced] = useState(false);
    const [manualJobs, setManualJobs] = useState<number>(1);
    const [manualThreads, setManualThreads] = useState<number>(0);
    const [showAbout, setShowAbout] = useState(false);

    const statusConfig = {
        running: {
            dot: "bg-emerald-500 animate-pulse",
            label: "Running",
            labelColor: "text-emerald-500",
        },
        paused: {
            dot: "bg-amber-500",
            label: "Paused",
            labelColor: "text-amber-500",
        },
        draining: {
            dot: "bg-blue-400",
            label: "Draining",
            labelColor: "text-blue-400",
        },
    } as const;

    const refreshEngineStatus = async () => {
        const data = await apiJson<EngineStatus>("/api/engine/status");
        setEngineStatus(data);
        return data;
    };

    const refreshEngineMode = async () => {
        const data = await apiJson<EngineMode>("/api/engine/mode");
        setEngineMode(data);
        return data;
    };

    useEffect(() => {
        let cancelled = false;

        const load = async () => {
            try {
                const [status, mode] = await Promise.all([
                    apiJson<EngineStatus>("/api/engine/status"),
                    apiJson<EngineMode>("/api/engine/mode"),
                ]);

                if (cancelled) {
                    return;
                }

                setEngineStatus(status);
                setEngineMode(mode);
                setManualJobs(mode.concurrent_limit);
                setManualThreads(0);
            } catch {
                // Ignore transient header control failures.
            }
        };

        const pollStatus = async () => {
            try {
                const status = await apiJson<EngineStatus>("/api/engine/status");
                if (!cancelled) {
                    setEngineStatus(status);
                }
            } catch {
                // Ignore transient polling failures.
            }
        };

        void load();
        const intervalId = window.setInterval(() => {
            void pollStatus();
        }, 5000);

        return () => {
            cancelled = true;
            window.clearInterval(intervalId);
        };
    }, []);

    const handleStart = async () => {
        setEngineLoading(true);
        try {
            await apiAction("/api/engine/resume", { method: "POST" });
            await refreshEngineStatus();
        } catch {
            showToast({
                kind: "error",
                title: "Engine",
                message: "Failed to update engine state.",
            });
        } finally {
            setEngineLoading(false);
        }
    };

    const handlePause = async () => {
        setEngineLoading(true);
        try {
            await apiAction("/api/engine/pause", { method: "POST" });
            await refreshEngineStatus();
        } catch {
            showToast({
                kind: "error",
                title: "Engine",
                message: "Failed to update engine state.",
            });
        } finally {
            setEngineLoading(false);
        }
    };

    const handleStop = async () => {
        setEngineLoading(true);
        try {
            await apiAction("/api/engine/drain", { method: "POST" });
            await refreshEngineStatus();
        } catch {
            showToast({
                kind: "error",
                title: "Engine",
                message: "Failed to update engine state.",
            });
        } finally {
            setEngineLoading(false);
        }
    };

    const handleCancelStop = async () => {
        setEngineLoading(true);
        try {
            await apiAction("/api/engine/stop-drain", { method: "POST" });
            await refreshEngineStatus();
        } catch {
            showToast({
                kind: "error",
                title: "Engine",
                message: "Failed to update engine state.",
            });
        } finally {
            setEngineLoading(false);
        }
    };

    const handleModeChange = async (mode: EngineStatus["mode"]) => {
        setEngineLoading(true);
        try {
            await apiAction("/api/engine/mode", {
                method: "POST",
                body: JSON.stringify({ mode }),
            });
            const [status, nextMode] = await Promise.all([
                refreshEngineStatus(),
                refreshEngineMode(),
            ]);
            setManualJobs(nextMode.concurrent_limit);
            setManualThreads(0);
            setEngineStatus(status);
        } catch {
            showToast({
                kind: "error",
                title: "Engine",
                message: "Failed to update engine mode.",
            });
        } finally {
            setEngineLoading(false);
        }
    };

    const handleApplyAdvanced = async () => {
        const currentMode = engineStatus?.mode ?? engineMode?.mode;
        if (!currentMode) {
            return;
        }

        setEngineLoading(true);
        try {
            await apiAction("/api/engine/mode", {
                method: "POST",
                body: JSON.stringify({
                    mode: currentMode,
                    concurrent_jobs_override: manualJobs,
                    threads_override: manualThreads,
                }),
            });
            await Promise.all([refreshEngineStatus(), refreshEngineMode()]);
        } catch {
            showToast({
                kind: "error",
                title: "Engine",
                message: "Failed to apply advanced engine settings.",
            });
        } finally {
            setEngineLoading(false);
        }
    };

    const handleLogout = async () => {
        try {
            await apiAction("/api/auth/logout", { method: "POST" });
        } catch {
            showToast({
                kind: "error",
                message: "Logout request failed. Redirecting to login.",
            });
        } finally {
            window.location.href = '/login';
        }
    };

    const status = engineStatus?.status ?? "paused";

    return (
        <>
            <div className="flex items-center gap-3">
                <div className="flex flex-col gap-1.5">
                    <div className="flex items-center gap-2">
                        <div className="flex items-center gap-1.5 rounded border border-helios-line/20 bg-helios-surface-soft/60 px-2 py-1">
                            <div className={`h-1.5 w-1.5 rounded-full ${statusConfig[status].dot}`} />
                            <span className={`text-xs font-medium ${statusConfig[status].labelColor}`}>
                                {statusConfig[status].label}
                            </span>
                        </div>

                        {(status === "paused" || status === "draining") && (
                            <button
                                onClick={handleStart}
                                disabled={engineLoading}
                                className="flex items-center gap-1.5 rounded bg-helios-solar px-3 py-1.5 text-xs font-semibold text-helios-main transition-opacity hover:opacity-90 disabled:opacity-50"
                            >
                                <Play size={13} />
                                Start
                            </button>
                        )}

                        {status === "running" && (
                            <button
                                onClick={handlePause}
                                disabled={engineLoading}
                                className="flex items-center gap-1.5 rounded border border-helios-line/20 px-3 py-1.5 text-xs font-medium text-helios-slate transition-colors hover:bg-helios-surface-soft hover:text-helios-ink disabled:opacity-50"
                            >
                                <Pause size={13} />
                                Pause
                            </button>
                        )}

                        {status === "running" && (
                            <button
                                onClick={handleStop}
                                disabled={engineLoading}
                                className="flex items-center gap-1.5 rounded border border-helios-line/20 px-3 py-1.5 text-xs font-medium text-helios-slate transition-colors hover:bg-helios-surface-soft hover:text-helios-ink disabled:opacity-50"
                            >
                                <Square size={13} />
                                Stop
                            </button>
                        )}

                        {status === "draining" && (
                            <button
                                onClick={handleCancelStop}
                                disabled={engineLoading}
                                className="flex items-center gap-1.5 rounded border border-blue-400/30 px-3 py-1.5 text-xs font-medium text-blue-400 transition-colors hover:bg-blue-400/10 disabled:opacity-50"
                            >
                                <X size={13} />
                                Cancel Stop
                            </button>
                        )}
                    </div>

                    <div className="flex items-center gap-1">
                        {(["background", "balanced", "throughput"] as const).map((m) => (
                            <button
                                key={m}
                                onClick={() => void handleModeChange(m)}
                                disabled={engineLoading}
                                className={clsx(
                                    "px-2.5 py-1 rounded text-[11px] font-medium capitalize transition-colors disabled:opacity-50",
                                    engineStatus?.mode === m
                                        ? "bg-helios-solar/15 text-helios-solar border border-helios-solar/30"
                                        : "text-helios-slate/70 hover:text-helios-slate border border-transparent hover:border-helios-line/20"
                                )}
                            >
                                {m}
                            </button>
                        ))}
                        {engineStatus?.is_manual_override && (
                            <span className="ml-1 text-[10px] italic text-helios-slate/50">
                                manual
                            </span>
                        )}
                    </div>

                    {engineStatus?.scheduler_paused && !engineStatus.manual_paused && (
                        <div className="text-[10px] text-helios-slate/50">Paused by schedule</div>
                    )}

                    <details onToggle={(e) => setShowAdvanced((e.target as HTMLDetailsElement).open)}>
                        <summary className="list-none w-fit cursor-pointer select-none text-[10px] text-helios-slate/50 hover:text-helios-slate/80">
                            {showAdvanced ? "▾" : "▸"} Advanced
                        </summary>
                        <div className="mt-2 min-w-[220px] space-y-2 rounded-md border border-helios-line/20 bg-helios-surface-soft/40 p-3">
                            {engineMode && (
                                <div className="space-y-1 text-[10px] text-helios-slate/60">
                                    <div>Auto limits at current mode:</div>
                                    <div className="pl-2 font-mono">
                                        background →{engineMode.computed_limits.background} job
                                        <br />
                                        balanced →{engineMode.computed_limits.balanced} jobs
                                        <br />
                                        throughput →{engineMode.computed_limits.throughput} jobs
                                    </div>
                                    <div className="text-[10px] text-helios-slate/40">
                                        Based on {engineMode.cpu_count} logical CPUs
                                    </div>
                                </div>
                            )}

                            <div className="space-y-1.5">
                                <label className="block text-xs font-medium text-helios-slate">
                                    Concurrent jobs
                                </label>
                                <div className="flex items-center gap-2">
                                    <input
                                        type="number"
                                        min={1}
                                        max={32}
                                        value={manualJobs}
                                        onChange={(e) =>
                                            setManualJobs(
                                                Math.max(1, parseInt(e.target.value, 10) || 1)
                                            )
                                        }
                                        className="w-16 rounded border border-helios-line/20 bg-helios-surface px-2 py-1 text-xs text-helios-ink outline-none focus:border-helios-solar"
                                    />
                                    <span className="text-[10px] text-helios-slate/60">
                                        (overrides auto)
                                    </span>
                                </div>
                            </div>

                            <div className="space-y-1.5">
                                <label className="block text-xs font-medium text-helios-slate">
                                    CPU threads per job
                                </label>
                                <div className="flex items-center gap-2">
                                    <input
                                        type="number"
                                        min={0}
                                        max={64}
                                        value={manualThreads}
                                        onChange={(e) =>
                                            setManualThreads(
                                                Math.max(0, parseInt(e.target.value, 10) || 0)
                                            )
                                        }
                                        className="w-16 rounded border border-helios-line/20 bg-helios-surface px-2 py-1 text-xs text-helios-ink outline-none focus:border-helios-solar"
                                    />
                                    <span className="text-[10px] text-helios-slate/60">
                                        0 = auto
                                    </span>
                                </div>
                            </div>

                            <div className="flex gap-2 pt-1">
                                <button
                                    onClick={handleApplyAdvanced}
                                    disabled={engineLoading}
                                    className="flex-1 rounded bg-helios-solar px-3 py-1.5 text-xs font-semibold text-helios-main transition-opacity hover:opacity-90 disabled:opacity-50"
                                >
                                    Apply
                                </button>
                                <button
                                    onClick={() => {
                                        if (engineStatus && engineMode) {
                                            void handleModeChange(engineStatus.mode);
                                            setManualJobs(
                                                engineMode.computed_limits[engineStatus.mode]
                                            );
                                            setManualThreads(0);
                                        }
                                    }}
                                    disabled={engineLoading}
                                    className="rounded border border-helios-line/20 px-3 py-1.5 text-xs font-medium text-helios-slate transition-colors hover:bg-helios-surface-soft disabled:opacity-50"
                                >
                                    Reset to auto
                                </button>
                            </div>
                        </div>
                    </details>
                </div>

                <div className="flex items-center gap-1 border-l border-helios-line/20 pl-3">
                    <motion.button
                        onClick={() => setShowAbout(true)}
                        whileHover={{ scale: 1.05 }}
                        whileTap={{ scale: 0.95 }}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-bold text-helios-slate hover:bg-helios-surface-soft hover:text-helios-ink transition-colors"
                    >
                        <Info size={16} />
                        <span>About</span>
                    </motion.button>
                    <button
                        onClick={handleLogout}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-bold text-red-500/80 hover:bg-red-500/10 hover:text-red-600 transition-colors"
                    >
                        <LogOut size={16} />
                        <span>Logout</span>
                    </button>
                </div>
            </div>

            <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
        </>
    );
}
