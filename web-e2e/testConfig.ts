import path from "node:path";

export const RUNTIME_DIR = path.resolve(process.cwd(), ".runtime");
export const MEDIA_DIR = path.join(RUNTIME_DIR, "media");
export const AUTH_STATE_PATH = path.join(RUNTIME_DIR, "auth-state.json");
export const CONFIG_PATH = path.join(RUNTIME_DIR, "config.toml");
export const DB_PATH = path.join(RUNTIME_DIR, "alchemist.db");

const rawPort = process.env.ALCHEMIST_E2E_PORT ?? "3000";
const parsedPort = Number.parseInt(rawPort, 10);

if (!Number.isInteger(parsedPort) || parsedPort <= 0 || parsedPort > 65535) {
  throw new Error(`ALCHEMIST_E2E_PORT must be a valid port, received "${rawPort}"`);
}

export const PORT = parsedPort;
export const BASE_URL = `http://127.0.0.1:${PORT}`;

export const TEST_USERNAME = "playwright";
export const TEST_PASSWORD = "playwright-password";
