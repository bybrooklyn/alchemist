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
