import { useState, useEffect } from "react";
import { 
    Cpu, 
    Save, 
    Video, 
    Gauge, 
    Zap, 
    Scale, 
    Film 
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface TranscodeSettingsPayload {
    concurrent_jobs: number;
    size_reduction_threshold: number;
    min_bpp_threshold: number;
    min_file_size_mb: number;
    output_codec: "av1" | "hevc";
    quality_profile: "quality" | "balanced" | "speed";
}

export default function TranscodeSettings() {
    const [settings, setSettings] = useState<TranscodeSettingsPayload | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");
    const [success, setSuccess] = useState(false);

    useEffect(() => {
        fetchSettings();
    }, []);

    const fetchSettings = async () => {
        try {
            const res = await fetch("/api/settings/transcode");
            if (!res.ok) throw new Error("Failed to load settings");
            const data = await res.json();
            setSettings(data);
        } catch (err) {
            setError("Unable to load current settings.");
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
            const res = await fetch("/api/settings/transcode", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
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
        return <div className="p-8 text-helios-slate animate-pulse">Loading settings...</div>;
    }

    if (!settings) {
        return <div className="p-8 text-red-500">Failed to load settings.</div>;
    }

    return (
        <div className="flex flex-col gap-6">
             <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight uppercase tracking-[0.1em]">Transcoding Engine</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Configure encoder behavior and performance limits.</p>
                </div>
                <div className="p-2 bg-helios-solar/10 rounded-xl text-helios-solar">
                    <Cpu size={20} />
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

            <div className="grid gap-6 md:grid-cols-2">
                {/* Codec Selection */}
                <div className="md:col-span-2 space-y-3">
                    <label className="text-xs font-bold uppercase tracking-wider text-helios-slate flex items-center gap-2">
                        <Video size={14} /> Output Codec
                    </label>
                    <div className="grid grid-cols-2 gap-4">
                        <button
                            onClick={() => setSettings({ ...settings, output_codec: "av1" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-2xl border transition-all",
                                settings.output_codec === "av1"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-lg">AV1</span>
                            <span className="text-xs text-center opacity-70">Best compression, requires modern hardware.</span>
                        </button>
                        <button
                            onClick={() => setSettings({ ...settings, output_codec: "hevc" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-2xl border transition-all",
                                settings.output_codec === "hevc"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-lg">HEVC (H.265)</span>
                            <span className="text-xs text-center opacity-70">Broad compatibility, faster hardware encoding.</span>
                        </button>
                    </div>
                </div>

                {/* Quality Profile */}
                <div className="md:col-span-2 space-y-3 pt-4">
                    <label className="text-xs font-bold uppercase tracking-wider text-helios-slate flex items-center gap-2">
                        <Gauge size={14} /> Quality Profile
                    </label>
                    <div className="grid grid-cols-3 gap-3">
                        {(["speed", "balanced", "quality"] as const).map((profile) => (
                            <button
                                key={profile}
                                onClick={() => setSettings({ ...settings, quality_profile: profile })}
                                className={cn(
                                    "flex flex-col items-center justify-center p-3 rounded-xl border transition-all h-20",
                                    settings.quality_profile === profile
                                        ? "bg-helios-solar/10 border-helios-solar text-helios-ink font-bold shadow-sm"
                                        : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                                )}
                            >
                                <span className="capitalize">{profile}</span>
                            </button>
                        ))}
                    </div>
                </div>

                {/* Numeric Inputs */}
                <div className="space-y-3">
                    <label className="text-xs font-bold uppercase tracking-wider text-helios-slate flex items-center gap-2">
                        <Zap size={14} /> Concurrent Jobs
                    </label>
                     <input
                        type="number"
                        min="1"
                        max="8"
                        value={settings.concurrent_jobs}
                        onChange={(e) => setSettings({ ...settings, concurrent_jobs: parseInt(e.target.value) || 1 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
                    <p className="text-[10px] text-helios-slate ml-1">Maximum number of files to process simultaneously.</p>
                </div>

                <div className="space-y-3">
                    <label className="text-xs font-bold uppercase tracking-wider text-helios-slate flex items-center gap-2">
                        <Scale size={14} /> Min. Reduction (%)
                    </label>
                     <input
                        type="number"
                        min="0"
                        max="100"
                        step="5"
                        value={Math.round(settings.size_reduction_threshold * 100)}
                        onChange={(e) => setSettings({ ...settings, size_reduction_threshold: (parseInt(e.target.value) || 0) / 100 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
                     <p className="text-[10px] text-helios-slate ml-1">Files must shrink by at least this percentage or they are reverted.</p>
                </div>

                <div className="space-y-3">
                    <label className="text-xs font-bold uppercase tracking-wider text-helios-slate flex items-center gap-2">
                        <Film size={14} /> Min. File Size (MB)
                    </label>
                     <input
                        type="number"
                        min="0"
                        value={settings.min_file_size_mb}
                        onChange={(e) => setSettings({ ...settings, min_file_size_mb: parseInt(e.target.value) || 0 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
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
