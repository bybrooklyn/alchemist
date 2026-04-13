import { useState, useEffect, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { RefreshCw, Trash2, Ban } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { useDebouncedValue } from "../lib/useDebouncedValue";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { withErrorBoundary } from "./ErrorBoundary";
import type { Job, JobDetail, TabType, SortField, ConfirmConfig, CountMessageResponse } from "./jobs/types";
import { SORT_OPTIONS, isJobActive, jobDetailEmptyState } from "./jobs/types";
import { normalizeDecisionExplanation, normalizeFailureExplanation } from "./jobs/JobExplanations";
import { useJobSSE } from "./jobs/useJobSSE";
import { JobsToolbar } from "./jobs/JobsToolbar";
import { JobsTable } from "./jobs/JobsTable";
import { JobDetailModal } from "./jobs/JobDetailModal";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

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
        (element) => !element.hasAttribute("disabled")
    );
}

function getStatusBadge(status: string) {
    const styles: Record<string, string> = {
        queued: "bg-helios-slate/10 text-helios-slate border-helios-slate/20",
        analyzing: "bg-blue-500/10 text-blue-500 border-blue-500/20",
        encoding: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
        remuxing: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
        completed: "bg-green-500/10 text-green-500 border-green-500/20",
        failed: "bg-red-500/10 text-red-500 border-red-500/20",
        cancelled: "bg-red-500/10 text-red-500 border-red-500/20",
        skipped: "bg-gray-500/10 text-gray-500 border-gray-500/20",
        archived: "bg-zinc-500/10 text-zinc-400 border-zinc-500/20",
        resuming: "bg-helios-solar/10 text-helios-solar border-helios-solar/20 animate-pulse",
    };
    return (
        <span className={cn("px-2.5 py-1 rounded-md text-xs font-medium border capitalize", styles[status] || styles.queued)}>
            {status}
        </span>
    );
}

function JobManager() {
    const [jobs, setJobs] = useState<Job[]>([]);
    const [loading, setLoading] = useState(true);
    const [selected, setSelected] = useState<Set<number>>(new Set());
    const [activeTab, setActiveTab] = useState<TabType>("all");
    const [searchInput, setSearchInput] = useState("");
    const [compactSearchOpen, setCompactSearchOpen] = useState(false);
    const debouncedSearch = useDebouncedValue(searchInput, 350);
    const [page, setPage] = useState(1);
    const [sortBy, setSortBy] = useState<SortField>("updated_at");
    const [sortDesc, setSortDesc] = useState(true);
    const [refreshing, setRefreshing] = useState(false);
    const [focusedJob, setFocusedJob] = useState<JobDetail | null>(null);
    const [detailLoading, setDetailLoading] = useState(false);
    const [actionError, setActionError] = useState<string | null>(null);
    const [menuJobId, setMenuJobId] = useState<number | null>(null);
    const menuRef = useRef<HTMLDivElement | null>(null);
    const detailDialogRef = useRef<HTMLDivElement | null>(null);
    const detailLastFocusedRef = useRef<HTMLElement | null>(null);
    const compactSearchRef = useRef<HTMLDivElement | null>(null);
    const compactSearchInputRef = useRef<HTMLInputElement | null>(null);
    const confirmOpenRef = useRef(false);
    const encodeStartTimes = useRef<Map<number, number>>(new Map());
    const [confirmState, setConfirmState] = useState<ConfirmConfig | null>(null);
    const [tick, setTick] = useState(0);

    useEffect(() => {
        const id = window.setInterval(() => setTick(t => t + 1), 30_000);
        return () => window.clearInterval(id);
    }, []);

    useEffect(() => {
        if (searchInput.trim()) {
            setCompactSearchOpen(true);
        }
    }, [searchInput]);

    useEffect(() => {
        if (!compactSearchOpen) {
            return;
        }

        compactSearchInputRef.current?.focus();

        const handlePointerDown = (event: MouseEvent) => {
            if (
                compactSearchRef.current &&
                !compactSearchRef.current.contains(event.target as Node) &&
                !searchInput.trim()
            ) {
                setCompactSearchOpen(false);
            }
        };

        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === "Escape" && !searchInput.trim()) {
                setCompactSearchOpen(false);
            }
        };

        document.addEventListener("mousedown", handlePointerDown);
        document.addEventListener("keydown", handleKeyDown);
        return () => {
            document.removeEventListener("mousedown", handlePointerDown);
            document.removeEventListener("keydown", handleKeyDown);
        };
    }, [compactSearchOpen, searchInput]);

    const formatJobActionError = (error: unknown, fallback: string) => {
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
    };

    const getStatusFilter = (tab: TabType) => {
        switch (tab) {
            case "active": return ["analyzing", "encoding", "remuxing", "resuming"];
            case "queued": return ["queued"];
            case "completed": return ["completed"];
            case "failed": return ["failed", "cancelled"];
            case "skipped": return ["skipped"];
            default: return [];
        }
    };

    const fetchJobs = useCallback(async (silent = false) => {
        if (!silent) {
            setRefreshing(true);
        }
        try {
            const params = new URLSearchParams({
                limit: "50",
                page: page.toString(),
                sort: sortBy,
                sort_desc: String(sortDesc),
                archived: String(activeTab === "archived"),
            });
            params.set("sort_by", sortBy);

            const statusFilter = getStatusFilter(activeTab);
            if (statusFilter.length > 0) {
                params.set("status", statusFilter.join(","));
            }
            if (debouncedSearch) {
                params.set("search", debouncedSearch);
            }

            const data = await apiJson<Job[]>(`/api/jobs/table?${params}`);
            setJobs((prev) =>
                data.map((serverJob) => {
                    const local = prev.find((j) => j.id === serverJob.id);
                    const terminal = ["completed", "skipped", "failed", "cancelled"];
                    const serverIsTerminal = terminal.includes(serverJob.status);
                    if (
                        local &&
                        local.status === serverJob.status &&
                        terminal.includes(local.status) &&
                        serverIsTerminal
                    ) {
                        return { ...serverJob, status: local.status };
                    }
                    return serverJob;
                })
            );
            setActionError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch jobs";
            setActionError(message);
            if (!silent) {
                showToast({ kind: "error", title: "Jobs", message });
            }
        } finally {
            setLoading(false);
            if (!silent) {
                setRefreshing(false);
            }
        }
    }, [activeTab, debouncedSearch, page, sortBy, sortDesc]);

    const fetchJobsRef = useRef<() => Promise<void>>(async () => undefined);

    useEffect(() => {
        fetchJobsRef.current = async () => {
            await fetchJobs(true);
        };
    }, [fetchJobs]);

    useEffect(() => {
        void fetchJobs(false);
    }, [fetchJobs]);

    useEffect(() => {
        const pollVisible = () => {
            if (document.visibilityState === "visible") {
                void fetchJobsRef.current();
            }
        };

        const interval = window.setInterval(pollVisible, 5000);
        document.addEventListener("visibilitychange", pollVisible);
        return () => {
            window.clearInterval(interval);
            document.removeEventListener("visibilitychange", pollVisible);
        };
    }, []);

    useJobSSE({ setJobs, setFocusedJob, fetchJobsRef, encodeStartTimes });

    useEffect(() => {
        const encodingJobIds = new Set<number>();
        const now = Date.now();

        for (const job of jobs) {
            if (job.status !== "encoding") {
                continue;
            }

            encodingJobIds.add(job.id);
            if (!encodeStartTimes.current.has(job.id)) {
                encodeStartTimes.current.set(job.id, now);
            }
        }

        for (const jobId of Array.from(encodeStartTimes.current.keys())) {
            if (!encodingJobIds.has(jobId)) {
                encodeStartTimes.current.delete(jobId);
            }
        }
    }, [jobs]);

    useEffect(() => {
        if (!menuJobId) return;
        const handleClick = (event: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
                setMenuJobId(null);
            }
        };
        document.addEventListener("mousedown", handleClick);
        return () => document.removeEventListener("mousedown", handleClick);
    }, [menuJobId]);

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

    const toggleSelect = (id: number) => {
        const newSet = new Set(selected);
        if (newSet.has(id)) newSet.delete(id);
        else newSet.add(id);
        setSelected(newSet);
    };

    const toggleSelectAll = () => {
        if (selected.size === jobs.length && jobs.length > 0) {
            setSelected(new Set());
        } else {
            setSelected(new Set(jobs.map(j => j.id)));
        }
    };

    const selectedJobs = jobs.filter((job) => selected.has(job.id));
    const hasSelectedActiveJobs = selectedJobs.some(isJobActive);
    const activeCount = jobs.filter((job) => isJobActive(job)).length;
    const failedCount = jobs.filter((job) => ["failed", "cancelled"].includes(job.status)).length;
    const completedCount = jobs.filter((job) => job.status === "completed").length;

    const handleBatch = async (action: "cancel" | "restart" | "delete") => {
        if (selected.size === 0) return;
        setActionError(null);

        try {
            await apiAction("/api/jobs/batch", {
                method: "POST",
                body: JSON.stringify({
                    action,
                    ids: Array.from(selected)
                })
            });
            setSelected(new Set());
            showToast({
                kind: "success",
                title: "Jobs",
                message: `${action[0].toUpperCase()}${action.slice(1)} request sent for selected jobs.`,
            });
            await fetchJobs();
        } catch (e) {
            const message = formatJobActionError(e, "Batch action failed");
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const clearCompleted = async () => {
        setActionError(null);
        try {
            const result = await apiJson<CountMessageResponse>("/api/jobs/clear-completed", {
                method: "POST",
            });
            showToast({ kind: "success", title: "Jobs", message: result.message });
            if (activeTab === "completed" && result.count > 0) {
                showToast({
                    kind: "info",
                    title: "Jobs",
                    message: "Completed jobs archived. View them in the Archived tab.",
                });
            }
            await fetchJobs();
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to clear completed jobs";
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const fetchJobDetails = async (id: number) => {
        setActionError(null);
        setDetailLoading(true);
        try {
            const data = await apiJson<JobDetail>(`/api/jobs/${id}/details`);
            setFocusedJob(data);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to fetch job details";
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        } finally {
            setDetailLoading(false);
        }
    };

    const handleAction = async (id: number, action: "cancel" | "restart" | "delete") => {
        setActionError(null);
        try {
            await apiAction(`/api/jobs/${id}/${action}`, { method: "POST" });
            if (action === "delete") {
                setFocusedJob((current) => (current?.job.id === id ? null : current));
            } else if (focusedJob?.job.id === id) {
                await fetchJobDetails(id);
            }
            await fetchJobs();
            showToast({
                kind: "success",
                title: "Jobs",
                message: `Job ${action} request completed.`,
            });
        } catch (e) {
            const message = formatJobActionError(e, `Job ${action} failed`);
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const handlePriority = async (job: Job, priority: number, label: string) => {
        setActionError(null);
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
            await fetchJobs();
            showToast({ kind: "success", title: "Jobs", message: `${label} for job #${job.id}.` });
        } catch (e) {
            const message = formatJobActionError(e, "Failed to update priority");
            setActionError(message);
            showToast({ kind: "error", title: "Jobs", message });
        }
    };

    const openConfirm = (config: ConfirmConfig) => {
        setConfirmState(config);
    };

    const focusedDecision = focusedJob
        ? normalizeDecisionExplanation(
            focusedJob.decision_explanation ?? focusedJob.job.decision_explanation,
            focusedJob.job.decision_reason,
        )
        : null;
    const focusedFailure = focusedJob
        ? normalizeFailureExplanation(
            focusedJob.failure_explanation,
            focusedJob.job_failure_summary,
            focusedJob.job_logs,
        )
        : null;
    const focusedJobLogs = focusedJob?.job_logs ?? [];
    const shouldShowFfmpegOutput = focusedJob
        ? ["failed", "completed", "skipped"].includes(focusedJob.job.status) && focusedJobLogs.length > 0
        : false;
    const completedEncodeStats = focusedJob?.job.status === "completed"
        ? focusedJob.encode_stats
        : null;
    const focusedEmptyState = focusedJob
        ? jobDetailEmptyState(focusedJob.job.status)
        : null;

    return (
        <div className="space-y-6 relative">
            <div className="flex items-center gap-4 px-1 text-xs text-helios-slate">
                <span>
                    <span className="font-medium text-helios-ink">{activeCount}</span>
                    {" "}active
                </span>
                <span>
                    <span className="font-medium text-red-500">{failedCount}</span>
                    {" "}failed
                </span>
                <span>
                    <span className="font-medium text-emerald-500">{completedCount}</span>
                    {" "}completed
                </span>
            </div>

            <JobsToolbar
                activeTab={activeTab}
                setActiveTab={setActiveTab}
                setPage={setPage}
                searchInput={searchInput}
                setSearchInput={setSearchInput}
                compactSearchOpen={compactSearchOpen}
                setCompactSearchOpen={setCompactSearchOpen}
                compactSearchRef={compactSearchRef}
                compactSearchInputRef={compactSearchInputRef}
                sortBy={sortBy}
                setSortBy={setSortBy}
                sortDesc={sortDesc}
                setSortDesc={setSortDesc}
                refreshing={refreshing}
                fetchJobs={fetchJobs}
            />

            {actionError && (
                <div role="alert" aria-live="polite" className="rounded-lg border border-status-error/30 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {actionError}
                </div>
            )}

            {/* Batch Actions Bar */}
            {selected.size > 0 && (
                <div className="flex items-center justify-between bg-helios-solar/10 border border-helios-solar/20 px-6 py-3 rounded-lg animate-in fade-in slide-in-from-top-2">
                    <div>
                        <span className="text-sm font-bold text-helios-solar">
                            {selected.size} jobs selected
                        </span>
                        {hasSelectedActiveJobs && (
                            <p className="text-xs text-helios-slate mt-1">
                                Active jobs must be cancelled before they can be restarted or deleted.
                            </p>
                        )}
                    </div>
                    <div className="flex gap-2">
                        <button
                            onClick={() =>
                                openConfirm({
                                    title: "Restart jobs",
                                    body: `Restart ${selected.size} selected jobs?`,
                                    confirmLabel: "Restart",
                                    onConfirm: () => handleBatch("restart"),
                                })
                            }
                            disabled={hasSelectedActiveJobs}
                            className="p-2 hover:bg-helios-solar/20 rounded-lg text-helios-solar disabled:opacity-40 disabled:hover:bg-transparent"
                            title="Restart"
                        >
                            <RefreshCw size={18} />
                        </button>
                        <button
                            onClick={() =>
                                openConfirm({
                                    title: "Cancel jobs",
                                    body: `Cancel ${selected.size} selected jobs?`,
                                    confirmLabel: "Cancel",
                                    confirmTone: "danger",
                                    onConfirm: () => handleBatch("cancel"),
                                })
                            }
                            className="p-2 hover:bg-helios-solar/20 rounded-lg text-helios-solar"
                            title="Cancel"
                        >
                            <Ban size={18} />
                        </button>
                        <button
                            onClick={() =>
                                openConfirm({
                                    title: "Delete jobs",
                                    body: `Delete ${selected.size} selected jobs from history?`,
                                    confirmLabel: "Delete",
                                    confirmTone: "danger",
                                    onConfirm: () => handleBatch("delete"),
                                })
                            }
                            disabled={hasSelectedActiveJobs}
                            className="p-2 hover:bg-red-500/10 rounded-lg text-red-500 disabled:opacity-40 disabled:hover:bg-transparent"
                            title="Delete"
                        >
                            <Trash2 size={18} />
                        </button>
                    </div>
                </div>
            )}

            <JobsTable
                jobs={jobs}
                loading={loading}
                selected={selected}
                focusedJobId={focusedJob?.job.id ?? null}
                tick={tick}
                encodeStartTimes={encodeStartTimes}
                menuJobId={menuJobId}
                menuRef={menuRef}
                toggleSelect={toggleSelect}
                toggleSelectAll={toggleSelectAll}
                fetchJobDetails={fetchJobDetails}
                setMenuJobId={setMenuJobId}
                openConfirm={openConfirm}
                handleAction={handleAction}
                handlePriority={handlePriority}
                getStatusBadge={getStatusBadge}
            />

            {/* Footer Actions */}
            <div className="flex justify-between items-center pt-2">
                <p className="text-xs text-helios-slate font-medium">Showing {jobs.length} jobs (Limit 50)</p>
                <button
                    onClick={() =>
                        openConfirm({
                            title: "Clear completed jobs",
                            body: "Remove all completed jobs from history?",
                            confirmLabel: "Clear",
                            confirmTone: "danger",
                            onConfirm: () => clearCompleted(),
                        })
                    }
                    className="text-xs text-red-500 hover:text-red-400 font-bold flex items-center gap-1 transition-colors"
                >
                    <Trash2 size={12} /> Clear Completed
                </button>
            </div>

            {/* Detail Overlay */}
            {typeof document !== "undefined" && createPortal(
                <JobDetailModal
                    focusedJob={focusedJob}
                    detailDialogRef={detailDialogRef}
                    detailLoading={detailLoading}
                    onClose={() => setFocusedJob(null)}
                    focusedDecision={focusedDecision}
                    focusedFailure={focusedFailure}
                    focusedJobLogs={focusedJobLogs}
                    shouldShowFfmpegOutput={shouldShowFfmpegOutput}
                    completedEncodeStats={completedEncodeStats}
                    focusedEmptyState={focusedEmptyState}
                    openConfirm={openConfirm}
                    handleAction={handleAction}
                    handlePriority={handlePriority}
                    getStatusBadge={getStatusBadge}
                />,
                document.body
            )}

            <ConfirmDialog
                open={confirmState !== null}
                title={confirmState?.title ?? ""}
                description={confirmState?.body ?? ""}
                confirmLabel={confirmState?.confirmLabel ?? "Confirm"}
                tone={confirmState?.confirmTone ?? "primary"}
                onClose={() => setConfirmState(null)}
                onConfirm={async () => {
                    if (!confirmState) {
                        return;
                    }
                    await confirmState.onConfirm();
                }}
            />
        </div>
    );
}

export default withErrorBoundary(JobManager, "Job Management");
