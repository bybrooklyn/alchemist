import { motion } from "framer-motion";
import { AlertCircle, CheckCircle, Loader2 } from "lucide-react";
import { ReviewCard } from "./SetupControls";
import type { FsPreviewResponse, HardwareInfo, SetupSettings, SetupSummaryItem } from "./types";

interface ReviewStepProps {
    setupSummary: SetupSummaryItem[];
    settings: SetupSettings;
    preview: FsPreviewResponse | null;
    previewError: string | null;
    previewLoading: boolean;
    hardware: HardwareInfo | null;
}

export default function ReviewStep({
    setupSummary,
    settings,
    preview,
    previewError,
    previewLoading,
    hardware,
}: ReviewStepProps) {
    const codec = settings.transcode.output_codec;
    const codecLabel = codec === "h264" ? "H.264" : codec.toUpperCase();
    const hardwarePending = hardware === null;
    const codecUnsupported =
        hardware !== null &&
        hardware.supported_codecs.length > 0 &&
        !hardware.supported_codecs.some((entry) => entry.toLowerCase() === codec);
    const previewWarning = previewError
        ? previewError
        : previewLoading
            ? "Library preview is still running. Wait for Alchemist to inspect the selected server folders before completing setup."
            : preview === null && settings.scanner.directories.length > 0
                ? "Library preview has not completed yet. Return to the Library step and confirm server path access."
                : null;

    return (
        <motion.div key="review" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
            <div className="space-y-2">
                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2"><CheckCircle size={20} className="text-helios-solar" />Final Review</h2>
                <p className="text-sm text-helios-slate">Review the effective server paths, processing rules, and automation choices before Alchemist writes the config and starts the first scan.</p>
            </div>

            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
                {setupSummary.map((item) => (
                    <div
                        key={item.label}
                        className="rounded-md border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-4"
                    >
                        <div className="text-xs font-medium text-helios-slate">
                            {item.label}
                        </div>
                        <div className={`mt-2 text-2xl font-bold font-mono ${item.value === "--" ? "text-helios-slate/40" : "text-helios-ink"}`}>
                            {item.value}
                        </div>
                    </div>
                ))}
            </div>

            <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
                <ReviewCard
                    title="Library"
                    lines={[
                        `${settings.scanner.directories.length} server folders selected`,
                        ...(preview ? [`${preview.total_media_files} supported media files previewed`] : []),
                        ...(previewWarning ? [previewWarning] : []),
                        settings.scanner.directories.length > 0
                            ? settings.scanner.directories.map((d) => d.split("/").pop() ?? d).join(", ")
                            : "No folders selected",
                    ]}
                />
                <ReviewCard title="Transcoding" lines={[`Target: ${settings.transcode.output_codec.toUpperCase()}`, `Profile: ${settings.transcode.quality_profile}`, `${settings.transcode.concurrent_jobs} concurrent jobs`, `Subtitle mode: ${settings.transcode.subtitle_mode}`]} />
                <ReviewCard title="Output" lines={[`Extension: .${settings.files.output_extension}`, `Suffix: ${settings.files.output_suffix || "(none)"}`, `Replace strategy: ${settings.files.replace_strategy}`, settings.files.output_root ? `Output root: ${settings.files.output_root}` : "Output beside source"]} />
                <ReviewCard title="Runtime" lines={[`${settings.transcode.concurrent_jobs} concurrent jobs`, `${settings.notifications.targets.length} notification targets`, `${settings.schedule.windows.length} schedule windows`, `Telemetry: ${settings.system.enable_telemetry ? "enabled" : "disabled"}`]} />
            </div>

            {previewWarning && (
                <div className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-200">
                    <AlertCircle size={16} className="mt-0.5 shrink-0" />
                    <p className="leading-relaxed">{previewWarning}</p>
                </div>
            )}

            {hardwarePending && (
                <div role="status" className="flex items-start gap-3 rounded-lg border border-blue-500/25 bg-blue-500/10 px-4 py-3 text-sm text-blue-300">
                    <Loader2 size={16} className="mt-0.5 shrink-0 animate-spin" />
                    <div>
                        <p className="font-semibold">Waiting for hardware detection to finish.</p>
                        <p className="mt-1 leading-relaxed text-blue-200/80">
                            Setup will unlock automatically once Alchemist knows which encoders this machine supports.
                        </p>
                    </div>
                </div>
            )}

            {codecUnsupported && (
                <div className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-200">
                    <AlertCircle size={16} className="mt-0.5 shrink-0" />
                    <p className="leading-relaxed">
                        The detected hardware can't encode {codecLabel}. Alchemist will fall back to CPU encoding for these jobs, which is slower. You can pick a different codec on the Processing step.
                    </p>
                </div>
            )}
        </motion.div>
    );
}
