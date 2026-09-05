// Shared byte/duration/relative-time formatting. Consolidates what used to be
// five separately drifting copies (different decimal precision, unguarded
// negative/NaN input, and duration formats confusingly sharing one name)
// across Dashboard, WatchFolders, StatsCharts, ConversionTool, TimeDisplay,
// and jobs/types.ts.

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

// Negative input is preserved (with a leading "-") rather than clamped to
// zero: totals like "bytes saved" can legitimately go negative when a
// transcode makes a file bigger, and that should stay visible, not vanish.
export function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes === 0) return "0 B";
    const k = 1024;
    const sign = bytes < 0 ? "-" : "";
    const abs = Math.abs(bytes);
    // Clamped at both ends: a sub-byte value (0 < |bytes| < 1) makes the log
    // negative, and a negative index reads past the start of BYTE_UNITS —
    // rendering "0.5 undefined".
    const i = Math.min(
        Math.max(Math.floor(Math.log(abs) / Math.log(k)), 0),
        BYTE_UNITS.length - 1,
    );
    return `${sign}${parseFloat((abs / Math.pow(k, i)).toFixed(2))} ${BYTE_UNITS[i]}`;
}

/** HH:MM:SS clock format, e.g. a completed encode's elapsed time. */
export function formatDurationClock(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) return "00:00:00";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    return [h, m, s].map((v) => v.toString().padStart(2, "0")).join(":");
}

/**
 * "1h 30m" / "3m 45s" / "45s" format that keeps second-level granularity for
 * short values, e.g. a source media file's duration. "--" for invalid input.
 */
export function formatDurationPrecise(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds <= 0) return "--";
    const rounded = Math.round(seconds);
    const hours = Math.floor(rounded / 3600);
    const minutes = Math.floor((rounded % 3600) / 60);
    const secs = rounded % 60;
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m ${secs}s`;
    return `${secs}s`;
}

/** Humanized "3h 20m" / "2d 4h" format, e.g. a queue ETA. */
export function formatDurationHuman(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) return "0m";
    const totalMinutes = Math.max(1, Math.round(seconds / 60));
    if (totalMinutes < 60) return `${totalMinutes}m`;

    const hours = Math.floor(totalMinutes / 60);
    const minutes = totalMinutes % 60;
    if (hours < 24) return `${hours}h ${minutes}m`;

    const days = Math.floor(hours / 24);
    const remainingHours = hours % 24;
    return `${days}d ${remainingHours}h`;
}

/** Relative time ("3h ago", "2mo ago") from a Date, ISO string, or nullish input. */
export function formatRelativeTime(input: Date | string | null | undefined): string {
    if (!input) return "Just now";
    const date = input instanceof Date ? input : new Date(input);
    if (Number.isNaN(date.getTime())) return "Just now";

    const diffMs = Math.max(0, Date.now() - date.getTime());
    const minutes = Math.floor(diffMs / 60_000);
    if (minutes < 1) return "Just now";
    if (minutes < 60) return `${minutes}m ago`;

    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;

    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d ago`;

    const months = Math.floor(days / 30);
    if (months < 12) return `${months}mo ago`;

    const years = Math.floor(days / 365);
    return `${years}y ago`;
}
