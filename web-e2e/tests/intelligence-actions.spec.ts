import { expect, test } from "@playwright/test";
import {
  type JobDetailFixture,
  fulfillJson,
  mockEngineStatus,
  mockJobDetails,
} from "./helpers";

const completedDetail: JobDetailFixture = {
  job: {
    id: 51,
    input_path: "/media/duplicates/movie-copy-1.mkv",
    output_path: "/output/movie-copy-1-av1.mkv",
    status: "completed",
    priority: 0,
    progress: 100,
    created_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-02T00:00:00Z",
    vmaf_score: 95.1,
  },
  metadata: {
    duration_secs: 120,
    codec_name: "hevc",
    width: 1920,
    height: 1080,
    bit_depth: 10,
    size_bytes: 2_000_000_000,
    video_bitrate_bps: 12_000_000,
    container_bitrate_bps: 12_500_000,
    fps: 24,
    container: "mkv",
    audio_codec: "aac",
    audio_channels: 2,
    dynamic_range: "hdr10",
  },
  encode_stats: {
    input_size_bytes: 2_000_000_000,
    output_size_bytes: 900_000_000,
    compression_ratio: 0.55,
    encode_time_seconds: 1800,
    encode_speed: 1.6,
    avg_bitrate_kbps: 6000,
    vmaf_score: 95.1,
  },
  job_logs: [],
};

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
});

test("intelligence actions queue remux opportunities and review duplicate jobs", async ({
  page,
}) => {
  let enqueueCount = 0;

  await page.route("**/api/library/intelligence", async (route) => {
    await fulfillJson(route, 200, {
      duplicate_groups: [
        {
          stem: "movie-copy",
          count: 2,
          paths: [
            { id: 51, path: "/media/duplicates/movie-copy-1.mkv", status: "completed" },
            { id: 52, path: "/media/duplicates/movie-copy-2.mkv", status: "queued" },
          ],
        },
      ],
      total_duplicates: 1,
      recommendation_counts: {
        duplicates: 1,
        remux_only_candidate: 2,
        wasteful_audio_layout: 0,
        commentary_cleanup_candidate: 0,
      },
      recommendations: [
        {
          type: "remux_only_candidate",
          title: "Remux movie one",
          summary: "The file can be normalized with a container-only remux.",
          path: "/media/remux/movie-one.mkv",
          suggested_action: "Queue a remux to normalize the container without re-encoding the video stream.",
        },
        {
          type: "remux_only_candidate",
          title: "Remux movie two",
          summary: "The file can be normalized with a container-only remux.",
          path: "/media/remux/movie-two.mkv",
          suggested_action: "Queue a remux to normalize the container without re-encoding the video stream.",
        },
      ],
    });
  });
  await page.route("**/api/jobs/enqueue", async (route) => {
    enqueueCount += 1;
    const body = route.request().postDataJSON() as { path: string };
    await fulfillJson(route, 200, {
      enqueued: true,
      message: `Enqueued ${body.path}.`,
    });
  });
  await mockJobDetails(page, { 51: completedDetail });

  await page.goto("/intelligence");

  await page.getByRole("button", { name: "Queue all" }).click();
  await expect.poll(() => enqueueCount).toBe(2);
  await expect(
    page.getByText("Queue all finished: 2 enqueued, 0 skipped, 0 failed.").first(),
  ).toBeVisible();

  await page.getByRole("button", { name: "Review" }).first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByText("Encode Results")).toBeVisible();
  await expect(page.getByRole("dialog").getByText("/media/duplicates/movie-copy-1.mkv")).toBeVisible();
});
