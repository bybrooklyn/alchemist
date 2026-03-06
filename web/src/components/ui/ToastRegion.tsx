import { useEffect, useMemo, useState } from "react";
import { AlertCircle, CheckCircle2, Info, X, type LucideIcon } from "lucide-react";
import { subscribeToToasts, type ToastKind, type ToastMessage } from "../../lib/toast";

const DEFAULT_DURATION_MS = 3500;
const MAX_TOASTS = 4;

function kindStyles(kind: ToastKind): { icon: LucideIcon; className: string } {
    if (kind === "success") {
        return {
            icon: CheckCircle2,
            className: "border-status-success/30 bg-status-success/10 text-status-success",
        };
    }
    if (kind === "error") {
        return {
            icon: AlertCircle,
            className: "border-status-error/30 bg-status-error/10 text-status-error",
        };
    }
    return {
        icon: Info,
        className: "border-helios-line/40 bg-helios-surface text-helios-ink",
    };
}

export default function ToastRegion() {
    const [toasts, setToasts] = useState<ToastMessage[]>([]);

    useEffect(() => {
        return subscribeToToasts((message) => {
            setToasts((prev) => {
                const next = [message, ...prev];
                return next.slice(0, MAX_TOASTS);
            });
        });
    }, []);

    useEffect(() => {
        if (toasts.length === 0) {
            return;
        }

        const timers = toasts.map((toast) =>
            window.setTimeout(() => {
                setToasts((prev) => prev.filter((item) => item.id !== toast.id));
            }, toast.durationMs ?? DEFAULT_DURATION_MS)
        );

        return () => {
            for (const timer of timers) {
                window.clearTimeout(timer);
            }
        };
    }, [toasts]);

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
                            className={`pointer-events-auto rounded-xl border p-3 shadow-xl ${className}`}
                        >
                            <div className="flex items-start gap-2">
                                <Icon size={16} />
                                <div className="min-w-0 flex-1">
                                    {toast.title && (
                                        <p className="text-xs font-bold uppercase tracking-wide">{toast.title}</p>
                                    )}
                                    <p className="text-sm break-words">{toast.message}</p>
                                </div>
                                <button
                                    type="button"
                                    className="rounded p-1 hover:bg-black/10"
                                    aria-label="Dismiss notification"
                                    onClick={() =>
                                        setToasts((prev) => prev.filter((item) => item.id !== toast.id))
                                    }
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
