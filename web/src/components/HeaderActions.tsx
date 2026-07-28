import { useEffect, useRef, useState } from "react";
import { Info, LogOut, Play, Square } from "lucide-react";
import { motion } from "framer-motion";
import AboutDialog from "./AboutDialog";
import { apiAction, apiJson } from "../lib/api";
import { useSharedStats } from "../lib/statsStore";
import { showToast } from "../lib/toast";

interface EngineStatus {
    status: "running" | "paused" | "draining";
    manual_paused: boolean;
    scheduler_paused: boolean;
    draining: boolean;
    disk_blocked?: boolean;
    disk_block_reason?: string | null;
    mode: "background" | "balanced" | "throughput";
    concurrent_limit: number;
    is_manual_override: boolean;
}

type EngineActionStatus = Pick<EngineStatus, "status">;

const DEFAULT_ENGINE_STATUS: EngineStatus = {
    status: "paused",
    manual_paused: true,
    scheduler_paused: false,
    draining: false,
    disk_blocked: false,
    disk_block_reason: null,
    mode: "background",
    concurrent_limit: 1,
    is_manual_override: false,
};

export default function HeaderActions() {
    const [engineStatus, setEngineStatus] = useState<EngineStatus | null>(null);
    const [engineLoading, setEngineLoading] = useState(false);
    const [showAbout, setShowAbout] = useState(false);
    const [aboutOrigin, setAboutOrigin] = useState<DOMRect | null>(null);
    const aboutButtonRef = useRef<HTMLButtonElement>(null);
    const { stats } = useSharedStats();

    const statusConfig = {
        running: {
            dot: "bg-status-success animate-pulse",
            label: "Running",
            labelColor: "text-status-success",
        },
        idle: {
            dot: "bg-helios-slate",
            label: "Idle",
            labelColor: "text-helios-slate",
        },
        paused: {
            dot: "bg-helios-solar",
            label: "Paused",
            labelColor: "text-helios-solar",
        },
        draining: {
            dot: "bg-helios-solar animate-pulse",
            label: "Stopping",
            labelColor: "text-helios-solar",
        },
        disk: {
            dot: "bg-helios-solar animate-pulse",
            label: "Low disk",
            labelColor: "text-helios-solar",
        },
    } as const;

    const status = engineStatus?.status ?? "paused";
    const isIdle = status === "running" && (stats?.active ?? 0) === 0;
    const displayStatus: keyof typeof statusConfig =
        status === "draining"
            ? "draining"
            : engineStatus?.disk_blocked
              ? "disk"
              : isIdle
                ? "idle"
                : status;

    const refreshEngineStatus = async () => {
        const data = await apiJson<EngineStatus>("/api/engine/status");
        setEngineStatus(data);
        return data;
    };

    const applyActionStatus = (actionStatus: EngineActionStatus) => {
        setEngineStatus((current) => ({
            ...(current ?? DEFAULT_ENGINE_STATUS),
            status: actionStatus.status,
            manual_paused: actionStatus.status === "running"
                ? false
                : actionStatus.status === "paused"
                  ? true
                  : current?.manual_paused ?? false,
            draining: actionStatus.status === "draining",
        }));
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

    // Fast poll during draining state for responsive UI
    useEffect(() => {
        if (status !== "draining") return;

        const id = window.setInterval(() => {
            void refreshEngineStatus();
        }, 1000);

        return () => window.clearInterval(id);
    }, [status]);

    const handleStart = async () => {
        setEngineLoading(true);
        try {
            const result = await apiJson<EngineActionStatus>("/api/engine/resume", {
                method: "POST",
            });
            // The action response confirms the state change. Keep the control
            // accurate if a transient network failure breaks the follow-up GET.
            applyActionStatus(result);
            try {
                await refreshEngineStatus();
            } catch {
                // Keep the acknowledged action state until the next poll.
            }
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
            const result = await apiJson<EngineActionStatus>("/api/engine/drain", {
                method: "POST",
            });
            applyActionStatus(result);
            try {
                await refreshEngineStatus();
            } catch {
                // Keep the acknowledged action state until the next poll.
            }
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
            window.location.href = "/login";
        }
    };

    return (
        <>
            <div className="flex items-center gap-2">

                {/* Status pill */}
                <div
                    className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border border-helios-line/20 bg-helios-surface-soft/60"
                    title={
                        engineStatus?.disk_blocked
                            ? (engineStatus.disk_block_reason ??
                              "Low disk space — the engine is holding jobs until space is reclaimed.")
                            : undefined
                    }
                >
                    <div className={`h-1.5 w-1.5 rounded-full shrink-0 ${statusConfig[displayStatus].dot}`} />
                    <span className={`text-xs font-medium ${statusConfig[displayStatus].labelColor}`}>
                        {statusConfig[displayStatus].label}
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
                    ref={aboutButtonRef}
                    onClick={() => {
                        setAboutOrigin(
                            aboutButtonRef.current?.getBoundingClientRect() ?? null,
                        );
                        setShowAbout(true);
                    }}
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
                originRect={aboutOrigin}
            />
        </>
    );
}
