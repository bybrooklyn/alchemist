import { motion } from "framer-motion";
import { ArrowRight } from "lucide-react";

interface WelcomeStepProps {
    onGetStarted: () => void;
}

export default function WelcomeStep(
    { onGetStarted }: WelcomeStepProps
) {
    return (
        <motion.div
            key="welcome"
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -16 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="flex flex-col items-center justify-center min-h-[60vh] gap-8 text-center px-6"
        >
            <div className="space-y-3">
                <h1 className="text-5xl font-extrabold tracking-tight text-helios-ink leading-none">
                    Alchemist
                </h1>
                <div className="h-0.5 w-12 bg-helios-solar mx-auto rounded-full" />
            </div>

            <p className="text-base text-helios-slate max-w-sm leading-relaxed">
                Self-hosted video transcoding.
                Set it up once, let it run.
            </p>

            <button
                type="button"
                onClick={onGetStarted}
                className="flex items-center gap-2 rounded-lg bg-helios-solar px-8 py-3 text-sm font-semibold text-helios-main hover:opacity-90 transition-opacity"
            >
                Get Started
                <ArrowRight size={16} />
            </button>
        </motion.div>
    );
}
