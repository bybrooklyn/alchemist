import { useState, useEffect, useRef } from "react";
import { Cpu, Zap, HardDrive, CheckCircle2, AlertCircle, XCircle } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface HardwareInfo {
    vendor: string;
    device_path: string | null;
    supported_codecs: string[];
    backends?: Array<{
        kind: string;
        codec: string;
        encoder: string;
        device_path: string | null;
    }>;
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
    const [loadError, setLoadError] = useState("");
    const [saveError, setSaveError] = useState("");
    const [saving, setSaving] = useState(false);
    const [draftDevicePath, setDraftDevicePath] = useState("");
    const [devicePathDirty, setDevicePathDirty] = useState(false);
    const hardwareSettingsRef = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        void Promise.all([fetchHardware(), fetchSettings(), fetchProbeLog()]).finally(() => setLoading(false));
    }, []);

    const fetchHardware = async () => {
        try {
            const data = await apiJson<HardwareInfo>("/api/system/hardware");
            setInfo(data);
            setLoadError("");
        } catch (err) {
            setLoadError(
                isApiError(err)
                    ? err.message
                    : "Unable to detect hardware acceleration support."
            );
        }
    };

    const fetchSettings = async () => {
        try {
            const data = await apiJson<HardwareSettings>("/api/settings/hardware");
            setSettings(data);
            if (!devicePathDirty) {
                setDraftDevicePath(data.device_path ?? "");
            }
            setLoadError("");
        } catch (err) {
            setLoadError(isApiError(err) ? err.message : "Failed to fetch hardware settings.");
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

    const persistSettings = async (
        previousSettings: HardwareSettings,
        nextSettings: HardwareSettings,
        message: string,
        syncDevicePath: boolean,
    ) => {
        setSaving(true);
        setSettings(nextSettings);
        setSaveError("");
        try {
            await apiAction("/api/settings/hardware", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(nextSettings),
            });
            if (syncDevicePath) {
                setDraftDevicePath(nextSettings.device_path ?? "");
                setDevicePathDirty(false);
            }
            await Promise.all([fetchHardware(), fetchProbeLog()]);
            showToast({ kind: "success", title: "Hardware", message });
        } catch (err) {
            const errorMessage = isApiError(err) ? err.message : "Failed to update hardware settings";
            setSettings(previousSettings);
            setSaveError(errorMessage);
            showToast({ kind: "error", title: "Hardware", message: errorMessage });
        } finally {
            setSaving(false);
        }
    };

    const normalizedDraftDevicePath = () => draftDevicePath.trim() || null;

    const saveImmediateSettings = async (patch: Partial<HardwareSettings>) => {
        if (!settings) return;
        const previousSettings = settings;
        const shouldSyncDevicePath = devicePathDirty;
        const nextSettings: HardwareSettings = {
            ...settings,
            ...patch,
            device_path: shouldSyncDevicePath ? normalizedDraftDevicePath() : settings.device_path,
        };
        await persistSettings(
            previousSettings,
            nextSettings,
            "Hardware settings saved.",
            shouldSyncDevicePath,
        );
    };

    const commitDevicePath = async () => {
        if (!settings) return;
        const nextDevicePath = normalizedDraftDevicePath();
        if (nextDevicePath === settings.device_path) {
            setDraftDevicePath(settings.device_path ?? "");
            setDevicePathDirty(false);
            return;
        }
        await persistSettings(
            settings,
            { ...settings, device_path: nextDevicePath },
            "Hardware settings saved.",
            true,
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

    if (loadError || !info) {
        return (
            <div className="p-6 bg-red-500/10 border border-red-500/20 text-red-500 rounded-lg flex items-center gap-3" aria-live="polite">
                <AlertCircle size={20} />
                <span className="font-semibold">{loadError || "Hardware detection failed."}</span>
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

    const intelTechLabel = (() => {
        const backendKinds = new Set((info.backends ?? []).map((backend) => backend.kind.toLowerCase()));
        const hasVaapi = backendKinds.has("vaapi");
        const hasQsv = backendKinds.has("qsv");
        if (hasVaapi && hasQsv) return "VAAPI/QSV";
        if (hasVaapi) return "VAAPI";
        if (hasQsv) return "QSV";
        return "Auto";
    })();

    const getVendorDetails = (vendor: string) => {
        switch (normalizeVendor(vendor)) {
            case "nvidia": return { name: "NVIDIA", tech: "NVENC", color: "text-emerald-500", bg: "bg-emerald-500/10" };
            case "amd": return { name: "AMD", tech: "VAAPI/AMF", color: "text-red-500", bg: "bg-red-500/10" };
            case "intel": return { name: "Intel", tech: intelTechLabel, color: "text-blue-500", bg: "bg-blue-500/10" };
            case "apple": return { name: "Apple", tech: "VideoToolbox", color: "text-helios-slate", bg: "bg-helios-slate/10" };
            default: return { name: "CPU", tech: "Software Fallback", color: "text-helios-solar", bg: "bg-helios-solar/10" };
        }
    };

    const vendor = normalizeVendor(info.vendor);
    const details = getVendorDetails(info.vendor);
    const detectionNotes = info.detection_notes ?? [];
    const failedProbeEntries = probeLog.entries.filter((entry) => !entry.success);
    const shouldShowProbeLog = vendor === "cpu" || failedProbeEntries.length > 0;
    const intelVaapiDetected = vendor === "intel" && (info.backends ?? []).some((backend) => backend.kind.toLowerCase() === "vaapi");

    const handleHardwareSettingsBlur = (event: React.FocusEvent<HTMLDivElement>) => {
        const nextTarget = event.relatedTarget as Node | null;
        if (nextTarget && hardwareSettingsRef.current?.contains(nextTarget)) {
            return;
        }
        void commitDevicePath();
    };

    return (
        <div className="flex flex-col gap-6" aria-live="polite">
            {saveError && (
                <div className="p-4 bg-red-500/10 border border-red-500/20 text-red-500 rounded-lg text-sm font-semibold">
                    {saveError}
                </div>
            )}

            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight">Transcoding Hardware</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Detected acceleration engines and codec support.</p>
                </div>
                <div className={`p-2 ${details.bg} rounded-lg ${details.color}`}>
                    {vendor === "cpu" ? <Cpu size={20} /> : <Zap size={20} />}
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="bg-helios-surface border border-helios-line/30 rounded-lg p-5 shadow-sm">
                    <div className="flex items-center gap-3 mb-4">
                        <div className={`p-2.5 rounded-lg ${details.bg} ${details.color}`}>
                            <HardDrive size={18} />
                        </div>
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink">Active Device</h4>
                            <p className="text-xs text-helios-slate font-bold">{details.name} {details.tech}</p>
                        </div>
                    </div>

                    <div className="space-y-4">
                        <div>
                            <span className="text-xs font-medium text-helios-slate block mb-1.5 ml-0.5">Device Path</span>
                            <div className="bg-helios-surface-soft border border-helios-line/30 rounded-lg px-3 py-2 font-mono text-xs text-helios-ink shadow-inner">
                                {info.device_path || (vendor === "nvidia" ? "NVIDIA Driver (Direct)" : "Auto-detected Interface")}
                            </div>
                        </div>
                    </div>
                </div>

                <div className="bg-helios-surface border border-helios-line/30 rounded-lg p-5 shadow-sm">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2.5 rounded-lg bg-purple-500/10 text-purple-500">
                            <CheckCircle2 size={18} />
                        </div>
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink">Codec Support</h4>
                            <p className="text-xs text-helios-slate font-bold">Hardware verified encoders</p>
                        </div>
                    </div>

                    <div className="flex flex-wrap gap-2">
                        {info.supported_codecs.length > 0 ? info.supported_codecs.map(codec => (
                            <div key={codec} className="px-3 py-1.5 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-500 text-xs font-bold flex items-center gap-2">
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

            {intelVaapiDetected && (
                <div className="rounded-lg border border-blue-500/20 bg-blue-500/5 px-4 py-3">
                    <div className="flex gap-3">
                        <AlertCircle className="shrink-0 text-blue-500" size={18} />
                        <p className="text-xs leading-relaxed text-helios-slate">
                            Intel Arc detected via VAAPI (i915/xe driver). This is the recommended path for Arc GPUs on Linux.
                        </p>
                    </div>
                </div>
            )}

            {vendor === "cpu" && (
                <div className="p-4 bg-helios-solar/5 border border-helios-solar/10 rounded-lg">
                    <div className="flex gap-3">
                        <AlertCircle className="text-helios-solar shrink-0" size={18} />
                        <div className="space-y-1">
                            <h5 className="text-sm font-bold text-helios-ink">CPU Fallback Active</h5>
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
                                        <p className="text-xs text-helios-slate">
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
                <div
                    ref={hardwareSettingsRef}
                    onBlurCapture={handleHardwareSettingsBlur}
                    className="bg-helios-surface border border-helios-line/30 rounded-lg p-5 shadow-sm space-y-5"
                >
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            <div className="p-2.5 rounded-lg bg-blue-500/10 text-blue-500">
                                <Cpu size={18} />
                            </div>
                            <div>
                                <h4 className="text-sm font-bold text-helios-ink">CPU Encoding</h4>
                                <p className="text-xs text-helios-slate font-bold">
                                    {settings.allow_cpu_encoding ? "Enabled - CPU can be used for encoding" : "Disabled - GPU only mode"}
                                </p>
                            </div>
                        </div>
                        <label className="relative inline-flex items-center cursor-pointer">
                            <input
                                type="checkbox"
                                aria-label="Allow CPU Encoding"
                                checked={settings.allow_cpu_encoding}
                                onChange={(e) => void saveImmediateSettings({ allow_cpu_encoding: e.target.checked })}
                                disabled={saving}
                                className="sr-only peer"
                            />
                            <div className="w-11 h-6 rounded-full bg-helios-line/20 peer-focus:outline-none after:absolute after:start-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:border after:bg-white after:content-[''] after:transition-all peer-checked:after:translate-x-full peer-checked:bg-helios-solar peer-disabled:cursor-not-allowed peer-disabled:opacity-60"></div>
                        </label>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4 border-t border-helios-line/10 pt-5">
                        <div className="space-y-2">
                            <label htmlFor="hardware-preferred-vendor" className="text-xs font-medium text-helios-slate">Preferred Vendor</label>
                            <select
                                id="hardware-preferred-vendor"
                                value={settings.preferred_vendor ?? ""}
                                onChange={(e) => void saveImmediateSettings({
                                    preferred_vendor: e.target.value || null,
                                })}
                                className="w-full rounded-lg border border-helios-line/30 bg-helios-surface px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
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
                            <label htmlFor="hardware-cpu-preset" className="text-xs font-medium text-helios-slate">CPU Preset</label>
                            <select
                                id="hardware-cpu-preset"
                                value={settings.cpu_preset}
                                onChange={(e) => void saveImmediateSettings({ cpu_preset: e.target.value })}
                                className="w-full rounded-lg border border-helios-line/30 bg-helios-surface px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
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
                            <p className="text-xs font-bold text-helios-slate">Allow CPU Fallback</p>
                            <p className="text-xs text-helios-slate mt-1">Permit software encoding when the preferred GPU path is unavailable.</p>
                        </div>
                        <label className="relative inline-flex items-center cursor-pointer">
                            <input
                                type="checkbox"
                                aria-label="Allow CPU Fallback"
                                checked={settings.allow_cpu_fallback}
                                onChange={(e) => void saveImmediateSettings({ allow_cpu_fallback: e.target.checked })}
                                disabled={saving}
                                className="sr-only peer"
                            />
                            <div className="w-11 h-6 rounded-full bg-helios-line/20 peer-focus:outline-none after:absolute after:start-[2px] after:top-[2px] after:h-5 after:w-5 after:rounded-full after:border after:bg-white after:content-[''] after:transition-all peer-checked:after:translate-x-full peer-checked:bg-helios-solar peer-disabled:cursor-not-allowed peer-disabled:opacity-60"></div>
                        </label>
                    </div>

                    <div className="border-t border-helios-line/10 pt-5 space-y-3">
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink">Explicit Device Path</h4>
                            <p className="text-xs text-helios-slate font-bold mt-1">
                                Optional — Linux only. Pin QSV or VAAPI detection to a specific render node, or leave blank to auto-detect.
                            </p>
                        </div>
                        <div className="flex flex-col gap-2">
                            <input
                                aria-label="Explicit Device Path"
                                type="text"
                                value={draftDevicePath}
                                onChange={(e) => {
                                    setDraftDevicePath(e.target.value);
                                    setDevicePathDirty(true);
                                }}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter") {
                                        e.preventDefault();
                                        void commitDevicePath();
                                    }
                                }}
                                placeholder="Optional — Linux only (e.g. /dev/dri/renderD128)"
                                className="flex-1 bg-helios-surface-soft border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink font-mono text-sm focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            />
                            <p className="text-xs text-helios-slate">
                                Saves on blur or Enter. Other hardware changes will also carry the current device-path draft if you tab or click away.
                            </p>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
