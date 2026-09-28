import { useEffect, useState } from "react";
import { apiJson, isApiError } from "./api";
import { createPollingLifecycle } from "./pollingStore";

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

function emit(): void {
    for (const listener of listeners) {
        listener(snapshot);
    }
}

function currentIntervalMs(): number | null {
    if (snapshot.status?.status === "draining") {
        return DRAINING_INTERVAL_MS;
    }
    return null;
}

// Singleton polling loop shared by every subscriber (see pollingStore.ts).
// Previously this file carried its own copy of the timer/visibility
// lifecycle, identical to statsStore.ts's.
const lifecycle = createPollingLifecycle({
    visibleIntervalMs: VISIBLE_INTERVAL_MS,
    hiddenIntervalMs: HIDDEN_INTERVAL_MS,
    intervalOverrideMs: currentIntervalMs,
    poll: () => {
        void pollNow();
    },
});

function scheduleNextPoll(): void {
    lifecycle.scheduleNextPoll();
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
    lifecycle.cancelScheduled();
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

function subscribe(listener: (value: EngineStatusSnapshot) => void): () => void {
    listeners.add(listener);
    listener(snapshot);
    if (listeners.size === 1) {
        lifecycle.startPolling();
    }

    return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
            lifecycle.stopPolling();
        }
    };
}

export function useEngineStatus(): EngineStatusSnapshot {
    const [value, setValue] = useState<EngineStatusSnapshot>(snapshot);

    useEffect(() => subscribe(setValue), []);
    return value;
}
