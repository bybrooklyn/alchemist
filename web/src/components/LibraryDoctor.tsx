import { useEffect, useState } from "react";
import { Activity } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface HealthSummary {
    total_checked: number;
    issues_found: number;
    last_run: string | null;
}

function formatRelativeTime(value: string | null): string {
    if (!value) {
        return "Never scanned";
    }

    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) {
        return "Never scanned";
    }

    const diffMs = Date.now() - parsed.getTime();
    const minutes = Math.floor(diffMs / 60_000);
    if (minutes < 1) {
        return "just now";
    }
    if (minutes < 60) {
        return `${minutes}m ago`;
    }
    const hours = Math.floor(minutes / 60);
    if (hours < 24) {
        return `${hours}h ago`;
    }
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}

export default function LibraryDoctor() {
    const [summary, setSummary] = useState<HealthSummary | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [scanning, setScanning] = useState(false);

    const fetchSummary = async (silent = false) => {
        try {
            const data = await apiJson<HealthSummary>("/api/library/health");
            setSummary(data);
            setError(null);
            return data;
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to load library health summary.";
            setError(message);
            if (!silent) {
                showToast({ kind: "error", title: "Library Doctor", message });
            }
            return null;
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        void fetchSummary();
    }, []);

    const startScan = async () => {
        if (scanning) {
            return;
        }

        setScanning(true);
        const baseline = summary;

        try {
            await apiAction("/api/library/health/scan", { method: "POST" });
            showToast({
                kind: "success",
                title: "Library Doctor",
                message:
                    "Library scan started — this may take a while depending on your library size.",
            });

            const deadline = Date.now() + 10 * 60 * 1000;
            let lastIssues = baseline?.issues_found ?? -1;
            let stableReads = 0;
            let observedNewRun = false;

            while (Date.now() < deadline) {
                await new Promise((resolve) => window.setTimeout(resolve, 5000));
                const next = await fetchSummary(true);
                if (!next) {
                    continue;
                }

                if (next.last_run && next.last_run !== baseline?.last_run) {
                    observedNewRun = true;
                }

                if (!observedNewRun) {
                    continue;
                }

                if (next.issues_found === lastIssues) {
                    stableReads += 1;
                } else {
                    stableReads = 0;
                    lastIssues = next.issues_found;
                }

                if (stableReads >= 1) {
                    break;
                }
            }
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to start library scan.";
            setError(message);
            showToast({ kind: "error", title: "Library Doctor", message });
        } finally {
            await fetchSummary(true);
            setScanning(false);
        }
    };

    if (loading) {
        return (
            <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
                <div className="h-5 w-36 animate-pulse rounded bg-helios-surface-soft/60" />
                <div className="mt-4 h-4 w-48 animate-pulse rounded bg-helios-surface-soft/60" />
                <div className="mt-3 h-4 w-32 animate-pulse rounded bg-helios-surface-soft/60" />
                <div className="mt-6 h-10 w-32 animate-pulse rounded bg-helios-surface-soft/60" />
            </div>
        );
    }

    return (
        <div className="rounded-lg border border-helios-line/40 bg-helios-surface p-6">
            <div className="flex items-center gap-3">
                <div className="rounded-lg bg-helios-solar/10 p-2 text-helios-solar">
                    <Activity size={18} />
                </div>
                <div>
                    <h3 className="font-semibold text-helios-ink">Library Doctor</h3>
                    <p className="text-sm text-helios-slate">
                        {summary
                            ? `${summary.total_checked} files checked · ${summary.issues_found} issues found`
                            : "No scan data yet"}
                    </p>
                    <p className="text-xs text-helios-slate mt-1">
                        {summary?.last_run
                            ? `Last scan: ${formatRelativeTime(summary.last_run)}`
                            : "Never scanned"}
                    </p>
                </div>
            </div>

            {error ? (
                <div className="mt-4 rounded-lg border border-status-error/30 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {error}
                </div>
            ) : null}

            <div className="mt-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <button
                    type="button"
                    onClick={() => void startScan()}
                    disabled={scanning}
                    className="rounded-md bg-helios-solar px-4 py-2 text-sm font-semibold text-helios-main disabled:opacity-60"
                >
                    {scanning ? "Scanning..." : "Scan Library"}
                </button>

                {summary && summary.issues_found > 0 ? (
                    <a
                        href="/jobs?tab=issues"
                        className="text-sm text-helios-solar hover:underline"
                    >
                        View Issues
                    </a>
                ) : null}
            </div>

            {summary && summary.issues_found === 0 && summary.last_run ? (
                <div className="mt-4 text-sm text-status-success">
                    ✓ No issues found in your last scan
                </div>
            ) : null}
        </div>
    );
}
