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

export default function HeaderActions() {
    const [engineStatus, setEngineStatus] = useState<EngineStatus | null>(null);
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

    useEffect(() => {
        let cancelled = false;

        const load = async () => {
            try {
                const status = await apiJson<EngineStatus>("/api/engine/status");

                if (cancelled) {
                    return;
                }

                setEngineStatus(status);
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

                {/* Single action button — changes based on state */}
                {status === "paused" && (
                    <button
                        onClick={() => void handleStart()}
                        disabled={engineLoading}
                        className="flex items-center gap-1.5 rounded-lg bg-helios-solar px-3 py-1.5 text-xs font-semibold text-helios-main hover:opacity-90 transition-opacity disabled:opacity-50"
                    >
                        <Play size={13} />
                        Start
                    </button>
                )}

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

                {status === "draining" && (
                    <button
                        disabled
                        className="flex items-center gap-1.5 rounded-lg border border-helios-line/20 px-3 py-1.5 text-xs font-medium text-helios-slate/50 opacity-60 cursor-not-allowed"
                    >
                        <Square size={13} className="animate-pulse" />
                        Stopping…
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
