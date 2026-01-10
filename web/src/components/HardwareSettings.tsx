import { useState, useEffect } from "react";
import { Cpu, Zap, HardDrive, CheckCircle2, AlertCircle } from "lucide-react";
import { apiFetch } from "../lib/api";

interface HardwareInfo {
    vendor: "Nvidia" | "Amd" | "Intel" | "Apple" | "Cpu";
    device_path: string | null;
    supported_codecs: string[];
}

export default function HardwareSettings() {
    const [info, setInfo] = useState<HardwareInfo | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState("");

    useEffect(() => {
        fetchHardware();
    }, []);

    const fetchHardware = async () => {
        try {
            const res = await apiFetch("/api/system/hardware");
            if (!res.ok) throw new Error("Failed to detect hardware");
            const data = await res.json();
            setInfo(data);
        } catch (err) {
            setError("Unable to detect hardware acceleration support.");
            console.error(err);
        } finally {
            setLoading(false);
        }
    };

    if (loading) {
        return (
            <div className="flex flex-col gap-4 animate-pulse">
                <div className="h-12 bg-helios-surface-soft rounded-2xl w-full" />
                <div className="h-40 bg-helios-surface-soft rounded-2xl w-full" />
            </div>
        );
    }

    if (error || !info) {
        return (
            <div className="p-6 bg-red-500/10 border border-red-500/20 text-red-500 rounded-2xl flex items-center gap-3">
                <AlertCircle size={20} />
                <span className="font-semibold">{error || "Hardware detection failed."}</span>
            </div>
        );
    }

    const getVendorDetails = (vendor: string) => {
        switch (vendor) {
            case "Nvidia": return { name: "NVIDIA", tech: "NVENC", color: "text-emerald-500", bg: "bg-emerald-500/10" };
            case "Amd": return { name: "AMD", tech: "VAAPI/AMF", color: "text-red-500", bg: "bg-red-500/10" };
            case "Intel": return { name: "Intel", tech: "QuickSync (QSV)", color: "text-blue-500", bg: "bg-blue-500/10" };
            case "Apple": return { name: "Apple", tech: "VideoToolbox", color: "text-helios-slate", bg: "bg-helios-slate/10" };
            default: return { name: "CPU", tech: "Software Fallback", color: "text-helios-solar", bg: "bg-helios-solar/10" };
        }
    };

    const details = getVendorDetails(info.vendor);

    return (
        <div className="flex flex-col gap-6">
            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight uppercase tracking-[0.1em]">Transcoding Hardware</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Detected acceleration engines and codec support.</p>
                </div>
                <div className={`p-2 ${details.bg} rounded-xl ${details.color}`}>
                    {info.vendor === "Cpu" ? <Cpu size={20} /> : <Zap size={20} />}
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="bg-helios-surface border border-helios-line/30 rounded-2xl p-5 shadow-sm">
                    <div className="flex items-center gap-3 mb-4">
                        <div className={`p-2.5 rounded-xl ${details.bg} ${details.color}`}>
                            <HardDrive size={18} />
                        </div>
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink uppercase tracking-wider">Active Device</h4>
                            <p className="text-[10px] text-helios-slate font-bold">{details.name} {details.tech}</p>
                        </div>
                    </div>

                    <div className="space-y-4">
                        <div>
                            <span className="text-[10px] font-bold text-helios-slate uppercase tracking-widest block mb-1.5 ml-0.5">Device Path</span>
                            <div className="bg-helios-surface-soft border border-helios-line/30 rounded-lg px-3 py-2 font-mono text-xs text-helios-ink shadow-inner">
                                {info.device_path || (info.vendor === "Nvidia" ? "NVIDIA Driver (Direct)" : "Auto-detected Interface")}
                            </div>
                        </div>
                    </div>
                </div>

                <div className="bg-helios-surface border border-helios-line/30 rounded-2xl p-5 shadow-sm">
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2.5 rounded-xl bg-purple-500/10 text-purple-500">
                            <CheckCircle2 size={18} />
                        </div>
                        <div>
                            <h4 className="text-sm font-bold text-helios-ink uppercase tracking-wider">Codec Support</h4>
                            <p className="text-[10px] text-helios-slate font-bold">Hardware verified encoders</p>
                        </div>
                    </div>

                    <div className="flex flex-wrap gap-2">
                        {info.supported_codecs.length > 0 ? info.supported_codecs.map(codec => (
                            <div key={codec} className="px-3 py-1.5 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-500 text-xs font-bold uppercase tracking-wider flex items-center gap-2">
                                <div className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                                {codec}
                            </div>
                        )) : (
                            <div className="text-xs text-helios-slate italic bg-helios-surface-soft w-full p-2 text-center rounded-lg">
                                No hardware accelerated codecs found.
                            </div>
                        )}
                    </div>
                </div>
            </div>

            {info.vendor === "Cpu" && (
                <div className="p-4 bg-helios-solar/5 border border-helios-solar/10 rounded-2xl">
                    <div className="flex gap-3">
                        <AlertCircle className="text-helios-solar shrink-0" size={18} />
                        <div className="space-y-1">
                            <h5 className="text-sm font-bold text-helios-ink uppercase tracking-wider">CPU Fallback Active</h5>
                            <p className="text-xs text-helios-slate leading-relaxed">
                                GPU acceleration was not detected or is incompatible. Alchemist will use software encoding (SVT-AV1 / x264), which is significantly more resource intensive.
                            </p>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
