import { useEffect, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowRight } from "lucide-react";
import clsx from "clsx";
import { SETUP_STEP_COUNT } from "./constants";
import { showToast } from "../../lib/toast";

interface SetupFrameProps {
    step: number;
    configMutable: boolean;
    error: string | null;
    submitting: boolean;
    onBack: () => void;
    onNext: () => void;
    children: ReactNode;
}

export default function SetupFrame({ step, configMutable, error, submitting, onBack, onNext, children }: SetupFrameProps) {
    useEffect(() => {
        if (error) {
            showToast({ kind: "error", title: "Setup", message: error });
        }
    }, [error]);

    return (
        <div className="flex flex-col flex-1 min-h-0">

            {/* Progress bar — 2px solar line at top of app-main */}
            <div className="h-0.5 w-full bg-helios-surface-soft shrink-0">
                <motion.div
                    className="bg-helios-solar h-full"
                    initial={{ width: 0 }}
                    animate={{
                        width: `${(step / SETUP_STEP_COUNT) * 100}%`
                    }}
                    transition={{ duration: 0.3 }}
                />
            </div>

            {/* Read-only config warning */}
            {!configMutable && (
                <div className="mx-6 mt-4 rounded-lg border
                    border-status-error/30 bg-status-error/10
                    px-4 py-3 text-sm text-status-error">
                    The config file is read-only. Setup cannot
                    finish until the TOML file is writable.
                </div>
            )}

            {/* Step content */}
            <div className="flex-1 overflow-y-auto">
                <div className="max-w-4xl mx-auto px-6 py-8">
                    <AnimatePresence mode="wait">
                        {children}
                    </AnimatePresence>
                </div>
            </div>

            {error && (
                <div className="mx-6 pb-4">
                    <div
                        role="alert"
                        aria-live="polite"
                        className="mx-auto max-w-4xl rounded-lg border border-status-error/30 bg-status-error/10 px-4 py-3 text-sm text-status-error"
                    >
                        {error}
                    </div>
                </div>
            )}

            {/* Navigation footer */}
            {step < 6 && (
                <div className="shrink-0 border-t border-helios-line/20
                    bg-helios-surface/50 px-6 py-4">
                    <div className="max-w-4xl mx-auto flex items-center
                        justify-between gap-4">
                        <button
                            type="button"
                            onClick={onBack}
                            disabled={step === 1 || submitting}
                            className={clsx(
                                "rounded-lg px-4 py-2 text-sm font-medium transition-colors",
                                step === 1
                                    ? "text-helios-slate/30 cursor-not-allowed"
                                    : "text-helios-slate hover:bg-helios-surface-soft hover:text-helios-ink"
                            )}
                        >
                            Back
                        </button>
                        <div className="flex items-center gap-3">
                            <span className="text-xs text-helios-slate/50">
                                Step {step} of {SETUP_STEP_COUNT}
                            </span>
                            <button
                                type="button"
                                onClick={onNext}
                                disabled={submitting || !configMutable}
                                className="flex items-center gap-2 rounded-lg
                                    bg-helios-solar px-6 py-2.5 text-sm
                                    font-semibold text-helios-main
                                    hover:opacity-90 transition-opacity
                                    disabled:opacity-50"
                            >
                                {submitting
                                    ? "Working..."
                                    : step === 5
                                        ? "Complete Setup"
                                        : "Next"}
                                {!submitting && <ArrowRight size={16} />}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
