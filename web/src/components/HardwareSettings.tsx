import { useState, useEffect } from "react";
import { Cpu, Zap, HardDrive, CheckCircle2, AlertCircle, Save, XCircle } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface HardwareInfo {
    vendor: string;
    device_path: string | null;
    supported_codecs: string[];
    detection_notes?: string[];
}

interface HardwareProbeEntry {
    encoder: string;
    backend: string;
    device_path: string | null;
    success: boolean;
    stderr?: string | null;
}

interface HardwareProbeLog {
    entries: HardwareProbeEntry[];
}

interface HardwareSettings {
    allow_cpu_fallback: boolean;
    allow_cpu_encoding: boolean;
    cpu_preset: string;
    preferred_vendor: string | null;
    device_path: string | null;
}

export default function HardwareSettings() {
    const [info, setInfo] = useState<HardwareInfo | null>(null);
    const [settings, setSettings] = useState<HardwareSettings | null>(null);
    const [probeLog, setProbeLog] = useState<HardwareProbeLog>({ entries: [] });
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState("");
    const [saving, setSaving] = useState(false);
    const [draftDevicePath, setDraftDevicePath] = useState("");

    useEffect(() => {
        void Promise.all([fetchHardware(), fetchSettings(), fetchProbeLog()]).finally(() => setLoading(false));
    }, []);

    const fetchHardware = async () => {
        try {
            const data = await apiJson<HardwareInfo>("/api/system/hardware");
            setInfo(data);
            setError("");
        } catch (err) {
            setError(isApiError(err) ? err.message : "Unable to detect hardware acceleration support.");
        }
    };

    const fetchSettings = async () => {
        try {
            const data = await apiJson<HardwareSettings>("/api/settings/hardware");
            setSettings(data);
            setDraftDevicePath(data.device_path ?? "");
        } catch (err) {
            if (!error) {
                setError(isApiError(err) ? err.message : "Failed to fetch hardware settings.");
            }
        }
    };

    const fetchProbeLog = async () => {
        try {
            const data = await apiJson<HardwareProbeLog>("/api/system/hardware/probe-log");
            setProbeLog(data);
        } catch {
            setProbeLog({ entries: [] });
        }
    };

    const persistSettings = async (nextSettings: HardwareSettings, message: string) => {
        setSaving(true);
        try {
            await apiAction("/api/settings/hardware", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(nextSettings),
            });
            setError("");
            await Promise.all([fetchHardware(), fetchSettings(), fetchProbeLog()]);
            showToast({ kind: "success", title: "Hardware", message });
        } catch (err) {
            const errorMessage = isApiError(err) ? err.message : "Failed to update hardware settings";
            setError(errorMessage);
            showToast({ kind: "error", title: "Hardware", message: errorMessage });
        } finally {
            setSaving(false);
        }
    };

    const updateCpuEncoding = async (enabled: boolean) => {
        if (!settings) return;
        await persistSettings({ ...settings, allow_cpu_encoding: enabled }, "Hardware settings saved.");
    };

    const saveAllSettings = async () => {
        if (!settings) return;
        await persistSettings(
            { ...settings, device_path: draftDevicePath.trim() || null },
            "Hardware settings saved.",
        );
    };

    if (loading) {
        return (
            <div className="flex flex-col gap-4 animate-pulse">
                <div className="h-12 bg-helios-surface-soft rounded-lg w-full" />
                <div className="h-40 bg-helios-surface-soft rounded-lg w-full" />
            </div>
        );
    }

    if (error || !info) {
        return (
            <div className="p-6 bg-red-500/10 border border-red-500/20 text-red-500 rounded-lg flex items-center gap-3" aria-live="polite">
                <AlertCircle size={20} />
                <span className="font-semibold">{error || "Hardware detection failed."}</span>
            </div>
        );
    }

    const normalizeVendor = (vendor: string): "nvidia" | "amd" | "intel" | "apple" | "cpu" => {
        switch (vendor.toLowerCase()) {
            case "nvidia": return "nvidia";
            case "amd": return "amd";
            case "intel": return "intel";
            case "apple": return "apple";
            default: return "cpu";
        }
    };

    const getVendorDetails = (vendor: string) => {
        switch (normalizeVendor(vendor)) {
            case "nvidia": return { name: "NVIDIA", tech: "NVENC", color: "text-emerald-500", bg: "bg-emerald-500/10" };
            case "amd": return { name: "AMD", tech: "VAAPI/AMF", color: "text-red-500", bg: "bg-red-500/10" };
            case "intel": return { name: "Intel", tech: "QuickSync (QSV)", color: "text-blue-500", bg: "bg-blue-500/10" };
            case "apple": return { name: "Apple", tech: "VideoToolbox", color: "text-helios-slate", bg: "bg-helios-slate/10" };
            default: return { name: "CPU", tech: "Software Fallback", color: "text-helios-solar", bg: "bg-helios-solar/10" };
        }
    };

    const vendor = normalizeVendor(info.vendor);
    const details = getVendorDetails(info.vendor);
    const detectionNotes = info.detection_notes ?? [];
    const failedProbeEntries = probeLog.entries.filter((entry) => !entry.success);
    const shouldShowProbeLog = vendor === "cpu" || failedProbeEntries.length > 0;

    return (
        <div className="flex flex-col gap-6" aria-live="polite">
            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight uppercase tracking-[0.1em]">Transcoding Hardware</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Detected acceleration engines and codec support.</p>
                </div>
                <div className={`p-2 ${details.bg} rounded-xl ${details.color}`}>
                    {vendor === "cpu" ? <Cpu size={20} /> : <Zap size={20} />}
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="bg-helios-surface border border-helios-line/30 rounded-lg p-5 shadow-sm">
                    <div className="flex items-center gap-3 mb-4">
                        <div className={`p-2.5 rounded-xl ${details.bg} ${details.color}`}>
                            <HardDrive size={18} />
                        </div>
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink uppercase tracking-wider">Active Device</h4>
                            <p className="text-[10px] text-helios-slate font-bold">{details.name} {details.tech}</p>
                        </div>
                    </div>

                    <div className="space-y-4">
                        <div>
                            <span className="text-xs font-medium text-helios-slate uppercase tracking-wide block mb-1.5 ml-0.5">Device Path</span>
                            <div className="bg-helios-surface-soft border border-helios-line/30 rounded-lg px-3 py-2 font-mono text-xs text-helios-ink shadow-inner">
                                {info.device_path || (vendor === "nvidia" ? "NVIDIA Driver (Direct)" : "Auto-detected Interface")}
                            </div>
                        </div>
                    </div>
                </div>

                <div className="bg-helios-surface border border-helios-line/30 rounded-lg p-5 shadow-sm">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2.5 rounded-xl bg-purple-500/10 text-purple-500">
                            <CheckCircle2 size={18} />
                        </div>
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink uppercase tracking-wider">Codec Support</h4>
                            <p className="text-[10px] text-helios-slate font-bold">Hardware verified encoders</p>
                        </div>
                    </div>

                    <div className="flex flex-wrap gap-2">
                        {info.supported_codecs.length > 0 ? info.supported_codecs.map(codec => (
                            <div key={codec} className="px-3 py-1.5 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-500 text-xs font-bold uppercase tracking-wider flex items-center gap-2">
                                <div className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                                {codec}
                            </div>
                        )) : (
                            <div className="text-xs text-helios-slate italic bg-helios-surface-soft w-full p-2 text-center rounded-lg">
                                No hardware accelerated codecs found.
                            </div>
                        )}
                    </div>
                </div>
            </div>

            {vendor === "cpu" && (
                <div className="p-4 bg-helios-solar/5 border border-helios-solar/10 rounded-lg">
                    <div className="flex gap-3">
                        <AlertCircle className="text-helios-solar shrink-0" size={18} />
                        <div className="space-y-1">
                            <h5 className="text-sm font-bold text-helios-ink uppercase tracking-wider">CPU Fallback Active</h5>
                            <p className="text-xs text-helios-slate leading-relaxed">
                                GPU acceleration was not detected or is incompatible. Alchemist will use software encoding (SVT-AV1 / x264), which is significantly more resource intensive.
                            </p>
                            {detectionNotes.length > 0 && (
                                <div className="bg-amber-500/10 border border-amber-500/20 rounded-md px-4 py-3 mt-3">
                                    <p className="text-xs font-semibold text-amber-600">What was tried:</p>
                                    <ul className="mt-2 list-disc pl-4 space-y-1 text-xs text-helios-slate leading-relaxed">
                                        {detectionNotes.map((note) => (
                                            <li key={note}>{note}</li>
                                        ))}
                                    </ul>
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}

            {vendor !== "cpu" && detectionNotes.length > 0 && (
                <details className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                    <summary className="cursor-pointer text-xs font-medium text-helios-slate hover:text-helios-ink">
                        Detection notes
                    </summary>
                    <ul className="mt-3 list-disc pl-4 space-y-1 text-xs text-helios-slate leading-relaxed">
                        {detectionNotes.map((note) => (
                            <li key={note}>{note}</li>
                        ))}
                    </ul>
                </details>
            )}

            {shouldShowProbeLog && (
                <details className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/30 px-4 py-3">
                    <summary className="cursor-pointer text-xs font-medium text-helios-slate hover:text-helios-ink">
                        Show detection log
                    </summary>
                    <div className="mt-3 space-y-2">
                        {probeLog.entries.length > 0 ? probeLog.entries.map((entry, index) => {
                            const firstLine = entry.stderr?.split("\n")[0]?.trim();
                            const iconClassName = entry.success ? "text-emerald-500" : "text-red-500";

                            return (
                                <details
                                    key={`${entry.encoder}-${entry.backend}-${entry.device_path ?? "auto"}-${index}`}
                                    className="rounded-md border border-helios-line/15 bg-helios-surface/60 px-3 py-2"
                                >
                                    <summary className="cursor-pointer text-xs text-helios-slate">
                                        <span className="inline-flex items-center gap-2">
                                            {entry.success ? <CheckCircle2 size={12} className={iconClassName} /> : <XCircle size={12} className={iconClassName} />}
                                            <span className="font-medium text-helios-ink">{entry.encoder}</span>
                                            {!entry.success && firstLine && (
                                                <span className="text-helios-slate">{firstLine}</span>
                                            )}
                                        </span>
                                    </summary>
                                    <div className="mt-2 space-y-2">
                                        <p className="text-[11px] text-helios-slate">
                                            {entry.backend}
                                            {entry.device_path ? ` • ${entry.device_path}` : ""}
                                        </p>
                                        {entry.stderr && (
                                            <pre className="overflow-x-auto rounded bg-helios-main/70 p-2 text-xs text-helios-slate font-mono whitespace-pre-wrap break-words">
                                                {entry.stderr}
                                            </pre>
                                        )}
                                    </div>
                                </details>
                            );
                        }) : (
                            <p className="text-xs text-helios-slate">No encoder probes were recorded during detection.</p>
                        )}
                    </div>
                </details>
            )}

            {settings && (
                <div className="bg-helios-surface border border-helios-line/30 rounded-lg p-5 shadow-sm space-y-5">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <div className="p-2.5 rounded-xl bg-blue-500/10 text-blue-500">
                                <Cpu size={18} />
                            </div>
                            <div>
                                <h4 className="text-sm font-bold text-helios-ink uppercase tracking-wider">CPU Encoding</h4>
                                <p className="text-[10px] text-helios-slate font-bold">
                                    {settings.allow_cpu_encoding ? "Enabled - CPU can be used for encoding" : "Disabled - GPU only mode"}
                                </p>
                            </div>
                        </div>
                        <button
                            onClick={() => void updateCpuEncoding(!settings.allow_cpu_encoding)}
                            disabled={saving}
                            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${settings.allow_cpu_encoding ? "bg-emerald-500" : "bg-helios-line/50"} ${saving ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
                        >
                            <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${settings.allow_cpu_encoding ? "translate-x-6" : "translate-x-1"}`} />
                        </button>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4 border-t border-helios-line/10 pt-5">
                        <div className="space-y-2">
                            <label className="text-xs font-medium uppercase tracking-wide text-helios-slate">Preferred Vendor</label>
                            <select
                                value={settings.preferred_vendor ?? ""}
                                onChange={(e) => setSettings({
                                    ...settings,
                                    preferred_vendor: e.target.value || null,
                                })}
                                className="w-full rounded-xl border border-helios-line/30 bg-helios-surface px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            >
                                <option value="">Auto-detect</option>
                                <option value="nvidia">NVIDIA</option>
                                <option value="amd">AMD</option>
                                <option value="intel">Intel</option>
                                <option value="apple">Apple</option>
                                <option value="cpu">CPU</option>
                            </select>
                        </div>

                        <div className="space-y-2">
                            <label className="text-xs font-medium uppercase tracking-wide text-helios-slate">CPU Preset</label>
                            <select
                                value={settings.cpu_preset}
                                onChange={(e) => setSettings({ ...settings, cpu_preset: e.target.value })}
                                className="w-full rounded-xl border border-helios-line/30 bg-helios-surface px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            >
                                <option value="slow">Slow</option>
                                <option value="medium">Medium</option>
                                <option value="fast">Fast</option>
                                <option value="faster">Faster</option>
                            </select>
                        </div>
                    </div>

                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/60 p-4 flex items-center justify-between">
                        <div>
                            <p className="text-xs font-bold uppercase tracking-wider text-helios-slate">Allow CPU Fallback</p>
                            <p className="text-[10px] text-helios-slate mt-1">Permit software encoding when the preferred GPU path is unavailable.</p>
                        </div>
                        <label className="relative inline-flex items-center cursor-pointer">
                            <input
                                type="checkbox"
                                checked={settings.allow_cpu_fallback}
                                onChange={(e) => setSettings({ ...settings, allow_cpu_fallback: e.target.checked })}
                                className="sr-only peer"
                            />
                            <div className="w-11 h-6 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-helios-solar"></div>
                        </label>
                    </div>

                    <div className="border-t border-helios-line/10 pt-5 space-y-3">
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink uppercase tracking-wider">Explicit Device Path</h4>
                            <p className="text-[10px] text-helios-slate font-bold mt-1">
                                Pin Linux QSV or VAAPI detection to a specific render node. Leave blank to auto-detect.
                            </p>
                        </div>
                        <div className="flex flex-col sm:flex-row gap-3">
                            <input
                                type="text"
                                value={draftDevicePath}
                                onChange={(e) => setDraftDevicePath(e.target.value)}
                                placeholder="/dev/dri/renderD128"
                                className="flex-1 bg-helios-surface-soft border border-helios-line/30 rounded-xl px-4 py-3 text-helios-ink font-mono text-sm focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            />
                            <button
                                onClick={() => void saveAllSettings()}
                                disabled={saving}
                                className="flex items-center justify-center gap-2 bg-helios-solar text-helios-main font-bold px-5 py-3 rounded-md hover:opacity-90 transition-opacity disabled:opacity-50"
                            >
                                <Save size={16} />
                                {saving ? "Saving..." : "Apply"}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
