import { useEffect, useState } from "react";
import { Upload, Wand2, Play, Download, Trash2 } from "lucide-react";
import { apiAction, apiFetch, apiJson, isApiError } from "../lib/api";
import { withBasePath } from "../lib/basePath";
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
        container: string;
        codec_name: string;
        width: number;
        height: number;
        dynamic_range: string;
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

interface PreviewResponse {
    normalized_settings: ConversionSettings;
    command_preview: string;
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

export default function ConversionTool() {
    const [uploading, setUploading] = useState(false);
    const [previewing, setPreviewing] = useState(false);
    const [starting, setStarting] = useState(false);
    const [status, setStatus] = useState<JobStatusResponse | null>(null);
    const [conversionJobId, setConversionJobId] = useState<number | null>(null);
    const [probe, setProbe] = useState<MediaAnalysis | null>(null);
    const [settings, setSettings] = useState<ConversionSettings>(DEFAULT_SETTINGS);
    const [commandPreview, setCommandPreview] = useState("");
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (!conversionJobId) return;
        const id = window.setInterval(() => {
            void apiJson<JobStatusResponse>(`/api/conversion/jobs/${conversionJobId}`)
                .then(setStatus)
                .catch(() => {});
        }, 2000);
        return () => window.clearInterval(id);
    }, [conversionJobId]);

    const updateSettings = (patch: Partial<ConversionSettings>) => {
        setSettings((current) => ({ ...current, ...patch }));
    };

    const uploadFile = async (file: File) => {
        setUploading(true);
        setError(null);
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

    const preview = async () => {
        if (!conversionJobId) return;
        setPreviewing(true);
        try {
            const payload = await apiJson<PreviewResponse>("/api/conversion/preview", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    conversion_job_id: conversionJobId,
                    settings,
                }),
            });
            setSettings(payload.normalized_settings);
            setCommandPreview(payload.command_preview);
            showToast({ kind: "success", title: "Conversion", message: "Preview updated." });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Preview failed";
            setError(message);
            showToast({ kind: "error", title: "Conversion", message });
        } finally {
            setPreviewing(false);
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
            showToast({ kind: "success", title: "Conversion", message: "Conversion job removed." });
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to remove conversion job";
            setError(message);
            showToast({ kind: "error", title: "Conversion", message });
        }
    };

    const download = async () => {
        if (!conversionJobId) return;
        window.location.href = withBasePath(`/api/conversion/jobs/${conversionJobId}/download`);
    };

    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-xl font-bold text-helios-ink">Conversion / Remux</h1>
                <p className="mt-1 text-sm text-helios-slate">
                    Upload a single file, inspect the streams, preview the generated FFmpeg command, and run it through Alchemist.
                </p>
            </div>

            {error && (
                <div className="rounded-lg border border-status-error/20 bg-status-error/10 px-4 py-3 text-sm text-status-error">
                    {error}
                </div>
            )}

            {!probe && (
                <label className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-helios-line/30 bg-helios-surface p-10 text-center cursor-pointer hover:bg-helios-surface-soft transition-colors">
                    <Upload size={28} className="text-helios-solar" />
                    <div>
                        <p className="text-sm font-semibold text-helios-ink">Upload a source file</p>
                        <p className="text-xs text-helios-slate mt-1">The uploaded file is stored temporarily under Alchemist-managed temp storage.</p>
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

            {probe && (
                <>
                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <h2 className="text-sm font-semibold text-helios-ink">Input</h2>
                        <div className="grid gap-3 md:grid-cols-4 text-sm">
                            <Stat label="Container" value={probe.metadata.container} />
                            <Stat label="Video" value={probe.metadata.codec_name} />
                            <Stat label="Resolution" value={`${probe.metadata.width}x${probe.metadata.height}`} />
                            <Stat label="Dynamic Range" value={probe.metadata.dynamic_range} />
                        </div>
                    </section>

                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <h2 className="text-sm font-semibold text-helios-ink">Output Container</h2>
                        <select value={settings.output_container} onChange={(event) => updateSettings({ output_container: event.target.value })} className="w-full md:w-60 bg-helios-surface-soft border border-helios-line/20 rounded p-2 text-sm text-helios-ink">
                            {["mkv", "mp4", "webm", "mov"].map((option) => (
                                <option key={option} value={option}>{option.toUpperCase()}</option>
                            ))}
                        </select>
                    </section>

                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <div className="flex items-center justify-between gap-4">
                            <h2 className="text-sm font-semibold text-helios-ink">Remux Mode</h2>
                            <label className="flex items-center gap-2 text-sm text-helios-ink">
                                <input
                                    type="checkbox"
                                    checked={settings.remux_only}
                                    onChange={(event) => updateSettings({ remux_only: event.target.checked })}
                                />
                                Remux only
                            </label>
                        </div>
                        <p className="text-xs text-helios-slate">
                            Remux mode forces stream copy and disables re-encoding controls.
                        </p>
                    </section>

                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <h2 className="text-sm font-semibold text-helios-ink">Video</h2>
                        <div className="grid gap-4 md:grid-cols-2">
                            <SelectField
                                label="Codec"
                                value={settings.video.codec}
                                disabled={settings.remux_only}
                                options={["copy", "h264", "hevc", "av1"]}
                                onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, codec: value } }))}
                            />
                            <SelectField
                                label="Mode"
                                value={settings.video.mode}
                                disabled={settings.remux_only || settings.video.codec === "copy"}
                                options={["crf", "bitrate"]}
                                onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, mode: value } }))}
                            />
                            <NumberField
                                label={settings.video.mode === "bitrate" ? "Bitrate (kbps)" : "Quality Value"}
                                value={settings.video.value ?? 0}
                                disabled={settings.remux_only || settings.video.codec === "copy"}
                                onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, value } }))}
                            />
                            <SelectField
                                label="Preset"
                                value={settings.video.preset ?? "medium"}
                                disabled={settings.remux_only || settings.video.codec === "copy"}
                                options={["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"]}
                                onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, preset: value } }))}
                            />
                            <SelectField
                                label="Resolution Mode"
                                value={settings.video.resolution.mode}
                                disabled={settings.remux_only || settings.video.codec === "copy"}
                                options={["original", "custom", "scale_factor"]}
                                onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, resolution: { ...current.video.resolution, mode: value } } }))}
                            />
                            <SelectField
                                label="HDR"
                                value={settings.video.hdr_mode}
                                disabled={settings.remux_only || settings.video.codec === "copy"}
                                options={["preserve", "tonemap", "strip_metadata"]}
                                onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, hdr_mode: value } }))}
                            />
                            {settings.video.resolution.mode === "custom" && (
                                <>
                                    <NumberField
                                        label="Width"
                                        value={settings.video.resolution.width ?? probe.metadata.width}
                                        disabled={settings.remux_only || settings.video.codec === "copy"}
                                        onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, resolution: { ...current.video.resolution, width: value } } }))}
                                    />
                                    <NumberField
                                        label="Height"
                                        value={settings.video.resolution.height ?? probe.metadata.height}
                                        disabled={settings.remux_only || settings.video.codec === "copy"}
                                        onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, resolution: { ...current.video.resolution, height: value } } }))}
                                    />
                                </>
                            )}
                            {settings.video.resolution.mode === "scale_factor" && (
                                <NumberField
                                    label="Scale Factor"
                                    value={settings.video.resolution.scale_factor ?? 1}
                                    disabled={settings.remux_only || settings.video.codec === "copy"}
                                    step="0.1"
                                    onChange={(value) => setSettings((current) => ({ ...current, video: { ...current.video, resolution: { ...current.video.resolution, scale_factor: value } } }))}
                                />
                            )}
                        </div>
                    </section>

                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <h2 className="text-sm font-semibold text-helios-ink">Audio</h2>
                        <div className="grid gap-4 md:grid-cols-3">
                            <SelectField
                                label="Codec"
                                value={settings.audio.codec}
                                disabled={settings.remux_only}
                                options={["copy", "aac", "opus", "mp3"]}
                                onChange={(value) => setSettings((current) => ({ ...current, audio: { ...current.audio, codec: value } }))}
                            />
                            <NumberField
                                label="Bitrate (kbps)"
                                value={settings.audio.bitrate_kbps ?? 160}
                                disabled={settings.remux_only || settings.audio.codec === "copy"}
                                onChange={(value) => setSettings((current) => ({ ...current, audio: { ...current.audio, bitrate_kbps: value } }))}
                            />
                            <SelectField
                                label="Channels"
                                value={settings.audio.channels ?? "auto"}
                                disabled={settings.remux_only || settings.audio.codec === "copy"}
                                options={["auto", "stereo", "5.1"]}
                                onChange={(value) => setSettings((current) => ({ ...current, audio: { ...current.audio, channels: value } }))}
                            />
                        </div>
                    </section>

                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <h2 className="text-sm font-semibold text-helios-ink">Subtitles</h2>
                        <SelectField
                            label="Mode"
                            value={settings.subtitles.mode}
                            disabled={settings.remux_only}
                            options={["copy", "burn", "remove"]}
                            onChange={(value) => setSettings((current) => ({ ...current, subtitles: { mode: value } }))}
                        />
                    </section>

                    <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <div className="flex flex-wrap gap-3">
                            <button onClick={() => void preview()} disabled={previewing} className="flex items-center gap-2 rounded-lg bg-helios-solar px-4 py-2 text-sm font-bold text-helios-main">
                                <Wand2 size={16} />
                                {previewing ? "Previewing..." : "Preview Command"}
                            </button>
                            <button onClick={() => void start()} disabled={starting || !commandPreview} className="flex items-center gap-2 rounded-lg border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink">
                                <Play size={16} />
                                {starting ? "Starting..." : "Start Job"}
                            </button>
                            <button onClick={() => void download()} disabled={!status?.download_ready} className="flex items-center gap-2 rounded-lg border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink disabled:opacity-50">
                                <Download size={16} />
                                Download Result
                            </button>
                            <button onClick={() => void remove()} className="flex items-center gap-2 rounded-lg border border-red-500/20 px-4 py-2 text-sm font-semibold text-red-500">
                                <Trash2 size={16} />
                                Remove
                            </button>
                        </div>
                        {commandPreview && (
                            <pre className="overflow-x-auto rounded-lg border border-helios-line/20 bg-helios-surface-soft p-4 text-xs text-helios-ink whitespace-pre-wrap">
                                {commandPreview}
                            </pre>
                        )}
                    </section>

                    {status && (
                        <section className="rounded-xl border border-helios-line/20 bg-helios-surface p-5 space-y-3">
                            <h2 className="text-sm font-semibold text-helios-ink">Status</h2>
                            <div className="grid gap-3 md:grid-cols-4 text-sm">
                                <Stat label="State" value={status.status} />
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

function Stat({ label, value }: { label: string; value: string }) {
    return (
        <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
            <div className="text-xs text-helios-slate">{label}</div>
            <div className="mt-1 font-mono text-sm font-semibold text-helios-ink">{value}</div>
        </div>
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
    options: string[];
    onChange: (value: string) => void;
    disabled?: boolean;
}) {
    return (
        <div>
            <label className="block text-xs font-medium text-helios-slate mb-1">{label}</label>
            <select
                value={value}
                onChange={(event) => onChange(event.target.value)}
                disabled={disabled}
                className="w-full bg-helios-surface-soft border border-helios-line/20 rounded p-2 text-sm text-helios-ink disabled:opacity-50"
            >
                {options.map((option) => (
                    <option key={option} value={option}>
                        {option}
                    </option>
                ))}
            </select>
        </div>
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
        <div>
            <label className="block text-xs font-medium text-helios-slate mb-1">{label}</label>
            <input
                type="number"
                value={value}
                step={step}
                disabled={disabled}
                onChange={(event) => onChange(Number(event.target.value))}
                className="w-full bg-helios-surface-soft border border-helios-line/20 rounded p-2 text-sm text-helios-ink disabled:opacity-50"
            />
        </div>
    );
}
