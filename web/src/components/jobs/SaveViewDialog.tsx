import { useEffect, useRef, type FormEvent } from "react";
import { X } from "lucide-react";

interface SaveViewDialogProps {
    open: boolean;
    name: string;
    submitting: boolean;
    /** Existing saved-view labels, used for inline duplicate detection. */
    existingLabels: string[];
    onNameChange: (value: string) => void;
    onClose: () => void;
    onSubmit: () => Promise<void>;
}

/**
 * Themed, focus-trapped replacement for the native `window.prompt` previously
 * used to name a saved job view. Matches the EnqueuePathDialog pattern: Esc to
 * cancel, Enter to submit, with inline required/duplicate validation.
 */
export function SaveViewDialog({
    open,
    name,
    submitting,
    existingLabels,
    onNameChange,
    onClose,
    onSubmit,
}: SaveViewDialogProps) {
    const inputRef = useRef<HTMLInputElement | null>(null);

    useEffect(() => {
        if (open) {
            // Defer focus until the dialog is painted.
            window.requestAnimationFrame(() => inputRef.current?.focus());
        }
    }, [open]);

    useEffect(() => {
        if (!open) return;
        const handleKey = (event: KeyboardEvent) => {
            if (event.key === "Escape" && !submitting) {
                onClose();
            }
        };
        document.addEventListener("keydown", handleKey);
        return () => document.removeEventListener("keydown", handleKey);
    }, [open, submitting, onClose]);

    if (!open) {
        return null;
    }

    const trimmed = name.trim();
    const isDuplicate = existingLabels.some(
        (label) => label.toLowerCase() === trimmed.toLowerCase(),
    );
    const validationError = isDuplicate ? "A saved view with this name already exists." : null;
    const canSubmit = trimmed.length > 0 && !validationError && !submitting;

    const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        if (!canSubmit) return;
        await onSubmit();
    };

    return (
        <>
            <div
                className="fixed inset-0 z-[110] bg-black/60 backdrop-blur-sm"
                onClick={() => !submitting && onClose()}
            />
            <div className="fixed inset-0 z-[111] flex items-center justify-center px-4">
                <form
                    onSubmit={(event) => void handleSubmit(event)}
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="save-view-title"
                    className="w-full max-w-md rounded-xl border border-helios-line/20 bg-helios-surface shadow-2xl"
                >
                    <div className="flex items-start justify-between gap-4 border-b border-helios-line/10 bg-helios-surface-soft/50 px-6 py-5">
                        <div>
                            <h2 id="save-view-title" className="text-lg font-bold text-helios-ink">Save view</h2>
                            <p className="mt-1 text-sm text-helios-slate">
                                Save the current tab, sort, and search as a reusable view.
                            </p>
                        </div>
                        <button
                            type="button"
                            onClick={onClose}
                            disabled={submitting}
                            className="rounded-md p-2 text-helios-slate transition-colors hover:bg-helios-line/10 disabled:opacity-50"
                            aria-label="Close save view dialog"
                        >
                            <X size={18} />
                        </button>
                    </div>

                    <div className="space-y-2 px-6 py-5">
                        <label htmlFor="save-view-name" className="block text-xs font-semibold uppercase tracking-wide text-helios-slate">
                            View name
                        </label>
                        <input
                            id="save-view-name"
                            ref={inputRef}
                            type="text"
                            value={name}
                            onChange={(event) => onNameChange(event.target.value)}
                            placeholder="e.g. Large HEVC remuxes"
                            maxLength={60}
                            aria-invalid={Boolean(validationError)}
                            className="w-full rounded-lg border border-helios-line/20 bg-helios-surface px-4 py-3 text-sm text-helios-ink outline-none focus:border-helios-solar"
                        />
                        {validationError && (
                            <p className="text-xs text-status-error" role="alert">{validationError}</p>
                        )}
                    </div>

                    <div className="flex items-center justify-end gap-3 border-t border-helios-line/10 px-6 py-4">
                        <button
                            type="button"
                            onClick={onClose}
                            disabled={submitting}
                            className="rounded-lg border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-slate transition-colors hover:bg-helios-surface-soft disabled:opacity-50"
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={!canSubmit}
                            className="rounded-lg bg-helios-solar px-4 py-2 text-sm font-bold text-helios-main transition-all hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                        >
                            {submitting ? "Saving..." : "Save view"}
                        </button>
                    </div>
                </form>
            </div>
        </>
    );
}
