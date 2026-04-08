import { useState, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Terminal, Server, Cpu, Activity, ShieldCheck, type LucideIcon } from "lucide-react";
import { apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface SystemInfo {
    version: string;
    os_version: string;
    is_docker: boolean;
    telemetry_enabled: boolean;
    ffmpeg_version: string;
}

interface UpdateInfo {
    current_version: string;
    latest_version: string | null;
    update_available: boolean;
    release_url: string | null;
}

interface AboutDialogProps {
    isOpen: boolean;
    onClose: () => void;
}

function focusableElements(root: HTMLElement): HTMLElement[] {
    const selector = [
        "a[href]",
        "button:not([disabled])",
        "input:not([disabled])",
        "select:not([disabled])",
        "textarea:not([disabled])",
        "[tabindex]:not([tabindex='-1'])",
    ].join(",");

    return Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(
        (element) => !element.hasAttribute("disabled")
    );
}

export default function AboutDialog({ isOpen, onClose }: AboutDialogProps) {
    const [info, setInfo] = useState<SystemInfo | null>(null);
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const dialogRef = useRef<HTMLDivElement | null>(null);
    const lastFocusedRef = useRef<HTMLElement | null>(null);

    useEffect(() => {
        if (isOpen && !info) {
            apiJson<SystemInfo>("/api/system/info")
                .then(setInfo)
                .catch((e: unknown) => {
                    const message = isApiError(e) ? e.message : "Failed to load version info";
                    showToast({ kind: "error", title: "About", message });
                });
        }
    }, [isOpen, info]);

    useEffect(() => {
        if (isOpen && !updateInfo) {
            apiJson<UpdateInfo>("/api/system/update")
                .then(setUpdateInfo)
                .catch(() => {
                    // Non-critical; keep update checks soft-fail.
                });
        }
    }, [isOpen, updateInfo]);

    useEffect(() => {
        if (!isOpen) {
            return;
        }

        lastFocusedRef.current = document.activeElement as HTMLElement | null;

        const dialog = dialogRef.current;
        if (dialog) {
            const focusables = focusableElements(dialog);
            if (focusables.length > 0) {
                focusables[0].focus();
            } else {
                dialog.focus();
            }
        }

        const onKeyDown = (event: KeyboardEvent) => {
            if (!isOpen) {
                return;
            }

            if (event.key === "Escape") {
                event.preventDefault();
                onClose();
                return;
            }

            if (event.key !== "Tab") {
                return;
            }

            const root = dialogRef.current;
            if (!root) {
                return;
            }

            const focusables = focusableElements(root);
            if (focusables.length === 0) {
                event.preventDefault();
                root.focus();
                return;
            }

            const first = focusables[0];
            const last = focusables[focusables.length - 1];
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
            if (lastFocusedRef.current) {
                lastFocusedRef.current.focus();
            }
        };
    }, [isOpen, onClose]);

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
                            ref={dialogRef}
                            role="dialog"
                            aria-modal="true"
                            aria-labelledby="about-dialog-title"
                            tabIndex={-1}
                            className="w-full max-w-md bg-helios-surface border border-helios-line/30 rounded-xl shadow-2xl overflow-hidden relative"
                        >
                            <div className="absolute top-0 left-0 w-full h-32 bg-gradient-to-b from-helios-solar/10 to-transparent pointer-events-none" />

                            <div className="p-8 relative">
                                <div className="flex items-center justify-end mb-6">
                                    <button
                                        onClick={onClose}
                                        className="p-2 hover:bg-helios-surface-soft rounded-full text-helios-slate hover:text-helios-ink transition-colors"
                                    >
                                        <X size={20} />
                                    </button>
                                </div>

                                <div className="mb-8">
                                    <h2 id="about-dialog-title" className="text-2xl font-bold text-helios-ink tracking-tight">Alchemist</h2>
                                    <p className="text-helios-slate font-medium">Media Transcoding Agent</p>
                                </div>

                                {info ? (
                                    <div className="space-y-3">
                                        <InfoRow icon={Terminal} label="Version" value={`v${info.version}`} />
                                        <InfoRow icon={Activity} label="FFmpeg" value={info.ffmpeg_version} />
                                        <InfoRow icon={Server} label="System" value={info.os_version} />
                                        <InfoRow icon={Cpu} label="Environment" value={info.is_docker ? "Docker Container" : "Native"} />
                                        <InfoRow icon={ShieldCheck} label="Telemetry" value={info.telemetry_enabled ? "Enabled" : "Disabled"} />
                                        {updateInfo?.latest_version && (
                                            <div className="rounded-xl bg-helios-surface-soft border border-helios-line/10 p-3">
                                                <div className="flex items-center justify-between gap-3">
                                                    <div>
                                                        <p className="text-xs font-medium text-helios-slate">Latest Stable</p>
                                                        <p className="text-sm font-bold text-helios-ink">v{updateInfo.latest_version}</p>
                                                    </div>
                                                    {updateInfo.update_available && updateInfo.release_url && (
                                                        <a
                                                            href={updateInfo.release_url}
                                                            target="_blank"
                                                            rel="noreferrer"
                                                            className="rounded-lg bg-helios-solar px-3 py-2 text-xs font-bold text-helios-main hover:opacity-90 transition-opacity"
                                                        >
                                                            Download Update
                                                        </a>
                                                    )}
                                                </div>
                                                <p className="mt-2 text-xs text-helios-slate">
                                                    {updateInfo.update_available
                                                        ? "A newer stable release is available."
                                                        : "You are on the latest stable release."}
                                                </p>
                                            </div>
                                        )}
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

interface InfoRowProps {
    icon: LucideIcon;
    label: string;
    value: string;
}

function InfoRow({ icon: Icon, label, value }: InfoRowProps) {
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
