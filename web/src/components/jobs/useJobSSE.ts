import { useEffect } from "react";
import type { MutableRefObject, Dispatch, SetStateAction } from "react";
import type { Job, JobDetail } from "./types";

interface UseJobSSEOptions {
    setJobs: Dispatch<SetStateAction<Job[]>>;
    setFocusedJob: Dispatch<SetStateAction<JobDetail | null>>;
    fetchJobsRef: MutableRefObject<() => Promise<void>>;
    encodeStartTimes: MutableRefObject<Map<number, number>>;
}

export function useJobSSE({ setJobs, setFocusedJob, fetchJobsRef, encodeStartTimes }: UseJobSSEOptions): void {
    useEffect(() => {
        let eventSource: EventSource | null = null;
        let cancelled = false;
        let reconnectTimeout: number | null = null;
        let reconnectAttempts = 0;

        const getReconnectDelay = () => {
            const baseDelay = 1000;
            const maxDelay = 30000;
            const delay = Math.min(baseDelay * Math.pow(2, reconnectAttempts), maxDelay);
            const jitter = delay * 0.25 * (Math.random() * 2 - 1);
            return Math.round(delay + jitter);
        };

        const connect = () => {
            if (cancelled) return;
            eventSource?.close();
            eventSource = new EventSource("/api/events");

            eventSource.onopen = () => {
                reconnectAttempts = 0;
            };

            eventSource.addEventListener("status", (e) => {
                try {
                    const { job_id, status } = JSON.parse(e.data) as {
                        job_id: number;
                        status: string;
                    };
                    const terminalStatuses = ["completed", "failed", "cancelled", "skipped"];
                    if (status === "encoding") {
                        encodeStartTimes.current.set(job_id, Date.now());
                    } else if (terminalStatuses.includes(status)) {
                        encodeStartTimes.current.delete(job_id);
                    }
                    setJobs((prev) =>
                        prev.map((job) => job.id === job_id ? { ...job, status } : job)
                    );
                    setFocusedJob((prev) =>
                        prev?.job.id === job_id ? { ...prev, job: { ...prev.job, status } } : prev
                    );
                } catch {
                    /* ignore malformed */
                }
            });

            eventSource.addEventListener("progress", (e) => {
                try {
                    const { job_id, percentage } = JSON.parse(e.data) as {
                        job_id: number;
                        percentage: number;
                    };
                    setJobs((prev) =>
                        prev.map((job) => job.id === job_id ? { ...job, progress: percentage } : job)
                    );
                } catch {
                    /* ignore malformed */
                }
            });

            eventSource.addEventListener("decision", () => {
                void fetchJobsRef.current();
            });

            eventSource.onerror = () => {
                eventSource?.close();
                if (!cancelled) {
                    reconnectAttempts++;
                    const delay = getReconnectDelay();
                    reconnectTimeout = window.setTimeout(connect, delay);
                }
            };
        };

        connect();

        return () => {
            cancelled = true;
            eventSource?.close();
            if (reconnectTimeout !== null) {
                window.clearTimeout(reconnectTimeout);
            }
        };
    }, []);
}
