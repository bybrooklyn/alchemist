import type {
    NotificationTargetConfig,
    ScheduleWindowConfig,
    SettingsBundleResponse,
    SetupSettings,
    SetupStatusResponse,
} from "./types";

export const SETUP_STEP_COUNT = 6;

export const THEME_OPTIONS = [
    { id: "helios-orange", name: "Helios Orange" },
    { id: "sunset", name: "Sunset" },
    { id: "midnight", name: "Midnight" },
    { id: "emerald", name: "Emerald" },
    { id: "deep-blue", name: "Deep Blue" },
    { id: "lavender", name: "Lavender" },
];

export const EVENT_OPTIONS = ["completed", "failed", "queued"] as const;
export const WEEKDAY_OPTIONS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;

export const DEFAULT_NOTIFICATION_DRAFT: NotificationTargetConfig = {
    name: "",
    target_type: "discord",
    endpoint_url: "",
    auth_token: null,
    events: ["completed", "failed"],
    enabled: true,
};

export const DEFAULT_SCHEDULE_DRAFT: ScheduleWindowConfig = {
    start_time: "22:00",
    end_time: "06:00",
    days_of_week: [0, 1, 2, 3, 4, 5, 6],
    enabled: true,
};

export const DEFAULT_SETTINGS: SetupSettings = {
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
        enable_telemetry: false,
        monitoring_poll_interval: 2,
    },
};

export function mergeSetupSettings(status: SetupStatusResponse, bundle: SettingsBundleResponse): SetupSettings {
    return {
        ...DEFAULT_SETTINGS,
        ...bundle.settings,
        appearance: { ...DEFAULT_SETTINGS.appearance, ...bundle.settings.appearance },
        scanner: { ...DEFAULT_SETTINGS.scanner, ...bundle.settings.scanner },
        transcode: { ...DEFAULT_SETTINGS.transcode, ...bundle.settings.transcode },
        hardware: { ...DEFAULT_SETTINGS.hardware, ...bundle.settings.hardware },
        files: { ...DEFAULT_SETTINGS.files, ...bundle.settings.files },
        quality: { ...DEFAULT_SETTINGS.quality, ...bundle.settings.quality },
        notifications: { ...DEFAULT_SETTINGS.notifications, ...bundle.settings.notifications },
        schedule: { ...DEFAULT_SETTINGS.schedule, ...bundle.settings.schedule },
        system: {
            ...DEFAULT_SETTINGS.system,
            ...bundle.settings.system,
            enable_telemetry: false,
        },
    };
}
