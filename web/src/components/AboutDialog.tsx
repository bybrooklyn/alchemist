import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Info, X, Terminal, Server, Cpu, Activity, ShieldCheck } from "lucide-react";
import { apiFetch } from "../lib/api";

interface SystemInfo {
    version: string;
    os_version: string;
    is_docker: boolean;
    telemetry_enabled: boolean;
    ffmpeg_version: string;
}

interface AboutDialogProps {
    isOpen: boolean;
    onClose: () => void;
}

export default function AboutDialog({ isOpen, onClose }: AboutDialogProps) {
    const [info, setInfo] = useState<SystemInfo | null>(null);

    useEffect(() => {
        if (isOpen && !info) {
            apiFetch("/api/system/info")
                .then(res => res.json())
                .then(setInfo)
                .catch(console.error);
        }
    }, [isOpen]);

    return (
        <AnimatePresence>
            {isOpen && (
                <>
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        onClick={onClose}
                        className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4"
                    >
                        <motion.div
                            initial={{ opacity: 0, scale: 0.95, y: 20 }}
                            animate={{ opacity: 1, scale: 1, y: 0 }}
                            exit={{ opacity: 0, scale: 0.95, y: 20 }}
                            onClick={e => e.stopPropagation()}
                            className="w-full max-w-md bg-helios-surface border border-helios-line/30 rounded-3xl shadow-2xl overflow-hidden relative"
                        >
                            <div className="absolute top-0 left-0 w-full h-32 bg-gradient-to-b from-helios-solar/10 to-transparent pointer-events-none" />

                            <div className="p-8 relative">
                                <div className="flex items-center justify-between mb-6">
                                    <div className="p-3 bg-helios-surface-soft border border-helios-line/20 rounded-2xl shadow-sm">
                                        <div className="w-8 h-8 rounded-lg bg-helios-solar text-helios-main flex items-center justify-center font-bold text-xl">
                                            Al
                                        </div>
                                    </div>
                                    <button
                                        onClick={onClose}
                                        className="p-2 hover:bg-helios-surface-soft rounded-full text-helios-slate hover:text-helios-ink transition-colors"
                                    >
                                        <X size={20} />
                                    </button>
                                </div>

                                <div className="mb-8">
                                    <h2 className="text-2xl font-bold text-helios-ink tracking-tight">Alchemist</h2>
                                    <p className="text-helios-slate font-medium">Media Transcoding Agent</p>
                                </div>

                                {info ? (
                                    <div className="space-y-3">
                                        <InfoRow icon={Terminal} label="Version" value={`v${info.version}`} />
                                        <InfoRow icon={Activity} label="FFmpeg" value={info.ffmpeg_version} />
                                        <InfoRow icon={Server} label="System" value={info.os_version} />
                                        <InfoRow icon={Cpu} label="Environment" value={info.is_docker ? "Docker Container" : "Native"} />
                                        <InfoRow icon={ShieldCheck} label="Telemetry" value={info.telemetry_enabled ? "Enabled" : "Disabled"} />
                                    </div>
                                ) : (
                                    <div className="flex justify-center p-8">
                                        <div className="w-6 h-6 border-2 border-helios-solar border-t-transparent rounded-full animate-spin" />
                                    </div>
                                )}

                                <div className="mt-8 pt-6 border-t border-helios-line/10 text-center">
                                    <p className="text-xs text-helios-slate/60">
                                        &copy; {new Date().getFullYear()} Alchemist Contributors. <br />
                                        Released under GPL-3.0 License.
                                    </p>
                                </div>
                            </div>
                        </motion.div>
                    </motion.div>
                </>
            )}
        </AnimatePresence>
    );
}

function InfoRow({ icon: Icon, label, value }: { icon: any, label: string, value: string }) {
    return (
        <div className="flex items-center justify-between p-3 rounded-xl bg-helios-surface-soft border border-helios-line/10">
            <div className="flex items-center gap-3">
                <Icon size={16} className="text-helios-slate" />
                <span className="text-sm font-medium text-helios-slate">{label}</span>
            </div>
            <span className="text-sm font-bold text-helios-ink font-mono break-all text-right max-w-[60%]">{value}</span>
        </div>
    );
}
