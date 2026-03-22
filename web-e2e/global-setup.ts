import { request, type FullConfig } from "@playwright/test";
import fs from "node:fs/promises";
import {
  AUTH_STATE_PATH,
  BASE_URL,
  MEDIA_DIR,
  TEST_PASSWORD,
  TEST_USERNAME,
} from "./testConfig";

interface SetupStatus {
  setup_required: boolean;
}

async function waitForSetupStatus(maxMs = 30_000): Promise<void> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < maxMs) {
    try {
      const api = await request.newContext({ baseURL: BASE_URL });
      const response = await api.get("/api/setup/status");
      await api.dispose();
      if (response.ok()) {
        return;
      }
    } catch {
      // Continue polling until timeout.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error("Timed out waiting for backend setup endpoint");
}

async function globalSetup(_config: FullConfig): Promise<void> {
  await fs.mkdir(MEDIA_DIR, { recursive: true });
  await waitForSetupStatus();

  const api = await request.newContext({ baseURL: BASE_URL });

  const statusResponse = await api.get("/api/setup/status");
  if (!statusResponse.ok()) {
    throw new Error(`Setup status request failed: ${statusResponse.status()} ${await statusResponse.text()}`);
  }

  const setupStatus = (await statusResponse.json()) as SetupStatus;

  if (setupStatus.setup_required) {
    const setupPayload = {
      username: TEST_USERNAME,
      password: TEST_PASSWORD,
      size_reduction_threshold: 0.3,
      min_bpp_threshold: 0.1,
      min_file_size_mb: 100,
      concurrent_jobs: 2,
      output_codec: "av1",
      quality_profile: "balanced",
      directories: [MEDIA_DIR],
      allow_cpu_encoding: true,
      enable_telemetry: false,
    };

    const setupResponse = await api.post("/api/setup/complete", { data: setupPayload });
    if (!setupResponse.ok()) {
      throw new Error(`Setup completion failed: ${setupResponse.status()} ${await setupResponse.text()}`);
    }
  }

  const loginResponse = await api.post("/api/auth/login", {
    data: {
      username: TEST_USERNAME,
      password: TEST_PASSWORD,
    },
  });

  if (!loginResponse.ok()) {
    throw new Error(`Login failed: ${loginResponse.status()} ${await loginResponse.text()}`);
  }

  await api.storageState({ path: AUTH_STATE_PATH });
  await api.dispose();
}

export default globalSetup;
