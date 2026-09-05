import { useEffect, useState } from "react";
import { apiJson, isApiError } from "./api";

// Shared engine-status polling, mirroring statsStore.ts's singleton pattern.
// HeaderActions and the Dashboard paused/draining banner used to each run
// their own independent fetch of /api/engine/status — one on a 5s timer, the
// other only once on mount — which could disagree about the same engine and
// left the dashboard banner stuck after a queue drained or the engine was
// started from the header. Centralizing here means one fetch serves every
// subscriber, and an action (Start/Stop) updates every subscriber at once.

export interface EngineStatus {
    status: "running" | "paused" | "draining";
    manual_paused: boolean;
    scheduler_paused: boolean;
    draining: boolean;
    disk_blocked?: boolean;
    disk_block_reason?: string | null;
    mode: "background" | "balanced" | "throughput";
    concurrent_limit: number;
    is_manual_override: boolean;
}

export type EngineActionStatus = Pick<EngineStatus, "status">;

export interface EngineStatusSnapshot {
    status: EngineStatus | null;
    loading: boolean;
    error: string | null;
}

export const DEFAULT_ENGINE_STATUS: EngineStatus = {
    status: "paused",
    manual_paused: true,
    scheduler_paused: false,
    draining: false,
    disk_blocked: false,
    disk_block_reason: null,
    mode: "background",
    concurrent_limit: 1,
    is_manual_override: false,
};

const VISIBLE_INTERVAL_MS = 5000;
const HIDDEN_INTERVAL_MS = 15000;
const DRAINING_INTERVAL_MS = 1000;

let snapshot: EngineStatusSnapshot = {
    status: null,
    loading: true,
    error: null,
};

const listeners = new Set<(value: EngineStatusSnapshot) => void>();
let pollTimer: number | null = null;
let polling = false;

function emit(): void {
    for (const listener of listeners) {
        listener(snapshot);
    }
}

function currentIntervalMs(): number {
    if (snapshot.status?.status === "draining") {
        return DRAINING_INTERVAL_MS;
    }
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

async function pollNow(): Promise<EngineStatus | null> {
    try {
        const data = await apiJson<EngineStatus>("/api/engine/status");
        snapshot = { status: data, loading: false, error: null };
        return data;
    } catch (error) {
        snapshot = {
            ...snapshot,
            loading: false,
            error: isApiError(error) ? error.message : "Engine status unavailable",
        };
        return null;
    } finally {
        emit();
        scheduleNextPoll();
    }
}

/** Force an immediate poll, bypassing the current timer — used to confirm an
 *  action's optimistic status update against the server. */
export async function refreshEngineStatusNow(): Promise<EngineStatus | null> {
    if (pollTimer !== null && typeof window !== "undefined") {
        window.clearTimeout(pollTimer);
        pollTimer = null;
    }
    return pollNow();
}

/** Optimistically merge an action's response (e.g. resume/drain) into the
 *  shared snapshot so every subscriber reflects it immediately, instead of
 *  waiting for the next poll tick. */
export function applyEngineActionStatus(actionStatus: EngineActionStatus): void {
    const current = snapshot.status ?? DEFAULT_ENGINE_STATUS;
    snapshot = {
        status: {
            ...current,
            status: actionStatus.status,
            manual_paused:
                actionStatus.status === "running"
                    ? false
                    : actionStatus.status === "paused"
                      ? true
                      : current.manual_paused,
            draining: actionStatus.status === "draining",
        },
        loading: false,
        error: null,
    };
    emit();
    // Draining needs the fast interval starting now, not after the next
    // (possibly 5s-away) scheduled tick.
    scheduleNextPoll();
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

function subscribe(listener: (value: EngineStatusSnapshot) => void): () => void {
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

export function useEngineStatus(): EngineStatusSnapshot {
    const [value, setValue] = useState<EngineStatusSnapshot>(snapshot);

    useEffect(() => subscribe(setValue), []);
    return value;
}
