import { useEffect, useState } from "react";

// Shared singleton polling lifecycle (extracted from statsStore.ts and
// engineStatusStore.ts, which carried identical copies). Owns the timer,
// the polling flag, and the visibility handling; the store supplies the
// fetch (`poll`) and the per-tick interval. First subscriber starts the
// loop, last unsubscriber stops it.

export interface PollingLifecycleConfig {
    visibleIntervalMs: number;
    hiddenIntervalMs: number;
    /** Current-tick interval override (e.g. a fast drain poll). Return
     *  null to use the visibility-based default. */
    intervalOverrideMs?: () => number | null;
    /** Fetch + snapshot update. The lifecycle reschedules after it. */
    poll: () => void;
}

export interface PollingLifecycle {
    startPolling: () => void;
    stopPolling: () => void;
    onVisibilityChange: () => void;
    scheduleNextPoll: () => void;
    cancelScheduled: () => void;
}

export function createPollingLifecycle(config: PollingLifecycleConfig): PollingLifecycle {
    let pollTimer: number | null = null;
    let polling = false;

    function currentIntervalMs(): number {
        const override = config.intervalOverrideMs?.();
        if (override !== null && override !== undefined) {
            return override;
        }
        if (typeof document !== "undefined" && document.visibilityState === "hidden") {
            return config.hiddenIntervalMs;
        }
        return config.visibleIntervalMs;
    }

    function cancelScheduled(): void {
        if (pollTimer !== null && typeof window !== "undefined") {
            window.clearTimeout(pollTimer);
            pollTimer = null;
        }
    }

    function scheduleNextPoll(): void {
        if (!polling || typeof window === "undefined") {
            return;
        }

        cancelScheduled();
        pollTimer = window.setTimeout(() => {
            config.poll();
        }, currentIntervalMs());
    }

    function onVisibilityChange(): void {
        if (!polling) {
            return;
        }

        if (typeof document !== "undefined" && document.visibilityState === "visible") {
            cancelScheduled();
            config.poll();
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
        config.poll();
    }

    function stopPolling(): void {
        if (!polling) {
            return;
        }

        polling = false;
        document.removeEventListener("visibilitychange", onVisibilityChange);
        cancelScheduled();
    }

    return { startPolling, stopPolling, onVisibilityChange, scheduleNextPoll, cancelScheduled };
}

/** React binding for a `createPollingLifecycle` loop: subscribes on mount,
 *  unsubscribes on unmount, and reseeds from the latest snapshot. */
export function usePollingSnapshot<T>(
    getSnapshot: () => T,
    subscribe: (listener: (value: T) => void) => () => void,
): T {
    const [value, setValue] = useState<T>(getSnapshot);

    useEffect(() => subscribe(setValue), [subscribe]);
    return value;
}
