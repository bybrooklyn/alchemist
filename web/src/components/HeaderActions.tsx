import { useState } from "react";
import { Info, LogOut } from "lucide-react";
import { motion } from "framer-motion";
import AboutDialog from "./AboutDialog";
import { apiAction } from "../lib/api";
import { showToast } from "../lib/toast";

export default function HeaderActions() {
    const [showAbout, setShowAbout] = useState(false);

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
