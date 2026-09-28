import { useCallback, useEffect, useRef, useState } from "react";
import { apiAction, apiJson, isApiError } from "../../lib/api";
import { showToast } from "../../lib/toast";
import { normalizeDecisionExplanation, normalizeFailureExplanation } from "./JobExplanations";
import { focusableElements, restoreFocus, setAppShellInert, trapTabNavigation } from "../../lib/focusUtils";
import type {
    ConfirmConfig,
    EncodeStats,
    ExplanationView,
    Job,
    JobDetail,
    LogEntry,
} from "./types";
import { formatJobActionError, jobDetailEmptyState } from "./types";

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

    // The dialog lifecycle (inert, initial focus, focus restore) must run once
    // per open/close — not on every SSE-driven `focusedJob` identity change,
    // which would re-capture `detailLastFocusedRef` from inside the modal and
    // bounce focus to the panel's first element mid-interaction.
    const detailOpen = focusedJob !== null;
    const detailOpenRef = useRef(detailOpen);

    // Sequence guard for competing detail fetches (P2-62): without it a
    // slow response for row A can overwrite row B's detail when clicked in
    // quick succession. Mirrors fetchSeqRef in JobManager.
    const detailSeqRef = useRef(0);

    useEffect(() => {
        detailOpenRef.current = focusedJob !== null;
    }, [focusedJob]);

    useEffect(() => {
        if (!detailOpen) {
            return;
        }

        setAppShellInert(true);
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
            if (!detailOpenRef.current || confirmOpenRef.current) {
                return;
            }

            if (event.key === "Escape") {
                event.preventDefault();
                setFocusedJob(null);
                return;
            }

            trapTabNavigation(event, detailDialogRef.current);
        };

        document.addEventListener("keydown", onKeyDown);
        return () => {
            document.removeEventListener("keydown", onKeyDown);
            setAppShellInert(false);
            restoreFocus(detailLastFocusedRef);
        };
    }, [detailOpen]);

    const openJobDetails = useCallback(async (id: number) => {
        const seq = ++detailSeqRef.current;
        setDetailLoading(true);
        try {
            const data = await apiJson<JobDetail>(`/api/jobs/${id}/details`);
            if (seq !== detailSeqRef.current) {
                return;
            }
            setFocusedJob(data);
        } catch (error) {
            if (seq !== detailSeqRef.current) {
                return;
            }
            const message = isApiError(error) ? error.message : "Failed to fetch job details";
            showToast({ kind: "error", title: "Jobs", message });
        } finally {
            if (seq === detailSeqRef.current) {
                setDetailLoading(false);
            }
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
