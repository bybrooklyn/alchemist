import { expect, test } from "@playwright/test";
import {
  type JobDetailFixture,
  type JobFixture,
  fulfillJson,
  mockEngineStatus,
  mockJobDetails,
} from "./helpers";

const completedJob: JobFixture = {
  id: 41,
  input_path: "/media/completed-stability.mkv",
  output_path: "/output/completed-stability-av1.mkv",
  status: "completed",
  priority: 0,
  progress: 100,
  created_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-04T00:00:00Z",
  vmaf_score: 95.4,
};

const queuedJob: JobFixture = {
  id: 44,
  input_path: "/media/queued-blocked.mkv",
  output_path: "/output/queued-blocked-av1.mkv",
  status: "queued",
  priority: 0,
  progress: 0,
  created_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-02T00:00:00Z",
};

const completedDetail: JobDetailFixture = {
  job: completedJob,
  metadata: {
    duration_secs: 120,
    codec_name: "hevc",
    width: 3840,
    height: 2160,
    bit_depth: 10,
    size_bytes: 4_000_000_000,
    video_bitrate_bps: 15_000_000,
    container_bitrate_bps: 15_500_000,
    fps: 24,
    container: "mkv",
    audio_codec: "aac",
    audio_channels: 6,
    dynamic_range: "hdr10",
  },
  encode_stats: {
    input_size_bytes: 4_000_000_000,
    output_size_bytes: 1_800_000_000,
    compression_ratio: 0.45,
    encode_time_seconds: 3600,
    encode_speed: 1.25,
    avg_bitrate_kbps: 7000,
    vmaf_score: 95.4,
  },
  job_logs: [
    {
      id: 10,
      level: "info",
      message: "Transcode completed successfully",
      created_at: "2025-01-04T00:00:02Z",
    },
  ],
};

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("failed jobs waiting to retry show a retry countdown", async ({ page }) => {
  const retryingJob: JobFixture = {
    id: 40,
    input_path: "/media/retrying.mkv",
    output_path: "/output/retrying-av1.mkv",
    status: "failed",
    priority: 1,
    progress: 100,
    attempt_count: 4,
    created_at: "2025-01-01T00:00:00Z",
    updated_at: new Date().toISOString(),
    decision_reason: "transcode_failed|ffmpeg exited 1",
  };

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [retryingJob]);
  });

  await page.goto("/jobs");

  await expect(page.getByText("Retrying in 6h")).toBeVisible();
});

test("completed job detail renders persisted encode stats", async ({ page }) => {
  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [completedJob]);
  });
  await mockJobDetails(page, { 41: completedDetail });

  await page.goto("/jobs");
  await page.getByTitle("/media/completed-stability.mkv").click();

  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByText("Encode Results")).toBeVisible();
  await expect(page.getByText("Input size")).toBeVisible();
  await expect(page.getByText("Output size")).toBeVisible();
  await expect(page.locator("span").filter({ hasText: /^55\.0% saved$/ })).toBeVisible();
  await expect(page.getByText("01:00:00")).toBeVisible();
  await expect(page.getByText("1.25× realtime")).toBeVisible();
  await expect(page.getByText("7000 kbps")).toBeVisible();
  await expect(page.getByText("95.4").first()).toBeVisible();
});

test("skipped job detail prefers structured decision explanation", async ({ page }) => {
  const skippedJob: JobFixture = {
    id: 42,
    input_path: "/media/skipped-structured.mkv",
    output_path: "/output/skipped-structured-av1.mkv",
    status: "skipped",
    priority: 0,
    progress: 0,
    created_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-02T00:00:00Z",
    decision_reason: "bpp_below_threshold|bpp=0.043,threshold=0.050",
  };

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [skippedJob]);
  });
  await mockJobDetails(page, {
    42: {
      job: skippedJob,
      job_logs: [],
      decision_explanation: {
        category: "decision",
        code: "bpp_below_threshold",
        summary: "Structured skip summary",
        detail: "Structured skip detail from the backend.",
        operator_guidance: "Structured skip guidance from the backend.",
        measured: { bpp: 0.043, threshold: 0.05 },
        legacy_reason: skippedJob.decision_reason!,
      },
    },
  });

  await page.goto("/jobs");
  await page.getByTitle("/media/skipped-structured.mkv").click();

  await expect(page.getByText("Structured skip summary")).toBeVisible();
  await expect(page.getByText("Structured skip detail from the backend.")).toBeVisible();
  await expect(page.getByText("Structured skip guidance from the backend.")).toBeVisible();
});

test("failed job detail prefers structured failure explanation", async ({ page }) => {
  const failedJob: JobFixture = {
    id: 43,
    input_path: "/media/failed-structured.mkv",
    output_path: "/output/failed-structured-av1.mkv",
    status: "failed",
    priority: 0,
    progress: 100,
    created_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-02T00:00:00Z",
  };

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [failedJob]);
  });
  await mockJobDetails(page, {
    43: {
      job: failedJob,
      job_logs: [],
      job_failure_summary: "Unknown encoder 'missing_encoder'",
      failure_explanation: {
        category: "failure",
        code: "encoder_unavailable",
        summary: "Structured failure summary",
        detail: "Structured failure detail from the backend.",
        operator_guidance: "Structured failure guidance from the backend.",
        measured: {},
        legacy_reason: "Unknown encoder 'missing_encoder'",
      },
    },
  });

  await page.goto("/jobs");
  await page.getByTitle("/media/failed-structured.mkv").click();

  await expect(page.getByText("Structured failure summary")).toBeVisible();
  await expect(page.getByText("Structured failure detail from the backend.")).toBeVisible();
  await expect(page.getByText("Structured failure guidance from the backend.")).toBeVisible();
});

test("queued job detail shows the processor blocked reason", async ({ page }) => {
  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [queuedJob]);
  });
  await mockJobDetails(page, {
    44: {
      job: queuedJob,
      job_logs: [],
      queue_position: 3,
    },
  });
  await page.route("**/api/processor/status", async (route) => {
    await fulfillJson(route, 200, {
      blocked_reason: "workers_busy",
      message: "All worker slots are currently busy.",
      manual_paused: false,
      scheduler_paused: false,
      draining: false,
      active_jobs: 1,
      concurrent_limit: 1,
    });
  });

  await page.goto("/jobs");
  await page.getByTitle("/media/queued-blocked.mkv").click();

  await expect(page.getByText("Queue position:")).toBeVisible();
  await expect(page.getByText("Blocked:")).toBeVisible();
  await expect(page.getByText("All worker slots are currently busy.")).toBeVisible();
});

test("add file submits the enqueue request and surfaces the response", async ({ page }) => {
  let postedPath = "";
  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, []);
  });
  await page.route("**/api/jobs/enqueue", async (route) => {
    const body = route.request().postDataJSON() as { path: string };
    postedPath = body.path;
    await fulfillJson(route, 200, {
      enqueued: true,
      message: `Enqueued ${body.path}.`,
    });
  });

  await page.goto("/jobs");
  await page.getByRole("button", { name: "Add file" }).click();
  await page.getByPlaceholder("/Volumes/Media/Movies/example.mkv").fill("/media/manual-add.mkv");
  await page.getByRole("dialog").getByRole("button", { name: "Add File", exact: true }).click();

  await expect.poll(() => postedPath).toBe("/media/manual-add.mkv");
  await expect(page.getByText("Enqueued /media/manual-add.mkv.").first()).toBeVisible();
});
