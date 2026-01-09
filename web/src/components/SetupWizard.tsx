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
    Save
} from 'lucide-react';
import clsx from 'clsx';

interface ConfigState {
    size_reduction_threshold: number;
    min_file_size_mb: number;
    concurrent_jobs: number;
    directories: string[];
    allow_cpu_encoding: boolean;
}

export default function SetupWizard() {
    const [step, setStep] = useState(1);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    const [config, setConfig] = useState<ConfigState>({
        size_reduction_threshold: 0.3,
        min_file_size_mb: 100,
        concurrent_jobs: 2,
        directories: ['/media/movies'],
        allow_cpu_encoding: false
    });

    const [dirInput, setDirInput] = useState('');

    const handleNext = () => setStep(s => Math.min(s + 1, 4));
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
                throw new Error(await res.text());
            }

            setSuccess(true);
        } catch (err: any) {
            setError(err.message || "Failed to save configuration");
        } finally {
            setLoading(false);
        }
    };

    if (success) {
        return (
            <div className="flex flex-col items-center justify-center p-12 text-center max-w-lg mx-auto">
                <div className="w-16 h-16 bg-status-success/20 rounded-full flex items-center justify-center mb-6 text-status-success">
                    <CheckCircle size={32} />
                </div>
                <h2 className="text-2xl font-bold text-helios-ink mb-4">Configuration Saved!</h2>
                <p className="text-helios-slate mb-8">
                    Alchemist has been successfully configured.
                    <br /><br />
                    <span className="font-bold text-helios-ink">Please restart the server (or Docker container) to apply changes.</span>
                </p>
            </div>
        );
    }

    return (
        <div className="bg-helios-surface border border-helios-line/60 rounded-2xl overflow-hidden shadow-2xl max-w-2xl w-full mx-auto">
            {/* Header */}
            <div className="h-1 bg-helios-surface-soft w-full flex">
                <motion.div
                    className="bg-helios-solar h-full"
                    initial={{ width: 0 }}
                    animate={{ width: `${(step / 4) * 100}%` }}
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
                                <Settings size={20} className="text-helios-solar" />
                                Transcoding Rules
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
                                    <p className="text-xs text-helios-slate mt-1">Files must shrink by at least this amount to be kept.</p>
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

                    {step === 2 && (
                        <motion.div
                            key="step2"
                            initial={{ opacity: 0, x: 20 }}
                            animate={{ opacity: 1, x: 0 }}
                            exit={{ opacity: 0, x: -20 }}
                            className="space-y-6"
                        >
                            <h2 className="text-lg font-semibold text-helios-ink flex items-center gap-2">
                                <Cpu size={20} className="text-helios-solar" />
                                Hardware & Performance
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

                                <div className="flex items-center gap-4 p-4 rounded-xl bg-helios-surface-soft border border-helios-line/40">
                                    <div className="flex-1">
                                        <span className="block text-sm font-medium text-helios-ink">Allow CPU Encoding</span>
                                        <span className="text-xs text-helios-slate">Fallback to software encoding if GPU is unavailable. Warning: Slow.</span>
                                    </div>
                                    <label className="relative inline-flex items-center cursor-pointer">
                                        <input
                                            type="checkbox"
                                            checked={config.allow_cpu_encoding}
                                            onChange={(e) => setConfig({ ...config, allow_cpu_encoding: e.target.checked })}
                                            className="sr-only peer"
                                        />
                                        <div className="w-11 h-6 bg-helios-surface rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-helios-solar"></div>
                                    </label>
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
                                <FolderOpen size={20} className="text-helios-solar" />
                                Media Directories
                            </h2>

                            <div className="space-y-4">
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

                                <div className="space-y-2 max-h-48 overflow-y-auto">
                                    {config.directories.length === 0 && (
                                        <p className="text-center text-helios-slate italic py-4">No directories added yet</p>
                                    )}
                                    {config.directories.map((dir, i) => (
                                        <div key={i} className="flex items-center justify-between p-3 rounded-lg bg-helios-surface-soft/50 border border-helios-line/20 group">
                                            <span className="font-mono text-sm text-helios-ink truncate">{dir}</span>
                                            <button
                                                onClick={() => removeDirectory(dir)}
                                                className="opacity-0 group-hover:opacity-100 p-1 text-status-error hover:bg-status-error/10 rounded transition-all"
                                            >
                                                <span className="sr-only">Remove</span>
                                                ×
                                            </button>
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
                                <Server size={20} className="text-helios-solar" />
                                Review & Save
                            </h2>

                            <div className="space-y-4 text-sm text-helios-slate">
                                <div className="p-4 rounded-xl bg-helios-surface-soft border border-helios-line/40 space-y-3">
                                    <div className="flex justify-between">
                                        <span>Reduction Threshold</span>
                                        <span className="text-helios-ink font-mono">{Math.round(config.size_reduction_threshold * 100)}%</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>Min File Size</span>
                                        <span className="text-helios-ink font-mono">{config.min_file_size_mb} MB</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>Concurrency</span>
                                        <span className="text-helios-ink font-mono">{config.concurrent_jobs} jobs</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span>CPU Encoding</span>
                                        <span className={config.allow_cpu_encoding ? "text-status-warning" : "text-helios-ink"}>
                                            {config.allow_cpu_encoding ? "Enabled" : "Disabled"}
                                        </span>
                                    </div>
                                    <div className="pt-2 border-t border-helios-line/20">
                                        <span className="block mb-1">Directories:</span>
                                        <ul className="list-disc list-inside font-mono text-xs text-helios-ink">
                                            {config.directories.map(d => <li key={d}>{d}</li>)}
                                        </ul>
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

                    {step < 4 ? (
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
                            disabled={loading}
                            className="flex items-center gap-2 px-6 py-2 rounded-lg bg-status-success text-white font-semibold hover:opacity-90 transition-opacity disabled:opacity-50"
                        >
                            {loading ? "Saving..." : "Save Configuration"}
                            {!loading && <Save size={18} />}
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
