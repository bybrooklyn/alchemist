import { motion } from "framer-motion";
import { FileCog, Info, Video } from "lucide-react";
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

export default function ProcessingStep({ transcode, files, quality, onTranscodeChange, onFilesChange, onQualityChange }: ProcessingStepProps) {
    const updateTranscode = (patch: Partial<SetupSettings["transcode"]>) => onTranscodeChange({ ...transcode, ...patch });
    const updateFiles = (patch: Partial<SetupSettings["files"]>) => onFilesChange({ ...files, ...patch });
    const updateQuality = (patch: Partial<SetupSettings["quality"]>) => onQualityChange({ ...quality, ...patch });
    const codecLabels: Record<SetupSettings["transcode"]["output_codec"], string> = {
        av1: "AV1",
        hevc: "HEVC",
        h264: "H.264",
    };

    return (
        <motion.div key="processing" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
            <div className="space-y-2">
                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2"><Video size={20} className="text-helios-solar" />Processing, Output & Quality</h2>
                <p className="text-sm text-helios-slate">Tune what Alchemist creates, how aggressive it should be, and how quality should be validated before replacing source material.</p>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                    <div className="text-sm font-semibold text-helios-ink">Transcoding Target</div>
                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                        {(["av1", "hevc", "h264"] as const).map((codec) => (
                            <button
                                key={codec}
                                type="button"
                                onClick={() => updateTranscode({ output_codec: codec })}
                                className={clsx("rounded-lg border px-4 py-4 text-left transition-all", transcode.output_codec === codec ? "border-helios-solar bg-helios-solar/10 text-helios-ink" : "border-helios-line/20 bg-helios-surface-soft/40 text-helios-slate")}
                            >
                                <div className="font-semibold uppercase">{codecLabels[codec]}</div>
                                <div className="text-[10px] mt-2 opacity-80">{codec === "av1" ? "Best compression" : codec === "hevc" ? "Broad modern compatibility" : "Maximum playback compatibility"}</div>
                            </button>
                        ))}
                    </div>

                    <div className="space-y-3">
                        <label className="text-xs font-medium text-helios-slate">
                            Quality Profile
                        </label>
                        <select value={transcode.quality_profile} onChange={(e) => updateTranscode({ quality_profile: e.target.value as SetupSettings["transcode"]["quality_profile"] })} className="w-full rounded-md border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink">
                            <option value="speed">Speed</option>
                            <option value="balanced">Balanced</option>
                            <option value="quality">Quality</option>
                        </select>
                        <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                            Controls the balance between file size and visual quality. Lower numbers = better quality but larger files. Higher numbers = smaller files with slightly lower quality. The default works well for most people.
                        </p>
                    </div>

                    <div>
                        <RangeControl label="Concurrent Jobs" min={1} max={8} step={1} value={transcode.concurrent_jobs} onChange={(concurrent_jobs) => updateTranscode({ concurrent_jobs })} />
                        <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                            How many videos to convert at the same time. More means faster overall progress but uses more CPU and GPU resources. Start with 1 or 2 if you're not sure — you can always increase it later.
                        </p>
                    </div>
                    <div>
                        <RangeControl label="Minimum Savings" min={0} max={0.9} step={0.05} value={transcode.size_reduction_threshold} onChange={(size_reduction_threshold) => updateTranscode({ size_reduction_threshold })} />
                        <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                            Alchemist will skip a file if the newly encoded version wouldn't be at least this much smaller than the original. This prevents pointless re-encoding of files that are already well-optimized.
                        </p>
                    </div>
                    <details className="group">
                        <summary className="flex items-center gap-1.5 text-xs text-helios-solar cursor-pointer hover:underline select-none list-none">
                            <Info size={13} />
                            What is the BPP threshold?
                        </summary>
                        <p className="mt-2 text-xs text-helios-slate leading-relaxed pl-5">
                            Bits Per Pixel — determines how compressed a file must
                            already be before Alchemist skips it. If a file is already
                            very compressed, re-encoding it could reduce quality without
                            saving much space. Leave this at the default unless you know
                            what you're doing.
                        </p>
                    </details>
                    <details className="group">
                        <summary className="flex items-center gap-1.5 text-xs text-helios-solar cursor-pointer hover:underline select-none list-none">
                            <Info size={13} />
                            Why are small files skipped?
                        </summary>
                        <p className="mt-2 text-xs text-helios-slate leading-relaxed pl-5">
                            Files smaller than this will be skipped entirely. Small
                            files rarely benefit from transcoding and it's usually not
                            worth the processing time.
                        </p>
                    </details>
                </div>

                <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                    <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><FileCog size={16} className="text-helios-solar" />Output Rules</div>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <LabeledInput label="Output Extension" value={files.output_extension} onChange={(output_extension) => updateFiles({ output_extension })} placeholder="mkv" />
                        <LabeledInput label="Output Suffix" value={files.output_suffix} onChange={(output_suffix) => updateFiles({ output_suffix })} placeholder="-alchemist" />
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <LabeledSelect label="Replace Strategy" value={files.replace_strategy} onChange={(replace_strategy) => updateFiles({ replace_strategy })} options={[{ value: "keep", label: "Keep existing output" }, { value: "replace", label: "Replace existing output" }]} />
                        <LabeledSelect label="Subtitle Handling" value={transcode.subtitle_mode} onChange={(subtitle_mode) => updateTranscode({ subtitle_mode: subtitle_mode as SetupSettings["transcode"]["subtitle_mode"] })} options={[{ value: "copy", label: "Copy subtitles" }, { value: "burn", label: "Burn one subtitle track" }, { value: "extract", label: "Extract to sidecar" }, { value: "none", label: "Drop subtitles" }]} />
                    </div>

                    <LabeledInput label="Optional Output Root" value={files.output_root ?? ""} onChange={(output_root) => updateFiles({ output_root: output_root || null })} placeholder="Leave blank to write beside the source" />

                    <div className="space-y-3">
                        <ToggleRow title="Allow Fallback" body="Permit alternate encoders if the preferred codec/hardware path is unavailable." checked={transcode.allow_fallback} onChange={(allow_fallback) => updateTranscode({ allow_fallback })} />
                        <ToggleRow title="Delete Source After Success" body="Remove the original file after a successful completed transcode." checked={files.delete_source} onChange={(delete_source) => updateFiles({ delete_source })} />
                        <ToggleRow title="Enable VMAF Validation" body="Score output quality after encoding and optionally revert if it drops too low." checked={quality.enable_vmaf} onChange={(enable_vmaf) => updateQuality({ enable_vmaf })} />
                    </div>

                    {quality.enable_vmaf && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <LabeledInput label="Minimum VMAF" value={String(quality.min_vmaf_score)} onChange={(value) => updateQuality({ min_vmaf_score: parseFloat(value) || 0 })} placeholder="90" type="number" />
                            <ToggleRow title="Revert On Low Quality" body="Keep the source when the VMAF score misses the minimum." checked={quality.revert_on_low_quality} onChange={(revert_on_low_quality) => updateQuality({ revert_on_low_quality })} />
                        </div>
                    )}
                </div>
            </div>
        </motion.div>
    );
}
