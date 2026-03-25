import { motion } from "framer-motion";
import { UserCircle } from "lucide-react";
import {
    TELEMETRY_TEMPORARILY_DISABLED,
    TELEMETRY_TEMPORARILY_DISABLED_MESSAGE,
    TELEMETRY_USAGE_COPY,
} from "../../lib/telemetryAvailability";
import { ToggleRow } from "./SetupControls";
import type { StepValidator } from "./types";

interface AdminAccountStepProps {
    username: string;
    password: string;
    onUsernameChange: (value: string) => void;
    onPasswordChange: (value: string) => void;
    registerValidator: (validator: StepValidator) => void;
}

export default function AdminAccountStep({
    username,
    password,
    onUsernameChange,
    onPasswordChange,
    registerValidator,
}: AdminAccountStepProps) {
    registerValidator(async () => {
        if (!username.trim() || !password.trim()) {
            return "Please provide an admin username and password.";
        }
        return null;
    });

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
                    body={TELEMETRY_TEMPORARILY_DISABLED_MESSAGE}
                    checked={false}
                    onChange={() => undefined}
                    disabled={TELEMETRY_TEMPORARILY_DISABLED}
                >
                    <p className="text-xs text-helios-slate/80">{TELEMETRY_USAGE_COPY}</p>
                </ToggleRow>
            </div>
        </motion.div>
    );
}
