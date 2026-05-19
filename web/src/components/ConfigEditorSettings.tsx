import { useEffect, useState } from "react";
import { CheckCircle2, RefreshCw, Save } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface SettingsConfigResponse {
    raw_toml: string;
    source_of_truth: string;
    projection_status: string;
}

interface ConfigValidationResponse {
    valid: boolean;
    warnings: string[];
    summary: {
        output_codec: string;
        replace_strategy: string;
        output_root_set: boolean;
        delete_source: boolean;
        watch_dirs: number;
        notification_targets: number;
        schedule_windows: number;
    };
}

export default function ConfigEditorSettings() {
    const [rawToml, setRawToml] = useState("");
    const [sourceOfTruth, setSourceOfTruth] = useState("toml");
    const [projectionStatus, setProjectionStatus] = useState("unknown");
    const [validation, setValidation] = useState<ConfigValidationResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [validating, setValidating] = useState(false);
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

    const handleValidate = async () => {
        setValidating(true);
        setError("");
        try {
            const result = await apiJson<ConfigValidationResponse>("/api/settings/config/validate", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ raw_toml: rawToml }),
            });
            setValidation(result);
        } catch (err) {
            setValidation(null);
            setError(isApiError(err) ? err.message : "Config validation failed.");
        } finally {
            setValidating(false);
        }
    };

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
            setValidation(null);
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
            <div className="flex items-center justify-between gap-4">
                <div className="flex flex-wrap gap-2">
                    <span className="rounded-full border border-helios-line/30 px-3 py-1 text-xs font-medium text-helios-slate">
                        Source: {sourceOfTruth}
                    </span>
                    <span className="rounded-full border border-emerald-500/20 bg-emerald-500/10 px-3 py-1 text-xs font-medium text-emerald-500">
                        Projection: {projectionStatus}
                    </span>
                </div>
                <button
                    onClick={() => void fetchConfig()}
                    className="flex items-center gap-2 rounded-lg border border-helios-line/30 bg-helios-surface px-4 py-2 text-sm font-semibold text-helios-ink hover:bg-helios-surface-soft transition-colors"
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
                aria-label="Raw TOML config"
                value={rawToml}
                onChange={(e) => {
                    setRawToml(e.target.value);
                    setValidation(null);
                }}
                className="min-h-[520px] w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-4 font-mono text-sm leading-6 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                spellCheck={false}
            />

            {validation && (
                <section
                    aria-labelledby="config-validation-title"
                    className="rounded-lg border border-helios-line/20 bg-helios-surface/70 p-4"
                >
                    <h3 id="config-validation-title" className="flex items-center gap-2 text-sm font-bold text-helios-ink">
                        <CheckCircle2 size={16} className="text-emerald-500" />
                        Validation passed
                    </h3>
                    <div className="mt-3 grid grid-cols-2 gap-3 text-xs sm:grid-cols-4">
                        <SummaryItem label="Codec" value={validation.summary.output_codec} />
                        <SummaryItem label="Replace" value={validation.summary.replace_strategy} />
                        <SummaryItem label="Watch dirs" value={String(validation.summary.watch_dirs)} />
                        <SummaryItem label="Notifications" value={String(validation.summary.notification_targets)} />
                    </div>
                    {validation.warnings.length > 0 ? (
                        <ul className="mt-3 space-y-1 text-xs text-helios-slate">
                            {validation.warnings.map((warning) => (
                                <li key={warning}>{warning}</li>
                            ))}
                        </ul>
                    ) : (
                        <p className="mt-3 text-xs text-helios-slate">
                            No high-risk file or library warnings detected.
                        </p>
                    )}
                </section>
            )}

            <div className="flex flex-wrap justify-end gap-3">
                <button
                    onClick={() => void handleValidate()}
                    disabled={validating || saving}
                    className="flex items-center gap-2 rounded-xl border border-helios-line/30 bg-helios-surface px-6 py-3 font-bold text-helios-ink hover:bg-helios-surface-soft transition-colors disabled:opacity-50"
                >
                    <CheckCircle2 size={18} />
                    {validating ? "Validating..." : "Validate"}
                </button>
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

function SummaryItem({ label, value }: { label: string; value: string }) {
    return (
        <div>
            <div className="text-helios-slate">{label}</div>
            <div className="mt-1 font-mono font-bold text-helios-ink">{value}</div>
        </div>
    );
}
