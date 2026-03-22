import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { apiAction, apiJson, isApiError } from "../../lib/api";
import type { ScanStatus } from "./types";

interface ScanStepProps {
    runId: number;
    onBackToReview: () => void;
}

export default function ScanStep({ runId, onBackToReview }: ScanStepProps) {
    const [scanStatus, setScanStatus] = useState<ScanStatus | null>(null);
    const [scanError, setScanError] = useState<string | null>(null);
    const [starting, setStarting] = useState(false);
    const scanIntervalRef = useRef<number | null>(null);

    const clearScanPolling = () => {
        if (scanIntervalRef.current !== null) {
            window.clearInterval(scanIntervalRef.current);
            scanIntervalRef.current = null;
        }
    };

    const pollScanStatus = async () => {
        clearScanPolling();
        const poll = async () => {
            try {
                const data = await apiJson<ScanStatus>("/api/scan/status");
                setScanStatus(data);
                setScanError(null);
                if (!data.is_running) {
                    clearScanPolling();
                    setStarting(false);
                }
            } catch (err) {
                const message = isApiError(err) ? err.message : "Scan status unavailable";
                setScanError(message);
                clearScanPolling();
                setStarting(false);
            }
        };
        await poll();
        scanIntervalRef.current = window.setInterval(() => void poll(), 1000);
    };

    const startScan = async () => {
        setStarting(true);
        setScanStatus(null);
        setScanError(null);
        try {
            await apiAction("/api/scan/start", { method: "POST" });
            await pollScanStatus();
        } catch (err) {
            const message = isApiError(err) ? err.message : "Failed to start scan";
            setScanError(message);
            setStarting(false);
        }
    };

    useEffect(() => {
        if (runId > 0) {
            void startScan();
        }
        return () => clearScanPolling();
    }, [runId]);

    return (
        <motion.div key="scan" initial={{ opacity: 0, scale: 0.98 }} animate={{ opacity: 1, scale: 1 }} className="space-y-8 py-8">
            <div className="text-center space-y-3">
                <div className="mx-auto w-20 h-20 rounded-full border-4 border-helios-solar/20 border-t-helios-solar animate-spin" />
                <h2 className="text-2xl font-bold text-helios-ink">Initial Library Scan</h2>
                <p className="text-sm text-helios-slate">Alchemist is validating the selected server folders and seeding the first queue. Encoding will stay paused until you press Start on the dashboard.</p>
            </div>

            {scanError && (
                <div className="rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-4 text-sm text-red-500 space-y-3">
                    <p className="font-semibold">The initial scan hit an error.</p>
                    <p>{scanError}</p>
                    <div className="flex flex-col sm:flex-row gap-2">
                        <button type="button" onClick={() => void startScan()} disabled={starting} className="rounded-xl bg-red-500/20 px-4 py-2 font-semibold disabled:opacity-50">{starting ? "Retrying..." : "Retry Scan"}</button>
                        <button type="button" onClick={onBackToReview} className="rounded-xl border border-red-500/30 px-4 py-2 font-semibold">Back to Review</button>
                    </div>
                </div>
            )}

            {scanStatus && (
                <div className="space-y-4">
                    <div className="flex justify-between text-[10px] font-bold uppercase tracking-widest text-helios-slate">
                        <span>Found: {scanStatus.files_found}</span>
                        <span>Queued: {scanStatus.files_added}</span>
                    </div>
                    <div className="h-3 rounded-full border border-helios-line/20 bg-helios-surface-soft overflow-hidden">
                        <motion.div className="h-full bg-helios-solar" animate={{ width: `${scanStatus.files_found > 0 ? (scanStatus.files_added / scanStatus.files_found) * 100 : 0}%` }} />
                    </div>
                    {scanStatus.current_folder && <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 font-mono text-xs text-helios-slate">{scanStatus.current_folder}</div>}
                    {!scanStatus.is_running && <button type="button" onClick={() => { window.location.href = "/"; }} className="w-full rounded-lg bg-helios-solar px-6 py-4 font-bold text-helios-main shadow-lg shadow-helios-solar/20 hover:opacity-90">Enter Dashboard</button>}
                </div>
            )}
        </motion.div>
    );
}
