import { useEffect, useState } from "react";
import Modal from "./Modal";
import { isApiError } from "../../lib/api";
import { showToast } from "../../lib/toast";

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

    useEffect(() => {
        if (!open) {
            setSubmitting(false);
        }
    }, [open]);

    return (
        <Modal
            open={open}
            onClose={onClose}
            labelledBy="confirm-dialog-title"
            describedBy="confirm-dialog-description"
            maxWidth="max-w-sm"
            panelClassName="p-6"
            disableClose={submitting}
            zIndexBase={200}
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
                        } catch (err) {
                            showToast({
                                kind: "error",
                                title: title,
                                message: isApiError(err) ? err.message : "Action failed",
                            });
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
        </Modal>
    );
}
