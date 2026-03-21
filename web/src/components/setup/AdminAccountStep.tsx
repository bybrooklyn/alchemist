import { useEffect } from "react";
import { motion } from "framer-motion";
import { Lock, Palette } from "lucide-react";
import clsx from "clsx";
import { THEME_OPTIONS } from "./constants";
import type { StepValidator } from "./types";

interface AdminAccountStepProps {
    username: string;
    password: string;
    telemetryEnabled: boolean;
    activeThemeId: string | null;
    onUsernameChange: (value: string) => void;
    onPasswordChange: (value: string) => void;
    onTelemetryChange: (value: boolean) => void;
    onThemeChange: (value: string) => void;
    registerValidator: (validator: StepValidator) => void;
}

export default function AdminAccountStep({
    username,
    password,
    telemetryEnabled,
    activeThemeId,
    onUsernameChange,
    onPasswordChange,
    onTelemetryChange,
    onThemeChange,
    registerValidator,
}: AdminAccountStepProps) {
    useEffect(() => {
        registerValidator(async () => {
            if (!username.trim() || !password.trim()) {
                return "Please provide an admin username and password.";
            }
            return null;
        });
    }, [password, registerValidator, username]);

    return (
        <motion.div key="account" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
            <div className="space-y-2">
                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                    <Lock size={20} className="text-helios-solar" />
                    Admin Access & Look
                </h2>
                <p className="text-sm text-helios-slate">Start with the basics: create the admin account and pick the default interface theme people will land on after setup.</p>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <div className="space-y-4">
                    <div>
                        <label className="block text-sm font-medium text-helios-slate mb-2">Admin Username</label>
                        <input type="text" value={username} onChange={(e) => onUsernameChange(e.target.value)} className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar outline-none" placeholder="admin" />
                    </div>
                    <div>
                        <label className="block text-sm font-medium text-helios-slate mb-2">Admin Password</label>
                        <input type="password" value={password} onChange={(e) => onPasswordChange(e.target.value)} className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar outline-none" placeholder="Choose a strong password" />
                    </div>
                    <label className="flex items-center justify-between rounded-2xl border border-helios-line/20 bg-helios-surface-soft/50 px-4 py-4">
                        <div>
                            <p className="text-sm font-semibold text-helios-ink">Anonymous Telemetry</p>
                            <p className="text-xs text-helios-slate mt-1">Help improve reliability and defaults with anonymous runtime signals.</p>
                        </div>
                        <input type="checkbox" checked={telemetryEnabled} onChange={(e) => onTelemetryChange(e.target.checked)} className="h-5 w-5 rounded border-helios-line/30 accent-helios-solar" />
                    </label>
                </div>

                <div className="space-y-4">
                    <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                        <Palette size={18} className="text-helios-solar" />
                        Default Theme
                    </div>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                        {THEME_OPTIONS.map((theme) => (
                            <button
                                key={theme.id}
                                type="button"
                                onClick={() => onThemeChange(theme.id)}
                                className={clsx(
                                    "rounded-2xl border px-4 py-4 text-left transition-all",
                                    activeThemeId === theme.id
                                        ? "border-helios-solar bg-helios-solar/10 text-helios-ink"
                                        : "border-helios-line/20 bg-helios-surface-soft/50 text-helios-slate hover:border-helios-solar/20"
                                )}
                            >
                                <div className="font-semibold">{theme.name}</div>
                                <div className="text-xs mt-1 opacity-80">Applied as the initial dashboard theme.</div>
                            </button>
                        ))}
                    </div>
                </div>
            </div>
        </motion.div>
    );
}
