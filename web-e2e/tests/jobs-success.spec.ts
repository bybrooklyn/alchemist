import { expect, test } from "@playwright/test";
import {
  JobDetailFixture,
  JobFixture,
  fulfillJson,
  mockEngineStatus,
  mockJobDetails,
} from "./helpers";

const queuedJob: JobFixture = {
  id: 1,
  input_path: "/media/queued.mkv",
  output_path: "/output/queued-av1.mkv",
  status: "queued",
  priority: 0,
  progress: 0,
  created_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-01T00:00:00Z",
};

const failedJob: JobFixture = {
  id: 2,
  input_path: "/media/failed.mkv",
  output_path: "/output/failed-av1.mkv",
  status: "failed",
  priority: 5,
  progress: 100,
  created_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-02T00:00:00Z",
  decision_reason: "transcode_failed|ffmpeg exited 1",
};

const cancelledJob: JobFixture = {
  id: 3,
  input_path: "/media/cancelled.mkv",
  output_path: "/output/cancelled-av1.mkv",
  status: "cancelled",
  priority: 1,
  progress: 12,
  created_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-03T00:00:00Z",
};

const completedJob: JobFixture = {
  id: 4,
  input_path: "/media/completed.mkv",
  output_path: "/output/completed-av1.mkv",
  status: "completed",
  priority: 0,
  progress: 100,
  created_at: "2025-01-01T00:00:00Z",
  updated_at: "2025-01-04T00:00:00Z",
  vmaf_score: 95.4,
};

const failedDetail: JobDetailFixture = {
  job: failedJob,
  metadata: {
    duration_secs: 90,
    codec_name: "h264",
    width: 1920,
    height: 1080,
    bit_depth: 8,
    size_bytes: 2_000_000_000,
    video_bitrate_bps: 8_000_000,
    container_bitrate_bps: 8_200_000,
    fps: 23.976,
    container: "mkv",
    audio_codec: "aac",
    audio_channels: 2,
    dynamic_range: "sdr",
  },
  job_logs: [
    {
      id: 1,
      level: "info",
      message: "frame=5 fps=10",
      created_at: "2025-01-02T00:00:01Z",
    },
    {
      id: 2,
      level: "error",
      message: "Unknown encoder 'missing_encoder'",
      created_at: "2025-01-02T00:00:02Z",
    },
  ],
  job_failure_summary: "Unknown encoder 'missing_encoder'",
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

test("search requests are debounced and failed job details show summary and logs", async ({
  page,
}) => {
  const requests: URL[] = [];

  await page.route("**/api/jobs/table**", async (route) => {
    const url = new URL(route.request().url());
    requests.push(url);
    await fulfillJson(route, 200, [failedJob]);
  });
  await mockJobDetails(page, { 2: failedDetail });

  await page.goto("/jobs");
  await page.getByPlaceholder("Search files...").fill("failed");

  await expect
    .poll(() => requests.some((url) => url.searchParams.get("search") === "failed"))
    .toBe(true);

  await page.getByTitle("/media/failed.mkv").click();

  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByText("What went wrong")).toBeVisible();
  await expect(page.getByText("Unknown encoder 'missing_encoder'").first()).toBeVisible();
  await page.getByText(/Show FFmpeg output \(2 lines\)/).click();
  await expect(page.getByText("frame=5 fps=10")).toBeVisible();
});

test("batch cancel, restart, delete, and clear completed update the job table", async ({
  page,
}) => {
  let jobs = [queuedJob, failedJob, cancelledJob, completedJob];

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, jobs);
  });
  await page.route("**/api/jobs/batch", async (route) => {
    const body = route.request().postDataJSON() as {
      action: "cancel" | "restart" | "delete";
      ids: number[];
    };
    if (body.action === "cancel") {
      jobs = jobs.map((job) =>
        body.ids.includes(job.id) ? { ...job, status: "cancelled", updated_at: "2025-01-05T00:00:00Z" } : job,
      );
    } else if (body.action === "restart") {
      jobs = jobs.map((job) =>
        body.ids.includes(job.id) ? { ...job, status: "queued", progress: 0 } : job,
      );
    } else {
      jobs = jobs.filter((job) => !body.ids.includes(job.id));
    }
    await fulfillJson(route, 200, { count: body.ids.length });
  });
  await page.route("**/api/jobs/clear-completed", async (route) => {
    const count = jobs.filter((job) => job.status === "completed").length;
    jobs = jobs.filter((job) => job.status !== "completed");
    await fulfillJson(route, 200, {
      count,
      message: "Cleared 1 completed job from the queue. Historical stats were preserved.",
    });
  });

  await page.goto("/jobs");

  const queuedRow = page.locator("tbody tr").filter({ has: page.getByTitle("/media/queued.mkv") });
  await queuedRow.locator("input[type='checkbox']").check();
  await page.getByRole("button", { name: "Cancel" }).first().click();
  await page.getByRole("dialog").getByRole("button", { name: "Cancel" }).last().click();
  await expect(queuedRow.getByText("cancelled")).toBeVisible();

  const failedRow = page.locator("tbody tr").filter({ has: page.getByTitle("/media/failed.mkv") });
  await failedRow.locator("input[type='checkbox']").check();
  await page.getByRole("button", { name: "Restart" }).first().click();
  await page.getByRole("dialog").getByRole("button", { name: "Restart" }).last().click();
  await expect(failedRow.getByText("queued")).toBeVisible();

  const cancelledRow = page
    .locator("tbody tr")
    .filter({ has: page.getByTitle("/media/cancelled.mkv") });
  await cancelledRow.locator("input[type='checkbox']").check();
  await page.getByRole("button", { name: "Delete" }).first().click();
  await page.getByRole("dialog").getByRole("button", { name: "Delete" }).last().click();
  await expect(page.getByTitle("/media/cancelled.mkv")).toHaveCount(0);

  await page.getByRole("button", { name: /Clear Completed/i }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Clear" }).click();
  await expect(page.getByTitle("/media/completed.mkv")).toHaveCount(0);
  await expect(
    page
      .getByText("Cleared 1 completed job from the queue. Historical stats were preserved.")
      .first(),
  ).toBeVisible();
});

test("row menu conflicts surface blocked job details from the API", async ({ page }) => {
  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, [cancelledJob]);
  });
  await page.route("**/api/jobs/3/restart", async (route) => {
    await fulfillJson(route, 409, {
      message: "restart is blocked while the job is active",
      blocked: [{ id: 3, status: "encoding" }],
    });
  });

  await page.goto("/jobs");
  await page.getByTitle("Actions").click();
  await page.getByRole("button", { name: "Retry" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Retry" }).click();

  await expect(
    page.getByText("restart is blocked while the job is active: #3 (encoding)").first(),
  ).toBeVisible();
});

test("detail modal delete action removes the job and closes the modal", async ({ page }) => {
  let jobs = [completedJob];

  await page.route("**/api/jobs/table**", async (route) => {
    await fulfillJson(route, 200, jobs);
  });
  await mockJobDetails(page, { 4: completedDetail });
  await page.route("**/api/jobs/4/delete", async (route) => {
    jobs = [];
    await fulfillJson(route, 200, { status: "ok" });
  });

  await page.goto("/jobs");
  await page.getByTitle("/media/completed.mkv").click();
  await expect(page.getByRole("dialog")).toBeVisible();

  await page.getByRole("button", { name: /^Delete$/ }).last().click();
  await page.getByRole("dialog").last().getByRole("button", { name: "Delete" }).click();

  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByTitle("/media/completed.mkv")).toHaveCount(0);
});
