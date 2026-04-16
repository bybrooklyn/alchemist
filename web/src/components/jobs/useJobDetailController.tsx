import { useCallback, useEffect, useRef, useState } from "react";
import { apiAction, apiJson, isApiError } from "../../lib/api";
import { showToast } from "../../lib/toast";
import { normalizeDecisionExplanation, normalizeFailureExplanation } from "./JobExplanations";
import type {
    ConfirmConfig,
    EncodeStats,
    ExplanationView,
    Job,
    JobDetail,
    LogEntry,
} from "./types";
import { jobDetailEmptyState } from "./types";

function focusableElements(root: HTMLElement): HTMLElement[] {
    const selector = [
        "a[href]",
        "button:not([disabled])",
        "input:not([disabled])",
        "select:not([disabled])",
        "textarea:not([disabled])",
        "[tabindex]:not([tabindex='-1'])",
    ].join(",");

    return Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(
        (element) => !element.hasAttribute("disabled"),
    );
}

function formatJobActionError(error: unknown, fallback: string) {
    if (!isApiError(error)) {
        return fallback;
    }

    const blocked = Array.isArray((error.body as { blocked?: unknown } | undefined)?.blocked)
        ? ((error.body as { blocked?: Array<{ id?: number; status?: string }> }).blocked ?? [])
        : [];
    if (blocked.length === 0) {
        return error.message;
    }

    const summary = blocked
        .map((job) => `#${job.id ?? "?"} (${job.status ?? "unknown"})`)
        .join(", ");
    return `${error.message}: ${summary}`;
}

interface UseJobDetailControllerOptions {
    onRefresh?: () => Promise<void>;
}

export function useJobDetailController(options: UseJobDetailControllerOptions = {}) {
    const [focusedJob, setFocusedJob] = useState<JobDetail | null>(null);
    const [detailLoading, setDetailLoading] = useState(false);
    const [confirmState, setConfirmState] = useState<ConfirmConfig | null>(null);
    const detailDialogRef = useRef<HTMLDivElement | null>(null);
    const detailLastFocusedRef = useRef<HTMLElement | null>(null);
    const confirmOpenRef = useRef(false);

    useEffect(() => {
        confirmOpenRef.current = confirmState !== null;
    }, [confirmState]);

    useEffect(() => {
        if (!focusedJob) {
            return;
        }

        detailLastFocusedRef.current = document.activeElement as HTMLElement | null;

        const root = detailDialogRef.current;
        if (root) {
            const focusables = focusableElements(root);
            if (focusables.length > 0) {
                focusables[0].focus();
            } else {
                root.focus();
            }
        }

        const onKeyDown = (event: KeyboardEvent) => {
            if (!focusedJob || confirmOpenRef.current) {
                return;
            }

            if (event.key === "Escape") {
                event.preventDefault();
                setFocusedJob(null);
                return;
            }

            if (event.key !== "Tab") {
                return;
            }

            const dialogRoot = detailDialogRef.current;
            if (!dialogRoot) {
                return;
            }

            const focusables = focusableElements(dialogRoot);
            if (focusables.length === 0) {
                event.preventDefault();
                dialogRoot.focus();
                return;
            }

            const first = focusables[0];
            const last = focusables[focusables.length - 1];
            const current = document.activeElement as HTMLElement | null;

            if (event.shiftKey && current === first) {
                event.preventDefault();
                last.focus();
            } else if (!event.shiftKey && current === last) {
                event.preventDefault();
                first.focus();
            }
        };

        document.addEventListener("keydown", onKeyDown);
        return () => {
            document.removeEventListener("keydown", onKeyDown);
            if (detailLastFocusedRef.current) {
                detailLastFocusedRef.current.focus();
            }
        };
    }, [focusedJob]);

    const openJobDetails = useCallback(async (id: number) => {
        setDetailLoading(true);
        try {
            const data = await apiJson<JobDetail>(`/api/jobs/${id}/details`);
            setFocusedJob(data);
        } catch (error) {
            const message = isApiError(error) ? error.message : "Failed to fetch job details";
            showToast({ kind: "error", title: "Jobs", message });
        } finally {
            setDetailLoading(false);
        }
    }, []);

    const handleAction = useCallback(async (id: number, action: "cancel" | "restart" | "delete") => {
        try {
            await apiAction(`/api/jobs/${id}/${action}`, { method: "POST" });
            if (action === "delete") {
                setFocusedJob((current) => (current?.job.id === id ? null : current));
            } else if (focusedJob?.job.id === id) {
                await openJobDetails(id);
            }
            if (options.onRefresh) {
                await options.onRefresh();
            }
            showToast({
                kind: "success",
                title: "Jobs",
                message: `Job ${action} request completed.`,
            });
        } catch (error) {
            const message = formatJobActionError(error, `Job ${action} failed`);
            showToast({ kind: "error", title: "Jobs", message });
        }
    }, [focusedJob?.job.id, openJobDetails, options]);

    const handlePriority = useCallback(async (job: Job, priority: number, label: string) => {
        try {
            await apiAction(`/api/jobs/${job.id}/priority`, {
                method: "POST",
                body: JSON.stringify({ priority }),
            });
            if (focusedJob?.job.id === job.id) {
                setFocusedJob({
                    ...focusedJob,
                    job: {
                        ...focusedJob.job,
                        priority,
                    },
                });
            }
            if (options.onRefresh) {
                await options.onRefresh();
            }
            showToast({ kind: "success", title: "Jobs", message: `${label} for job #${job.id}.` });
        } catch (error) {
            const message = formatJobActionError(error, "Failed to update priority");
            showToast({ kind: "error", title: "Jobs", message });
        }
    }, [focusedJob, options]);

    const openConfirm = useCallback((config: ConfirmConfig) => {
        setConfirmState(config);
    }, []);

    const focusedDecision: ExplanationView | null = focusedJob
        ? normalizeDecisionExplanation(
            focusedJob.decision_explanation ?? focusedJob.job.decision_explanation,
            focusedJob.job.decision_reason,
        )
        : null;
    const focusedFailure: ExplanationView | null = focusedJob
        ? normalizeFailureExplanation(
            focusedJob.failure_explanation,
            focusedJob.job_failure_summary,
            focusedJob.job_logs,
        )
        : null;
    const focusedJobLogs: LogEntry[] = focusedJob?.job_logs ?? [];
    const shouldShowFfmpegOutput = focusedJob
        ? ["failed", "completed", "skipped"].includes(focusedJob.job.status) && focusedJobLogs.length > 0
        : false;
    const completedEncodeStats: EncodeStats | null = focusedJob?.job.status === "completed"
        ? focusedJob.encode_stats
        : null;
    const focusedEmptyState = focusedJob
        ? jobDetailEmptyState(focusedJob.job.status)
        : null;

    return {
        focusedJob,
        setFocusedJob,
        detailLoading,
        confirmState,
        detailDialogRef,
        openJobDetails,
        handleAction,
        handlePriority,
        openConfirm,
        setConfirmState,
        closeJobDetails: () => setFocusedJob(null),
        focusedDecision,
        focusedFailure,
        focusedJobLogs,
        shouldShowFfmpegOutput,
        completedEncodeStats,
        focusedEmptyState,
    };
}
