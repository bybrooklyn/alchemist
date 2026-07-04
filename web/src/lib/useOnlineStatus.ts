import { useEffect, useState } from "react";

const HEALTH_URL = "/api/health";
const POLL_INTERVAL_MS = 15000;
const REQUEST_TIMEOUT_MS = 5000;

async function pingServer(): Promise<boolean> {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
    try {
        const res = await fetch(HEALTH_URL, {
            method: "GET",
            cache: "no-store",
            signal: controller.signal,
        });
        return res.ok;
    } catch {
        return false;
    } finally {
        window.clearTimeout(timeout);
    }
}

/**
 * Tracks whether the Alchemist *server* is reachable from this client by polling
 * `/api/health` — not the browser's WAN connectivity (`navigator.onLine`), which is
 * meaningless for a self-hosted app reached over a LAN and produced a false "offline"
 * banner on a perfectly working instance. Starts optimistic so the banner never flashes
 * during normal use; only reports offline after a health check actually fails. A real
 * server outage (or the tab losing the network) still surfaces it.
 */
export function useOnlineStatus(): boolean {
    const [online, setOnline] = useState(true);

    useEffect(() => {
        let cancelled = false;

        const check = async () => {
            const reachable = await pingServer();
            if (!cancelled) {
                setOnline(reachable);
            }
        };

        void check();
        const interval = window.setInterval(() => void check(), POLL_INTERVAL_MS);
        // Browser connectivity changes are only a hint to re-check the server sooner.
        const recheck = () => void check();
        window.addEventListener("online", recheck);
        window.addEventListener("offline", recheck);

        return () => {
            cancelled = true;
            window.clearInterval(interval);
            window.removeEventListener("online", recheck);
            window.removeEventListener("offline", recheck);
        };
    }, []);

    return online;
}
