import { useState, useEffect } from "react";
import { Activity, Save } from "lucide-react";
import { apiFetch } from "../lib/api";

interface SystemSettingsPayload {
    monitoring_poll_interval: number;
    enable_telemetry: boolean;
}

export default function SystemSettings() {
    const [settings, setSettings] = useState<SystemSettingsPayload | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");
    const [success, setSuccess] = useState(false);

    useEffect(() => {
        fetchSettings();
    }, []);

    const fetchSettings = async () => {
        try {
            const res = await apiFetch("/api/settings/system");
            if (!res.ok) throw new Error("Failed to load settings");
            const data = await res.json();
            setSettings(data);
        } catch (err) {
            setError("Unable to load system settings.");
            console.error(err);
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
            const res = await apiFetch("/api/settings/system", {
                method: "POST",
                body: JSON.stringify(settings),
            });
            if (!res.ok) throw new Error("Failed to save settings");
            setSuccess(true);
            setTimeout(() => setSuccess(false), 3000);
        } catch (err) {
            setError("Failed to save settings.");
        } finally {
            setSaving(false);
        }
    };

    if (loading) {
        return <div className="p-8 text-helios-slate animate-pulse">Loading system settings...</div>;
    }

    if (!settings) {
        return <div className="p-8 text-red-500">Failed to load system settings.</div>;
    }

    return (
        <div className="flex flex-col gap-6">
            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight uppercase tracking-[0.1em]">System Monitoring</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Configure dashboard resource monitoring behavior.</p>
                </div>
                <div className="p-2 bg-helios-solar/10 rounded-xl text-helios-solar">
                    <Activity size={20} />
                </div>
            </div>

            {error && (
                <div className="p-4 bg-red-500/10 border border-red-500/20 text-red-500 rounded-xl text-sm font-semibold">
                    {error}
                </div>
            )}

            {success && (
                <div className="p-4 bg-green-500/10 border border-green-500/20 text-green-500 rounded-xl text-sm font-semibold">
                    Settings saved successfully.
                </div>
            )}

            <div className="space-y-3">
                <label className="text-xs font-bold uppercase tracking-wider text-helios-slate flex items-center gap-2">
                    <Activity size={14} /> Monitoring Poll Interval
                </label>
                <div className="flex items-center gap-4 bg-helios-surface border border-helios-line/30 rounded-xl p-4">
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
                <p className="text-[10px] text-helios-slate ml-1 pt-1">Determine how frequently the dashboard updates system stats. Lower values update faster but use slightly more CPU. Default is 2.0s.</p>
            </div>

            <div className="pt-4 border-t border-helios-line/10">
                <div className="flex items-center justify-between">
                    <div>
                        <h4 className="text-xs font-bold uppercase tracking-wider text-helios-slate">Anonymous Telemetry</h4>
                        <p className="text-[10px] text-helios-slate mt-1">Help improve the app by sending anonymous crash reports and usage data.</p>
                    </div>
                    <label className="relative inline-flex items-center cursor-pointer">
                        <input
                            type="checkbox"
                            checked={settings.enable_telemetry}
                            onChange={(e) => setSettings({ ...settings, enable_telemetry: e.target.checked })}
                            className="sr-only peer"
                        />
                        <div className="w-11 h-6 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-helios-solar"></div>
                    </label>
                </div>
            </div>

            <div className="flex justify-end pt-4 border-t border-helios-line/10">
                <button
                    onClick={handleSave}
                    disabled={saving}
                    className="flex items-center gap-2 bg-helios-solar text-helios-main font-bold px-6 py-3 rounded-xl hover:opacity-90 transition-opacity disabled:opacity-50"
                >
                    <Save size={18} />
                    {saving ? "Saving..." : "Save Settings"}
                </button>
            </div>
        </div>
    );
}
