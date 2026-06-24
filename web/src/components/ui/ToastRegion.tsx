import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AlertCircle, AlertTriangle, CheckCircle2, Info, X, type LucideIcon } from "lucide-react";
import { subscribeToToasts, type ToastKind, type ToastMessage } from "../../lib/toast";

const DEFAULT_DURATION_MS = 3500;
const MAX_TOASTS = 4;

function kindStyles(kind: ToastKind): { icon: LucideIcon; className: string } {
    if (kind === "success") {
        return {
            icon: CheckCircle2,
            className: "border-status-success/35 bg-helios-surface/95 text-status-success supports-[backdrop-filter]:bg-helios-surface/80 backdrop-blur-xl",
        };
    }
    if (kind === "error") {
        return {
            icon: AlertCircle,
            className: "border-status-error/35 bg-helios-surface/95 text-status-error supports-[backdrop-filter]:bg-helios-surface/80 backdrop-blur-xl",
        };
    }
    if (kind === "warning") {
        return {
            icon: AlertTriangle,
            className: "border-amber-500/35 bg-helios-surface/95 text-amber-500 supports-[backdrop-filter]:bg-helios-surface/80 backdrop-blur-xl",
        };
    }
    return {
        icon: Info,
        className: "border-helios-line/40 bg-helios-surface/95 text-helios-ink supports-[backdrop-filter]:bg-helios-surface/80 backdrop-blur-xl",
    };
}

export default function ToastRegion() {
    const [toasts, setToasts] = useState<ToastMessage[]>([]);
    const [hoveredId, setHoveredId] = useState<string | null>(null);
    const timerRefs = useRef<Map<string, number>>(new Map());
    const remainingRefs = useRef<Map<string, number>>(new Map());
    const startRefs = useRef<Map<string, number>>(new Map());

    const clearAllTimers = useCallback(() => {
        for (const timer of timerRefs.current.values()) {
            window.clearTimeout(timer);
        }
        timerRefs.current.clear();
    }, []);

    const dismissToast = useCallback((id: string) => {
        setToasts((prev) => prev.filter((item) => item.id !== id));
        const timer = timerRefs.current.get(id);
        if (timer !== undefined) {
            window.clearTimeout(timer);
            timerRefs.current.delete(id);
        }
        remainingRefs.current.delete(id);
        startRefs.current.delete(id);
    }, []);

    const startTimer = useCallback((id: string, durationMs: number) => {
        const existing = timerRefs.current.get(id);
        if (existing !== undefined) {
            window.clearTimeout(existing);
        }
        startRefs.current.set(id, Date.now());
        remainingRefs.current.set(id, durationMs);
        const timer = window.setTimeout(() => dismissToast(id), durationMs);
        timerRefs.current.set(id, timer);
    }, [dismissToast]);

    useEffect(() => {
        return subscribeToToasts((message) => {
            setToasts((prev) => {
                const next = [message, ...prev];
                return next.slice(0, MAX_TOASTS);
            });
        });
    }, []);

    // Manage auto-dismiss timers; pause on hover/focus. Running timers are left
    // untouched across re-renders so countdowns are honest (no reset when a new
    // toast arrives or another is dismissed).
    useEffect(() => {
        const activeIds = new Set(toasts.map((t) => t.id));

        // Drop timers/state for toasts that no longer exist.
        for (const [id, timer] of timerRefs.current) {
            if (!activeIds.has(id)) {
                window.clearTimeout(timer);
                timerRefs.current.delete(id);
                remainingRefs.current.delete(id);
                startRefs.current.delete(id);
            }
        }

        for (const toast of toasts) {
            if (toast.id === hoveredId) {
                // Pause: only when a timer is actually running, record remaining.
                const timer = timerRefs.current.get(toast.id);
                if (timer !== undefined) {
                    const elapsed = Date.now() - (startRefs.current.get(toast.id) ?? Date.now());
                    const remaining = (remainingRefs.current.get(toast.id) ?? (toast.durationMs ?? DEFAULT_DURATION_MS)) - elapsed;
                    remainingRefs.current.set(toast.id, Math.max(remaining, 500));
                    window.clearTimeout(timer);
                    timerRefs.current.delete(toast.id);
                }
            } else if (!timerRefs.current.has(toast.id)) {
                // Start (new toast) or resume (after unhover) with remaining time.
                const remaining = remainingRefs.current.get(toast.id);
                startTimer(toast.id, remaining ?? (toast.durationMs ?? DEFAULT_DURATION_MS));
            }
        }
    }, [toasts, hoveredId, startTimer]);

    // Clear every pending timer on unmount.
    useEffect(() => clearAllTimers, [clearAllTimers]);

    const liveMessage = useMemo(() => {
        if (toasts.length === 0) {
            return "";
        }
        const top = toasts[0];
        return top.title ? `${top.title}: ${top.message}` : top.message;
    }, [toasts]);

    if (toasts.length === 0) {
        return <div className="sr-only" aria-live="polite" aria-atomic="true">{liveMessage}</div>;
    }

    return (
        <>
            <div className="sr-only" aria-live="polite" aria-atomic="true">
                {liveMessage}
            </div>
            <div className="fixed top-4 right-4 z-[300] flex w-[min(92vw,360px)] flex-col gap-2 pointer-events-none">
                {toasts.map((toast) => {
                    const { icon: Icon, className } = kindStyles(toast.kind);
                    return (
                        <div
                            key={toast.id}
                            role={toast.kind === "error" ? "alert" : "status"}
                            className={`pointer-events-auto rounded-lg border p-3 shadow-xl shadow-black/30 ${className}`}
                            onMouseEnter={() => setHoveredId(toast.id)}
                            onMouseLeave={() => setHoveredId((prev) => (prev === toast.id ? null : prev))}
                            onFocus={() => setHoveredId(toast.id)}
                            onBlur={() => setHoveredId((prev) => (prev === toast.id ? null : prev))}
                        >
                            <div className="flex items-start gap-2">
                                <Icon size={16} />
                                <div className="min-w-0 flex-1">
                                    {toast.title && (
                                        <p className="text-xs font-medium">{toast.title}</p>
                                    )}
                                    <p className="text-sm break-words">{toast.message}</p>
                                </div>
                                <button
                                    type="button"
                                    className="rounded p-1 hover:bg-black/10"
                                    aria-label="Dismiss notification"
                                    onClick={() => dismissToast(toast.id)}
                                >
                                    <X size={14} />
                                </button>
                            </div>
                        </div>
                    );
                })}
            </div>
        </>
    );
}
