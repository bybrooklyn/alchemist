import { useState, useEffect } from "react";
import {
    Cpu,
    Save,
    Video,
    Gauge,
    Zap,
    Scale,
    Film
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface TranscodeSettingsPayload {
    concurrent_jobs: number;
    size_reduction_threshold: number;
    min_bpp_threshold: number;
    min_file_size_mb: number;
    output_codec: "av1" | "hevc" | "h264";
    quality_profile: "quality" | "balanced" | "speed";
    threads: number;
    allow_fallback: boolean;
    hdr_mode: "preserve" | "tonemap";
    tonemap_algorithm: "hable" | "mobius" | "reinhard" | "clip";
    tonemap_peak: number;
    tonemap_desat: number;
    subtitle_mode: "copy" | "burn" | "extract" | "none";
    stream_rules: {
        strip_audio_by_title: string[];
        keep_audio_languages: string[];
        keep_only_default_audio: boolean;
    };
}

const defaultStreamRules: TranscodeSettingsPayload["stream_rules"] = {
    strip_audio_by_title: [],
    keep_audio_languages: [],
    keep_only_default_audio: false,
};

const commentaryShortcutKeywords = ["commentary", "director"];

function parseCommaSeparatedList(value: string): string[] {
    return value
        .split(",")
        .map((entry) => entry.trim())
        .filter((entry) => entry.length > 0);
}

function CommaSeparatedInput({ 
    value, 
    onChange, 
    placeholder,
    className 
}: { 
    value: string[]; 
    onChange: (val: string[]) => void; 
    placeholder?: string;
    className?: string;
}) {
    const [draft, setDraft] = useState(value.join(", "));
    
    // Sync draft if external value changes completely
    useEffect(() => {
        const currentParsed = parseCommaSeparatedList(draft);
        if (JSON.stringify(currentParsed) !== JSON.stringify(value)) {
            setDraft(value.join(", "));
        }
    }, [value]);

    return (
        <input
            type="text"
            value={draft}
            onChange={(e) => {
                const newVal = e.target.value;
                setDraft(newVal);
                onChange(parseCommaSeparatedList(newVal));
            }}
            placeholder={placeholder}
            className={cn(
                "w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all",
                className
            )}
        />
    );
}

export default function TranscodeSettings() {
    const [settings, setSettings] = useState<TranscodeSettingsPayload | null>(null);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");
    const [success, setSuccess] = useState(false);

    useEffect(() => {
        void fetchSettings();
    }, []);

    const fetchSettings = async () => {
        try {
            const data = await apiJson<TranscodeSettingsPayload>("/api/settings/transcode");
            setSettings({
                ...data,
                stream_rules: data.stream_rules ?? defaultStreamRules,
            });
            setError("");
        } catch (err) {
            setError(isApiError(err) ? err.message : "Unable to load current settings.");
        } finally {
            setLoading(false);
        }
    };

    const handleSave = async () => {
        if (!settings) return;
        setSaving(true);
        setError("");
        setSuccess(false);

        try {
            await apiAction("/api/settings/transcode", {
                method: "POST",
                body: JSON.stringify(settings),
            });
            setSuccess(true);
            showToast({ kind: "success", title: "Transcoding", message: "Transcode settings saved." });
            setTimeout(() => setSuccess(false), 3000);
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to save settings.";
            setError(message);
            showToast({ kind: "error", title: "Transcoding", message });
        } finally {
            setSaving(false);
        }
    };

    if (loading) {
        return <div className="p-8 text-helios-slate animate-pulse">Loading settings...</div>;
    }

    if (!settings) {
        return <div className="p-8 text-red-500">Failed to load settings.</div>;
    }

    const commentaryShortcutEnabled = commentaryShortcutKeywords.every((keyword) =>
        settings.stream_rules.strip_audio_by_title.some(
            (entry) => entry.trim().toLowerCase() === keyword
        )
    );

    const updateStreamRules = (
        updates: Partial<TranscodeSettingsPayload["stream_rules"]>
    ) => {
        setSettings({
            ...settings,
            stream_rules: {
                ...settings.stream_rules,
                ...updates,
            },
        });
    };

    return (
        <div className="flex flex-col gap-6">
            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight">Transcoding Engine</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Configure encoder behavior and performance limits.</p>
                </div>
                <div className="p-2 bg-helios-solar/10 rounded-lg text-helios-solar">
                    <Cpu size={20} />
                </div>
            </div>

            {error && (
                <div className="p-4 bg-red-500/10 border border-red-500/20 text-red-500 rounded-lg text-sm font-semibold">
                    {error}
                </div>
            )}

            {success && (
                <div className="p-4 bg-green-500/10 border border-green-500/20 text-green-500 rounded-lg text-sm font-semibold">
                    Settings saved successfully.
                </div>
            )}

            <div className="grid gap-6 md:grid-cols-2">
                {/* Codec Selection */}
                <div className="md:col-span-2 space-y-3">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Video size={14} /> Preferred Codec
                    </label>
                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                        <button
                            onClick={() => setSettings({ ...settings, output_codec: "av1" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-lg border transition-all",
                                settings.output_codec === "av1"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-lg">AV1</span>
                            <span className="text-xs text-center opacity-70">Best compression, depends on encoder availability.</span>
                        </button>
                        <button
                            onClick={() => setSettings({ ...settings, output_codec: "hevc" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-lg border transition-all",
                                settings.output_codec === "hevc"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-lg">HEVC (H.265)</span>
                            <span className="text-xs text-center opacity-70">Broad compatibility, faster hardware encoding.</span>
                        </button>
                        <button
                            onClick={() => setSettings({ ...settings, output_codec: "h264" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-lg border transition-all",
                                settings.output_codec === "h264"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-lg">H.264</span>
                            <span className="text-xs text-center opacity-70">Maximum compatibility, larger files.</span>
                        </button>
                    </div>
                </div>

                {/* Quality Profile */}
                <div className="md:col-span-2 space-y-3 pt-4">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Gauge size={14} /> Quality Profile
                    </label>
                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                        {(["speed", "balanced", "quality"] as const).map((profile) => (
                            <button
                                key={profile}
                                onClick={() => setSettings({ ...settings, quality_profile: profile })}
                                className={cn(
                                    "flex flex-col items-center justify-center p-3 rounded-md border transition-all h-20",
                                    settings.quality_profile === profile
                                        ? "bg-helios-solar/10 border-helios-solar text-helios-ink font-bold shadow-sm"
                                        : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                                )}
                            >
                                <span className="capitalize">{profile}</span>
                            </button>
                        ))}
                    </div>
                </div>

                <div className="md:col-span-2 flex items-center justify-between rounded-lg border border-helios-line/20 bg-helios-surface-soft/60 p-4">
                    <div>
                        <p className="text-xs font-medium text-helios-slate">Allow Fallback</p>
                        <p className="text-xs text-helios-slate mt-1">If preferred codec is unavailable, use the best available fallback.</p>
                    </div>
                    <div className="relative inline-flex items-center cursor-pointer">
                        <input
                            id="fallback-toggle"
                            type="checkbox"
                            checked={settings.allow_fallback}
                            onChange={(e) => setSettings({ ...settings, allow_fallback: e.target.checked })}
                            className="sr-only peer"
                        />
                        <div className="w-10 h-5 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-helios-ink after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-helios-ink after:border-helios-line/30 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-helios-solar"></div>
                    </div>
                </div>

                <div className="md:col-span-2 space-y-3 pt-2">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Film size={14} /> Subtitle Handling
                    </label>
                    <select
                        value={settings.subtitle_mode}
                        onChange={(e) => setSettings({ ...settings, subtitle_mode: e.target.value as TranscodeSettingsPayload["subtitle_mode"] })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    >
                        <option value="copy">Copy subtitles</option>
                        <option value="none">Drop subtitles</option>
                        <option value="burn">Burn into video</option>
                        <option value="extract">Extract sidecar subtitles</option>
                    </select>
                    {settings.subtitle_mode === "extract" && (
                        <p className="text-xs text-helios-slate leading-relaxed mt-1">
                            Text-based subtitle tracks (SRT, ASS, VTT) will be extracted as separate files alongside the output.
                            Image-based subtitles (Blu-ray PGS, DVD) cannot be extracted and will be dropped.
                        </p>
                    )}
                </div>

                <div className="md:col-span-2 space-y-4 pt-2">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Film size={14} /> Stream Rules
                    </label>

                    <div className="flex items-center justify-between rounded-lg border border-helios-line/20 bg-helios-surface-soft/60 p-4">
                        <div>
                            <p className="text-xs font-medium text-helios-slate">Strip commentary tracks</p>
                            <p className="text-xs text-helios-slate mt-1">Adds built-in title keywords for common commentary tracks.</p>
                        </div>
                        <div className="relative inline-flex items-center cursor-pointer">
                            <input
                                id="commentary-toggle"
                                type="checkbox"
                                checked={commentaryShortcutEnabled}
                                onChange={(e) => {
                                    const checked = e.target.checked;
                                    if (checked) {
                                        const nextKeywords = [...settings.stream_rules.strip_audio_by_title];
                                        for (const keyword of commentaryShortcutKeywords) {
                                            if (
                                                !nextKeywords.some(
                                                    (entry) => entry.trim().toLowerCase() === keyword
                                                )
                                            ) {
                                                nextKeywords.push(keyword);
                                            }
                                        }
                                        updateStreamRules({ strip_audio_by_title: nextKeywords });
                                        return;
                                    }

                                    updateStreamRules({
                                        strip_audio_by_title:
                                            settings.stream_rules.strip_audio_by_title.filter(
                                                (entry) =>
                                                    !commentaryShortcutKeywords.includes(
                                                        entry.trim().toLowerCase()
                                                    )
                                            ),
                                    });
                                }}
                                className="sr-only peer"
                            />
                            <div className="w-10 h-5 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-helios-ink after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-helios-ink after:border-helios-line/30 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-helios-solar"></div>
                        </div>
                    </div>

                    <div className="space-y-3">
                        <label className="text-xs font-medium text-helios-slate">
                            Strip Audio Tracks By Title Keyword
                        </label>
                        <input
                            type="text"
                            value={settings.stream_rules.strip_audio_by_title.join(", ")}
                            onChange={(e) =>
                                updateStreamRules({
                                    strip_audio_by_title: parseCommaSeparatedList(e.target.value),
                                })
                            }
                            placeholder="commentary, director's commentary"
                            className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                        />
                        <p className="text-xs text-helios-slate ml-1">
                            Audio tracks whose title contains any of these words will be removed. Separate multiple keywords with commas.
                        </p>
                    </div>

                    <div className="space-y-3">
                        <label className="text-xs font-medium text-helios-slate">
                            Keep Only These Audio Languages
                        </label>
                        <CommaSeparatedInput
                            value={settings.stream_rules.keep_audio_languages}
                            onChange={(val) =>
                                updateStreamRules({
                                    keep_audio_languages: val,
                                })
                            }
                            placeholder="eng, jpn"
                        />
                        <p className="text-xs text-helios-slate ml-1">
                            Only keep audio tracks matching these language codes. Tracks with no language tag are always kept. Leave blank to keep all languages.
                        </p>
                    </div>

                    <div className="flex items-center justify-between rounded-lg border border-helios-line/20 bg-helios-surface-soft/60 p-4">
                        <div>
                            <p className="text-xs font-medium text-helios-slate">Keep only default audio track</p>
                            <p className="text-xs text-helios-slate mt-1">Strip all audio tracks except the one marked as default by the source file.</p>
                        </div>
                        <div className="relative inline-flex items-center cursor-pointer">
                            <input
                                id="default-audio-toggle"
                                type="checkbox"
                                checked={settings.stream_rules.keep_only_default_audio}
                                onChange={(e) =>
                                    updateStreamRules({
                                        keep_only_default_audio: e.target.checked,
                                    })
                                }
                                className="sr-only peer"
                            />
                            <div className="w-10 h-5 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-helios-ink after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-helios-ink after:border-helios-line/30 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-helios-solar"></div>
                        </div>
                    </div>
                </div>

                {/* HDR + Tonemapping */}
                <div className="md:col-span-2 space-y-3 pt-2">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Film size={14} /> HDR Handling
                    </label>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <button
                            onClick={() => setSettings({ ...settings, hdr_mode: "preserve" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-lg border transition-all",
                                settings.hdr_mode === "preserve"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-sm">Preserve HDR</span>
                            <span className="text-xs text-center opacity-70">Keep HDR metadata and color space intact.</span>
                        </button>
                        <button
                            onClick={() => setSettings({ ...settings, hdr_mode: "tonemap" })}
                            className={cn(
                                "flex flex-col items-center gap-2 p-4 rounded-lg border transition-all",
                                settings.hdr_mode === "tonemap"
                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm ring-1 ring-helios-solar/20"
                                    : "bg-helios-surface border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                            )}
                        >
                            <span className="font-bold text-sm">Tonemap to SDR</span>
                            <span className="text-xs text-center opacity-70">Convert HDR to SDR for compatibility.</span>
                        </button>
                    </div>
                </div>

                {settings.hdr_mode === "tonemap" && (
                    <>
                        <div className="space-y-3">
                            <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                                <Gauge size={14} /> Tonemap Algorithm
                            </label>
                            <select
                                value={settings.tonemap_algorithm}
                                onChange={(e) => setSettings({ ...settings, tonemap_algorithm: e.target.value as TranscodeSettingsPayload["tonemap_algorithm"] })}
                                className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            >
                                <option value="hable">Hable</option>
                                <option value="mobius">Mobius</option>
                                <option value="reinhard">Reinhard</option>
                                <option value="clip">Clip</option>
                            </select>
                            <p className="text-xs text-helios-slate ml-1">Choose the tone curve for HDR → SDR conversion.</p>
                        </div>

                        <div className="space-y-3">
                            <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                                <Scale size={14} /> Tonemap Peak (nits)
                            </label>
                            <input
                                type="number"
                                min="50"
                                max="1000"
                                value={settings.tonemap_peak}
                                onChange={(e) => setSettings({ ...settings, tonemap_peak: parseFloat(e.target.value) || 100 })}
                                className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            />
                            <p className="text-xs text-helios-slate ml-1">Peak brightness used for tone mapping.</p>
                        </div>

                        <div className="space-y-3">
                            <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                                <Zap size={14} /> Tonemap Desaturation
                            </label>
                            <input
                                type="number"
                                min="0"
                                max="1"
                                step="0.1"
                                value={settings.tonemap_desat}
                                onChange={(e) => setSettings({ ...settings, tonemap_desat: parseFloat(e.target.value) || 0 })}
                                className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                            />
                            <p className="text-xs text-helios-slate ml-1">Reduce oversaturated highlights after tonemapping.</p>
                        </div>
                    </>
                )}

                {/* Numeric Inputs */}
                <div className="space-y-3">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Cpu size={14} /> Encoding Threads (libsvtav1/x265)
                    </label>
                    <input
                        type="number"
                        min="0"
                        value={settings.threads}
                        onChange={(e) => setSettings({ ...settings, threads: parseInt(e.target.value) || 0 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
                    <p className="text-xs text-helios-slate ml-1">Number of threads to allocate for software encoding (0 = Auto).</p>
                </div>

                <div className="space-y-3">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Zap size={14} /> Concurrent Jobs
                    </label>
                    <input
                        type="number"
                        min="1"
                        max="8"
                        value={settings.concurrent_jobs}
                        onChange={(e) => setSettings({ ...settings, concurrent_jobs: parseInt(e.target.value) || 1 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
                    <p className="text-xs text-helios-slate ml-1">Maximum number of files to process simultaneously.</p>
                </div>

                <div className="space-y-3">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Scale size={14} /> Min. Reduction (%)
                    </label>
                    <input
                        type="number"
                        min="0"
                        max="100"
                        step="5"
                        value={Math.round(settings.size_reduction_threshold * 100)}
                        onChange={(e) => setSettings({ ...settings, size_reduction_threshold: (parseInt(e.target.value) || 0) / 100 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
                    <p className="text-xs text-helios-slate ml-1">Files must shrink by at least this percentage or they are reverted.</p>
                </div>

                <div className="space-y-3">
                    <label className="text-xs font-medium text-helios-slate flex items-center gap-2">
                        <Film size={14} /> Min. File Size (MB)
                    </label>
                    <input
                        type="number"
                        min="0"
                        value={settings.min_file_size_mb}
                        onChange={(e) => setSettings({ ...settings, min_file_size_mb: parseInt(e.target.value) || 0 })}
                        className="w-full bg-helios-surface border border-helios-line/30 rounded-lg px-4 py-3 text-helios-ink focus:border-helios-solar focus:ring-1 focus:ring-helios-solar outline-none transition-all"
                    />
                </div>
            </div>

            <div className="flex justify-end pt-4 border-t border-helios-line/10">
                <button
                    onClick={handleSave}
                    disabled={saving}
                    className="flex items-center gap-2 bg-helios-solar text-helios-main font-bold px-6 py-3 rounded-md hover:opacity-90 transition-opacity disabled:opacity-50"
                >
                    <Save size={18} />
                    {saving ? "Saving..." : "Save Settings"}
                </button>
            </div>
        </div>
    );
}
