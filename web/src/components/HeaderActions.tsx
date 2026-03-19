import { useEffect, useState } from "react";
import { Info, LogOut, Pause, Play } from "lucide-react";
import { motion } from "framer-motion";
import AboutDialog from "./AboutDialog";
import { apiAction, apiJson } from "../lib/api";
import { showToast } from "../lib/toast";

export default function HeaderActions() {
    const [showAbout, setShowAbout] = useState(false);
    const [engineStatus, setEngineStatus] = useState<"paused" | "running">("paused");
    const [engineLoading, setEngineLoading] = useState(false);

    const showEngineControl =
        typeof window !== "undefined" && window.location.pathname === "/";

    useEffect(() => {
        if (!showEngineControl) return;
        void apiJson<{ status: "paused" | "running" }>("/api/engine/status")
            .then((data) => setEngineStatus(data.status))
            .catch(() => undefined);
    }, [showEngineControl]);

    const toggleEngine = async () => {
        setEngineLoading(true);
        try {
            const nextAction = engineStatus === "paused" ? "resume" : "pause";
            await apiAction(`/api/engine/${nextAction}`, { method: "POST" });
            setEngineStatus(engineStatus === "paused" ? "running" : "paused");
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

    return (
        <>
            <div className="flex items-center gap-2">
                {showEngineControl && (
                    <motion.button
                        onClick={() => void toggleEngine()}
                        whileHover={{ scale: 1.05 }}
                        whileTap={{ scale: 0.95 }}
                        disabled={engineLoading}
                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-bold text-helios-slate hover:bg-helios-surface-soft hover:text-helios-ink transition-colors disabled:opacity-50"
                    >
                        {engineStatus === "paused" ? <Play size={16} /> : <Pause size={16} />}
                        <span>{engineStatus === "paused" ? "Start" : "Pause"}</span>
                    </motion.button>
                )}
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

            <AboutDialog isOpen={showAbout} onClose={() => setShowAbout(false)} />
        </>
    );
}
