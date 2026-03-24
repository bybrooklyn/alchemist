import { useEffect, useState } from "react";
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
            <div className="flex items-center gap-2">

                {/* Status pill */}
                <div className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-helios-line/20 bg-helios-surface-soft/60">
                    <div className={`h-1.5 w-1.5 rounded-full shrink-0 ${statusConfig[status].dot}`} />
                    <span className={`text-xs font-medium ${statusConfig[status].labelColor}`}>
                        {statusConfig[status].label}
                    </span>
                </div>

                {/* Start — shown when paused or draining */}
                {(status === "paused" || status === "draining") && (
                    <button
                        onClick={() => void handleStart()}
                        disabled={engineLoading}
                        className="flex items-center gap-1.5 rounded-lg bg-helios-solar px-3 py-1.5 text-xs font-semibold text-helios-main hover:opacity-90 transition-opacity disabled:opacity-50"
                    >
                        <Play size={13} />
                        Start
                    </button>
                )}

                {/* Pause — shown when running */}
                {status === "running" && (
                    <button
                        onClick={() => void handlePause()}
                        disabled={engineLoading}
                        className="flex items-center gap-1.5 rounded-lg border border-helios-line/20 px-3 py-1.5 text-xs font-medium text-helios-slate hover:bg-helios-surface-soft hover:text-helios-ink transition-colors disabled:opacity-50"
                    >
                        <Pause size={13} />
                        Pause
                    </button>
                )}

                {/* Stop — shown when running */}
                {status === "running" && (
                    <button
                        onClick={() => void handleStop()}
                        disabled={engineLoading}
                        className="flex items-center gap-1.5 rounded-lg border border-helios-line/20 px-3 py-1.5 text-xs font-medium text-helios-slate hover:bg-helios-surface-soft hover:text-helios-ink transition-colors disabled:opacity-50"
                    >
                        <Square size={13} />
                        Stop
                    </button>
                )}

                {/* Cancel Stop — shown when draining */}
                {status === "draining" && (
                    <button
                        onClick={() => void handleCancelStop()}
                        disabled={engineLoading}
                        className="flex items-center gap-1.5 rounded-lg border border-blue-400/30 px-3 py-1.5 text-xs font-medium text-blue-400 hover:bg-blue-400/10 transition-colors disabled:opacity-50"
                    >
                        <X size={13} />
                        Cancel Stop
                    </button>
                )}

                {/* Scheduler paused note */}
                {engineStatus?.scheduler_paused && !engineStatus.manual_paused && (
                    <span className="text-xs text-helios-slate/50 italic">
                        (schedule)
                    </span>
                )}

                {/* Divider */}
                <div className="w-px h-4 bg-helios-line/30 mx-1" />

                {/* About */}
                <motion.button
                    onClick={() => setShowAbout(true)}
                    whileHover={{ scale: 1.05 }}
                    whileTap={{ scale: 0.95 }}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium text-helios-slate hover:bg-helios-surface-soft hover:text-helios-ink transition-colors"
                >
                    <Info size={15} />
                    <span>About</span>
                </motion.button>

                {/* Logout */}
                <button
                    onClick={() => void handleLogout()}
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium text-status-error/70 hover:bg-status-error/10 hover:text-status-error transition-colors"
                >
                    <LogOut size={15} />
                    <span>Logout</span>
                </button>

            </div>

            <AboutDialog
                isOpen={showAbout}
                onClose={() => setShowAbout(false)}
            />
        </>
    );
}
