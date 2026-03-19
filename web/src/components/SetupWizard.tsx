import { useEffect, useMemo, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
    ArrowRight,
    CheckCircle,
    Cpu,
    FileCog,
    FolderOpen,
    Lock,
    Palette,
    Search,
    ShieldCheck,
    Sparkles,
    Bell,
    Calendar,
    Video,
} from "lucide-react";
import clsx from "clsx";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ServerDirectoryPicker from "./ui/ServerDirectoryPicker";

interface NotificationTargetConfig {
    name: string;
    target_type: string;
    endpoint_url: string;
    auth_token: string | null;
    events: string[];
    enabled: boolean;
}

interface ScheduleWindowConfig {
    start_time: string;
    end_time: string;
    days_of_week: number[];
    enabled: boolean;
}

interface SetupSettings {
    appearance: {
        active_theme_id: string | null;
    };
    scanner: {
        directories: string[];
        watch_enabled: boolean;
        extra_watch_dirs: Array<{ path: string; is_recursive: boolean }>;
    };
    transcode: {
        concurrent_jobs: number;
        size_reduction_threshold: number;
        min_bpp_threshold: number;
        min_file_size_mb: number;
        output_codec: "av1" | "hevc" | "h264";
        quality_profile: "quality" | "balanced" | "speed";
        allow_fallback: boolean;
        subtitle_mode: "copy" | "burn" | "extract" | "none";
    };
    hardware: {
        allow_cpu_encoding: boolean;
        allow_cpu_fallback: boolean;
        preferred_vendor: string | null;
        cpu_preset: "slow" | "medium" | "fast" | "faster";
        device_path: string | null;
    };
    files: {
        delete_source: boolean;
        output_extension: string;
        output_suffix: string;
        replace_strategy: string;
        output_root: string | null;
    };
    quality: {
        enable_vmaf: boolean;
        min_vmaf_score: number;
        revert_on_low_quality: boolean;
    };
    notifications: {
        enabled: boolean;
        targets: NotificationTargetConfig[];
    };
    schedule: {
        windows: ScheduleWindowConfig[];
    };
    system: {
        enable_telemetry: boolean;
        monitoring_poll_interval: number;
    };
}

interface SettingsBundleResponse {
    settings: SetupSettings;
}

interface SetupStatusResponse {
    setup_required: boolean;
    enable_telemetry?: boolean;
    config_mutable?: boolean;
}

interface HardwareInfo {
    vendor: "Nvidia" | "Amd" | "Intel" | "Apple" | "Cpu";
    device_path: string | null;
    supported_codecs: string[];
}

interface ScanStatus {
    is_running: boolean;
    files_found: number;
    files_added: number;
    current_folder: string | null;
}

interface FsRecommendation {
    path: string;
    label: string;
    reason: string;
    media_hint: "high" | "medium" | "low" | "unknown";
}

interface FsRecommendationsResponse {
    recommendations: FsRecommendation[];
}

interface FsPreviewDirectory {
    path: string;
    exists: boolean;
    readable: boolean;
    media_files: number;
    sample_files: string[];
    media_hint: "high" | "medium" | "low" | "unknown";
    warnings: string[];
}

interface FsPreviewResponse {
    directories: FsPreviewDirectory[];
    total_media_files: number;
    warnings: string[];
}

const THEME_OPTIONS = [
    { id: "helios-orange", name: "Helios Orange" },
    { id: "sunset", name: "Sunset" },
    { id: "midnight", name: "Midnight" },
    { id: "emerald", name: "Emerald" },
    { id: "deep-blue", name: "Deep Blue" },
    { id: "lavender", name: "Lavender" },
];

const EVENT_OPTIONS = ["completed", "failed", "queued"];
const WEEKDAY_OPTIONS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const DEFAULT_SETTINGS: SetupSettings = {
    appearance: { active_theme_id: "helios-orange" },
    scanner: { directories: [], watch_enabled: true, extra_watch_dirs: [] },
    transcode: {
        concurrent_jobs: 2,
        size_reduction_threshold: 0.3,
        min_bpp_threshold: 0.1,
        min_file_size_mb: 100,
        output_codec: "av1",
        quality_profile: "balanced",
        allow_fallback: true,
        subtitle_mode: "copy",
    },
    hardware: {
        allow_cpu_encoding: true,
        allow_cpu_fallback: true,
        preferred_vendor: null,
        cpu_preset: "medium",
        device_path: null,
    },
    files: {
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "keep",
        output_root: null,
    },
    quality: {
        enable_vmaf: false,
        min_vmaf_score: 90,
        revert_on_low_quality: true,
    },
    notifications: {
        enabled: false,
        targets: [],
    },
    schedule: {
        windows: [],
    },
    system: {
        enable_telemetry: true,
        monitoring_poll_interval: 2,
    },
};

export default function SetupWizard() {
    const [step, setStep] = useState(1);
    const [loading, setLoading] = useState(true);
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [hardware, setHardware] = useState<HardwareInfo | null>(null);
    const [configMutable, setConfigMutable] = useState(true);
    const [settings, setSettings] = useState<SetupSettings>(DEFAULT_SETTINGS);
    const [username, setUsername] = useState("");
    const [password, setPassword] = useState("");
    const [dirInput, setDirInput] = useState("");
    const [scheduleDraft, setScheduleDraft] = useState<ScheduleWindowConfig>({
        start_time: "22:00",
        end_time: "06:00",
        days_of_week: [0, 1, 2, 3, 4, 5, 6],
        enabled: true,
    });
    const [notificationDraft, setNotificationDraft] = useState<NotificationTargetConfig>({
        name: "",
        target_type: "discord",
        endpoint_url: "",
        auth_token: null,
        events: ["completed", "failed"],
        enabled: true,
    });
    const [recommendations, setRecommendations] = useState<FsRecommendation[]>([]);
    const [preview, setPreview] = useState<FsPreviewResponse | null>(null);
    const [previewLoading, setPreviewLoading] = useState(false);
    const [previewError, setPreviewError] = useState<string | null>(null);
    const [scanStatus, setScanStatus] = useState<ScanStatus | null>(null);
    const [scanError, setScanError] = useState<string | null>(null);
    const [pickerOpen, setPickerOpen] = useState(false);
    const scanIntervalRef = useRef<number | null>(null);

    const loadBootstrap = async () => {
        setLoading(true);
        try {
            const [status, bundle, hw, recommendationData] = await Promise.all([
                apiJson<SetupStatusResponse>("/api/setup/status"),
                apiJson<SettingsBundleResponse>("/api/settings/bundle"),
                apiJson<HardwareInfo>("/api/system/hardware"),
                apiJson<FsRecommendationsResponse>("/api/fs/recommendations"),
            ]);

            setConfigMutable(status.config_mutable ?? true);
            setHardware(hw);
            setRecommendations(recommendationData.recommendations);
            setSettings({
                ...DEFAULT_SETTINGS,
                ...bundle.settings,
                appearance: {
                    ...DEFAULT_SETTINGS.appearance,
                    ...bundle.settings.appearance,
                },
                scanner: {
                    ...DEFAULT_SETTINGS.scanner,
                    ...bundle.settings.scanner,
                },
                transcode: {
                    ...DEFAULT_SETTINGS.transcode,
                    ...bundle.settings.transcode,
                },
                hardware: {
                    ...DEFAULT_SETTINGS.hardware,
                    ...bundle.settings.hardware,
                },
                files: {
                    ...DEFAULT_SETTINGS.files,
                    ...bundle.settings.files,
                },
                quality: {
                    ...DEFAULT_SETTINGS.quality,
                    ...bundle.settings.quality,
                },
                notifications: {
                    ...DEFAULT_SETTINGS.notifications,
                    ...bundle.settings.notifications,
                },
                schedule: {
                    ...DEFAULT_SETTINGS.schedule,
                    ...bundle.settings.schedule,
                },
                system: {
                    ...DEFAULT_SETTINGS.system,
                    ...bundle.settings.system,
                    enable_telemetry:
                        typeof status.enable_telemetry === "boolean"
                            ? status.enable_telemetry
                            : bundle.settings.system.enable_telemetry,
                },
            });
            setError(null);
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to load setup defaults.";
            setError(message);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        void loadBootstrap();
        return () => {
            if (scanIntervalRef.current !== null) {
                window.clearInterval(scanIntervalRef.current);
            }
        };
    }, []);

    useEffect(() => {
        if (step !== 2 || settings.scanner.directories.length === 0) {
            return;
        }
        const handle = window.setTimeout(() => {
            void fetchPreview();
        }, 350);
        return () => window.clearTimeout(handle);
    }, [step, settings.scanner.directories]);

    const fetchPreview = async (): Promise<FsPreviewResponse | null> => {
        if (settings.scanner.directories.length === 0) {
            setPreview(null);
            setPreviewError(null);
            return null;
        }
        setPreviewLoading(true);
        try {
            const data = await apiJson<FsPreviewResponse>("/api/fs/preview", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ directories: settings.scanner.directories }),
            });
            setPreview(data);
            setPreviewError(null);
            return data;
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to preview selected folders.";
            setPreviewError(message);
            return null;
        } finally {
            setPreviewLoading(false);
        }
    };

    const clearScanPolling = () => {
        if (scanIntervalRef.current !== null) {
            window.clearInterval(scanIntervalRef.current);
            scanIntervalRef.current = null;
        }
    };

    const pollScanStatus = async () => {
        clearScanPolling();
        const poll = async () => {
            try {
                const data = await apiJson<ScanStatus>("/api/scan/status");
                setScanStatus(data);
                setScanError(null);
                if (!data.is_running) {
                    clearScanPolling();
                    setSubmitting(false);
                }
            } catch (err) {
                const message = isApiError(err) ? err.message : "Scan status unavailable";
                setScanError(message);
                clearScanPolling();
                setSubmitting(false);
            }
        };
        await poll();
        scanIntervalRef.current = window.setInterval(() => {
            void poll();
        }, 1000);
    };

    const startScan = async () => {
        setSubmitting(true);
        setScanError(null);
        try {
            await apiAction("/api/scan/start", { method: "POST" });
            await pollScanStatus();
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to start scan";
            setScanError(message);
            setSubmitting(false);
        }
    };

    const addDirectory = (path: string) => {
        const normalized = path.trim();
        if (!normalized || settings.scanner.directories.includes(normalized)) {
            return;
        }
        setSettings((prev) => ({
            ...prev,
            scanner: {
                ...prev.scanner,
                directories: [...prev.scanner.directories, normalized],
            },
        }));
        setDirInput("");
    };

    const removeDirectory = (path: string) => {
        setSettings((prev) => ({
            ...prev,
            scanner: {
                ...prev.scanner,
                directories: prev.scanner.directories.filter((dir) => dir !== path),
            },
        }));
    };

    const addNotificationTarget = () => {
        if (!notificationDraft.name.trim() || !notificationDraft.endpoint_url.trim()) {
            return;
        }
        setSettings((prev) => ({
            ...prev,
            notifications: {
                ...prev.notifications,
                targets: [...prev.notifications.targets, { ...notificationDraft }],
            },
        }));
        setNotificationDraft({
            name: "",
            target_type: "discord",
            endpoint_url: "",
            auth_token: null,
            events: ["completed", "failed"],
            enabled: true,
        });
    };

    const addScheduleWindow = () => {
        if (!scheduleDraft.start_time || !scheduleDraft.end_time || scheduleDraft.days_of_week.length === 0) {
            return;
        }
        setSettings((prev) => ({
            ...prev,
            schedule: {
                windows: [...prev.schedule.windows, { ...scheduleDraft }],
            },
        }));
    };

    const validateStep = async () => {
        if (step === 1) {
            if (!username.trim() || !password.trim()) {
                setError("Please provide an admin username and password.");
                return false;
            }
        }

        if (step === 2) {
            if (settings.scanner.directories.length === 0) {
                setError("Select at least one server folder before continuing.");
                return false;
            }
            const nextPreview = await fetchPreview();
            if (nextPreview && nextPreview.total_media_files === 0) {
                setError("Preview did not find any supported media files yet. Double-check the chosen folders.");
                return false;
            }
        }

        setError(null);
        return true;
    };

    const handleNext = async () => {
        const valid = await validateStep();
        if (!valid) return;

        if (step === 5) {
            await handleSubmit();
            return;
        }

        setStep((current) => Math.min(current + 1, 6));
    };

    const handleSubmit = async () => {
        setSubmitting(true);
        setError(null);
        try {
            await apiAction("/api/setup/complete", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    username,
                    password,
                    settings,
                }),
            });

            setStep(6);
            setScanStatus(null);
            setScanError(null);
            await startScan();
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to save setup configuration.";
            setError(message);
            setSubmitting(false);
        }
    };

    const setupSummary = useMemo(
        () => [
            { label: "Server folders", value: `${settings.scanner.directories.length}` },
            { label: "Previewed media files", value: preview ? `${preview.total_media_files}` : "Pending" },
            { label: "Notification targets", value: `${settings.notifications.targets.length}` },
            { label: "Schedule windows", value: `${settings.schedule.windows.length}` },
        ],
        [preview, settings.notifications.targets.length, settings.schedule.windows.length, settings.scanner.directories.length]
    );

    return (
        <div className="bg-helios-surface border border-helios-line/60 rounded-3xl overflow-hidden shadow-2xl max-w-5xl w-full mx-auto">
            <div className="h-1 bg-helios-surface-soft w-full flex">
                <motion.div
                    className="bg-helios-solar h-full"
                    initial={{ width: 0 }}
                    animate={{ width: `${(step / 6) * 100}%` }}
                />
            </div>

            <div className="p-8 lg:p-10">
                <header className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between mb-8">
                    <div className="flex items-center gap-4">
                        <div className="w-12 h-12 rounded-xl bg-helios-solar text-helios-main flex items-center justify-center font-bold text-2xl shadow-lg shadow-helios-solar/20">
                            A
                        </div>
                        <div>
                            <h1 className="text-2xl font-bold text-helios-ink">Alchemist Setup</h1>
                            <p className="text-sm text-helios-slate">
                                Configure the server once, preview the library, and leave with a production-ready baseline.
                            </p>
                        </div>
                    </div>

                    <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/50 px-4 py-3 text-xs text-helios-slate max-w-sm">
                        <p className="font-bold uppercase tracking-wider text-helios-ink">Server-side selection</p>
                        <p className="mt-1">
                            All folders here refer to the filesystem available to the Alchemist server process, not your browser’s local machine.
                        </p>
                    </div>
                </header>

                {!configMutable && (
                    <div className="mb-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                        The config file is read-only right now. Setup cannot finish until the TOML file is writable.
                    </div>
                )}

                <AnimatePresence mode="wait">
                    {step === 1 && (
                        <motion.div
                            key="account"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-8"
                        >
                            <div className="space-y-2">
                                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                                    <Lock size={20} className="text-helios-solar" />
                                    Admin Access & Look
                                </h2>
                                <p className="text-sm text-helios-slate">
                                    Start with the basics: create the admin account and pick the default interface theme people will land on after setup.
                                </p>
                            </div>

                            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                                <div className="space-y-4">
                                    <div>
                                        <label className="block text-sm font-medium text-helios-slate mb-2">Admin Username</label>
                                        <input
                                            type="text"
                                            value={username}
                                            onChange={(e) => setUsername(e.target.value)}
                                            className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar outline-none"
                                            placeholder="admin"
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-sm font-medium text-helios-slate mb-2">Admin Password</label>
                                        <input
                                            type="password"
                                            value={password}
                                            onChange={(e) => setPassword(e.target.value)}
                                            className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-xl px-4 py-3 text-helios-ink focus:border-helios-solar outline-none"
                                            placeholder="Choose a strong password"
                                        />
                                    </div>
                                    <label className="flex items-center justify-between rounded-2xl border border-helios-line/20 bg-helios-surface-soft/50 px-4 py-4">
                                        <div>
                                            <p className="text-sm font-semibold text-helios-ink">Anonymous Telemetry</p>
                                            <p className="text-xs text-helios-slate mt-1">Help improve reliability and defaults with anonymous runtime signals.</p>
                                        </div>
                                        <input
                                            type="checkbox"
                                            checked={settings.system.enable_telemetry}
                                            onChange={(e) => setSettings({
                                                ...settings,
                                                system: {
                                                    ...settings.system,
                                                    enable_telemetry: e.target.checked,
                                                },
                                            })}
                                            className="h-5 w-5 rounded border-helios-line/30 accent-helios-solar"
                                        />
                                    </label>
                                </div>

                                <div className="space-y-4">
                                    <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                        <Palette size={18} className="text-helios-solar" />
                                        Default Theme
                                    </div>
                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                        {THEME_OPTIONS.map((theme) => (
                                            <button
                                                key={theme.id}
                                                type="button"
                                                onClick={() => setSettings({
                                                    ...settings,
                                                    appearance: { active_theme_id: theme.id },
                                                })}
                                                className={clsx(
                                                    "rounded-2xl border px-4 py-4 text-left transition-all",
                                                    settings.appearance.active_theme_id === theme.id
                                                        ? "border-helios-solar bg-helios-solar/10 text-helios-ink"
                                                        : "border-helios-line/20 bg-helios-surface-soft/50 text-helios-slate hover:border-helios-solar/20"
                                                )}
                                            >
                                                <div className="font-semibold">{theme.name}</div>
                                                <div className="text-xs mt-1 opacity-80">Applied as the initial dashboard theme.</div>
                                            </button>
                                        ))}
                                    </div>
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 2 && (
                        <motion.div
                            key="library"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-8"
                        >
                            <div className="space-y-2">
                                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                                    <FolderOpen size={20} className="text-helios-solar" />
                                    Library Selection
                                </h2>
                                <p className="text-sm text-helios-slate">
                                    Choose the server folders Alchemist should scan and keep watching. Recommendations and preview are here to remove the guesswork.
                                </p>
                            </div>

                            <div className="grid grid-cols-1 xl:grid-cols-[1.2fr_0.8fr] gap-6">
                                <div className="space-y-5">
                                    <div className="rounded-3xl border border-helios-line/20 bg-helios-surface-soft/40 p-5 space-y-4">
                                        <div className="flex items-start justify-between gap-4">
                                            <div>
                                                <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                                    <Sparkles size={16} className="text-helios-solar" />
                                                    Suggested Server Folders
                                                </div>
                                                <p className="text-xs text-helios-slate mt-1">
                                                    Auto-discovered media-like folders from the server filesystem. Review and add what you actually want watched.
                                                </p>
                                            </div>
                                            <button
                                                type="button"
                                                onClick={() => setPickerOpen(true)}
                                                className="rounded-xl border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink hover:border-helios-solar/30"
                                            >
                                                Browse Server Folders
                                            </button>
                                        </div>
                                        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                            {recommendations.map((recommendation) => (
                                                <button
                                                    key={recommendation.path}
                                                    type="button"
                                                    onClick={() => addDirectory(recommendation.path)}
                                                    className="rounded-2xl border border-helios-line/20 bg-helios-surface px-4 py-4 text-left hover:border-helios-solar/30 transition-all"
                                                >
                                                    <div className="flex items-center justify-between gap-3">
                                                        <span className="font-semibold text-helios-ink">{recommendation.label}</span>
                                                        <span className="rounded-full border border-helios-line/20 px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-helios-slate">
                                                            {recommendation.media_hint}
                                                        </span>
                                                    </div>
                                                    <p className="mt-2 font-mono text-[11px] text-helios-slate break-all">{recommendation.path}</p>
                                                    <p className="mt-2 text-xs text-helios-slate">{recommendation.reason}</p>
                                                </button>
                                            ))}
                                        </div>
                                    </div>

                                    <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                            <FolderOpen size={16} className="text-helios-solar" />
                                            Selected Library Roots
                                        </div>
                                        <div className="flex gap-2">
                                            <input
                                                type="text"
                                                value={dirInput}
                                                onChange={(e) => setDirInput(e.target.value)}
                                                placeholder="Paste a server path or use Browse"
                                                className="flex-1 rounded-xl border border-helios-line/20 bg-helios-surface-soft px-4 py-3 font-mono text-sm text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
                                            />
                                            <button
                                                type="button"
                                                onClick={() => addDirectory(dirInput)}
                                                className="rounded-xl bg-helios-solar px-5 py-3 text-sm font-semibold text-helios-main"
                                            >
                                                Add
                                            </button>
                                        </div>
                                        <div className="space-y-2">
                                            {settings.scanner.directories.map((dir) => (
                                                <div key={dir} className="flex items-center justify-between rounded-2xl border border-helios-line/20 bg-helios-surface-soft/50 px-4 py-3">
                                                    <div className="min-w-0">
                                                        <p className="font-mono text-sm text-helios-ink truncate" title={dir}>{dir}</p>
                                                        <p className="text-[11px] text-helios-slate mt-1">Watched recursively and used as a library root.</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        onClick={() => removeDirectory(dir)}
                                                        className="rounded-xl border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-500 hover:bg-red-500/10"
                                                    >
                                                        Remove
                                                    </button>
                                                </div>
                                            ))}
                                            {settings.scanner.directories.length === 0 && (
                                                <p className="text-sm text-helios-slate">No server folders selected yet.</p>
                                            )}
                                        </div>
                                    </div>
                                </div>

                                <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                    <div className="flex items-center justify-between gap-3">
                                        <div>
                                            <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                                <Search size={16} className="text-helios-solar" />
                                                Library Preview
                                            </div>
                                            <p className="text-xs text-helios-slate mt-1">
                                                See what Alchemist will likely ingest before you finish setup.
                                            </p>
                                        </div>
                                        <button
                                            type="button"
                                            onClick={() => void fetchPreview()}
                                            disabled={previewLoading || settings.scanner.directories.length === 0}
                                            className="rounded-xl border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink hover:border-helios-solar/30 disabled:opacity-50"
                                        >
                                            {previewLoading ? "Previewing..." : "Refresh Preview"}
                                        </button>
                                    </div>

                                    {previewError && (
                                        <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                                            {previewError}
                                        </div>
                                    )}

                                    {preview ? (
                                        <div className="space-y-4">
                                            <div className="rounded-2xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3">
                                                <p className="text-[10px] font-bold uppercase tracking-wider text-emerald-500">
                                                    Estimated Supported Media
                                                </p>
                                                <p className="mt-2 text-2xl font-bold text-helios-ink">{preview.total_media_files}</p>
                                            </div>

                                            {preview.warnings.length > 0 && (
                                                <div className="space-y-2">
                                                    {preview.warnings.map((warning) => (
                                                        <div key={warning} className="rounded-2xl border border-amber-500/20 bg-amber-500/10 px-4 py-3 text-xs text-amber-500">
                                                            {warning}
                                                        </div>
                                                    ))}
                                                </div>
                                            )}

                                            <div className="space-y-3">
                                                {preview.directories.map((directory) => (
                                                    <div key={directory.path} className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4">
                                                        <div className="flex items-center justify-between gap-3">
                                                            <div className="min-w-0">
                                                                <p className="font-mono text-sm text-helios-ink break-all">{directory.path}</p>
                                                                <p className="text-xs text-helios-slate mt-1">
                                                                    {directory.media_files} supported files found
                                                                </p>
                                                            </div>
                                                            <span className="rounded-full border border-helios-line/20 px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-helios-slate">
                                                                {directory.media_hint}
                                                            </span>
                                                        </div>
                                                        {directory.sample_files.length > 0 && (
                                                            <div className="mt-3 space-y-1">
                                                                {directory.sample_files.map((sample) => (
                                                                    <p key={sample} className="text-[11px] font-mono text-helios-slate truncate" title={sample}>
                                                                        {sample}
                                                                    </p>
                                                                ))}
                                                            </div>
                                                        )}
                                                    </div>
                                                ))}
                                            </div>
                                        </div>
                                    ) : (
                                        <div className="rounded-2xl border border-dashed border-helios-line/20 px-4 py-8 text-sm text-helios-slate text-center">
                                            Add one or more server folders to preview what Alchemist will scan.
                                        </div>
                                    )}
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 3 && (
                        <motion.div
                            key="processing"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-8"
                        >
                            <div className="space-y-2">
                                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                                    <Video size={20} className="text-helios-solar" />
                                    Processing, Output & Quality
                                </h2>
                                <p className="text-sm text-helios-slate">
                                    Tune what Alchemist creates, how aggressive it should be, and how quality should be validated before replacing source material.
                                </p>
                            </div>

                            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                                <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                    <div className="text-sm font-semibold text-helios-ink">Transcoding Target</div>
                                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                                        {(["av1", "hevc", "h264"] as const).map((codec) => (
                                            <button
                                                key={codec}
                                                type="button"
                                                onClick={() => setSettings({
                                                    ...settings,
                                                    transcode: { ...settings.transcode, output_codec: codec },
                                                })}
                                                className={clsx(
                                                    "rounded-2xl border px-4 py-4 text-left transition-all",
                                                    settings.transcode.output_codec === codec
                                                        ? "border-helios-solar bg-helios-solar/10 text-helios-ink"
                                                        : "border-helios-line/20 bg-helios-surface-soft/40 text-helios-slate"
                                                )}
                                            >
                                                <div className="font-semibold uppercase">{codec}</div>
                                                <div className="text-[10px] mt-2 opacity-80">
                                                    {codec === "av1"
                                                        ? "Best compression"
                                                        : codec === "hevc"
                                                            ? "Broad modern compatibility"
                                                            : "Maximum playback compatibility"}
                                                </div>
                                            </button>
                                        ))}
                                    </div>

                                    <div className="space-y-3">
                                        <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">Quality Profile</label>
                                        <select
                                            value={settings.transcode.quality_profile}
                                            onChange={(e) => setSettings({
                                                ...settings,
                                                transcode: {
                                                    ...settings.transcode,
                                                    quality_profile: e.target.value as SetupSettings["transcode"]["quality_profile"],
                                                },
                                            })}
                                            className="w-full rounded-xl border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink"
                                        >
                                            <option value="speed">Speed</option>
                                            <option value="balanced">Balanced</option>
                                            <option value="quality">Quality</option>
                                        </select>
                                    </div>

                                    <RangeControl
                                        label="Concurrent Jobs"
                                        min={1}
                                        max={8}
                                        step={1}
                                        value={settings.transcode.concurrent_jobs}
                                        onChange={(value) => setSettings({
                                            ...settings,
                                            transcode: { ...settings.transcode, concurrent_jobs: value },
                                        })}
                                    />

                                    <RangeControl
                                        label={`Minimum Savings (${Math.round(settings.transcode.size_reduction_threshold * 100)}%)`}
                                        min={0}
                                        max={0.9}
                                        step={0.05}
                                        value={settings.transcode.size_reduction_threshold}
                                        onChange={(value) => setSettings({
                                            ...settings,
                                            transcode: { ...settings.transcode, size_reduction_threshold: value },
                                        })}
                                    />
                                </div>

                                <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                    <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                        <FileCog size={16} className="text-helios-solar" />
                                        Output Rules
                                    </div>

                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                        <LabeledInput
                                            label="Output Extension"
                                            value={settings.files.output_extension}
                                            onChange={(value) => setSettings({
                                                ...settings,
                                                files: { ...settings.files, output_extension: value },
                                            })}
                                            placeholder="mkv"
                                        />
                                        <LabeledInput
                                            label="Output Suffix"
                                            value={settings.files.output_suffix}
                                            onChange={(value) => setSettings({
                                                ...settings,
                                                files: { ...settings.files, output_suffix: value },
                                            })}
                                            placeholder="-alchemist"
                                        />
                                    </div>

                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                        <LabeledSelect
                                            label="Replace Strategy"
                                            value={settings.files.replace_strategy}
                                            onChange={(value) => setSettings({
                                                ...settings,
                                                files: { ...settings.files, replace_strategy: value },
                                            })}
                                            options={[
                                                { value: "keep", label: "Keep existing output" },
                                                { value: "replace", label: "Replace existing output" },
                                            ]}
                                        />
                                        <LabeledSelect
                                            label="Subtitle Handling"
                                            value={settings.transcode.subtitle_mode}
                                            onChange={(value) => setSettings({
                                                ...settings,
                                                transcode: {
                                                    ...settings.transcode,
                                                    subtitle_mode: value as SetupSettings["transcode"]["subtitle_mode"],
                                                },
                                            })}
                                            options={[
                                                { value: "copy", label: "Copy subtitles" },
                                                { value: "burn", label: "Burn one subtitle track" },
                                                { value: "extract", label: "Extract to sidecar" },
                                                { value: "none", label: "Drop subtitles" },
                                            ]}
                                        />
                                    </div>

                                    <LabeledInput
                                        label="Optional Output Root"
                                        value={settings.files.output_root ?? ""}
                                        onChange={(value) => setSettings({
                                            ...settings,
                                            files: { ...settings.files, output_root: value || null },
                                        })}
                                        placeholder="Leave blank to write beside the source"
                                    />

                                    <div className="space-y-3">
                                        <ToggleRow
                                            title="Allow Fallback"
                                            body="Permit alternate encoders if the preferred codec/hardware path is unavailable."
                                            checked={settings.transcode.allow_fallback}
                                            onChange={(checked) => setSettings({
                                                ...settings,
                                                transcode: { ...settings.transcode, allow_fallback: checked },
                                            })}
                                        />
                                        <ToggleRow
                                            title="Delete Source After Success"
                                            body="Remove the original file after a successful completed transcode."
                                            checked={settings.files.delete_source}
                                            onChange={(checked) => setSettings({
                                                ...settings,
                                                files: { ...settings.files, delete_source: checked },
                                            })}
                                        />
                                        <ToggleRow
                                            title="Enable VMAF Validation"
                                            body="Score output quality after encoding and optionally revert if it drops too low."
                                            checked={settings.quality.enable_vmaf}
                                            onChange={(checked) => setSettings({
                                                ...settings,
                                                quality: { ...settings.quality, enable_vmaf: checked },
                                            })}
                                        />
                                    </div>

                                    {settings.quality.enable_vmaf && (
                                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                            <LabeledInput
                                                label="Minimum VMAF"
                                                value={String(settings.quality.min_vmaf_score)}
                                                onChange={(value) => setSettings({
                                                    ...settings,
                                                    quality: {
                                                        ...settings.quality,
                                                        min_vmaf_score: parseFloat(value) || 0,
                                                    },
                                                })}
                                                placeholder="90"
                                                type="number"
                                            />
                                            <ToggleRow
                                                title="Revert On Low Quality"
                                                body="Keep the source when the VMAF score misses the minimum."
                                                checked={settings.quality.revert_on_low_quality}
                                                onChange={(checked) => setSettings({
                                                    ...settings,
                                                    quality: {
                                                        ...settings.quality,
                                                        revert_on_low_quality: checked,
                                                    },
                                                })}
                                            />
                                        </div>
                                    )}
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 4 && (
                        <motion.div
                            key="runtime"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-8"
                        >
                            <div className="space-y-2">
                                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                                    <ShieldCheck size={20} className="text-helios-solar" />
                                    Hardware, Notifications & Automation
                                </h2>
                                <p className="text-sm text-helios-slate">
                                    Finish the long-term operating profile: hardware policy, alerts, and allowed runtime windows.
                                </p>
                            </div>

                            <div className="grid grid-cols-1 xl:grid-cols-[1fr_1fr] gap-6">
                                <div className="space-y-6">
                                    <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                            <Cpu size={16} className="text-helios-solar" />
                                            Hardware Policy
                                        </div>

                                        {hardware && (
                                            <div className="rounded-2xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3 text-sm text-helios-ink">
                                                Detected <span className="font-bold">{hardware.vendor}</span> with {hardware.supported_codecs.join(", ").toUpperCase()} support.
                                            </div>
                                        )}

                                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                            <LabeledSelect
                                                label="Preferred Vendor"
                                                value={settings.hardware.preferred_vendor ?? ""}
                                                onChange={(value) => setSettings({
                                                    ...settings,
                                                    hardware: {
                                                        ...settings.hardware,
                                                        preferred_vendor: value || null,
                                                    },
                                                })}
                                                options={[
                                                    { value: "", label: "Auto detect" },
                                                    { value: "nvidia", label: "NVIDIA" },
                                                    { value: "amd", label: "AMD" },
                                                    { value: "intel", label: "Intel" },
                                                    { value: "apple", label: "Apple" },
                                                    { value: "cpu", label: "CPU" },
                                                ]}
                                            />
                                            <LabeledSelect
                                                label="CPU Preset"
                                                value={settings.hardware.cpu_preset}
                                                onChange={(value) => setSettings({
                                                    ...settings,
                                                    hardware: {
                                                        ...settings.hardware,
                                                        cpu_preset: value as SetupSettings["hardware"]["cpu_preset"],
                                                    },
                                                })}
                                                options={[
                                                    { value: "slow", label: "Slow" },
                                                    { value: "medium", label: "Medium" },
                                                    { value: "fast", label: "Fast" },
                                                    { value: "faster", label: "Faster" },
                                                ]}
                                            />
                                        </div>

                                        <LabeledInput
                                            label="Explicit Device Path"
                                            value={settings.hardware.device_path ?? ""}
                                            onChange={(value) => setSettings({
                                                ...settings,
                                                hardware: { ...settings.hardware, device_path: value || null },
                                            })}
                                            placeholder="/dev/dri/renderD128"
                                        />

                                        <ToggleRow
                                            title="Allow CPU Fallback"
                                            body="Use software encoding if the preferred GPU path is unavailable."
                                            checked={settings.hardware.allow_cpu_fallback}
                                            onChange={(checked) => setSettings({
                                                ...settings,
                                                hardware: { ...settings.hardware, allow_cpu_fallback: checked },
                                            })}
                                        />
                                        <ToggleRow
                                            title="Allow CPU Encoding"
                                            body="Permit CPU encoders even when GPU options exist."
                                            checked={settings.hardware.allow_cpu_encoding}
                                            onChange={(checked) => setSettings({
                                                ...settings,
                                                hardware: { ...settings.hardware, allow_cpu_encoding: checked },
                                            })}
                                        />
                                    </div>

                                    <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                            <Calendar size={16} className="text-helios-solar" />
                                            Schedule Windows
                                        </div>
                                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                            <LabeledInput
                                                label="Start"
                                                type="time"
                                                value={scheduleDraft.start_time}
                                                onChange={(value) => setScheduleDraft({ ...scheduleDraft, start_time: value })}
                                            />
                                            <LabeledInput
                                                label="End"
                                                type="time"
                                                value={scheduleDraft.end_time}
                                                onChange={(value) => setScheduleDraft({ ...scheduleDraft, end_time: value })}
                                            />
                                        </div>
                                        <div className="flex flex-wrap gap-2">
                                            {WEEKDAY_OPTIONS.map((day, index) => {
                                                const selected = scheduleDraft.days_of_week.includes(index);
                                                return (
                                                    <button
                                                        key={day}
                                                        type="button"
                                                        onClick={() => setScheduleDraft({
                                                            ...scheduleDraft,
                                                            days_of_week: selected
                                                                ? scheduleDraft.days_of_week.filter((value) => value !== index)
                                                                : [...scheduleDraft.days_of_week, index].sort(),
                                                        })}
                                                        className={clsx(
                                                            "rounded-full border px-3 py-2 text-xs font-semibold transition-all",
                                                            selected
                                                                ? "border-helios-solar bg-helios-solar/10 text-helios-ink"
                                                                : "border-helios-line/20 text-helios-slate"
                                                        )}
                                                    >
                                                        {day}
                                                    </button>
                                                );
                                            })}
                                        </div>
                                        <button
                                            type="button"
                                            onClick={addScheduleWindow}
                                            className="rounded-xl border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink"
                                        >
                                            Add Schedule Window
                                        </button>
                                        <div className="space-y-2">
                                            {settings.schedule.windows.map((window, index) => (
                                                <div key={`${window.start_time}-${window.end_time}-${index}`} className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 flex items-center justify-between gap-4">
                                                    <div>
                                                        <div className="text-sm font-semibold text-helios-ink">{window.start_time} - {window.end_time}</div>
                                                        <div className="text-xs text-helios-slate mt-1">
                                                            {window.days_of_week.map((day) => WEEKDAY_OPTIONS[day]).join(", ")}
                                                        </div>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        onClick={() => setSettings({
                                                            ...settings,
                                                            schedule: {
                                                                windows: settings.schedule.windows.filter((_, current) => current !== index),
                                                            },
                                                        })}
                                                        className="rounded-xl border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-500 hover:bg-red-500/10"
                                                    >
                                                        Remove
                                                    </button>
                                                </div>
                                            ))}
                                            {settings.schedule.windows.length === 0 && (
                                                <p className="text-sm text-helios-slate">No restricted schedule windows configured. Processing will run whenever work is available.</p>
                                            )}
                                        </div>
                                    </div>
                                </div>

                                <div className="space-y-6">
                                    <div className="rounded-3xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                            <Bell size={16} className="text-helios-solar" />
                                            Notifications
                                        </div>
                                        <ToggleRow
                                            title="Enable Notifications"
                                            body="Send alerts when jobs succeed or fail."
                                            checked={settings.notifications.enabled}
                                            onChange={(checked) => setSettings({
                                                ...settings,
                                                notifications: { ...settings.notifications, enabled: checked },
                                            })}
                                        />

                                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                            <LabeledInput
                                                label="Target Name"
                                                value={notificationDraft.name}
                                                onChange={(value) => setNotificationDraft({ ...notificationDraft, name: value })}
                                                placeholder="Discord"
                                            />
                                            <LabeledSelect
                                                label="Type"
                                                value={notificationDraft.target_type}
                                                onChange={(value) => setNotificationDraft({ ...notificationDraft, target_type: value })}
                                                options={[
                                                    { value: "discord", label: "Discord" },
                                                    { value: "webhook", label: "Webhook" },
                                                    { value: "gotify", label: "Gotify" },
                                                ]}
                                            />
                                        </div>
                                        <LabeledInput
                                            label="Endpoint URL"
                                            value={notificationDraft.endpoint_url}
                                            onChange={(value) => setNotificationDraft({ ...notificationDraft, endpoint_url: value })}
                                            placeholder="https://example.com/webhook"
                                        />
                                        <div className="flex flex-wrap gap-2">
                                            {EVENT_OPTIONS.map((eventName) => {
                                                const selected = notificationDraft.events.includes(eventName);
                                                return (
                                                    <button
                                                        key={eventName}
                                                        type="button"
                                                        onClick={() => setNotificationDraft({
                                                            ...notificationDraft,
                                                            events: selected
                                                                ? notificationDraft.events.filter((candidate) => candidate !== eventName)
                                                                : [...notificationDraft.events, eventName],
                                                        })}
                                                        className={clsx(
                                                            "rounded-full border px-3 py-2 text-xs font-semibold transition-all",
                                                            selected
                                                                ? "border-helios-solar bg-helios-solar/10 text-helios-ink"
                                                                : "border-helios-line/20 text-helios-slate"
                                                        )}
                                                    >
                                                        {eventName}
                                                    </button>
                                                );
                                            })}
                                        </div>
                                        <button
                                            type="button"
                                            onClick={addNotificationTarget}
                                            className="rounded-xl border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink"
                                        >
                                            Add Notification Target
                                        </button>

                                        <div className="space-y-2">
                                            {settings.notifications.targets.map((target, index) => (
                                                <div key={`${target.name}-${target.endpoint_url}-${index}`} className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 flex items-center justify-between gap-4">
                                                    <div className="min-w-0">
                                                        <div className="text-sm font-semibold text-helios-ink">{target.name}</div>
                                                        <div className="text-xs text-helios-slate mt-1 truncate" title={target.endpoint_url}>{target.endpoint_url}</div>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        onClick={() => setSettings({
                                                            ...settings,
                                                            notifications: {
                                                                ...settings.notifications,
                                                                targets: settings.notifications.targets.filter((_, current) => current !== index),
                                                            },
                                                        })}
                                                        className="rounded-xl border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-500 hover:bg-red-500/10"
                                                    >
                                                        Remove
                                                    </button>
                                                </div>
                                            ))}
                                            {settings.notifications.targets.length === 0 && (
                                                <p className="text-sm text-helios-slate">No notification targets configured yet.</p>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 5 && (
                        <motion.div
                            key="review"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-8"
                        >
                            <div className="space-y-2">
                                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2">
                                    <CheckCircle size={20} className="text-helios-solar" />
                                    Final Review
                                </h2>
                                <p className="text-sm text-helios-slate">
                                    Review the effective server paths, processing rules, and automation choices before Alchemist writes the config and starts the first scan.
                                </p>
                            </div>

                            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                                {setupSummary.map((item) => (
                                    <div key={item.label} className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4">
                                        <div className="text-[10px] font-bold uppercase tracking-wider text-helios-slate">{item.label}</div>
                                        <div className="mt-2 text-2xl font-bold text-helios-ink">{item.value}</div>
                                    </div>
                                ))}
                            </div>

                            <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
                                <ReviewCard
                                    title="Library"
                                    lines={[
                                        `${settings.scanner.directories.length} server folders selected`,
                                        preview ? `${preview.total_media_files} supported media files previewed` : "Preview pending",
                                        settings.scanner.directories.join(" | ") || "No folders selected",
                                    ]}
                                />
                                <ReviewCard
                                    title="Transcoding"
                                    lines={[
                                        `Target: ${settings.transcode.output_codec.toUpperCase()}`,
                                        `Profile: ${settings.transcode.quality_profile}`,
                                        `${settings.transcode.concurrent_jobs} concurrent jobs`,
                                        `Subtitle mode: ${settings.transcode.subtitle_mode}`,
                                    ]}
                                />
                                <ReviewCard
                                    title="Output"
                                    lines={[
                                        `Extension: .${settings.files.output_extension}`,
                                        `Suffix: ${settings.files.output_suffix || "(none)"}`,
                                        `Replace strategy: ${settings.files.replace_strategy}`,
                                        settings.files.output_root ? `Output root: ${settings.files.output_root}` : "Output beside source",
                                    ]}
                                />
                                <ReviewCard
                                    title="Runtime"
                                    lines={[
                                        `Theme: ${settings.appearance.active_theme_id ?? "default"}`,
                                        `${settings.notifications.targets.length} notification targets`,
                                        `${settings.schedule.windows.length} schedule windows`,
                                        `Telemetry: ${settings.system.enable_telemetry ? "enabled" : "disabled"}`,
                                    ]}
                                />
                            </div>

                            {error && (
                                <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                                    {error}
                                </div>
                            )}
                        </motion.div>
                    )}

                    {step === 6 && (
                        <motion.div
                            key="scan"
                            initial={{ opacity: 0, scale: 0.98 }}
                            animate={{ opacity: 1, scale: 1 }}
                            className="space-y-8 py-8"
                        >
                            <div className="text-center space-y-3">
                                <div className="mx-auto w-20 h-20 rounded-full border-4 border-helios-solar/20 border-t-helios-solar animate-spin" />
                                <h2 className="text-2xl font-bold text-helios-ink">Initial Library Scan</h2>
                                <p className="text-sm text-helios-slate">
                                    Alchemist is validating the selected server folders and seeding the first queue. Encoding will stay paused until you press Start on the dashboard.
                                </p>
                            </div>

                            {scanError && (
                                <div className="rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-4 text-sm text-red-500 space-y-3">
                                    <p className="font-semibold">The initial scan hit an error.</p>
                                    <p>{scanError}</p>
                                    <div className="flex flex-col sm:flex-row gap-2">
                                        <button
                                            type="button"
                                            onClick={() => void startScan()}
                                            className="rounded-xl bg-red-500/20 px-4 py-2 font-semibold"
                                        >
                                            Retry Scan
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => setStep(5)}
                                            className="rounded-xl border border-red-500/30 px-4 py-2 font-semibold"
                                        >
                                            Back to Review
                                        </button>
                                    </div>
                                </div>
                            )}

                            {scanStatus && (
                                <div className="space-y-4">
                                    <div className="flex justify-between text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                                        <span>Found: {scanStatus.files_found}</span>
                                        <span>Queued: {scanStatus.files_added}</span>
                                    </div>
                                    <div className="h-3 rounded-full border border-helios-line/20 bg-helios-surface-soft overflow-hidden">
                                        <motion.div
                                            className="h-full bg-helios-solar"
                                            animate={{
                                                width: `${scanStatus.files_found > 0 ? (scanStatus.files_added / scanStatus.files_found) * 100 : 0}%`,
                                            }}
                                        />
                                    </div>
                                    {scanStatus.current_folder && (
                                        <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 font-mono text-xs text-helios-slate">
                                            {scanStatus.current_folder}
                                        </div>
                                    )}
                                    {!scanStatus.is_running && (
                                        <button
                                            type="button"
                                            onClick={() => {
                                                window.location.href = "/";
                                            }}
                                            className="w-full rounded-2xl bg-helios-solar px-6 py-4 font-bold text-helios-main shadow-lg shadow-helios-solar/20 hover:opacity-90"
                                        >
                                            Enter Dashboard
                                        </button>
                                    )}
                                </div>
                            )}
                        </motion.div>
                    )}
                </AnimatePresence>

                {error && step < 6 && (
                    <div className="mt-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">
                        {error}
                    </div>
                )}

                {step < 6 && (
                    <div className="mt-8 flex items-center justify-between gap-4 border-t border-helios-line/20 pt-6">
                        <button
                            type="button"
                            onClick={() => setStep((current) => Math.max(current - 1, 1))}
                            disabled={step === 1 || submitting}
                            className={clsx(
                                "rounded-xl px-4 py-2 text-sm font-semibold transition-colors",
                                step === 1
                                    ? "text-helios-line cursor-not-allowed"
                                    : "text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            Back
                        </button>
                        <button
                            type="button"
                            onClick={() => void handleNext()}
                            disabled={submitting || !configMutable}
                            className="flex items-center gap-2 rounded-xl bg-helios-solar px-6 py-3 font-semibold text-helios-main hover:opacity-90 transition-opacity disabled:opacity-50"
                        >
                            {submitting ? "Working..." : step === 5 ? "Build Engine" : "Next"}
                            {!submitting && <ArrowRight size={18} />}
                        </button>
                    </div>
                )}
            </div>

            <ServerDirectoryPicker
                open={pickerOpen}
                title="Browse Server Folders"
                description="Navigate the server filesystem, review guardrails, and choose the folder Alchemist should treat as a media root."
                onClose={() => setPickerOpen(false)}
                onSelect={(path) => {
                    addDirectory(path);
                    setPickerOpen(false);
                }}
            />
        </div>
    );
}

function RangeControl({
    label,
    min,
    max,
    step,
    value,
    onChange,
}: {
    label: string;
    min: number;
    max: number;
    step: number;
    value: number;
    onChange: (value: number) => void;
}) {
    return (
        <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">{label}</label>
            <input
                type="range"
                min={min}
                max={max}
                step={step}
                value={value}
                onChange={(e) => onChange(parseFloat(e.target.value))}
                className="w-full accent-helios-solar"
            />
            <div className="text-sm font-semibold text-helios-ink">{value}</div>
        </div>
    );
}

function LabeledInput({
    label,
    value,
    onChange,
    placeholder,
    type = "text",
}: {
    label: string;
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    type?: string;
}) {
    return (
        <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">{label}</label>
            <input
                type={type}
                value={value}
                onChange={(e) => onChange(e.target.value)}
                placeholder={placeholder}
                className="w-full rounded-xl border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
            />
        </div>
    );
}

function LabeledSelect({
    label,
    value,
    onChange,
    options,
}: {
    label: string;
    value: string;
    onChange: (value: string) => void;
    options: Array<{ value: string; label: string }>;
}) {
    return (
        <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-helios-slate">{label}</label>
            <select
                value={value}
                onChange={(e) => onChange(e.target.value)}
                className="w-full rounded-xl border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none"
            >
                {options.map((option) => (
                    <option key={option.value} value={option.value}>
                        {option.label}
                    </option>
                ))}
            </select>
        </div>
    );
}

function ToggleRow({
    title,
    body,
    checked,
    onChange,
}: {
    title: string;
    body: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
}) {
    return (
        <label className="flex items-center justify-between gap-4 rounded-2xl border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4">
            <div>
                <p className="text-sm font-semibold text-helios-ink">{title}</p>
                <p className="text-xs text-helios-slate mt-1">{body}</p>
            </div>
            <input
                type="checkbox"
                checked={checked}
                onChange={(e) => onChange(e.target.checked)}
                className="h-5 w-5 rounded border-helios-line/30 accent-helios-solar"
            />
        </label>
    );
}

function ReviewCard({ title, lines }: { title: string; lines: string[] }) {
    return (
        <div className="rounded-3xl border border-helios-line/20 bg-helios-surface-soft/40 px-5 py-5">
            <div className="text-sm font-semibold text-helios-ink">{title}</div>
            <div className="mt-3 space-y-2">
                {lines.map((line) => (
                    <p key={line} className="text-sm text-helios-slate break-words">
                        {line}
                    </p>
                ))}
            </div>
        </div>
    );
}
