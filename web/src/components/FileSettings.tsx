import { useState, useEffect } from "react";
import { AlertTriangle, Save } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface FileSettings {
    delete_source: boolean;
    output_extension: string;
    output_suffix: string;
    replace_strategy: string;
    output_root: string | null;
}

export default function FileSettings() {
    const [settings, setSettings] = useState<FileSettings>({
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "keep",
        output_root: null,
    });
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        void fetchSettings();
    }, []);

    const fetchSettings = async () => {
        try {
            const data = await apiJson<FileSettings>("/api/settings/files");
            setSettings(data);
            setError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to load file settings";
            setError(message);
        } finally {
            setLoading(false);
        }
    };

    const handleSave = async () => {
        setSaving(true);
        setError(null);
        try {
            await apiAction("/api/settings/files", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(settings),
            });
            showToast({ kind: "success", title: "Files", message: "File settings saved." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to save file settings";
            setError(message);
            showToast({ kind: "error", title: "Files", message });
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="space-y-6" aria-live="polite">
            {error && (
                <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error text-sm">
                    {error}
                </div>
            )}

            {loading ? (
                <div className="text-sm text-helios-slate animate-pulse">Loading settings…</div>
            ) : (
                <div className="space-y-4">
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <div>
                            <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Output Suffix</label>
                            <input
                                type="text"
                                value={settings.output_suffix}
                                onChange={e => setSettings({ ...settings, output_suffix: e.target.value })}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                                placeholder="-alchemist"
                            />
                            <p className="text-xs text-helios-slate mt-1">
                                Appended to filename (e.g. video
                                <span className="text-helios-solar">{settings.output_suffix}</span>.{settings.output_extension})
                            </p>
                        </div>
                        <div>
                            <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Extension</label>
                            <select
                                value={settings.output_extension}
                                onChange={e => setSettings({ ...settings, output_extension: e.target.value })}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                            >
                                <option value="mkv">mkv</option>
                                <option value="mp4">mp4</option>
                            </select>
                        </div>
                    </div>

                    <div>
                        <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Output Root</label>
                        <input
                            type="text"
                            value={settings.output_root ?? ""}
                            onChange={e => setSettings({ ...settings, output_root: e.target.value || null })}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                            placeholder="Optional mirrored output directory"
                        />
                        <p className="text-xs text-helios-slate mt-1">
                            Leave blank to write alongside the source file. When set, Alchemist mirrors the source folder structure under this directory.
                        </p>
                    </div>

                    <div>
                        <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Existing Output Policy</label>
                        <select
                            value={settings.replace_strategy}
                            onChange={e => setSettings({ ...settings, replace_strategy: e.target.value })}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                        >
                            <option value="keep">Keep existing output</option>
                            <option value="replace">Replace after verified success</option>
                        </select>
                        <p className="text-xs text-helios-slate mt-1">
                            Replace mode now encodes to a temp file first and only promotes it after all verification gates pass.
                        </p>
                    </div>

                    <div className="p-4 bg-red-500/5 border border-red-500/20 rounded-xl space-y-3">
                        <div className="flex items-start gap-3">
                            <AlertTriangle className="text-red-500 shrink-0 mt-0.5" size={16} />
                            <div className="flex-1">
                                <h3 className="text-sm font-bold text-red-600 dark:text-red-400">Destructive Policy</h3>
                                <p className="text-xs text-helios-slate mt-1 mb-3">
                                    Enabling "Delete Source" will permanently remove the original file after a successful transcode. This action cannot be undone.
                                </p>
                                <label className="flex items-center gap-2 cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={settings.delete_source}
                                        onChange={e => setSettings({ ...settings, delete_source: e.target.checked })}
                                        className="rounded border-red-500/30 text-red-500 focus:ring-red-500 bg-red-500/10"
                                    />
                                    <span className="text-sm font-medium text-helios-ink">Delete source file after success</span>
                                </label>
                            </div>
                        </div>
                    </div>

                    <div className="flex justify-end pt-2">
                        <button
                            onClick={handleSave}
                            disabled={saving}
                            className="flex items-center gap-2 px-6 py-2 bg-helios-solar text-helios-main font-bold rounded-lg hover:opacity-90 transition-opacity disabled:opacity-50"
                        >
                            <Save size={16} />
                            {saving ? "Saving..." : "Save Settings"}
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
}
