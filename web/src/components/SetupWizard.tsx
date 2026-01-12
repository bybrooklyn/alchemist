import React, { useEffect, useState } from 'react';
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
    // System
    enable_telemetry: boolean;
}

interface HardwareInfo {
    vendor: "Nvidia" | "Amd" | "Intel" | "Apple" | "Cpu";
    device_path: string | null;
    supported_codecs: string[];
}

interface ScanStatus {
    is_running: boolean;
    files_found: number;
    files_added: number;
    current_folder: string | null;
}

export default function SetupWizard() {
    const [step, setStep] = useState(1);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);
    const [hardware, setHardware] = useState<HardwareInfo | null>(null);
    const [scanStatus, setScanStatus] = useState<ScanStatus | null>(null);

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
        allow_cpu_encoding: false,
        enable_telemetry: true
    });

    const [dirInput, setDirInput] = useState('');

    const getAuthHeaders = () => {
        const token = localStorage.getItem('alchemist_token');
        return token ? { Authorization: `Bearer ${token}` } : {};
    };

    useEffect(() => {
        const loadSetupDefaults = async () => {
            try {
                const res = await fetch('/api/setup/status');
                if (!res.ok) return;
                const data = await res.json();
                if (typeof data.enable_telemetry === 'boolean') {
                    setConfig(prev => ({ ...prev, enable_telemetry: data.enable_telemetry }));
                }
            } catch (e) {
                console.error("Failed to load setup defaults", e);
            }
        };

        loadSetupDefaults();
    }, []);

    const handleNext = async () => {
        if (step === 1 && (!config.username || !config.password)) {
            setError("Please fill in both username and password.");
            return;
        }

        if (step === 2) {
            // Hardware step - fetch if not already
            if (!hardware) {
                setLoading(true);
                try {
                    const res = await fetch('/api/system/hardware', {
                        headers: getAuthHeaders()
                    });
                    if (!res.ok) {
                        throw new Error(`Hardware detection failed (${res.status})`);
                    }
                    const data = await res.json();
                    setHardware(data);
                } catch (e) {
                    console.error("Hardware detection failed", e);
                } finally {
                    setLoading(false);
                }
            }
        }

        if (step === 4) {
            // Save & Start Scan step
            await handleSubmit();
            return;
        }

        setError(null);
        setStep(s => Math.min(s + 1, 6));
    };

    const handleBack = () => setStep(s => Math.max(s - 1, 1));

    const startScan = async () => {
        try {
            const res = await fetch('/api/scan/start', {
                method: 'POST',
                headers: getAuthHeaders()
            });
            if (!res.ok) {
                throw new Error(await res.text());
            }
            pollScanStatus();
        } catch (e) {
            console.error("Failed to start scan", e);
            setError("Failed to start scan. Please check authentication.");
        }
    };

    const pollScanStatus = async () => {
        const interval = setInterval(async () => {
            try {
                const res = await fetch('/api/scan/status', {
                    headers: getAuthHeaders()
                });
                if (!res.ok) {
                    throw new Error(await res.text());
                }
                const data = await res.json();
                setScanStatus(data);
                if (!data.is_running) {
                    clearInterval(interval);
                    setLoading(false);
                }
            } catch (e) {
                console.error("Polling failed", e);
                setError("Scan status unavailable. Please refresh and try again.");
                clearInterval(interval);
                setLoading(false);
            }
        }, 1000);
    };

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
                localStorage.setItem('alchemist_token', data.token);
            }

            setStep(5); // Move to Scan Progress
            startScan();

        } catch (err: any) {
            setError(err.message || "Failed to save configuration");
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="bg-helios-surface border border-helios-line/60 rounded-2xl overflow-hidden shadow-2xl max-w-2xl w-full mx-auto">
            {/* Header */}
            <div className="h-1 bg-helios-surface-soft w-full flex">
                <motion.div
                    className="bg-helios-solar h-full"
                    initial={{ width: 0 }}
                    animate={{ width: `${(step / 6) * 100}%` }}
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
                                Hardware & Rules
                            </h2>

                            <div className="space-y-6">
                                <div className="space-y-3">
                                    <label className="text-sm font-bold uppercase tracking-wider text-helios-slate">Transcoding Target</label>
                                    <div className="grid grid-cols-2 gap-4">
                                        <button
                                            onClick={() => setConfig({ ...config, output_codec: "av1" })}
                                            className={clsx(
                                                "flex flex-col items-center gap-2 p-4 rounded-xl border transition-all",
                                                config.output_codec === "av1" ? "bg-helios-solar/10 border-helios-solar text-helios-ink" : "bg-helios-surface-soft border-helios-line/30 text-helios-slate"
                                            )}
                                        >
                                            <span className="font-bold">AV1</span>
                                            <span className="text-[10px] opacity-70">Extreme compression. Needs modern GPU.</span>
                                        </button>
                                        <button
                                            onClick={() => setConfig({ ...config, output_codec: "hevc" })}
                                            className={clsx(
                                                "flex flex-col items-center gap-2 p-4 rounded-xl border transition-all",
                                                config.output_codec === "hevc" ? "bg-helios-solar/10 border-helios-solar text-helios-ink" : "bg-helios-surface-soft border-helios-line/30 text-helios-slate"
                                            )}
                                        >
                                            <span className="font-bold">HEVC</span>
                                            <span className="text-[10px] opacity-70">Broad support. High efficiency.</span>
                                        </button>
                                    </div>
                                </div>

                                <div className="space-y-3">
                                    <label className="text-sm font-bold uppercase tracking-wider text-helios-slate">Min Savings Threshold ({Math.round(config.size_reduction_threshold * 100)}%)</label>
                                    <input
                                        type="range" min="0.1" max="0.9" step="0.05"
                                        value={config.size_reduction_threshold}
                                        onChange={(e) => setConfig({ ...config, size_reduction_threshold: parseFloat(e.target.value) })}
                                        className="w-full accent-helios-solar h-2 bg-helios-surface-soft rounded-lg appearance-none cursor-pointer"
                                    />
                                    <p className="text-[10px] text-helios-slate italic">If encodes don't save at least {Math.round(config.size_reduction_threshold * 100)}%, they are discarded.</p>
                                </div>

                                <div className="space-y-3">
                                    <label className="text-sm font-bold uppercase tracking-wider text-helios-slate">Concurrent Jobs ({config.concurrent_jobs})</label>
                                    <input
                                        type="range" min="1" max="8" step="1"
                                        value={config.concurrent_jobs}
                                        onChange={(e) => setConfig({ ...config, concurrent_jobs: parseInt(e.target.value) })}
                                        className="w-full accent-helios-solar h-2 bg-helios-surface-soft rounded-lg appearance-none cursor-pointer"
                                    />
                                    <p className="text-[10px] text-helios-slate italic">How many files to process at the same time.</p>
                                </div>

                                <div className="pt-2 border-t border-helios-line/10">
                                    <label className="flex items-center justify-between group cursor-pointer">
                                        <div className="flex items-center gap-2">
                                            <span className="text-xs font-bold uppercase tracking-wider text-helios-slate">Anonymous Telemetry</span>
                                            <div className="relative">
                                                <button
                                                    onMouseEnter={() => setActiveTooltip('telemetry')}
                                                    onMouseLeave={() => setActiveTooltip(null)}
                                                    className="text-helios-slate/40 hover:text-helios-solar transition-colors"
                                                >
                                                    <Info size={14} />
                                                </button>
                                                <AnimatePresence>
                                                    {activeTooltip === 'telemetry' && (
                                                        <motion.div
                                                            initial={{ opacity: 0, y: 10 }}
                                                            animate={{ opacity: 1, y: 0 }}
                                                            exit={{ opacity: 0, y: 10 }}
                                                            className="absolute bottom-full left-0 mb-2 w-48 p-2 bg-helios-surface-soft border border-helios-line/40 rounded-lg shadow-xl text-[10px] text-helios-slate z-50 leading-relaxed"
                                                        >
                                                            Help improve Alchemist by sending anonymous usage statistics and error reports. No filenames or personal data are ever collected.
                                                        </motion.div>
                                                    )}
                                                </AnimatePresence>
                                            </div>
                                        </div>
                                        <div className="relative inline-flex items-center cursor-pointer">
                                            <input
                                                type="checkbox"
                                                checked={config.enable_telemetry}
                                                onChange={(e) => setConfig({ ...config, enable_telemetry: e.target.checked })}
                                                className="sr-only peer"
                                            />
                                            <div className="w-9 h-5 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-helios-solar"></div>
                                        </div>
                                    </label>
                                </div>

                                {hardware && (
                                    <div className="space-y-4">
                                        <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/40 flex items-center gap-3">
                                            <div className="p-2 bg-emerald-500/10 text-emerald-500 rounded-lg">
                                                <Cpu size={18} />
                                            </div>
                                            <div>
                                                <p className="text-xs font-bold text-helios-ink">Detected: {hardware.vendor} Hardware Acceleration</p>
                                                <p className="text-[10px] text-helios-slate">{hardware.supported_codecs.join(', ').toUpperCase()} Encoders Found</p>
                                            </div>
                                        </div>

                                        {/* CPU Encoding Fallback Toggle */}
                                        <label className="flex items-center justify-between group cursor-pointer p-4 rounded-xl border border-helios-line/20 hover:bg-helios-surface-soft/50 transition-colors">
                                            <div className="flex flex-col gap-1">
                                                <span className="text-sm font-bold text-helios-ink flex items-center gap-2">
                                                    Allow CPU Encoding
                                                    <span className="px-1.5 py-0.5 rounded text-[10px] bg-amber-500/10 text-amber-500 font-mono">SLOW</span>
                                                </span>
                                                <p className="text-xs text-helios-slate">Enable software encoding fallback if GPU is unavailable or fails.</p>
                                            </div>
                                            <div className="relative inline-flex items-center cursor-pointer">
                                                <input
                                                    type="checkbox"
                                                    checked={config.allow_cpu_encoding}
                                                    onChange={(e) => setConfig({ ...config, allow_cpu_encoding: e.target.checked })}
                                                    className="sr-only peer"
                                                />
                                                <div className="w-9 h-5 bg-helios-line/20 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-amber-500"></div>
                                            </div>
                                        </label>
                                    </div>
                                )}
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
                                <FolderOpen size={20} className="text-helios-solar" />
                                Watch Directories
                            </h2>
                            <p className="text-sm text-helios-slate">Add folders to monitor for new media files.</p>

                            <div className="space-y-4">
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        placeholder="/movies"
                                        value={dirInput}
                                        onChange={(e) => setDirInput(e.target.value)}
                                        onKeyDown={(e) => e.key === 'Enter' && addDirectory()}
                                        className="flex-1 bg-helios-surface-soft border border-helios-line/40 rounded-lg px-3 py-2 text-helios-ink focus:border-helios-solar outline-none font-mono text-sm"
                                    />
                                    <button onClick={addDirectory} className="px-4 py-2 bg-helios-solar text-helios-main rounded-lg font-bold">Add</button>
                                </div>
                                <div className="space-y-2 max-h-40 overflow-y-auto pr-1">
                                    {config.directories.map((dir, i) => (
                                        <div key={i} className="flex items-center justify-between p-3 rounded-lg bg-helios-surface-soft/50 border border-helios-line/20 group animate-in fade-in slide-in-from-right-2">
                                            <span className="font-mono text-xs text-helios-ink">{dir}</span>
                                            <button onClick={() => removeDirectory(dir)} className="text-red-500 opacity-50 hover:opacity-100 transition-opacity font-bold">×</button>
                                        </div>
                                    ))}
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
                                <CheckCircle size={20} className="text-helios-solar" />
                                Final Review
                            </h2>

                            <div className="grid grid-cols-2 gap-4">
                                <div className="p-4 rounded-xl bg-helios-surface-soft/50 border border-helios-line/20 text-xs text-helios-slate space-y-2">
                                    <p>ACCOUNT: <span className="text-helios-ink font-bold">{config.username}</span></p>
                                    <p>TARGET: <span className="text-helios-ink font-bold uppercase">{config.output_codec}</span></p>
                                    <p>CONCURRENCY: <span className="text-helios-ink font-bold">{config.concurrent_jobs} Jobs</span></p>
                                </div>
                                <div className="p-4 rounded-xl bg-helios-surface-soft/50 border border-helios-line/20 text-xs text-helios-slate space-y-2">
                                    <p>FOLDERS: <span className="text-helios-ink font-bold">{config.directories.length} total</span></p>
                                    <p>THRESHOLD: <span className="text-helios-ink font-bold">{Math.round(config.size_reduction_threshold * 100)}%</span></p>
                                </div>
                            </div>

                            {error && <p className="text-xs text-red-500 font-bold bg-red-500/10 p-2 rounded border border-red-500/20">{error}</p>}
                        </motion.div>
                    )}

                    {step === 5 && (
                        <motion.div
                            key="step5"
                            initial={{ opacity: 0, scale: 0.95 }}
                            animate={{ opacity: 1, scale: 1 }}
                            className="space-y-8 py-10 text-center"
                        >
                            <div className="flex justify-center">
                                <div className="relative">
                                    <div className="w-20 h-20 rounded-full border-4 border-helios-solar/20 border-t-helios-solar animate-spin" />
                                    <div className="absolute inset-0 flex items-center justify-center">
                                        <SearchIcon className="text-helios-solar" size={24} />
                                    </div>
                                </div>
                            </div>
                            <div>
                                <h2 className="text-xl font-bold text-helios-ink mb-2">Primary Library Scan</h2>
                                <p className="text-sm text-helios-slate">Building your transcoding queue. This might take a moment.</p>
                            </div>

                            {scanStatus && (
                                <div className="space-y-3">
                                    <div className="flex justify-between text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                                        <span>Found: {scanStatus.files_found}</span>
                                        <span>Added: {scanStatus.files_added}</span>
                                    </div>
                                    <div className="h-2 bg-helios-surface-soft rounded-full overflow-hidden border border-helios-line/20">
                                        <motion.div
                                            className="h-full bg-helios-solar"
                                            animate={{ width: `${scanStatus.files_found > 0 ? (scanStatus.files_added / scanStatus.files_found) * 100 : 0}%` }}
                                        />
                                    </div>
                                    {scanStatus.current_folder && (
                                        <p className="text-[10px] text-helios-slate font-mono truncate px-4">{scanStatus.current_folder}</p>
                                    )}

                                    {!scanStatus.is_running && (
                                        <motion.button
                                            initial={{ opacity: 0, y: 10 }}
                                            animate={{ opacity: 1, y: 0 }}
                                            onClick={() => window.location.href = '/'}
                                            className="w-full py-3 bg-helios-solar text-helios-main font-bold rounded-xl mt-4 shadow-lg shadow-helios-solar/20 hover:scale-[1.02] active:scale-[0.98] transition-all"
                                        >
                                            Enter Dashboard
                                        </motion.button>
                                    )}
                                </div>
                            )}

                            {!scanStatus && loading && (
                                <p className="text-xs text-helios-slate animate-pulse">Initializing scanner...</p>
                            )}
                        </motion.div>
                    )}
                </AnimatePresence>

                {step < 5 && (
                    <div className="mt-8 flex justify-between pt-6 border-t border-helios-line/40">
                        <button
                            onClick={handleBack}
                            disabled={step === 1 || loading}
                            className={clsx(
                                "px-4 py-2 rounded-lg font-medium transition-colors",
                                step === 1 ? "text-helios-line cursor-not-allowed" : "text-helios-slate hover:text-helios-ink"
                            )}
                        >
                            Back
                        </button>

                        <button
                            onClick={handleNext}
                            disabled={loading}
                            className="flex items-center gap-2 px-6 py-2 rounded-lg bg-helios-solar text-helios-main font-semibold hover:opacity-90 transition-opacity disabled:opacity-50"
                        >
                            {loading ? "Searching..." : step === 4 ? "Build Engine" : "Next"}
                            {!loading && <ArrowRight size={18} />}
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}

function SearchIcon({ className, size }: { className?: string; size?: number }) {
    return (
        <svg
            className={className}
            width={size}
            height={size}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
        >
            <circle cx="11" cy="11" r="8" />
            <path d="m21 21-4.3-4.3" />
        </svg>
    );
}
