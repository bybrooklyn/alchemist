import type { FormEvent } from "react";
import { X } from "lucide-react";

interface EnqueuePathDialogProps {
    open: boolean;
    path: string;
    submitting: boolean;
    onPathChange: (value: string) => void;
    onClose: () => void;
    onSubmit: () => Promise<void>;
}

export function EnqueuePathDialog({
    open,
    path,
    submitting,
    onPathChange,
    onClose,
    onSubmit,
}: EnqueuePathDialogProps) {
    if (!open) {
        return null;
    }

    const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        await onSubmit();
    };

    return (
        <>
            <div
                className="fixed inset-0 z-[110] bg-black/60 backdrop-blur-sm"
                onClick={onClose}
            />
            <div className="fixed inset-0 z-[111] flex items-center justify-center px-4">
                <form
                    onSubmit={(event) => void handleSubmit(event)}
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="enqueue-path-title"
                    className="w-full max-w-xl rounded-xl border border-helios-line/20 bg-helios-surface shadow-2xl"
                >
                    <div className="flex items-start justify-between gap-4 border-b border-helios-line/10 bg-helios-surface-soft/50 px-6 py-5">
                        <div>
                            <h2 id="enqueue-path-title" className="text-lg font-bold text-helios-ink">Add File</h2>
                            <p className="mt-1 text-sm text-helios-slate">
                                Enqueue one absolute filesystem path without running a full scan.
                            </p>
                        </div>
                        <button
                            type="button"
                            onClick={onClose}
                            className="rounded-md p-2 text-helios-slate transition-colors hover:bg-helios-line/10"
                            aria-label="Close add file dialog"
                        >
                            <X size={18} />
                        </button>
                    </div>

                    <div className="space-y-3 px-6 py-5">
                        <label className="block text-xs font-semibold uppercase tracking-wide text-helios-slate">
                            Absolute Path
                        </label>
                        <input
                            type="text"
                            value={path}
                            onChange={(event) => onPathChange(event.target.value)}
                            placeholder="/Volumes/Media/Movies/example.mkv"
                            className="w-full rounded-lg border border-helios-line/20 bg-helios-surface px-4 py-3 text-sm text-helios-ink outline-none focus:border-helios-solar"
                            autoFocus
                        />
                        <p className="text-xs text-helios-slate">
                            Supported media files only. Paths are resolved on the server before enqueue.
                        </p>
                    </div>

                    <div className="flex items-center justify-end gap-3 border-t border-helios-line/10 px-6 py-4">
                        <button
                            type="button"
                            onClick={onClose}
                            className="rounded-lg border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-slate transition-colors hover:bg-helios-surface-soft"
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={submitting}
                            className="rounded-lg bg-helios-solar px-4 py-2 text-sm font-bold text-helios-main transition-all hover:brightness-110 disabled:opacity-60"
                        >
                            {submitting ? "Adding..." : "Add File"}
                        </button>
                    </div>
                </form>
            </div>
        </>
    );
}
