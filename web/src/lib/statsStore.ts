import { useEffect, useState } from "react";
import { apiJson, isApiError } from "./api";

export interface SharedStats {
    active: number;
    concurrent_limit: number;
    completed: number;
    failed: number;
    total: number;
}

export interface SharedStatsSnapshot {
    stats: SharedStats | null;
    loading: boolean;
    error: string | null;
    lastUpdatedAt: number | null;
}

const VISIBLE_INTERVAL_MS = 5000;
const HIDDEN_INTERVAL_MS = 15000;

let snapshot: SharedStatsSnapshot = {
    stats: null,
    loading: true,
    error: null,
    lastUpdatedAt: null,
};

const listeners = new Set<(value: SharedStatsSnapshot) => void>();
let pollTimer: number | null = null;
let polling = false;

function emit(): void {
    for (const listener of listeners) {
        listener(snapshot);
    }
}

function currentIntervalMs(): number {
    if (typeof document !== "undefined" && document.visibilityState === "hidden") {
        return HIDDEN_INTERVAL_MS;
    }
    return VISIBLE_INTERVAL_MS;
}

function scheduleNextPoll(): void {
    if (!polling || typeof window === "undefined") {
        return;
    }

    if (pollTimer !== null) {
        window.clearTimeout(pollTimer);
    }

    pollTimer = window.setTimeout(() => {
        void pollNow();
    }, currentIntervalMs());
}

function normalizeStats(input: Partial<SharedStats>): SharedStats {
    return {
        active: Number(input.active ?? 0),
        concurrent_limit: Math.max(1, Number(input.concurrent_limit ?? 1)),
        completed: Number(input.completed ?? 0),
        failed: Number(input.failed ?? 0),
        total: Number(input.total ?? 0),
    };
}

async function pollNow(): Promise<void> {
    try {
        const data = await apiJson<Partial<SharedStats>>("/api/stats");
        snapshot = {
            stats: normalizeStats(data),
            loading: false,
            error: null,
            lastUpdatedAt: Date.now(),
        };
    } catch (error) {
        snapshot = {
            ...snapshot,
            loading: false,
            error: isApiError(error) ? error.message : "Status unavailable",
        };
    } finally {
        emit();
        scheduleNextPoll();
    }
}

function onVisibilityChange(): void {
    if (!polling) {
        return;
    }

    if (typeof document !== "undefined" && document.visibilityState === "visible") {
        if (pollTimer !== null && typeof window !== "undefined") {
            window.clearTimeout(pollTimer);
            pollTimer = null;
        }
        void pollNow();
        return;
    }

    scheduleNextPoll();
}

function startPolling(): void {
    if (polling || typeof window === "undefined") {
        return;
    }

    polling = true;
    document.addEventListener("visibilitychange", onVisibilityChange);
    void pollNow();
}

function stopPolling(): void {
    if (!polling) {
        return;
    }

    polling = false;
    document.removeEventListener("visibilitychange", onVisibilityChange);
    if (pollTimer !== null && typeof window !== "undefined") {
        window.clearTimeout(pollTimer);
        pollTimer = null;
    }
}

function subscribe(listener: (value: SharedStatsSnapshot) => void): () => void {
    listeners.add(listener);
    listener(snapshot);
    if (listeners.size === 1) {
        startPolling();
    }

    return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
            stopPolling();
        }
    };
}

export function useSharedStats(): SharedStatsSnapshot {
    const [value, setValue] = useState<SharedStatsSnapshot>(snapshot);

    useEffect(() => subscribe(setValue), []);
    return value;
}
