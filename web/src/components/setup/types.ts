export interface NotificationTargetConfig {
    name: string;
    target_type: string;
    endpoint_url: string;
    auth_token: string | null;
    events: string[];
    enabled: boolean;
}

export interface ScheduleWindowConfig {
    start_time: string;
    end_time: string;
    days_of_week: number[];
    enabled: boolean;
}

export interface SetupSettings {
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

export interface SettingsBundleResponse {
    settings: SetupSettings;
}

export interface SetupStatusResponse {
    setup_required: boolean;
    enable_telemetry?: boolean;
    config_mutable?: boolean;
}

export interface HardwareInfo {
    vendor: "Nvidia" | "Amd" | "Intel" | "Apple" | "Cpu";
    device_path: string | null;
    supported_codecs: string[];
}

export interface ScanStatus {
    is_running: boolean;
    files_found: number;
    files_added: number;
    current_folder: string | null;
}

export interface FsRecommendation {
    path: string;
    label: string;
    reason: string;
    media_hint: "high" | "medium" | "low" | "unknown";
}

export interface FsRecommendationsResponse {
    recommendations: FsRecommendation[];
}

export interface FsPreviewDirectory {
    path: string;
    exists: boolean;
    readable: boolean;
    media_files: number;
    sample_files: string[];
    media_hint: "high" | "medium" | "low" | "unknown";
    warnings: string[];
}

export interface FsPreviewResponse {
    directories: FsPreviewDirectory[];
    total_media_files: number;
    warnings: string[];
}

export interface SetupSummaryItem {
    label: string;
    value: string;
}

export type StepValidator = () => Promise<string | null>;
