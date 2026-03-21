import type { ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowRight } from "lucide-react";
import clsx from "clsx";
import { SETUP_STEP_COUNT } from "./constants";

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
    return (
        <div className="bg-helios-surface border border-helios-line/60 rounded-3xl overflow-hidden shadow-2xl max-w-5xl w-full mx-auto">
            <div className="h-1 bg-helios-surface-soft w-full flex">
                <motion.div className="bg-helios-solar h-full" initial={{ width: 0 }} animate={{ width: `${(step / SETUP_STEP_COUNT) * 100}%` }} />
            </div>

            <div className="p-8 lg:p-10">
                <header className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between mb-8">
                    <div className="flex items-center gap-4">
                        <div className="w-12 h-12 rounded-xl bg-helios-solar text-helios-main flex items-center justify-center font-bold text-2xl shadow-lg shadow-helios-solar/20">A</div>
                        <div>
                            <h1 className="text-2xl font-bold text-helios-ink">Alchemist Setup</h1>
                            <p className="text-sm text-helios-slate">Configure the server once, preview the library, and leave with a production-ready baseline.</p>
                        </div>
                    </div>

                    <div className="rounded-2xl border border-helios-line/20 bg-helios-surface-soft/50 px-4 py-3 text-xs text-helios-slate max-w-sm">
                        <p className="font-bold uppercase tracking-wider text-helios-ink">Server-side selection</p>
                        <p className="mt-1">All folders here refer to the filesystem available to the Alchemist server process, not your browser’s local machine.</p>
                    </div>
                </header>

                {!configMutable && <div className="mb-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">The config file is read-only right now. Setup cannot finish until the TOML file is writable.</div>}

                <AnimatePresence mode="wait">{children}</AnimatePresence>

                {error && step < 6 && <div className="mt-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-500">{error}</div>}

                {step < 6 && (
                    <div className="mt-8 flex items-center justify-between gap-4 border-t border-helios-line/20 pt-6">
                        <button type="button" onClick={onBack} disabled={step === 1 || submitting} className={clsx("rounded-xl px-4 py-2 text-sm font-semibold transition-colors", step === 1 ? "text-helios-line cursor-not-allowed" : "text-helios-slate hover:bg-helios-surface-soft")}>
                            Back
                        </button>
                        <button type="button" onClick={onNext} disabled={submitting || !configMutable} className="flex items-center gap-2 rounded-xl bg-helios-solar px-6 py-3 font-semibold text-helios-main hover:opacity-90 transition-opacity disabled:opacity-50">
                            {submitting ? "Working..." : step === 5 ? "Build Engine" : "Next"}
                            {!submitting && <ArrowRight size={18} />}
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
