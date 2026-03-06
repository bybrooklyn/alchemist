import path from "node:path";

export const RUNTIME_DIR = path.resolve(process.cwd(), ".runtime");
export const MEDIA_DIR = path.join(RUNTIME_DIR, "media");
export const AUTH_STATE_PATH = path.join(RUNTIME_DIR, "auth-state.json");
export const CONFIG_PATH = path.join(RUNTIME_DIR, "config.toml");
export const DB_PATH = path.join(RUNTIME_DIR, "alchemist.db");

export const TEST_USERNAME = "playwright";
export const TEST_PASSWORD = "playwright-password";
