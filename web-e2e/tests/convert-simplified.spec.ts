import { expect, test } from "@playwright/test";
import { fulfillJson, mockEngineStatus } from "./helpers";

const normalizedSettings = {
  output_container: "mkv",
  remux_only: false,
  video: {
    codec: "hevc",
    mode: "crf",
    value: 24,
    preset: "medium",
    resolution: {
      mode: "original",
      width: null,
      height: null,
      scale_factor: null,
    },
    hdr_mode: "preserve",
  },
  audio: {
    codec: "copy",
    bitrate_kbps: 160,
    channels: "auto",
  },
  subtitles: {
    mode: "copy",
  },
};

const probe = {
  metadata: {
    path: "/tmp/Movie Sample.mkv",
    duration_secs: 7265,
    container: "mkv",
    codec_name: "h264",
    width: 1920,
    height: 1080,
    dynamic_range: "sdr",
    size_bytes: 4_294_967_296,
    video_bitrate_bps: 4_200_000,
    audio_codec: "aac",
    audio_bitrate_bps: 192_000,
    audio_channels: 2,
    audio_streams: [
      {
        stream_index: 1,
        codec_name: "aac",
        language: "eng",
        title: "Main",
        channels: 2,
      },
    ],
    subtitle_streams: [
      {
        stream_index: 2,
        codec_name: "subrip",
        language: "eng",
        title: "English",
        burnable: true,
      },
    ],
  },
};

const previewSummary = {
  source: {
    file_name: "Movie Sample.mkv",
    container: "mkv",
    video_codec: "h264",
    resolution: "1920x1080",
    dynamic_range: "sdr",
    duration_secs: 7265,
    size_bytes: 4_294_967_296,
    audio: "aac / 2ch",
    subtitle_count: 1,
  },
  planned_output: {
    mode: "compress",
    container: "mkv",
    video_codec: "hevc",
    resolution: "1920x1080",
    hdr_mode: "preserve",
    audio: "copy",
    subtitles: "copy compatible",
    encoder: "libx265",
    backend: "cpu",
  },
  estimate: {
    estimated_output_bytes: 2_362_232_013,
    estimated_savings_bytes: 1_932_735_283,
    estimated_savings_percent: 45.0,
    confidence: "medium",
    note: "Estimated from source bitrate, target codec, selected quality, audio plan, and container overhead.",
  },
};

test.use({ storageState: undefined });

test.beforeEach(async ({ page }) => {
  await mockEngineStatus(page);
  await page.route("**/api/conversion/uploads", async (route) => {
    await fulfillJson(route, 200, {
      conversion_job_id: 42,
      probe,
      normalized_settings: normalizedSettings,
    });
  });
  await page.route("**/api/conversion/preview", async (route) => {
    await fulfillJson(route, 200, {
      normalized_settings: normalizedSettings,
      command_preview: "ffmpeg -i input.mkv -c:v libx265 -crf 24 output.mkv",
      summary: previewSummary,
    });
  });
  await page.route("**/api/conversion/jobs/42", async (route) => {
    await fulfillJson(route, 200, {
      id: 42,
      status: "uploaded",
      progress: 0,
      linked_job_id: null,
      output_path: null,
      download_ready: false,
      probe,
    });
  });
});

test("convert page shows a simple summary first and hides precise controls behind advanced", async ({
  page,
}) => {
  await page.goto("/convert");

  await page
    .locator('input[type="file"]')
    .setInputFiles({
      name: "Movie Sample.mkv",
      mimeType: "video/x-matroska",
      buffer: Buffer.from("sample"),
    });

  await expect(page.getByRole("heading", { name: "Movie Sample.mkv" })).toBeVisible();
  await expect(page.getByText("Planned Output")).toBeVisible();
  await expect(page.getByText("1.8 GB saved (45.0%)")).toBeVisible();
  await expect(page.getByLabel("Mode")).toHaveValue("compress");
  await expect(page.getByLabel("Quality")).toHaveValue("balanced");
  await expect(page.getByLabel("Container")).toHaveValue("mkv");

  await expect(page.getByLabel("Video Codec")).toHaveCount(0);
  await expect(page.getByText("FFmpeg Command")).toHaveCount(0);

  await page.getByRole("button", { name: /Advanced/i }).click();

  await expect(page.getByText("Power Controls")).toBeVisible();
  await expect(page.getByLabel("Video Codec")).toBeVisible();
  await expect(page.getByText("FFmpeg Command")).toBeVisible();
  await expect(page.getByText("ffmpeg -i input.mkv")).toBeVisible();
});
