import type { ReactNode } from "react";
import { motion } from "framer-motion";
import { FileCog, Gauge, Info, ShieldCheck, SlidersHorizontal, Trash2, Video, Zap } from "lucide-react";
import clsx from "clsx";
import { LabeledInput, LabeledSelect, RangeControl, ToggleRow } from "./SetupControls";
import type { SetupSettings } from "./types";

interface ProcessingStepProps {
    transcode: SetupSettings["transcode"];
    files: SetupSettings["files"];
    quality: SetupSettings["quality"];
    onTranscodeChange: (value: SetupSettings["transcode"]) => void;
    onFilesChange: (value: SetupSettings["files"]) => void;
    onQualityChange: (value: SetupSettings["quality"]) => void;
}

interface SectionHeaderProps {
    icon: ReactNode;
    eyebrow: string;
    title: string;
    body: string;
}

function SectionHeader({ icon, eyebrow, title, body }: SectionHeaderProps) {
    return (
        <div className="flex items-start gap-3">
            <div className="rounded-lg bg-helios-solar/10 p-2 text-helios-solar">
                {icon}
            </div>
            <div>
                <p className="text-xs font-semibold uppercase text-helios-slate/70">{eyebrow}</p>
                <h3 className="mt-1 text-base font-semibold text-helios-ink">{title}</h3>
                <p className="mt-1 text-sm leading-relaxed text-helios-slate">{body}</p>
            </div>
        </div>
    );
}

const formatPercent = (value: number) => `${Math.round(value * 100)}%`;
const formatBpp = (value: number) => `${value.toFixed(3).replace(/0+$/, "").replace(/\.$/, "")} BPP`;

export default function ProcessingStep({ transcode, files, quality, onTranscodeChange, onFilesChange, onQualityChange }: ProcessingStepProps) {
    const updateTranscode = (patch: Partial<SetupSettings["transcode"]>) => onTranscodeChange({ ...transcode, ...patch });
    const updateFiles = (patch: Partial<SetupSettings["files"]>) => onFilesChange({ ...files, ...patch });
    const updateQuality = (patch: Partial<SetupSettings["quality"]>) => onQualityChange({ ...quality, ...patch });
    const codecLabels: Record<SetupSettings["transcode"]["output_codec"], string> = {
        av1: "AV1",
        hevc: "HEVC",
        h264: "H.264",
    };
    const codecOptions: Array<{ value: SetupSettings["transcode"]["output_codec"]; title: string; body: string }> = [
        { value: "av1", title: "Smallest files", body: "Best compression when your playback devices can handle it." },
        { value: "hevc", title: "Modern default", body: "Strong savings with broad TV, phone, and Jellyfin support." },
        { value: "h264", title: "Maximum reach", body: "Use when compatibility matters more than storage savings." },
    ];
    const qualityOptions: Array<{ value: SetupSettings["transcode"]["quality_profile"]; title: string; body: string }> = [
        { value: "speed", title: "Speed", body: "Prioritize faster completion and lighter resource use." },
        { value: "balanced", title: "Balanced", body: "Good default for unattended library processing." },
        { value: "quality", title: "Quality", body: "Spend more time per encode to protect detail." },
    ];

    return (
        <motion.div key="processing" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
            <div className="space-y-2">
                <h2 className="flex items-center gap-2 text-xl font-semibold text-helios-ink">
                    <Video size={20} className="text-helios-solar" />
                    Processing, Output & Quality
                </h2>
                <p className="max-w-3xl text-sm leading-relaxed text-helios-slate">
                    Choose the target format, decide how conservative Alchemist should be, and keep output behavior explicit before the engine touches real media.
                </p>
                <div className="grid gap-3 sm:grid-cols-3">
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                        <p className="text-xs font-medium text-helios-slate">Codec</p>
                        <p className="mt-1 text-sm font-semibold text-helios-ink">{codecLabels[transcode.output_codec]}</p>
                    </div>
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                        <p className="text-xs font-medium text-helios-slate">Profile</p>
                        <p className="mt-1 text-sm font-semibold capitalize text-helios-ink">{transcode.quality_profile}</p>
                    </div>
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                        <p className="text-xs font-medium text-helios-slate">Skip target</p>
                        <p className="mt-1 text-sm font-semibold text-helios-ink">{formatPercent(transcode.size_reduction_threshold)} minimum savings</p>
                    </div>
                </div>
            </div>

            <div className="grid grid-cols-1 gap-6 xl:grid-cols-[1.05fr_0.95fr]">
                <section className="space-y-5 rounded-lg border border-helios-line/20 bg-helios-surface p-5">
                    <SectionHeader
                        icon={<Zap size={18} />}
                        eyebrow="Encoding target"
                        title="Pick the output codec and effort profile"
                        body="These settings decide what every planned transcode is trying to produce."
                    />
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                        {codecOptions.map((option) => {
                            const selected = transcode.output_codec === option.value;
                            return (
                                <button
                                    key={option.value}
                                    type="button"
                                    onClick={() => updateTranscode({ output_codec: option.value })}
                                    className={clsx(
                                        "rounded-lg border px-4 py-4 text-left transition-all",
                                        selected
                                            ? "border-helios-solar bg-helios-solar/10 text-helios-ink ring-1 ring-helios-solar/20"
                                            : "border-helios-line/20 bg-helios-surface-soft/40 text-helios-slate hover:border-helios-solar/40 hover:text-helios-ink"
                                    )}
                                >
                                    <div className="text-sm font-semibold uppercase">{codecLabels[option.value]}</div>
                                    <div className="mt-2 text-sm font-medium">{option.title}</div>
                                    <p className="mt-1 text-xs leading-relaxed opacity-75">{option.body}</p>
                                </button>
                            );
                        })}
                    </div>

                    <div className="space-y-3">
                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                            <SlidersHorizontal size={16} className="text-helios-solar" />
                            Quality profile
                        </div>
                        <div className="grid gap-3 sm:grid-cols-3">
                            {qualityOptions.map((option) => {
                                const selected = transcode.quality_profile === option.value;
                                return (
                                    <button
                                        key={option.value}
                                        type="button"
                                        onClick={() => updateTranscode({ quality_profile: option.value })}
                                        className={clsx(
                                            "rounded-lg border px-4 py-3 text-left transition-all",
                                            selected
                                                ? "border-helios-solar bg-helios-solar/10 text-helios-ink ring-1 ring-helios-solar/20"
                                                : "border-helios-line/20 bg-helios-surface-soft/40 text-helios-slate hover:border-helios-solar/40 hover:text-helios-ink"
                                        )}
                                    >
                                        <p className="text-sm font-semibold">{option.title}</p>
                                        <p className="mt-1 text-xs leading-relaxed opacity-75">{option.body}</p>
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                </section>

                <section className="space-y-5 rounded-lg border border-helios-line/20 bg-helios-surface p-5">
                    <SectionHeader
                        icon={<Gauge size={18} />}
                        eyebrow="Throughput and skips"
                        title="Control load and avoid pointless encodes"
                        body="The defaults keep the engine conservative until you have watched it run on your hardware."
                    />
                    <RangeControl
                        label="Concurrent Jobs"
                        min={1}
                        max={8}
                        step={1}
                        value={transcode.concurrent_jobs}
                        valueLabel={`${transcode.concurrent_jobs} ${transcode.concurrent_jobs === 1 ? "job" : "jobs"}`}
                        helperText="More jobs increase throughput, but they also compete for CPU, GPU, disk, and memory."
                        onChange={(concurrent_jobs) => updateTranscode({ concurrent_jobs })}
                    />
                    <RangeControl
                        label="Minimum Savings"
                        min={0}
                        max={0.9}
                        step={0.05}
                        value={transcode.size_reduction_threshold}
                        valueLabel={formatPercent(transcode.size_reduction_threshold)}
                        helperText="Skip files when the estimated output is not smaller enough to justify re-encoding."
                        onChange={(size_reduction_threshold) => updateTranscode({ size_reduction_threshold })}
                    />
                    <details className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                        <summary className="flex cursor-pointer list-none items-center gap-2 text-sm font-semibold text-helios-ink">
                            <Info size={15} className="text-helios-solar" />
                            Advanced skip guards
                        </summary>
                        <div className="mt-4 space-y-4 border-t border-helios-line/10 pt-4">
                            <RangeControl
                                label="BPP Skip Threshold"
                                min={0.03}
                                max={0.25}
                                step={0.005}
                                value={transcode.min_bpp_threshold}
                                valueLabel={formatBpp(transcode.min_bpp_threshold)}
                                helperText="Bits per pixel helps identify files that are already aggressively compressed."
                                onChange={(min_bpp_threshold) => updateTranscode({ min_bpp_threshold })}
                            />
                            <RangeControl
                                label="Minimum File Size"
                                min={0}
                                max={2000}
                                step={25}
                                value={transcode.min_file_size_mb}
                                valueLabel={`${transcode.min_file_size_mb} MB`}
                                helperText="Skip tiny files where encode time is unlikely to produce meaningful storage savings."
                                onChange={(min_file_size_mb) => updateTranscode({ min_file_size_mb })}
                            />
                        </div>
                    </details>
                </section>

                <section className="space-y-5 rounded-lg border border-helios-line/20 bg-helios-surface p-5 xl:col-span-2">
                    <SectionHeader
                        icon={<FileCog size={18} />}
                        eyebrow="Output and safety"
                        title="Decide where finished files go and what can be removed"
                        body="Alchemist should never surprise you with destructive output behavior. Keep these choices explicit."
                    />
                    <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                        <LabeledInput label="Output Extension" value={files.output_extension} onChange={(output_extension) => updateFiles({ output_extension })} placeholder="mkv" />
                        <LabeledInput label="Output Suffix" value={files.output_suffix} onChange={(output_suffix) => updateFiles({ output_suffix })} placeholder="-alchemist" />
                        <LabeledSelect label="Existing Output Policy" value={files.replace_strategy} onChange={(replace_strategy) => updateFiles({ replace_strategy })} options={[{ value: "keep", label: "Keep existing output" }, { value: "replace", label: "Replace existing output" }]} />
                        <LabeledSelect label="Subtitle Handling" value={transcode.subtitle_mode} onChange={(subtitle_mode) => updateTranscode({ subtitle_mode: subtitle_mode as SetupSettings["transcode"]["subtitle_mode"] })} options={[{ value: "copy", label: "Copy subtitles" }, { value: "burn", label: "Burn one subtitle track" }, { value: "extract", label: "Extract to sidecar" }, { value: "none", label: "Drop subtitles" }]} />
                    </div>
                    <LabeledInput label="Optional Output Root" value={files.output_root ?? ""} onChange={(output_root) => updateFiles({ output_root: output_root || null })} placeholder="Leave blank to write beside the source" />

                    <div className="grid grid-cols-1 gap-3 lg:grid-cols-3">
                        <ToggleRow title="Allow Encoder Fallback" body="Permit alternate encoders if the preferred codec or hardware path is unavailable." checked={transcode.allow_fallback} onChange={(allow_fallback) => updateTranscode({ allow_fallback })} />
                        <ToggleRow title="Enable VMAF Validation" body="Score output quality after encoding and optionally reject low-quality output." checked={quality.enable_vmaf} onChange={(enable_vmaf) => updateQuality({ enable_vmaf })} />
                        <ToggleRow title="Delete Source After Success" body="Remove the original file after a completed transcode." checked={files.delete_source} onChange={(delete_source) => updateFiles({ delete_source })} />
                    </div>

                    {(files.delete_source || quality.enable_vmaf) && (
                        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                            {files.delete_source && (
                                <div className="flex items-start gap-3 rounded-lg border border-red-500/25 bg-red-500/10 px-4 py-3 text-sm text-helios-ink">
                                    <Trash2 size={16} className="mt-0.5 shrink-0 text-red-400" />
                                    <p className="leading-relaxed">Source deletion is only safe after you have verified output quality and backup behavior on real files.</p>
                                </div>
                            )}
                            {quality.enable_vmaf && (
                                <div className="space-y-4 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 p-4">
                                    <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                                        <ShieldCheck size={16} className="text-helios-solar" />
                                        VMAF guardrail
                                    </div>
                                    <div className="grid gap-4 sm:grid-cols-2">
                                        <LabeledInput label="Minimum VMAF" value={String(quality.min_vmaf_score)} onChange={(value) => updateQuality({ min_vmaf_score: parseFloat(value) || 0 })} placeholder="90" type="number" />
                                        <ToggleRow title="Reject Low Quality" body="Keep the source when the VMAF score misses the minimum." checked={quality.revert_on_low_quality} onChange={(revert_on_low_quality) => updateQuality({ revert_on_low_quality })} />
                                    </div>
                                </div>
                            )}
                        </div>
                    )}
                </section>
            </div>
        </motion.div>
    );
}
