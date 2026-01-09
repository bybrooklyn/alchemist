import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
    ArrowRight,
    Settings,
    Cpu,
    FolderOpen,
    CheckCircle,
    AlertTriangle,
    Server,
    Save,
    User,
    Lock,
    Video,
    Info
} from 'lucide-react';
import clsx from 'clsx';

interface ConfigState {
    // Auth
    username: string;
    password: string;
    // Transcode
    size_reduction_threshold: number;
    min_file_size_mb: number;
    concurrent_jobs: number;
    output_codec: "av1" | "hevc";
    quality_profile: "quality" | "balanced" | "speed";
    // Hardware
    allow_cpu_encoding: boolean;
    // Scanner
    directories: string[];
}

export default function SetupWizard() {
    const [step, setStep] = useState(1);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    // Tooltip state
    const [activeTooltip, setActiveTooltip] = useState<string | null>(null);

    const [config, setConfig] = useState<ConfigState>({
        username: '',
        password: '',
        size_reduction_threshold: 0.3,
        min_file_size_mb: 100,
        concurrent_jobs: 2,
        output_codec: 'av1',
        quality_profile: 'balanced',
        directories: ['/media/movies'],
        allow_cpu_encoding: false
    });

    const [dirInput, setDirInput] = useState('');

    const handleNext = () => {
        if (step === 1 && (!config.username || !config.password)) {
            setError("Please fill in both username and password.");
            return;
        }
        setError(null);
        setStep(s => Math.min(s + 1, 5));
    };

    const handleBack = () => setStep(s => Math.max(s - 1, 1));

    const addDirectory = () => {
        if (dirInput && !config.directories.includes(dirInput)) {
            setConfig(prev => ({
                ...prev,
                directories: [...prev.directories, dirInput]
            }));
            setDirInput('');
        }
    };

    const removeDirectory = (dir: string) => {
        setConfig(prev => ({
            ...prev,
            directories: prev.directories.filter(d => d !== dir)
        }));
    };

    const handleSubmit = async () => {
        setLoading(true);
        setError(null);
        try {
            const res = await fetch('/api/setup/complete', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(config)
            });

            if (!res.ok) {
                const text = await res.text();
                throw new Error(text);
            }

            const data = await res.json();
            if (data.token) {
                // Save token
                localStorage.setItem('alchemist_token', data.token);
                // Also set basic auth for legacy (optional) or just rely on new check
            }

            setSuccess(true);

            // Auto redirect after short delay
            setTimeout(() => {
                window.location.reload();
            }, 1000);

        } catch (err: any) {
            setError(err.message || "Failed to save configuration");
            setLoading(false);
        }
    };

    // Total steps = 5
    // 1: Account
    // 2: Transcoding (Codec, Profile)
    // 3: Thresholds
    // 4: Hardware
    // 5: Review & Save

    // Combined Steps for brevity:
    // 1: Account
    // 2: Transcode Rules (Codec, Profile, Thresholds)
    // 3: Hardware & Directories
    // 4: Review

    // Let's stick to 4 logical steps but grouped differently?
    // User requested "username and password be the first step".

    // Step 1: Account
    // Step 2: Codec & Quality (New)
    // Step 3: Performance & Directories (Merged old 2/3)
    // Step 4: Review

    return (
        <div className="bg-helios-surface border border-helios-line/60 rounded-2xl overflow-hidden shadow-2xl max-w-2xl w-full mx-auto">
            {/* Header */}
            <div className="h-1 bg-helios-surface-soft w-full flex">
                <motion.div
                    className="bg-helios-solar h-full"
                    initial={{ width: 0 }}
                    animate={{ width: `${(step / 5) * 100}%` }}
                />
            </div>

            <div className="p-8">
                <header className="flex items-center gap-4 mb-8">
                    <div className="w-10 h-10 rounded-lg bg-helios-solar text-helios-main flex items-center justify-center font-bold text-xl">
                        A
                    </div>
                    <div>
                        <h1 className="text-xl font-bold text-helios-ink">Alchemist Setup</h1>
                        <p className="text-helios-slate text-sm">Configure your transcoding pipeline</p>
                    </div>
                </header>

                <AnimatePresence mode="wait">
                    {step === 1 && (
                        <motion.div
                            key="step1"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-6"
                        >
                            <h2 className="text-lg font-semibold text-helios-ink flex items-center gap-2">
                                <User size={20} className="text-helios-solar" />
                                Create Account
                            </h2>
                            <p className="text-sm text-helios-slate">
                                Secure your Alchemist dashboard with a username and password.
                            </p>

                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm font-medium text-helios-slate mb-2">Username</label>
                                    <div className="relative">
                                        <User className="absolute left-3 top-2.5 text-helios-slate opacity-50" size={18} />
                                        <input
                                            type="text"
                                            value={config.username}
                                            onChange={(e) => setConfig({ ...config, username: e.target.value })}
                                            className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-lg pl-10 pr-3 py-2 text-helios-ink focus:border-helios-solar outline-none"
                                            placeholder="admin"
                                        />
                                    </div>
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-helios-slate mb-2">Password</label>
                                    <div className="relative">
                                        <Lock className="absolute left-3 top-2.5 text-helios-slate opacity-50" size={18} />
                                        <input
                                            type="password"
                                            value={config.password}
                                            onChange={(e) => setConfig({ ...config, password: e.target.value })}
                                            className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-lg pl-10 pr-3 py-2 text-helios-ink focus:border-helios-solar outline-none"
                                            placeholder="••••••••"
                                        />
                                    </div>
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 2 && (
                        <motion.div
                            key="step2"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-6"
                        >
                            <h2 className="text-lg font-semibold text-helios-ink flex items-center gap-2">
                                <Video size={20} className="text-helios-solar" />
                                Transcoding Preferences
                            </h2>

                            <div className="space-y-6">
                                {/* Codec Selector */}
                                <div className="space-y-3">
                                    <div className="flex items-center gap-2">
                                        <label className="text-sm font-bold uppercase tracking-wider text-helios-slate">Output Codec</label>
                                        <div className="relative group">
                                            <Info size={14} className="text-helios-slate cursor-help hover:text-helios-solar transition-colors" />
                                            <div className="absolute left-full ml-2 top-1/2 -translate-y-1/2 w-48 p-2 bg-helios-ink text-helios-main text-[10px] rounded-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10">
                                                Determines the video format for processed files. AV1 provides better compression but requires newer hardware.
                                            </div>
                                        </div>
                                    </div>

                                    <div className="grid grid-cols-2 gap-4">
                                        <button
                                            onClick={() => setConfig({ ...config, output_codec: "av1" })}
                                            className={clsx(
                                                "flex flex-col items-center gap-2 p-4 rounded-xl border transition-all relative group",
                                                config.output_codec === "av1"
                                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm"
                                                    : "bg-helios-surface-soft border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft/80"
                                            )}
                                        >
                                            <span className="font-bold">AV1</span>
                                            <span className="text-xs text-center opacity-70">Best compression. Requires Arc/RTX 4000+.</span>
                                            {/* Hover Tooltip */}
                                            <div className="absolute inset-0 bg-helios-ink/90 text-helios-main p-4 rounded-xl opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center text-xs text-center">
                                                Excellent efficiency. Ideal for Intel Arc GPUs.
                                            </div>
                                        </button>
                                        <button
                                            onClick={() => setConfig({ ...config, output_codec: "hevc" })}
                                            className={clsx(
                                                "flex flex-col items-center gap-2 p-4 rounded-xl border transition-all relative group",
                                                config.output_codec === "hevc"
                                                    ? "bg-helios-solar/10 border-helios-solar text-helios-ink shadow-sm"
                                                    : "bg-helios-surface-soft border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft/80"
                                            )}
                                        >
                                            <span className="font-bold">HEVC</span>
                                            <span className="text-xs text-center opacity-70">Broad compatibility. Fast encoding.</span>
                                            {/* Hover Tooltip */}
                                            <div className="absolute inset-0 bg-helios-ink/90 text-helios-main p-4 rounded-xl opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center text-xs text-center">
                                                Standard H.265. Compatible with most modern devices.
                                            </div>
                                        </button>
                                    </div>
                                </div>

                                {/* Quality Profile */}
                                <div className="space-y-3">
                                    <label className="text-sm font-bold uppercase tracking-wider text-helios-slate">Quality Profile</label>
                                    <div className="grid grid-cols-3 gap-3">
                                        {(["speed", "balanced", "quality"] as const).map((profile) => (
                                            <button
                                                key={profile}
                                                onClick={() => setConfig({ ...config, quality_profile: profile })}
                                                className={clsx(
                                                    "p-3 rounded-lg border capitalize transition-all",
                                                    config.quality_profile === profile
                                                        ? "bg-helios-solar/10 border-helios-solar text-helios-ink font-bold"
                                                        : "bg-helios-surface-soft border-helios-line/30 text-helios-slate"
                                                )}
                                            >
                                                {profile}
                                            </button>
                                        ))}
                                    </div>
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 3 && (
                        <motion.div
                            key="step3"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-6"
                        >
                            <h2 className="text-lg font-semibold text-helios-ink flex items-center gap-2">
                                <Settings size={20} className="text-helios-solar" />
                                Processing Rules
                            </h2>

                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm font-medium text-helios-slate mb-2">
                                        Size Reduction Threshold ({Math.round(config.size_reduction_threshold * 100)}%)
                                    </label>
                                    <input
                                        type="range"
                                        min="0.1"
                                        max="0.9"
                                        step="0.05"
                                        value={config.size_reduction_threshold}
                                        onChange={(e) => setConfig({ ...config, size_reduction_threshold: parseFloat(e.target.value) })}
                                        className="w-full accent-helios-solar h-2 bg-helios-surface-soft rounded-lg appearance-none cursor-pointer"
                                    />
                                    <p className="text-xs text-helios-slate mt-1">Files will be reverted if they don't shrink by this much.</p>
                                </div>

                                <div>
                                    <label className="block text-sm font-medium text-helios-slate mb-2">
                                        Minimum File Size ({config.min_file_size_mb} MB)
                                    </label>
                                    <input
                                        type="number"
                                        value={config.min_file_size_mb}
                                        onChange={(e) => setConfig({ ...config, min_file_size_mb: parseInt(e.target.value) })}
                                        className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-lg px-3 py-2 text-helios-ink focus:border-helios-solar outline-none"
                                    />
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 4 && (
                        <motion.div
                            key="step4"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-6"
                        >
                            <h2 className="text-lg font-semibold text-helios-ink flex items-center gap-2">
                                <Cpu size={20} className="text-helios-solar" />
                                Environment
                            </h2>

                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm font-medium text-helios-slate mb-2">
                                        Concurrent Jobs ({config.concurrent_jobs})
                                    </label>
                                    <input
                                        type="number"
                                        min="1"
                                        max="16"
                                        value={config.concurrent_jobs}
                                        onChange={(e) => setConfig({ ...config, concurrent_jobs: parseInt(e.target.value) })}
                                        className="w-full bg-helios-surface-soft border border-helios-line/40 rounded-lg px-3 py-2 text-helios-ink focus:border-helios-solar outline-none"
                                    />
                                </div>

                                <div className="space-y-4 pt-4 border-t border-helios-line/20">
                                    <label className="block text-sm font-medium text-helios-slate mb-2">Media Directories</label>
                                    <div className="flex gap-2">
                                        <input
                                            type="text"
                                            placeholder="/path/to/media"
                                            value={dirInput}
                                            onChange={(e) => setDirInput(e.target.value)}
                                            onKeyDown={(e) => e.key === 'Enter' && addDirectory()}
                                            className="flex-1 bg-helios-surface-soft border border-helios-line/40 rounded-lg px-3 py-2 text-helios-ink focus:border-helios-solar outline-none font-mono text-sm"
                                        />
                                        <button
                                            onClick={addDirectory}
                                            className="px-4 py-2 bg-helios-surface-soft hover:bg-helios-surface-soft/80 text-helios-ink rounded-lg font-medium transition-colors"
                                        >
                                            Add
                                        </button>
                                    </div>
                                    <div className="space-y-2 max-h-32 overflow-y-auto">
                                        {config.directories.map((dir, i) => (
                                            <div key={i} className="flex items-center justify-between p-2 rounded-lg bg-helios-surface-soft/50 border border-helios-line/20 group">
                                                <span className="font-mono text-xs text-helios-ink truncate">{dir}</span>
                                                <button onClick={() => removeDirectory(dir)} className="text-status-error hover:text-status-error/80">×</button>
                                            </div>
                                        ))}
                                    </div>
                                </div>
                            </div>
                        </motion.div>
                    )}

                    {step === 5 && (
                        <motion.div
                            key="step5"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-6"
                        >
                            <h2 className="text-lg font-semibold text-helios-ink flex items-center gap-2">
                                <Server size={20} className="text-helios-solar" />
                                Review & Save
                            </h2>

                            <div className="space-y-4 text-sm text-helios-slate">
                                <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/40 space-y-3">
                                    <div className="flex justify-between">
                                        <span>User</span>
                                        <span className="text-helios-ink font-bold">{config.username}</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>Codec</span>
                                        <span className="text-helios-ink font-mono uppercase">{config.output_codec}</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>Profile</span>
                                        <span className="text-helios-ink capitalize">{config.quality_profile}</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>Concurrency</span>
                                        <span className="text-helios-ink font-mono">{config.concurrent_jobs} jobs</span>
                                    </div>
                                    <div className="pt-2 border-t border-helios-line/20">
                                        <span className="block mb-1">Directories ({config.directories.length}):</span>
                                        <div className="flex flex-wrap gap-1">
                                            {config.directories.map(d => (
                                                <span key={d} className="px-1.5 py-0.5 bg-helios-surface border border-helios-line/30 rounded text-xs font-mono">{d}</span>
                                            ))}
                                        </div>
                                    </div>
                                </div>

                                {error && (
                                    <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error flex items-center gap-2">
                                        <AlertTriangle size={16} />
                                        <span>{error}</span>
                                    </div>
                                )}
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>

                <div className="mt-8 flex justify-between pt-6 border-t border-helios-line/40">
                    <button
                        onClick={handleBack}
                        disabled={step === 1}
                        className={clsx(
                            "px-4 py-2 rounded-lg font-medium transition-colors",
                            step === 1 ? "text-helios-line cursor-not-allowed" : "text-helios-slate hover:text-helios-ink"
                        )}
                    >
                        Back
                    </button>

                    {step < 5 ? (
                        <button
                            onClick={handleNext}
                            className="flex items-center gap-2 px-6 py-2 rounded-lg bg-helios-solar text-helios-main font-semibold hover:opacity-90 transition-opacity"
                        >
                            Next
                            <ArrowRight size={18} />
                        </button>
                    ) : (
                        <button
                            onClick={handleSubmit}
                            disabled={loading || success}
                            className="flex items-center gap-2 px-6 py-2 rounded-lg bg-status-success text-white font-semibold hover:opacity-90 transition-opacity disabled:opacity-50"
                        >
                            {loading ? "Activating..." : success ? "Redirecting..." : "Launch Alchemist"}
                            {!loading && !success && <Save size={18} />}
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
