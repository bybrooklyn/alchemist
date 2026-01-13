import { useState, useEffect } from "react";
import { FileOutput, AlertTriangle, Save } from "lucide-react";
import { apiFetch } from "../lib/api";

interface FileSettings {
    delete_source: boolean;
    output_extension: string;
    output_suffix: string;
    replace_strategy: string;
}

export default function FileSettings() {
    const [settings, setSettings] = useState<FileSettings>({
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "keep"
    });
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        fetchSettings();
    }, []);

    const fetchSettings = async () => {
        try {
            const res = await apiFetch("/api/settings/files");
            if (res.ok) setSettings(await res.json());
        } catch (e) { console.error(e); }
        finally { setLoading(false); }
    };

    const handleSave = async () => {
        setSaving(true);
        try {
            await apiFetch("/api/settings/files", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(settings)
            });
        } catch (e) {
            console.error(e);
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="space-y-6">
            <div className="flex items-center gap-3 mb-6">
                <div className="p-2 bg-helios-solar/10 rounded-lg">
                    <FileOutput className="text-helios-solar" size={20} />
                </div>
                <div>
                    <h2 className="text-lg font-semibold text-helios-ink">File Handling</h2>
                    <p className="text-xs text-helios-slate">Configure output naming and source file policies.</p>
                </div>
            </div>

            <div className="space-y-4">
                {/* Naming */}
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
                        <p className="text-[10px] text-helios-slate mt-1">Appended to filename (e.g. video<span className="text-helios-solar">{settings.output_suffix}</span>.{settings.output_extension})</p>
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

                {/* Deletion Policy */}
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
        </div>
    );
}
