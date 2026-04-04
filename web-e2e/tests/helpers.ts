import { expect, type Page, type Route } from "@playwright/test";

export async function fulfillJson(route: Route, status: number, body: unknown): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}

export async function fulfillEmpty(route: Route, status = 204): Promise<void> {
  await route.fulfill({
    status,
    body: "",
  });
}

export interface EngineStatusResponse {
  status: "running" | "paused" | "draining";
  manual_paused: boolean;
  scheduler_paused: boolean;
  draining: boolean;
  mode: "background" | "balanced" | "throughput";
  concurrent_limit: number;
  is_manual_override: boolean;
}

export interface EngineModeResponse {
  mode: "background" | "balanced" | "throughput";
  is_manual_override: boolean;
  concurrent_limit: number;
  cpu_count: number;
  computed_limits: {
    background: number;
    balanced: number;
    throughput: number;
  };
}

export interface DashboardStatsResponse {
  active: number;
  concurrent_limit: number;
  completed: number;
  failed: number;
  total: number;
}

export interface SystemSettingsResponse {
  monitoring_poll_interval: number;
  enable_telemetry: boolean;
  watch_enabled?: boolean;
}

export interface SystemResourcesResponse {
  cpu_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_percent: number;
  uptime_seconds: number;
  active_jobs: number;
  concurrent_limit: number;
  cpu_count: number;
  gpu_utilization: number | null;
  gpu_memory_percent: number | null;
}

export interface NotificationTargetFixture {
  id: number;
  name: string;
  target_type: "discord" | "gotify" | "webhook";
  endpoint_url: string;
  auth_token: string | null;
  events: string;
  enabled: boolean;
}

export interface ScheduleWindowFixture {
  id: number;
  start_time: string;
  end_time: string;
  days_of_week: string;
  enabled: boolean;
}

export interface WatchDirFixture {
  id: number;
  path: string;
  is_recursive: boolean;
  profile_id: number | null;
}

export interface LibraryProfileFixture {
  id: number;
  name: string;
  preset: string;
  codec: "av1" | "hevc" | "h264";
  quality_profile: "speed" | "balanced" | "quality";
  hdr_mode: "preserve" | "tonemap";
  audio_mode: "copy" | "aac" | "aac_stereo";
  crf_override: number | null;
  notes: string | null;
  builtin: boolean;
}

export interface JobFixture {
  id: number;
  input_path: string;
  output_path: string;
  status: string;
  priority: number;
  progress: number;
  created_at: string;
  updated_at: string;
  attempt_count?: number;
  vmaf_score?: number;
  decision_reason?: string;
}

export interface JobDetailFixture {
  job: JobFixture;
  metadata?: {
    duration_secs: number;
    codec_name: string;
    width: number;
    height: number;
    bit_depth?: number;
    size_bytes: number;
    video_bitrate_bps?: number;
    container_bitrate_bps?: number;
    fps: number;
    container: string;
    audio_codec?: string;
    audio_channels?: number;
    dynamic_range?: string;
  };
  encode_stats?: {
    input_size_bytes: number;
    output_size_bytes: number;
    compression_ratio: number;
    encode_time_seconds: number;
    encode_speed: number;
    avg_bitrate_kbps: number;
    vmaf_score?: number;
  };
  job_logs?: Array<{
    id: number;
    level: string;
    message: string;
    created_at: string;
  }>;
  job_failure_summary?: string;
}

interface SettingsBundle {
  settings: {
    appearance: { active_theme_id: string | null };
    scanner: {
      directories: string[];
      watch_enabled: boolean;
      extra_watch_dirs: string[];
    };
    transcode: {
      concurrent_jobs: number;
      size_reduction_threshold: number;
      min_bpp_threshold: number;
      min_file_size_mb: number;
      output_codec: "av1" | "hevc" | "h264";
      quality_profile: "speed" | "balanced" | "quality";
      allow_fallback: boolean;
      subtitle_mode: "copy" | "burn" | "extract" | "none";
    };
    hardware: {
      allow_cpu_encoding: boolean;
      allow_cpu_fallback: boolean;
      preferred_vendor: string | null;
      cpu_preset: string;
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
      targets: NotificationTargetFixture[];
    };
    schedule: {
      windows: ScheduleWindowFixture[];
    };
    system: {
      enable_telemetry: boolean;
      monitoring_poll_interval: number;
    };
  };
}

export function createEngineStatus(
  overrides: Partial<EngineStatusResponse> = {},
): EngineStatusResponse {
  return {
    status: "paused",
    manual_paused: true,
    scheduler_paused: false,
    draining: false,
    mode: "balanced",
    concurrent_limit: 2,
    is_manual_override: false,
    ...overrides,
  };
}

export function createEngineMode(
  overrides: Partial<EngineModeResponse> = {},
): EngineModeResponse {
  return {
    mode: "balanced",
    is_manual_override: false,
    concurrent_limit: 2,
    cpu_count: 8,
    computed_limits: {
      background: 1,
      balanced: 4,
      throughput: 4,
    },
    ...overrides,
  };
}

export function createSettingsBundle(
  overrides: Partial<SettingsBundle["settings"]> = {},
): SettingsBundle {
  return {
    settings: {
      appearance: {
        active_theme_id: "helios-orange",
        ...(overrides.appearance ?? {}),
      },
      scanner: {
        directories: ["/media/movies"],
        watch_enabled: true,
        extra_watch_dirs: [],
        ...(overrides.scanner ?? {}),
      },
      transcode: {
        concurrent_jobs: 2,
        size_reduction_threshold: 0.3,
        min_bpp_threshold: 0.1,
        min_file_size_mb: 100,
        output_codec: "av1",
        quality_profile: "balanced",
        allow_fallback: true,
        subtitle_mode: "copy",
        ...(overrides.transcode ?? {}),
      },
      hardware: {
        allow_cpu_encoding: true,
        allow_cpu_fallback: true,
        preferred_vendor: null,
        cpu_preset: "medium",
        device_path: null,
        ...(overrides.hardware ?? {}),
      },
      files: {
        delete_source: false,
        output_extension: "mkv",
        output_suffix: "-alchemist",
        replace_strategy: "keep",
        output_root: null,
        ...(overrides.files ?? {}),
      },
      quality: {
        enable_vmaf: false,
        min_vmaf_score: 90,
        revert_on_low_quality: true,
        ...(overrides.quality ?? {}),
      },
      notifications: {
        enabled: false,
        targets: [],
        ...(overrides.notifications ?? {}),
      },
      schedule: {
        windows: [],
        ...(overrides.schedule ?? {}),
      },
      system: {
        enable_telemetry: false,
        monitoring_poll_interval: 2,
        ...(overrides.system ?? {}),
      },
    },
  };
}

export async function mockSettingsBundle(
  page: Page,
  overrides: Partial<SettingsBundle["settings"]> = {},
): Promise<void> {
  const bundle = createSettingsBundle(overrides);
  await page.route("**/api/settings/bundle", async (route) => {
    await fulfillJson(route, 200, bundle);
  });
}

export async function mockDashboardData(
  page: Page,
  options: {
    jobs?: JobFixture[];
    stats?: Partial<DashboardStatsResponse>;
    systemSettings?: Partial<SystemSettingsResponse>;
    resources?: Partial<SystemResourcesResponse>;
    bundle?: Partial<SettingsBundle["settings"]>;
  } = {},
): Promise<void> {
  const stats: DashboardStatsResponse = {
    active: 0,
    concurrent_limit: 2,
    completed: 0,
    failed: 0,
    total: 0,
    ...(options.stats ?? {}),
  };
  const systemSettings: SystemSettingsResponse = {
    monitoring_poll_interval: 2,
    enable_telemetry: false,
    watch_enabled: true,
    ...(options.systemSettings ?? {}),
  };
  const resources: SystemResourcesResponse = {
    cpu_percent: 8,
    memory_used_mb: 1024,
    memory_total_mb: 8192,
    memory_percent: 12.5,
    uptime_seconds: 3600,
    active_jobs: stats.active,
    concurrent_limit: stats.concurrent_limit,
    cpu_count: 8,
    gpu_utilization: 0,
    gpu_memory_percent: 0,
    ...(options.resources ?? {}),
  };

  await mockJobsTable(page, options.jobs ?? []);
  await mockSettingsBundle(page, options.bundle ?? {});

  await page.route("**/api/stats", async (route) => {
    await fulfillJson(route, 200, stats);
  });
  await page.route("**/api/settings/system", async (route) => {
    await fulfillJson(route, 200, systemSettings);
  });
  await page.route("**/api/system/resources", async (route) => {
    await fulfillJson(route, 200, resources);
  });
}

export async function mockEngineStatus(
  page: Page,
  statusOverrides: Partial<EngineStatusResponse> = {},
  modeOverrides: Partial<EngineModeResponse> = {},
): Promise<void> {
  const status = createEngineStatus(statusOverrides);
  const mode = createEngineMode({
    mode: status.mode,
    concurrent_limit: status.concurrent_limit,
    is_manual_override: status.is_manual_override,
    ...modeOverrides,
  });

  await page.route("**/api/engine/status", async (route) => {
    await fulfillJson(route, 200, status);
  });
  await page.route("**/api/engine/mode", async (route) => {
    await fulfillJson(route, 200, mode);
  });
}

export async function mockJobsTable(
  page: Page,
  jobs: JobFixture[],
  onRequest?: (url: URL) => void,
): Promise<void> {
  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    onRequest?.(url);
    await fulfillJson(route, 200, jobs);
  });
}

export async function mockJobDetails(
  page: Page,
  details: Record<number, JobDetailFixture>,
): Promise<void> {
  await page.route("**/api/jobs/*/details", async (route) => {
    const match = route.request().url().match(/\/api\/jobs\/(\d+)\/details/);
    const id = match ? Number.parseInt(match[1], 10) : NaN;
    const detail = details[id];
    if (!detail) {
      await fulfillJson(route, 404, { message: "not found" });
      return;
    }
    await fulfillJson(route, 200, detail);
  });
}

export async function mockSetupBootstrap(
  page: Page,
  options: {
    setupRequired?: boolean;
    configMutable?: boolean;
    bundle?: Partial<SettingsBundle["settings"]>;
    recommendations?: Array<{
      path: string;
      label: string;
      reason: string;
      media_hint: string;
    }>;
    hardware?: {
      vendor: string;
      device_path: string | null;
      supported_codecs: string[];
    };
  } = {},
): Promise<void> {
  await page.route("**/api/setup/status", async (route) => {
    await fulfillJson(route, 200, {
      setup_required: options.setupRequired ?? true,
      config_mutable: options.configMutable ?? true,
      enable_telemetry: false,
    });
  });
  await page.route("**/api/system/hardware", async (route) => {
    await fulfillJson(
      route,
      200,
      options.hardware ?? {
        vendor: "Cpu",
        device_path: null,
        supported_codecs: ["h264", "hevc", "av1"],
      },
    );
  });
  await mockSettingsBundle(page, options.bundle ?? {});
  await page.route("**/api/fs/recommendations", async (route) => {
    await fulfillJson(route, 200, {
      recommendations: options.recommendations ?? [],
    });
  });
}

export async function expectVisibleError(page: Page, message: string): Promise<void> {
  await expect(page.getByText(message).first()).toBeVisible();
}
