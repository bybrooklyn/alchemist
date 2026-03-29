import { useState, useEffect } from "react";
import { Activity, Save } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import {
    TELEMETRY_TEMPORARILY_DISABLED,
    TELEMETRY_TEMPORARILY_DISABLED_MESSAGE,
    TELEMETRY_USAGE_COPY,
} from "../lib/telemetryAvailability";
import { showToast } from "../lib/toast";
import LibraryDoctor from "./LibraryDoctor";

interface SystemSettingsPayload {
    monitoring_poll_interval: number;
    enable_telemetry: boolean;
    watch_enabled: boolean;
}

interface EngineStatus {
    mode: "background" | "balanced" | "throughput";
    concurrent_limit: number;
    is_manual_override: boolean;
}

interface EngineMode {
    mode: "background" | "balanced" | "throughput";
    computed_limits: {
        background: number;
        balanced: number;
        throughput: number;
    };
    cpu_count: number;
}

export default function SystemSettings() {
    const [settings, setSettings] = useState<SystemSettingsPayload | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");
    const [success, setSuccess] = useState(false);
    const [engineMode, setEngineMode] = useState<EngineMode | null>(null);
    const [engineStatus, setEngineStatus] =
        useState<EngineStatus | null>(null);
    const [modeLoading, setModeLoading] = useState(false);

    useEffect(() => {
        void fetchSettings();

        const fetchEngineMode = async () => {
            try {
                const [mode, status] = await Promise.all([
                    apiJson<EngineMode>("/api/engine/mode"),
                    apiJson<EngineStatus>("/api/engine/status"),
                ]);
                setEngineMode(mode);
                setEngineStatus(status);
            } catch {
                // Non-critical — engine mode section stays hidden on error
            }
        };

        void fetchEngineMode();
    }, []);

    const fetchSettings = async () => {
        try {
            const data = await apiJson<SystemSettingsPayload>("/api/settings/system");
            setSettings({ ...data, enable_telemetry: false });
            setError("");
        } catch (err) {
            setError(isApiError(err) ? err.message : "Unable to load system settings.");
        } finally {
            setLoading(false);
        }
    };

    const handleSave = async () => {
        if (!settings) return;
        setSaving(true);
        setError("");
        setSuccess(false);

        try {
            await apiAction("/api/settings/system", {
                method: "POST",
                body: JSON.stringify({ ...settings, enable_telemetry: false }),
            });
            setSuccess(true);
            showToast({ kind: "success", title: "System", message: "System settings saved." });
            setTimeout(() => setSuccess(false), 3000);
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to save settings.";
            setError(message);
            showToast({ kind: "error", title: "System", message });
        } finally {
            setSaving(false);
        }
    };

    const handleModeChange = async (
        mode: "background" | "balanced" | "throughput"
    ) => {
        setModeLoading(true);
        try {
            await apiAction("/api/engine/mode", {
                method: "POST",
                body: JSON.stringify({ mode }),
            });
            const [updatedMode, updatedStatus] = await Promise.all([
                apiJson<EngineMode>("/api/engine/mode"),
                apiJson<EngineStatus>("/api/engine/status"),
            ]);
            setEngineMode(updatedMode);
            setEngineStatus(updatedStatus);
            showToast({
                kind: "success",
                title: "Engine",
                message: `Mode set to ${mode}.`,
            });
        } catch (err) {
            showToast({
                kind: "error",
                title: "Engine",
                message: isApiError(err)
                    ? err.message
                    : "Failed to update engine mode.",
            });
        } finally {
            setModeLoading(false);
        }
    };

    if (loading) {
        return <div className="p-8 text-helios-slate animate-pulse">Loading system settings...</div>;
    }

    if (!settings) {
        return <div className="p-8 text-red-500">Failed to load system settings.</div>;
    }

    return (
        <div className="flex flex-col gap-6" aria-live="polite">
            {/* Engine Mode */}
            {engineMode && engineStatus && (
                <div className="space-y-4">
                    <div className="flex items-center justify-between
                        pb-2 border-b border-helios-line/10">
                        <div>
                            <h3 className="text-base font-semibold
                                text-helios-ink">
                                Engine Mode
                            </h3>
                            <p className="text-xs text-helios-slate mt-0.5">
                                Controls how many jobs run concurrently.
                            </p>
                        </div>
                    </div>

                    <div className="flex gap-2">
                        {(["background", "balanced", "throughput"] as const).map((m) => (
                            <button
                                key={m}
                                type="button"
                                onClick={() => void handleModeChange(m)}
                                disabled={modeLoading}
                                className={`flex-1 rounded-lg border px-3
                                    py-2.5 text-sm font-medium capitalize
                                    transition-all disabled:opacity-50 ${
                                    engineStatus.mode === m
                                        ? "border-helios-solar bg-helios-solar/10 text-helios-solar"
                                        : "border-helios-line/20 text-helios-slate hover:border-helios-solar/30 hover:text-helios-ink"
                                }`}
                            >
                                {m}
                            </button>
                        ))}
                    </div>

                    <div className="rounded-lg border border-helios-line/20
                        bg-helios-surface-soft/40 px-4 py-3 space-y-1.5">
                        <p className="text-xs text-helios-slate">
                            Computed limits on this machine
                            ({engineMode.cpu_count} CPUs):
                        </p>
                        <div className="flex gap-4 text-xs font-mono">
                            <span className="text-helios-slate/70">
                                Background →{" "}
                                <span className="text-helios-ink font-medium">
                                    {engineMode.computed_limits.background}
                                </span>
                            </span>
                            <span className="text-helios-slate/70">
                                Balanced →{" "}
                                <span className="text-helios-ink font-medium">
                                    {engineMode.computed_limits.balanced}
                                </span>
                            </span>
                            <span className="text-helios-slate/70">
                                Throughput →{" "}
                                <span className="text-helios-ink font-medium">
                                    {engineMode.computed_limits.throughput}
                                </span>
                            </span>
                        </div>
                        {engineStatus.is_manual_override && (
                            <p className="text-xs text-helios-solar/80 italic">
                                Manual override active —{" "}
                                {engineStatus.concurrent_limit} concurrent job
                                {engineStatus.concurrent_limit !== 1 ? "s" : ""}.
                                Change mode to reset.
                            </p>
                        )}
                    </div>
                </div>
            )}

            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-semibold text-helios-ink">
                        System Monitoring
                    </h3>
                    <p className="text-xs text-helios-slate mt-0.5">Configure dashboard resource monitoring behavior.</p>
                </div>
                <div className="p-2 bg-helios-solar/10 rounded-lg text-helios-solar">
                    <Activity size={20} />
                </div>
            </div>

            {error && (
                <div className="p-4 bg-red-500/10 border border-red-500/20 text-red-500 rounded-lg text-sm font-semibold">{error}</div>
            )}

            {success && (
                <div className="p-4 bg-green-500/10 border border-green-500/20 text-green-500 rounded-lg text-sm font-semibold">
                    Settings saved successfully.
                </div>
            )}

            <div className="space-y-3">
                <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                    <Activity size={14} /> Monitoring Poll Interval
                </label>
                <div className="flex items-center gap-4 bg-helios-surface border border-helios-line/30 rounded-lg p-4">
                    <input
                        type="range"
                        min="0.5"
                        max="10"
                        step="0.5"
                        value={settings.monitoring_poll_interval}
                        onChange={(e) => setSettings({ ...settings, monitoring_poll_interval: parseFloat(e.target.value) })}
                        className="flex-1 h-2 bg-helios-surface-soft rounded-lg appearance-none cursor-pointer accent-helios-solar"
                    />
                    <span className="font-mono bg-helios-surface-soft border border-helios-line/30 rounded px-3 py-1 text-helios-ink w-20 text-center font-bold">
                        {settings.monitoring_poll_interval.toFixed(1)}s
                    </span>
                </div>
                <p className="text-xs text-helios-slate ml-1 pt-1">
                    Determine how frequently the dashboard updates system stats. Lower values update faster but use slightly more CPU. Default is 2.0s.
                </p>
            </div>

            <div className="pt-4 border-t border-helios-line/10">
                <div className="flex items-center justify-between">
                    <div>
                        <h4 className="text-xs font-medium text-helios-slate">
                            Watch Library Folders
                        </h4>
                        <p className="text-xs text-helios-slate mt-1">Automatically watch the library folders configured during setup. Custom watch folders remain active separately.</p>
                    </div>
                    <label className="relative inline-flex items-center cursor-pointer">
                        <input
                            type="checkbox"
                            checked={settings.watch_enabled}
                            onChange={(e) => setSettings({ ...settings, watch_enabled: e.target.checked })}
                            className="sr-only peer"
                        />
                        <div className="w-11 h-6 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-helios-solar"></div>
                    </label>
                </div>
            </div>

            <div className="pt-4 border-t border-helios-line/10">
                <div className="flex items-center justify-between">
                    <div>
                        <h4 className="text-xs font-medium text-helios-slate">
                            Anonymous Telemetry
                        </h4>
                        <p className="mt-1 text-xs text-helios-slate">{TELEMETRY_TEMPORARILY_DISABLED_MESSAGE}</p>
                        <p className="mt-1 text-xs text-helios-slate/80">{TELEMETRY_USAGE_COPY}</p>
                    </div>
                    <label className="relative inline-flex items-center cursor-pointer">
                        <input
                            type="checkbox"
                            aria-label="Anonymous Telemetry"
                            checked={false}
                            disabled={TELEMETRY_TEMPORARILY_DISABLED}
                            onChange={(e) => setSettings({ ...settings, enable_telemetry: e.target.checked })}
                            className="sr-only peer"
                        />
                        <div className="w-11 h-6 rounded-full bg-helios-line/20 peer-focus:outline-none after:absolute after:start-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:border after:border-gray-300 after:bg-white after:content-[''] after:transition-all peer-checked:after:translate-x-full peer-checked:after:border-white peer-checked:bg-helios-solar rtl:peer-checked:after:-translate-x-full peer-disabled:cursor-not-allowed peer-disabled:opacity-60"></div>
                    </label>
                </div>
            </div>

            <div className="flex justify-end pt-4 border-t border-helios-line/10">
                <button
                    onClick={handleSave}
                    disabled={saving}
                    className="flex items-center gap-2 bg-helios-solar text-helios-main text-sm font-semibold px-6 py-2.5 rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50"
                >
                    <Save size={18} />
                    {saving ? "Saving..." : "Save Settings"}
                </button>
            </div>

            <div className="border-t border-helios-line/10 pt-6">
                <LibraryDoctor />
            </div>
        </div>
    );
}
