import { motion } from "framer-motion";
import { UserCircle } from "lucide-react";
import {
    TELEMETRY_TEMPORARILY_DISABLED,
    TELEMETRY_TEMPORARILY_DISABLED_MESSAGE,
    TELEMETRY_USAGE_COPY,
} from "../../lib/telemetryAvailability";
import { LabeledInput, ToggleRow } from "./SetupControls";

interface AdminAccountStepProps {
    username: string;
    password: string;
    usernameError?: string;
    passwordError?: string;
    onUsernameChange: (value: string) => void;
    onPasswordChange: (value: string) => void;
    onEnter: () => void;
}

export default function AdminAccountStep({
    username,
    password,
    usernameError,
    passwordError,
    onUsernameChange,
    onPasswordChange,
    onEnter,
}: AdminAccountStepProps) {
    const handleEnter = (event: { key: string }) => {
        if (event.key === "Enter") {
            onEnter();
        }
    };

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
                <LabeledInput
                    label="Admin Username"
                    value={username}
                    onChange={onUsernameChange}
                    onKeyDown={handleEnter}
                    placeholder="admin"
                    helperText="At least 3 characters."
                    error={usernameError}
                />

                <LabeledInput
                    label="Admin Password"
                    type="password"
                    value={password}
                    onChange={onPasswordChange}
                    onKeyDown={handleEnter}
                    placeholder="Choose a strong password"
                    helperText="At least 8 characters."
                    error={passwordError}
                />

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
