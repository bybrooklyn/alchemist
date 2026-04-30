import { useCallback, useEffect, useMemo, useState } from "react";
import {
    ArrowRight,
    CheckCircle2,
    ChevronDown,
    Download,
    FileVideo,
    Play,
    RefreshCw,
    Settings2,
    SlidersHorizontal,
    Trash2,
    Upload,
} from "lucide-react";
import { apiAction, apiFetch, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

interface SubtitleStreamMetadata {
    stream_index: number;
    codec_name: string;
    language?: string;
    title?: string;
    burnable: boolean;
}

interface AudioStreamMetadata {
    stream_index: number;
    codec_name: string;
    language?: string;
    title?: string;
    channels?: number;
}

interface MediaAnalysis {
    metadata: {
        path?: string;
        duration_secs: number;
        container: string;
        codec_name: string;
        width: number;
        height: number;
        dynamic_range: string;
        size_bytes: number;
        video_bitrate_bps?: number | null;
        audio_codec?: string | null;
        audio_bitrate_bps?: number | null;
        audio_channels?: number | null;
        audio_streams: AudioStreamMetadata[];
        subtitle_streams: SubtitleStreamMetadata[];
    };
}

interface ConversionSettings {
    output_container: string;
    remux_only: boolean;
    video: {
        codec: string;
        mode: string;
        value: number | null;
        preset: string | null;
        resolution: {
            mode: string;
            width: number | null;
            height: number | null;
            scale_factor: number | null;
        };
        hdr_mode: string;
    };
    audio: {
        codec: string;
        bitrate_kbps: number | null;
        channels: string | null;
    };
    subtitles: {
        mode: string;
    };
}

interface UploadResponse {
    conversion_job_id: number;
    probe: MediaAnalysis;
    normalized_settings: ConversionSettings;
}

interface PreviewEstimate {
    estimated_output_bytes: number;
    estimated_savings_bytes: number;
    estimated_savings_percent: number;
    confidence: string;
    note: string;
}

interface PreviewSummary {
    source: {
        file_name: string;
        container: string;
        video_codec: string;
        resolution: string;
        dynamic_range: string;
        duration_secs: number;
        size_bytes: number;
        audio: string;
        subtitle_count: number;
    };
    planned_output: {
        mode: string;
        container: string;
        video_codec: string;
        resolution: string;
        hdr_mode: string;
        audio: string;
        subtitles: string;
        encoder: string | null;
        backend: string | null;
    };
    estimate: PreviewEstimate;
}

interface PreviewResponse {
    normalized_settings: ConversionSettings;
    command_preview: string;
    summary: PreviewSummary;
}

interface JobStatusResponse {
    id: number;
    status: string;
    progress: number;
    linked_job_id: number | null;
    output_path: string | null;
    download_ready: boolean;
    probe: MediaAnalysis | null;
}

const DEFAULT_SETTINGS: ConversionSettings = {
    output_container: "mkv",
    remux_only: false,
    video: {
        codec: "hevc",
        mode: "crf",
        value: 24,
        preset: "medium",
        resolution: {
            mode: "original",
            width: null,
            height: null,
            scale_factor: null,
        },
        hdr_mode: "preserve",
    },
    audio: {
        codec: "copy",
        bitrate_kbps: 160,
        channels: "auto",
    },
    subtitles: {
        mode: "copy",
    },
};

const QUALITY_VALUES = {
    high: 20,
    balanced: 24,
    small: 28,
} as const;

type QualityKey = keyof typeof QUALITY_VALUES | "custom";

export function ConversionTool() {
    const [uploading, setUploading] = useState(false);
    const [previewing, setPreviewing] = useState(false);
    const [starting, setStarting] = useState(false);
    const [advancedOpen, setAdvancedOpen] = useState(false);
    const [status, setStatus] = useState<JobStatusResponse | null>(null);
    const [conversionJobId, setConversionJobId] = useState<number | null>(null);
    const [probe, setProbe] = useState<MediaAnalysis | null>(null);
    const [settings, setSettings] = useState<ConversionSettings>(DEFAULT_SETTINGS);
    const [commandPreview, setCommandPreview] = useState("");
    const [previewSummary, setPreviewSummary] = useState<PreviewSummary | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [previewError, setPreviewError] = useState<string | null>(null);

    useEffect(() => {
        if (!conversionJobId) return;
        const id = window.setInterval(() => {
            void apiJson<JobStatusResponse>(`/api/conversion/jobs/${conversionJobId}`)
                .then(setStatus)
                .catch(() => undefined);
        }, 2000);
        return () => window.clearInterval(id);
    }, [conversionJobId]);

    const runPreview = useCallback(
        async (
            settingsToPreview: ConversionSettings,
            options: { silent?: boolean; signal?: AbortSignal } = {}
        ) => {
            if (!conversionJobId) return;
            setPreviewing(true);
            setPreviewError(null);
            try {
                const payload = await apiJson<PreviewResponse>("/api/conversion/preview", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    signal: options.signal,
                    body: JSON.stringify({
                        conversion_job_id: conversionJobId,
                        settings: settingsToPreview,
                    }),
                });
                const normalizedSignature = JSON.stringify(payload.normalized_settings);
                setSettings((current) =>
                    JSON.stringify(current) === normalizedSignature
                        ? current
                        : payload.normalized_settings
                );
                setCommandPreview(payload.command_preview);
                setPreviewSummary(payload.summary);
                if (!options.silent) {
                    showToast({ kind: "success", title: "Conversion", message: "Preview updated." });
                }
            } catch (err) {
                if (err instanceof DOMException && err.name === "AbortError") {
                    return;
                }
                const message = isApiError(err) ? err.message : "Preview failed";
                setCommandPreview("");
                setPreviewSummary(null);
                setPreviewError(message);
                if (!options.silent) {
                    showToast({ kind: "error", title: "Conversion", message });
                }
            } finally {
                setPreviewing(false);
            }
        },
        [conversionJobId]
    );

    const settingsSignature = JSON.stringify(settings);
    useEffect(() => {
        if (!conversionJobId || !probe) return;
        const controller = new AbortController();
        const timeout = window.setTimeout(() => {
            void runPreview(settings, { silent: true, signal: controller.signal });
        }, 350);
        return () => {
            window.clearTimeout(timeout);
            controller.abort();
        };
    }, [conversionJobId, probe, settingsSignature, runPreview]);

    const uploadFile = async (file: File) => {
        setUploading(true);
        setError(null);
        setPreviewError(null);
        try {
            const formData = new FormData();
            formData.append("file", file);
            const response = await apiFetch("/api/conversion/uploads", {
                method: "POST",
                body: formData,
            });
            if (!response.ok) {
                throw new Error(await response.text());
            }
            const payload = (await response.json()) as UploadResponse;
            setConversionJobId(payload.conversion_job_id);
            setProbe(payload.probe);
            setSettings(payload.normalized_settings);
            setStatus(null);
            setCommandPreview("");
            setPreviewSummary(null);
            showToast({
                kind: "success",
                title: "Conversion",
                message: "File uploaded and probed.",
            });
        } catch (err) {
            const message = err instanceof Error ? err.message : "Upload failed";
            setError(message);
            showToast({ kind: "error", title: "Conversion", message });
        } finally {
            setUploading(false);
        }
    };

    const start = async () => {
        if (!conversionJobId) return;
        setStarting(true);
        try {
            await apiAction(`/api/conversion/jobs/${conversionJobId}/start`, { method: "POST" });
            const payload = await apiJson<JobStatusResponse>(`/api/conversion/jobs/${conversionJobId}`);
            setStatus(payload);
            showToast({ kind: "success", title: "Conversion", message: "Conversion job queued." });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to start conversion";
            setError(message);
            showToast({ kind: "error", title: "Conversion", message });
        } finally {
            setStarting(false);
        }
    };

    const remove = async () => {
        if (!conversionJobId) return;
        try {
            await apiAction(`/api/conversion/jobs/${conversionJobId}`, { method: "DELETE" });
            setConversionJobId(null);
            setProbe(null);
            setStatus(null);
            setSettings(DEFAULT_SETTINGS);
            setCommandPreview("");
            setPreviewSummary(null);
            setPreviewError(null);
            showToast({ kind: "success", title: "Conversion", message: "Conversion job removed." });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to remove conversion job";
            setError(message);
            showToast({ kind: "error", title: "Conversion", message });
        }
    };

    const download = async () => {
        if (!conversionJobId) return;
        window.location.href = `/api/conversion/jobs/${conversionJobId}/download`;
    };

    const qualityKey = inferQualityKey(settings);
    const sourceSummary = useMemo(() => buildSourceSummary(probe, previewSummary), [probe, previewSummary]);
    const outputSummary = previewSummary?.planned_output ?? null;
    const estimate = previewSummary?.estimate ?? null;

    return (
        <div className="space-y-5">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
                <div>
                    <h1 className="text-xl font-bold text-helios-ink">Convert</h1>
                    <p className="mt-1 text-sm text-helios-slate">
                        Prepare one file with safe defaults, then tune details when needed.
                    </p>
                </div>
                {probe && (
                    <div className="flex flex-wrap gap-2">
                        <button
                            onClick={() => void start()}
                            disabled={starting || previewing || !commandPreview || Boolean(previewError)}
                            className="inline-flex items-center gap-2 rounded-lg bg-helios-solar px-4 py-2 text-sm font-bold text-helios-main disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            <Play size={16} />
                            {starting ? "Starting..." : "Start Job"}
                        </button>
                        <button
                            onClick={() => void download()}
                            disabled={!status?.download_ready}
                            className="inline-flex items-center gap-2 rounded-lg border border-helios-line/30 px-4 py-2 text-sm font-semibold text-helios-ink disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            <Download size={16} />
                            Download
                        </button>
                    </div>
                )}
            </div>

            {error && (
                <div className="rounded-lg border border-status-error/20 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {error}
                </div>
            )}

            {!probe && (
                <label className="flex min-h-[280px] cursor-pointer flex-col items-center justify-center gap-4 rounded-lg border border-dashed border-helios-line/40 bg-helios-surface px-6 py-12 text-center transition-colors hover:bg-helios-surface-soft">
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft p-3 text-helios-solar">
                        <Upload size={28} />
                    </div>
                    <div>
                        <p className="text-base font-semibold text-helios-ink">Upload a source file</p>
                        <p className="mt-1 text-sm text-helios-slate">MKV, MP4, MOV, WebM, and other FFmpeg-readable media.</p>
                    </div>
                    <input
                        type="file"
                        className="hidden"
                        onChange={(event) => {
                            const file = event.target.files?.[0];
                            if (file) {
                                void uploadFile(file);
                            }
                        }}
                        disabled={uploading}
                    />
                    <span className="rounded-lg bg-helios-solar px-4 py-2 text-sm font-bold text-helios-main">
                        {uploading ? "Uploading..." : "Choose File"}
                    </span>
                </label>
            )}

            {probe && sourceSummary && (
                <>
                    <section className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_72px_minmax(0,1fr)] xl:items-stretch">
                        <SummaryPanel
                            eyebrow="Source"
                            title={sourceSummary.fileName}
                            icon={<FileVideo size={18} />}
                            rows={[
                                ["Container", sourceSummary.container],
                                ["Video", sourceSummary.videoCodec],
                                ["Resolution", sourceSummary.resolution],
                                ["Dynamic range", sourceSummary.dynamicRange],
                                ["Duration", formatDuration(sourceSummary.durationSecs)],
                                ["Size", formatBytes(sourceSummary.sizeBytes)],
                                ["Audio", sourceSummary.audio],
                                ["Subtitles", `${sourceSummary.subtitleCount}`],
                            ]}
                        />
                        <div className="flex items-center justify-center text-helios-solar">
                            <div className="flex h-12 w-12 items-center justify-center rounded-full border border-helios-solar/30 bg-helios-solar/10">
                                <ArrowRight size={24} className="rotate-90 xl:rotate-0" />
                            </div>
                        </div>
                        <SummaryPanel
                            eyebrow={previewing ? "Planning..." : "Planned Output"}
                            title={outputSummary ? `${outputSummary.container.toUpperCase()} ${formatCodec(outputSummary.video_codec)}` : "Waiting for preview"}
                            icon={previewError ? <Settings2 size={18} /> : <CheckCircle2 size={18} />}
                            rows={[
                                ["Mode", outputSummary ? humanize(outputSummary.mode) : "--"],
                                ["Container", outputSummary?.container.toUpperCase() ?? settings.output_container.toUpperCase()],
                                ["Video", outputSummary ? formatCodec(outputSummary.video_codec) : formatCodec(settings.video.codec)],
                                ["Resolution", outputSummary?.resolution ?? `${probe.metadata.width}x${probe.metadata.height}`],
                                ["HDR", outputSummary ? humanize(outputSummary.hdr_mode) : humanize(settings.video.hdr_mode)],
                                ["Audio", outputSummary ? humanize(outputSummary.audio) : humanize(settings.audio.codec)],
                                ["Subtitles", outputSummary ? humanize(outputSummary.subtitles) : humanize(settings.subtitles.mode)],
                                ["Est. size", estimate ? formatBytes(estimate.estimated_output_bytes) : "--"],
                            ]}
                            footer={
                                estimate
                                    ? `${formatBytes(estimate.estimated_savings_bytes)} saved (${estimate.estimated_savings_percent.toFixed(1)}%)`
                                    : previewError ?? "Estimated savings will appear after preview."
                            }
                            footerTone={previewError ? "error" : "accent"}
                        />
                    </section>

                    <section className="rounded-lg border border-helios-line/20 bg-helios-surface p-5">
                        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                            <SelectField
                                label="Mode"
                                value={settings.remux_only ? "remux" : "compress"}
                                options={[
                                    { value: "compress", label: "Balanced Compress" },
                                    { value: "remux", label: "Remux Only" },
                                ]}
                                onChange={(value) => setMode(value, setSettings)}
                            />
                            <SelectField
                                label="Quality"
                                value={qualityKey}
                                disabled={settings.remux_only}
                                options={[
                                    { value: "balanced", label: "Balanced" },
                                    { value: "high", label: "Higher Quality" },
                                    { value: "small", label: "Smallest File" },
                                    { value: "custom", label: "Custom", disabled: true },
                                ]}
                                onChange={(value) => setQualityPreset(value as QualityKey, setSettings)}
                            />
                            <SelectField
                                label="Container"
                                value={settings.output_container}
                                options={["mkv", "mp4", "webm", "mov"].map((value) => ({
                                    value,
                                    label: value.toUpperCase(),
                                }))}
                                onChange={(value) => updateSettings(setSettings, { output_container: value })}
                            />
                        </div>

                        <div className="mt-5 grid gap-3 md:grid-cols-3">
                            <ToggleControl
                                label="Preserve HDR"
                                checked={settings.video.hdr_mode === "preserve"}
                                disabled={settings.remux_only}
                                onChange={(checked) =>
                                    setSettings((current) => ({
                                        ...current,
                                        video: { ...current.video, hdr_mode: checked ? "preserve" : "tonemap" },
                                    }))
                                }
                            />
                            <ToggleControl
                                label="Copy Audio"
                                checked={settings.audio.codec === "copy"}
                                disabled={settings.remux_only}
                                onChange={(checked) =>
                                    setSettings((current) => ({
                                        ...current,
                                        audio: { ...current.audio, codec: checked ? "copy" : "aac" },
                                    }))
                                }
                            />
                            <ToggleControl
                                label="Keep Subtitles"
                                checked={settings.subtitles.mode === "copy"}
                                disabled={settings.remux_only}
                                onChange={(checked) =>
                                    setSettings((current) => ({
                                        ...current,
                                        subtitles: { mode: checked ? "copy" : "remove" },
                                    }))
                                }
                            />
                        </div>

                        <div className="mt-5 flex flex-wrap items-center gap-3">
                            <button
                                onClick={() => void runPreview(settings)}
                                disabled={previewing}
                                className="inline-flex items-center gap-2 rounded-lg border border-helios-line/30 px-4 py-2 text-sm font-semibold text-helios-ink disabled:opacity-50"
                            >
                                <RefreshCw size={16} className={previewing ? "animate-spin" : ""} />
                                {previewing ? "Updating..." : "Refresh Preview"}
                            </button>
                            <button
                                onClick={() => setAdvancedOpen((open) => !open)}
                                className="inline-flex items-center gap-2 rounded-lg border border-helios-line/30 px-4 py-2 text-sm font-semibold text-helios-ink"
                                aria-expanded={advancedOpen}
                            >
                                <SlidersHorizontal size={16} />
                                Advanced
                                <ChevronDown size={16} className={advancedOpen ? "rotate-180 transition-transform" : "transition-transform"} />
                            </button>
                            <button
                                onClick={() => void remove()}
                                className="inline-flex items-center gap-2 rounded-lg border border-status-error/30 px-4 py-2 text-sm font-semibold text-status-error"
                            >
                                <Trash2 size={16} />
                                Remove
                            </button>
                            {previewError && <span className="text-sm text-status-error">{previewError}</span>}
                        </div>
                    </section>

                    {advancedOpen && (
                        <AdvancedPanel
                            probe={probe}
                            settings={settings}
                            setSettings={setSettings}
                            commandPreview={commandPreview}
                            estimate={estimate}
                        />
                    )}

                    {status && (
                        <section className="rounded-lg border border-helios-line/20 bg-helios-surface p-5">
                            <div className="grid gap-3 md:grid-cols-4">
                                <Stat label="State" value={humanize(status.status)} />
                                <Stat label="Progress" value={`${status.progress.toFixed(1)}%`} />
                                <Stat label="Linked Job" value={status.linked_job_id ? `#${status.linked_job_id}` : "None"} />
                                <Stat label="Download" value={status.download_ready ? "Ready" : "Pending"} />
                            </div>
                        </section>
                    )}
                </>
            )}
        </div>
    );
}

function AdvancedPanel({
    probe,
    settings,
    setSettings,
    commandPreview,
    estimate,
}: {
    probe: MediaAnalysis;
    settings: ConversionSettings;
    setSettings: React.Dispatch<React.SetStateAction<ConversionSettings>>;
    commandPreview: string;
    estimate: PreviewEstimate | null;
}) {
    return (
        <section className="rounded-lg border border-helios-line/20 bg-helios-surface p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                <Settings2 size={16} className="text-helios-solar" />
                Power Controls
            </div>

            <div className="mt-5 grid gap-5 xl:grid-cols-2">
                <div className="space-y-5">
                    <FieldGroup title="Video">
                        <SelectField
                            label="Video Codec"
                            value={settings.video.codec}
                            disabled={settings.remux_only}
                            options={["copy", "h264", "hevc", "av1"].map((value) => ({ value, label: formatCodec(value) }))}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    video: { ...current.video, codec: value },
                                }))
                            }
                        />
                        <SelectField
                            label="Mode"
                            value={settings.video.mode}
                            disabled={settings.remux_only || settings.video.codec === "copy"}
                            options={[
                                { value: "crf", label: "Quality" },
                                { value: "bitrate", label: "Bitrate" },
                            ]}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    video: { ...current.video, mode: value },
                                }))
                            }
                        />
                        <NumberField
                            label={settings.video.mode === "bitrate" ? "Bitrate (kbps)" : "Quality Value"}
                            value={settings.video.value ?? 0}
                            disabled={settings.remux_only || settings.video.codec === "copy"}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    video: { ...current.video, value },
                                }))
                            }
                        />
                        <SelectField
                            label="Preset"
                            value={settings.video.preset ?? "medium"}
                            disabled={settings.remux_only || settings.video.codec === "copy"}
                            options={["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"].map((value) => ({
                                value,
                                label: humanize(value),
                            }))}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    video: { ...current.video, preset: value },
                                }))
                            }
                        />
                        <SelectField
                            label="Resolution"
                            value={settings.video.resolution.mode}
                            disabled={settings.remux_only || settings.video.codec === "copy"}
                            options={[
                                { value: "original", label: "Original" },
                                { value: "custom", label: "Custom" },
                                { value: "scale_factor", label: "Scale Factor" },
                            ]}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    video: {
                                        ...current.video,
                                        resolution: { ...current.video.resolution, mode: value },
                                    },
                                }))
                            }
                        />
                        <SelectField
                            label="HDR"
                            value={settings.video.hdr_mode}
                            disabled={settings.remux_only || settings.video.codec === "copy"}
                            options={[
                                { value: "preserve", label: "Preserve" },
                                { value: "tonemap", label: "Tonemap" },
                                { value: "strip_metadata", label: "Strip Metadata" },
                            ]}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    video: { ...current.video, hdr_mode: value },
                                }))
                            }
                        />
                        {settings.video.resolution.mode === "custom" && (
                            <>
                                <NumberField
                                    label="Width"
                                    value={settings.video.resolution.width ?? probe.metadata.width}
                                    disabled={settings.remux_only || settings.video.codec === "copy"}
                                    onChange={(value) =>
                                        setSettings((current) => ({
                                            ...current,
                                            video: {
                                                ...current.video,
                                                resolution: { ...current.video.resolution, width: value },
                                            },
                                        }))
                                    }
                                />
                                <NumberField
                                    label="Height"
                                    value={settings.video.resolution.height ?? probe.metadata.height}
                                    disabled={settings.remux_only || settings.video.codec === "copy"}
                                    onChange={(value) =>
                                        setSettings((current) => ({
                                            ...current,
                                            video: {
                                                ...current.video,
                                                resolution: { ...current.video.resolution, height: value },
                                            },
                                        }))
                                    }
                                />
                            </>
                        )}
                        {settings.video.resolution.mode === "scale_factor" && (
                            <NumberField
                                label="Scale Factor"
                                value={settings.video.resolution.scale_factor ?? 1}
                                disabled={settings.remux_only || settings.video.codec === "copy"}
                                step="0.1"
                                onChange={(value) =>
                                    setSettings((current) => ({
                                        ...current,
                                        video: {
                                            ...current.video,
                                            resolution: { ...current.video.resolution, scale_factor: value },
                                        },
                                    }))
                                }
                            />
                        )}
                    </FieldGroup>

                    <FieldGroup title="Audio & Subtitles">
                        <SelectField
                            label="Audio Codec"
                            value={settings.audio.codec}
                            disabled={settings.remux_only}
                            options={["copy", "aac", "opus", "mp3", "remove"].map((value) => ({ value, label: humanize(value) }))}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    audio: { ...current.audio, codec: value },
                                }))
                            }
                        />
                        <NumberField
                            label="Audio Bitrate (kbps)"
                            value={settings.audio.bitrate_kbps ?? 160}
                            disabled={settings.remux_only || settings.audio.codec === "copy" || settings.audio.codec === "remove"}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    audio: { ...current.audio, bitrate_kbps: value },
                                }))
                            }
                        />
                        <SelectField
                            label="Audio Channels"
                            value={settings.audio.channels ?? "auto"}
                            disabled={settings.remux_only || settings.audio.codec === "copy" || settings.audio.codec === "remove"}
                            options={["auto", "stereo", "5.1"].map((value) => ({ value, label: humanize(value) }))}
                            onChange={(value) =>
                                setSettings((current) => ({
                                    ...current,
                                    audio: { ...current.audio, channels: value },
                                }))
                            }
                        />
                        <SelectField
                            label="Subtitles"
                            value={settings.subtitles.mode}
                            disabled={settings.remux_only}
                            options={[
                                { value: "copy", label: "Copy Compatible" },
                                { value: "burn", label: "Burn In" },
                                { value: "remove", label: "Remove" },
                            ]}
                            onChange={(value) => setSettings((current) => ({ ...current, subtitles: { mode: value } }))}
                        />
                    </FieldGroup>
                </div>

                <div className="space-y-4">
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 p-4">
                        <div className="text-xs font-semibold uppercase tracking-[0.08em] text-helios-slate">Estimate</div>
                        <div className="mt-3 grid gap-3 sm:grid-cols-2">
                            <Stat label="Output Size" value={estimate ? formatBytes(estimate.estimated_output_bytes) : "--"} />
                            <Stat label="Savings" value={estimate ? `${formatBytes(estimate.estimated_savings_bytes)} (${estimate.estimated_savings_percent.toFixed(1)}%)` : "--"} />
                            <Stat label="Confidence" value={estimate ? humanize(estimate.confidence) : "--"} />
                            <Stat label="Container" value={settings.output_container.toUpperCase()} />
                        </div>
                        {estimate && <p className="mt-3 text-xs leading-relaxed text-helios-slate">{estimate.note}</p>}
                    </div>

                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 p-4">
                        <div className="text-xs font-semibold uppercase tracking-[0.08em] text-helios-slate">FFmpeg Command</div>
                        {commandPreview ? (
                            <pre className="mt-3 max-h-[360px] overflow-auto whitespace-pre-wrap rounded-lg border border-helios-line/20 bg-helios-main/60 p-4 font-mono text-xs leading-5 text-helios-ink">
                                {commandPreview}
                            </pre>
                        ) : (
                            <div className="mt-3 rounded-lg border border-helios-line/20 bg-helios-main/40 px-4 py-8 text-center text-sm text-helios-slate">
                                Preview is not available yet.
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </section>
    );
}

function SummaryPanel({
    eyebrow,
    title,
    icon,
    rows,
    footer,
    footerTone = "accent",
}: {
    eyebrow: string;
    title: string;
    icon: React.ReactNode;
    rows: Array<[string, string]>;
    footer?: string;
    footerTone?: "accent" | "error";
}) {
    return (
        <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5">
            <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                    <p className="text-xs font-semibold uppercase tracking-[0.08em] text-helios-slate">{eyebrow}</p>
                    <h2 className="mt-1 truncate text-lg font-bold text-helios-ink" title={title}>
                        {title}
                    </h2>
                </div>
                <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft p-2 text-helios-solar">
                    {icon}
                </div>
            </div>
            <div className="mt-5 grid gap-2 sm:grid-cols-2">
                {rows.map(([label, value]) => (
                    <InfoRow key={label} label={label} value={value} />
                ))}
            </div>
            {footer && (
                <div className={`mt-4 rounded-lg border px-3 py-2 text-sm font-semibold ${
                    footerTone === "error"
                        ? "border-status-error/20 bg-status-error/10 text-status-error"
                        : "border-helios-solar/20 bg-helios-solar/10 text-helios-solar"
                }`}>
                    {footer}
                </div>
            )}
        </div>
    );
}

function FieldGroup({ title, children }: { title: string; children: React.ReactNode }) {
    return (
        <div>
            <h3 className="text-xs font-semibold uppercase tracking-[0.08em] text-helios-slate">{title}</h3>
            <div className="mt-3 grid gap-4 md:grid-cols-2">{children}</div>
        </div>
    );
}

function InfoRow({ label, value }: { label: string; value: string }) {
    return (
        <div className="rounded-lg border border-helios-line/15 bg-helios-surface-soft/40 px-3 py-2">
            <div className="text-[11px] font-medium text-helios-slate">{label}</div>
            <div className="mt-0.5 truncate text-sm font-semibold text-helios-ink" title={value}>
                {value}
            </div>
        </div>
    );
}

function Stat({ label, value }: { label: string; value: string }) {
    return (
        <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
            <div className="text-xs text-helios-slate">{label}</div>
            <div className="mt-1 font-mono text-sm font-semibold text-helios-ink">{value}</div>
        </div>
    );
}

function ToggleControl({
    label,
    checked,
    onChange,
    disabled,
}: {
    label: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
    disabled?: boolean;
}) {
    return (
        <label className="flex min-h-[48px] items-center justify-between gap-3 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 text-sm font-semibold text-helios-ink">
            <span>{label}</span>
            <input
                type="checkbox"
                checked={checked}
                disabled={disabled}
                onChange={(event) => onChange(event.target.checked)}
                className="h-5 w-5 accent-helios-solar disabled:opacity-50"
            />
        </label>
    );
}

function SelectField({
    label,
    value,
    options,
    onChange,
    disabled,
}: {
    label: string;
    value: string;
    options: Array<{ value: string; label: string; disabled?: boolean }>;
    onChange: (value: string) => void;
    disabled?: boolean;
}) {
    return (
        <label className="block">
            <span className="mb-1 block text-xs font-medium text-helios-slate">{label}</span>
            <select
                value={value}
                onChange={(event) => onChange(event.target.value)}
                disabled={disabled}
                className="w-full rounded border border-helios-line/20 bg-helios-surface-soft p-2 text-sm text-helios-ink disabled:opacity-50"
            >
                {options.map((option) => (
                    <option key={option.value} value={option.value} disabled={option.disabled}>
                        {option.label}
                    </option>
                ))}
            </select>
        </label>
    );
}

function NumberField({
    label,
    value,
    onChange,
    disabled,
    step = "1",
}: {
    label: string;
    value: number;
    onChange: (value: number) => void;
    disabled?: boolean;
    step?: string;
}) {
    return (
        <label className="block">
            <span className="mb-1 block text-xs font-medium text-helios-slate">{label}</span>
            <input
                type="number"
                value={value}
                step={step}
                disabled={disabled}
                onChange={(event) => onChange(Number(event.target.value))}
                className="w-full rounded border border-helios-line/20 bg-helios-surface-soft p-2 text-sm text-helios-ink disabled:opacity-50"
            />
        </label>
    );
}

function updateSettings(
    setSettings: React.Dispatch<React.SetStateAction<ConversionSettings>>,
    patch: Partial<ConversionSettings>
) {
    setSettings((current) => ({ ...current, ...patch }));
}

function setMode(mode: string, setSettings: React.Dispatch<React.SetStateAction<ConversionSettings>>) {
    setSettings((current) => {
        if (mode === "remux") {
            return {
                ...current,
                remux_only: true,
                video: {
                    ...current.video,
                    codec: "copy",
                    mode: "crf",
                    value: null,
                    resolution: { mode: "original", width: null, height: null, scale_factor: null },
                    hdr_mode: "preserve",
                },
                audio: { ...current.audio, codec: "copy" },
                subtitles: { mode: "copy" },
            };
        }

        return {
            ...current,
            remux_only: false,
            video: {
                ...current.video,
                codec: current.video.codec === "copy" ? "hevc" : current.video.codec,
                mode: current.video.mode || "crf",
                value: current.video.value ?? QUALITY_VALUES.balanced,
                preset: current.video.preset ?? "medium",
            },
        };
    });
}

function setQualityPreset(
    quality: QualityKey,
    setSettings: React.Dispatch<React.SetStateAction<ConversionSettings>>
) {
    if (quality === "custom") return;
    setSettings((current) => ({
        ...current,
        remux_only: false,
        video: {
            ...current.video,
            codec: current.video.codec === "copy" ? "hevc" : current.video.codec,
            mode: "crf",
            value: QUALITY_VALUES[quality],
            preset: current.video.preset ?? "medium",
        },
    }));
}

function inferQualityKey(settings: ConversionSettings): QualityKey {
    if (settings.video.mode !== "crf") return "custom";
    const value = settings.video.value;
    if (value === QUALITY_VALUES.high) return "high";
    if (value === QUALITY_VALUES.balanced) return "balanced";
    if (value === QUALITY_VALUES.small) return "small";
    return "custom";
}

function buildSourceSummary(probe: MediaAnalysis | null, summary: PreviewSummary | null) {
    if (!probe) return null;
    const metadata = probe.metadata;
    const fileName = summary?.source.file_name ?? fileNameFromPath(metadata.path) ?? "Source file";
    return {
        fileName,
        container: summary?.source.container.toUpperCase() ?? metadata.container.toUpperCase(),
        videoCodec: summary ? formatCodec(summary.source.video_codec) : formatCodec(metadata.codec_name),
        resolution: summary?.source.resolution ?? `${metadata.width}x${metadata.height}`,
        dynamicRange: summary ? humanize(summary.source.dynamic_range) : humanize(metadata.dynamic_range),
        durationSecs: summary?.source.duration_secs ?? metadata.duration_secs,
        sizeBytes: summary?.source.size_bytes ?? metadata.size_bytes,
        audio: summary?.source.audio ? humanize(summary.source.audio) : sourceAudio(metadata),
        subtitleCount: summary?.source.subtitle_count ?? metadata.subtitle_streams.length,
    };
}

function fileNameFromPath(path?: string) {
    if (!path) return null;
    const normalized = path.replaceAll("\\", "/");
    const name = normalized.split("/").filter(Boolean).pop();
    return name ?? null;
}

function sourceAudio(metadata: MediaAnalysis["metadata"]) {
    if (metadata.audio_streams.length > 0) {
        const first = metadata.audio_streams[0];
        const channels = first.channels ? ` / ${first.channels}ch` : "";
        const suffix = metadata.audio_streams.length > 1 ? ` + ${metadata.audio_streams.length - 1} more` : "";
        return `${formatCodec(first.codec_name)}${channels}${suffix}`;
    }
    if (metadata.audio_codec) {
        const channels = metadata.audio_channels ? ` / ${metadata.audio_channels}ch` : "";
        return `${formatCodec(metadata.audio_codec)}${channels}`;
    }
    return "None";
}

function formatBytes(bytes: number) {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit += 1;
    }
    const precision = value >= 10 || unit === 0 ? 0 : 1;
    return `${value.toFixed(precision)} ${units[unit]}`;
}

function formatDuration(seconds: number) {
    if (!Number.isFinite(seconds) || seconds <= 0) return "--";
    const rounded = Math.round(seconds);
    const hours = Math.floor(rounded / 3600);
    const minutes = Math.floor((rounded % 3600) / 60);
    const secs = rounded % 60;
    if (hours > 0) return `${hours}h ${minutes}m`;
    if (minutes > 0) return `${minutes}m ${secs}s`;
    return `${secs}s`;
}

function formatCodec(value: string) {
    const normalized = value.toLowerCase();
    if (normalized === "h264") return "H.264";
    if (normalized === "h265" || normalized === "hevc") return "HEVC";
    if (normalized === "av1") return "AV1";
    if (normalized === "aac") return "AAC";
    if (normalized === "opus") return "Opus";
    if (normalized === "mp3") return "MP3";
    if (normalized === "copy") return "Copy";
    return humanize(value);
}

function humanize(value: string) {
    return value
        .replaceAll("_", " ")
        .replaceAll("/", " / ")
        .split(" ")
        .filter(Boolean)
        .map((part) => {
            if (part.length <= 3 && part.toUpperCase() === part) return part;
            return part.charAt(0).toUpperCase() + part.slice(1);
        })
        .join(" ");
}
