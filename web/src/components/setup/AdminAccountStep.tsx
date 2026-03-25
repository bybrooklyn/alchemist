import { useEffect } from "react";
import { motion } from "framer-motion";
import { UserCircle } from "lucide-react";
import { ToggleRow } from "./SetupControls";
import type { StepValidator } from "./types";

interface AdminAccountStepProps {
    username: string;
    password: string;
    telemetryEnabled: boolean;
    onUsernameChange: (value: string) => void;
    onPasswordChange: (value: string) => void;
    onTelemetryChange: (value: boolean) => void;
    registerValidator: (validator: StepValidator) => void;
}

export default function AdminAccountStep({
    username,
    password,
    telemetryEnabled,
    onUsernameChange,
    onPasswordChange,
    onTelemetryChange,
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
        <motion.div
            key="account"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="space-y-8"
        >
            <div className="space-y-2">
                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                    <UserCircle size={20} className="text-helios-solar" />
                    Create Your Admin Account
                </h2>
                <p className="text-sm text-helios-slate">
                    Set up the account you'll use to access Alchemist.
                    You can change the interface theme after setup from
                    the Appearance settings.
                </p>
            </div>

            <div className="max-w-lg space-y-4">
                <div>
                    <label className="block text-xs font-medium text-helios-slate mb-2">
                        Admin Username
                    </label>
                    <input
                        type="text"
                        value={username}
                        onChange={(e) => onUsernameChange(e.target.value)}
                        className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-md px-4 py-3 text-helios-ink focus:border-helios-solar outline-none"
                        placeholder="admin"
                    />
                </div>

                <div>
                    <label className="block text-xs font-medium text-helios-slate mb-2">
                        Admin Password
                    </label>
                    <input
                        type="password"
                        value={password}
                        onChange={(e) => onPasswordChange(e.target.value)}
                        className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-md px-4 py-3 text-helios-ink focus:border-helios-solar outline-none"
                        placeholder="Choose a strong password"
                    />
                </div>

                <ToggleRow
                    title="Anonymous Usage Telemetry"
                    body="Alchemist can send anonymous, non-identifying signals to help improve hardware compatibility and default settings. No file names, paths, library contents, or personal data are ever collected. Off by default."
                    checked={telemetryEnabled}
                    onChange={onTelemetryChange}
                >
                    <details>
                        <summary className="flex list-none items-center gap-1 cursor-pointer select-none text-xs text-helios-solar hover:underline">
                            What gets sent?
                        </summary>
                        <ul className="mt-2 list-disc space-y-1 pl-4 text-xs text-helios-slate/80">
                            <li>App version and OS/architecture</li>
                            <li>Whether running in Docker</li>
                            <li>CPU core count and total RAM (no identifiers)</li>
                            <li>Encoder type (NVENC, QSV, CPU, etc.)</li>
                            <li>Codec and resolution bucket (1080p, 4K) — no filenames</li>
                            <li>Encode speed and success/failure outcome</li>
                        </ul>
                    </details>
                </ToggleRow>
            </div>
        </motion.div>
    );
}
