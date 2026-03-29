import { useEffect, useState } from "react";
import { Save } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface SettingsBundleResponse {
    settings: {
        quality: {
            enable_vmaf: boolean;
            min_vmaf_score: number;
            revert_on_low_quality: boolean;
        };
        [key: string]: unknown;
    };
    source_of_truth: string;
    projection_status: string;
}

export default function QualitySettings() {
    const [bundle, setBundle] = useState<SettingsBundleResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");

    const fetchBundle = async () => {
        try {
            const data = await apiJson<SettingsBundleResponse>("/api/settings/bundle");
            setBundle(data);
            setError("");
        } catch (err) {
            setError(isApiError(err) ? err.message : "Unable to load quality settings.");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        void fetchBundle();
    }, []);

    const handleSave = async () => {
        if (!bundle) return;
        setSaving(true);
        try {
            await apiAction("/api/settings/bundle", {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(bundle.settings),
            });
            showToast({ kind: "success", title: "Quality", message: "Quality settings saved." });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to save quality settings.";
            setError(message);
            showToast({ kind: "error", title: "Quality", message });
        } finally {
            setSaving(false);
        }
    };

    if (loading) {
        return <div className="p-8 text-helios-slate animate-pulse">Loading quality settings…</div>;
    }

    if (!bundle) {
        return <div className="p-8 text-red-500">Failed to load quality settings.</div>;
    }

    return (
        <div className="space-y-6" aria-live="polite">
            {error && (
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                    {error}
                </div>
            )}

            <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/60 p-4 flex items-center justify-between">
                <div>
                    <p className="text-xs font-bold uppercase tracking-wider text-helios-slate">Enable VMAF</p>
                    <p className="text-xs text-helios-slate mt-1">Compute a quality score after encoding.</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                    <input
                        type="checkbox"
                        checked={bundle.settings.quality.enable_vmaf}
                        onChange={(e) => setBundle({
                            ...bundle,
                            settings: {
                                ...bundle.settings,
                                quality: {
                                    ...bundle.settings.quality,
                                    enable_vmaf: e.target.checked,
                                },
                            },
                        })}
                        className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-helios-solar"></div>
                </label>
            </div>

            <div className="space-y-3">
                <label className="text-xs font-bold uppercase tracking-wider text-helios-slate">Minimum VMAF Score</label>
                <input
                    type="number"
                    min="0"
                    max="100"
                    step="0.5"
                    value={bundle.settings.quality.min_vmaf_score}
                    onChange={(e) => setBundle({
                        ...bundle,
                        settings: {
                            ...bundle.settings,
                            quality: {
                                ...bundle.settings.quality,
                                min_vmaf_score: parseFloat(e.target.value) || 0,
                            },
                        },
                    })}
                    className="w-full rounded-xl border border-helios-line/30 bg-helios-surface px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                />
            </div>

            <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/60 p-4 flex items-center justify-between">
                <div>
                    <p className="text-xs font-bold uppercase tracking-wider text-helios-slate">Revert on Low Quality</p>
                    <p className="text-xs text-helios-slate mt-1">Keep the source if the VMAF score drops below the threshold.</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                    <input
                        type="checkbox"
                        checked={bundle.settings.quality.revert_on_low_quality}
                        onChange={(e) => setBundle({
                            ...bundle,
                            settings: {
                                ...bundle.settings,
                                quality: {
                                    ...bundle.settings.quality,
                                    revert_on_low_quality: e.target.checked,
                                },
                            },
                        })}
                        className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-helios-solar"></div>
                </label>
            </div>

            <div className="flex justify-end">
                <button
                    onClick={() => void handleSave()}
                    disabled={saving}
                    className="flex items-center gap-2 rounded-md bg-helios-solar px-6 py-3 font-bold text-helios-main hover:opacity-90 transition-opacity disabled:opacity-50"
                >
                    <Save size={18} />
                    {saving ? "Saving..." : "Save Quality Settings"}
                </button>
            </div>
        </div>
    );
}
