import { useEffect, useRef, useState } from "react";

interface ConfirmDialogProps {
    open: boolean;
    title: string;
    description: string;
    confirmLabel?: string;
    cancelLabel?: string;
    tone?: "primary" | "danger";
    onConfirm: () => Promise<void> | void;
    onClose: () => void;
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

export default function ConfirmDialog({
    open,
    title,
    description,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    tone = "primary",
    onConfirm,
    onClose,
}: ConfirmDialogProps) {
    const [submitting, setSubmitting] = useState(false);
    const panelRef = useRef<HTMLDivElement | null>(null);
    const lastFocusedRef = useRef<HTMLElement | null>(null);

    useEffect(() => {
        if (!open) {
            return;
        }

        lastFocusedRef.current = document.activeElement as HTMLElement | null;

        const panel = panelRef.current;
        if (panel) {
            const focusables = focusableElements(panel);
            if (focusables.length > 0) {
                focusables[0].focus();
            } else {
                panel.focus();
            }
        }

        const onKeyDown = (event: KeyboardEvent) => {
            if (!open) {
                return;
            }

            if (event.key === "Escape") {
                event.preventDefault();
                if (!submitting) {
                    onClose();
                }
                return;
            }

            if (event.key !== "Tab") {
                return;
            }

            const root = panelRef.current;
            if (!root) {
                return;
            }

            const focusables = focusableElements(root);
            if (focusables.length === 0) {
                event.preventDefault();
                root.focus();
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
            if (lastFocusedRef.current) {
                lastFocusedRef.current.focus();
            }
        };
    }, [open, onClose, submitting]);

    useEffect(() => {
        if (!open) {
            setSubmitting(false);
        }
    }, [open]);

    if (!open) {
        return null;
    }

    return (
        <div className="fixed inset-0 z-[200]">
            <button
                type="button"
                aria-label="Close dialog"
                onClick={() => !submitting && onClose()}
                className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            />
            <div className="absolute inset-0 flex items-center justify-center px-4">
                <div
                    ref={panelRef}
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="confirm-dialog-title"
                    aria-describedby="confirm-dialog-description"
                    tabIndex={-1}
                    className="w-full max-w-sm rounded-2xl border border-helios-line/30 bg-helios-surface p-6 shadow-2xl outline-none"
                >
                    <h3 id="confirm-dialog-title" className="text-lg font-bold text-helios-ink">
                        {title}
                    </h3>
                    <p id="confirm-dialog-description" className="mt-2 text-sm text-helios-slate">
                        {description}
                    </p>
                    <div className="mt-6 flex justify-end gap-2">
                        <button
                            type="button"
                            onClick={onClose}
                            disabled={submitting}
                            className="rounded-lg px-4 py-2 text-sm font-semibold text-helios-slate hover:bg-helios-surface-soft"
                        >
                            {cancelLabel}
                        </button>
                        <button
                            type="button"
                            disabled={submitting}
                            onClick={async () => {
                                setSubmitting(true);
                                try {
                                    await onConfirm();
                                    onClose();
                                } finally {
                                    setSubmitting(false);
                                }
                            }}
                            className={
                                tone === "danger"
                                    ? "rounded-lg bg-status-error/20 px-4 py-2 text-sm font-semibold text-status-error hover:bg-status-error/30"
                                    : "rounded-lg bg-helios-solar px-4 py-2 text-sm font-semibold text-helios-main hover:brightness-110"
                            }
                        >
                            {submitting ? "Working..." : confirmLabel}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
