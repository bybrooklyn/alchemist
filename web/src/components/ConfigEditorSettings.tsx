import { useEffect, useState } from "react";
import { FileCode2, RefreshCw, Save } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface SettingsConfigResponse {
    raw_toml: string;
    source_of_truth: string;
    projection_status: string;
}

export default function ConfigEditorSettings() {
    const [rawToml, setRawToml] = useState("");
    const [sourceOfTruth, setSourceOfTruth] = useState("toml");
    const [projectionStatus, setProjectionStatus] = useState("unknown");
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");

    const fetchConfig = async () => {
        try {
            const data = await apiJson<SettingsConfigResponse>("/api/settings/config");
            setRawToml(data.raw_toml);
            setSourceOfTruth(data.source_of_truth);
            setProjectionStatus(data.projection_status);
            setError("");
        } catch (err) {
            setError(isApiError(err) ? err.message : "Unable to load config file.");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        void fetchConfig();
    }, []);

    const handleSave = async () => {
        setSaving(true);
        setError("");
        try {
            await apiAction("/api/settings/config", {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ raw_toml: rawToml }),
            });
            showToast({
                kind: "success",
                title: "Config",
                message: "TOML saved and synchronized.",
            });
            await fetchConfig();
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to save TOML.";
            setError(message);
            showToast({ kind: "error", title: "Config", message });
        } finally {
            setSaving(false);
        }
    };

    if (loading) {
        return <div className="p-8 text-helios-slate animate-pulse">Loading config editor…</div>;
    }

    return (
        <div className="space-y-6" aria-live="polite">
            <div className="flex items-start justify-between gap-4">
                <div className="space-y-2">
                    <div className="flex items-center gap-3">
                        <div className="p-2 bg-helios-solar/10 rounded-lg text-helios-solar">
                            <FileCode2 size={20} />
                        </div>
                        <div>
                            <h3 className="text-lg font-semibold text-helios-ink">Config Editor</h3>
                            <p className="text-xs text-helios-slate">
                                TOML is authoritative. Form saves and raw edits both write the same settings file and synchronized DB projection.
                            </p>
                        </div>
                    </div>
                    <div className="flex flex-wrap gap-2">
                        <span className="rounded-full border border-helios-line/30 px-3 py-1 text-xs font-medium text-helios-slate">
                            Source: {sourceOfTruth}
                        </span>
                        <span className="rounded-full border border-emerald-500/20 bg-emerald-500/10 px-3 py-1 text-xs font-medium text-emerald-500">
                            Projection: {projectionStatus}
                        </span>
                    </div>
                </div>

                <button
                    onClick={() => void fetchConfig()}
                    className="flex items-center gap-2 rounded-xl border border-helios-line/30 bg-helios-surface px-4 py-2 text-sm font-semibold text-helios-ink hover:bg-helios-surface-soft transition-colors"
                >
                    <RefreshCw size={16} />
                    Reload
                </button>
            </div>

            {error && (
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                    {error}
                </div>
            )}

            <textarea
                value={rawToml}
                onChange={(e) => setRawToml(e.target.value)}
                className="min-h-[520px] w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-4 font-mono text-sm leading-6 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                spellCheck={false}
            />

            <div className="flex justify-end">
                <button
                    onClick={() => void handleSave()}
                    disabled={saving}
                    className="flex items-center gap-2 rounded-xl bg-helios-solar px-6 py-3 font-bold text-helios-main hover:opacity-90 transition-opacity disabled:opacity-50"
                >
                    <Save size={18} />
                    {saving ? "Saving..." : "Validate & Apply"}
                </button>
            </div>
        </div>
    );
}
