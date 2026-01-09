import { useState, useEffect } from "react";

interface Stats {
    active: number;
    concurrent_limit: number;
}

export default function SystemStatus() {
    const [stats, setStats] = useState<Stats | null>(null);

    // Initial fetch
    useEffect(() => {
        const fetchStats = async () => {
            try {
                const res = await fetch("/api/stats");
                if (res.ok) {
                    const data = await res.json();
                    setStats({
                        active: data.active || 0,
                        concurrent_limit: data.concurrent_limit || 1
                    });
                }
            } catch (e) {
                console.error("Failed to fetch system status", e);
            }
        };

        fetchStats();
        // Poll every 5 seconds
        const interval = setInterval(fetchStats, 5000);
        return () => clearInterval(interval);
    }, []);

    if (!stats) {
        return (
            <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">System Status</span>
                <span className="w-2 h-2 rounded-full bg-helios-slate/50 animate-pulse"></span>
            </div>
        );
    }

    const isActive = stats.active > 0;
    const isFull = stats.active >= stats.concurrent_limit;
    const percentage = Math.min((stats.active / stats.concurrent_limit) * 100, 100);

    return (
        <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <span className="relative flex h-2 w-2">
                        <span className={`animate-ping absolute inline-flex h-full w-full rounded-full opacity-75 ${isActive ? 'bg-status-success' : 'bg-helios-slate'}`}></span>
                        <span className={`relative inline-flex rounded-full h-2 w-2 ${isActive ? 'bg-status-success' : 'bg-helios-slate'}`}></span>
                    </span>
                    <span className="text-xs font-bold text-helios-slate uppercase tracking-wider">Engine Status</span>
                </div>
                <span className={`text-[10px] font-bold px-1.5 py-0.5 rounded-md ${isActive ? 'bg-status-success/10 text-status-success' : 'bg-helios-slate/10 text-helios-slate'}`}>
                    {isActive ? 'ONLINE' : 'IDLE'}
                </span>
            </div>

            <div className="space-y-1.5">
                <div className="flex items-end justify-between text-helios-ink">
                    <span className="text-xs font-medium opacity-80">Active Jobs</span>
                    <div className="flex items-baseline gap-0.5">
                        <span className={`text-lg font-bold ${isFull ? 'text-status-warning' : 'text-helios-solar'}`}>
                            {stats.active}
                        </span>
                        <span className="text-xs text-helios-slate">
                            / {stats.concurrent_limit}
                        </span>
                    </div>
                </div>

                <div className="h-1.5 w-full bg-helios-line/20 rounded-full overflow-hidden relative">
                    <div
                        className={`h-full transition-all duration-700 ease-out rounded-full ${isFull ? 'bg-status-warning' : 'bg-helios-solar'}`}
                        style={{ width: `${percentage}%` }}
                    />
                    {/* Tick marks for job slots */}
                    <div className="absolute inset-0 flex justify-between px-[1px]">
                        {Array.from({ length: stats.concurrent_limit }).map((_, i) => (
                            <div key={i} className="w-[1px] h-full bg-helios-main/20" style={{ left: `${((i + 1) / stats.concurrent_limit) * 100}%` }} />
                        ))}
                    </div>
                </div>
            </div>

            {isActive && (
                <div className="text-[10px] text-helios-slate flex items-center gap-1.5 animate-pulse">
                    <svg className="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
                    </svg>
                    <span>Processing media...</span>
                </div>
            )}
        </div>
    );
}
