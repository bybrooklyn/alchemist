import { motion } from "framer-motion";
import { Bell, Calendar, Cpu, ShieldCheck } from "lucide-react";
import clsx from "clsx";
import { DEFAULT_NOTIFICATION_DRAFT, EVENT_OPTIONS, WEEKDAY_OPTIONS } from "./constants";
import { LabeledInput, LabeledSelect, ToggleRow } from "./SetupControls";
import type { HardwareInfo, NotificationTargetConfig, ScheduleWindowConfig, SetupSettings } from "./types";

interface RuntimeStepProps {
    hardwareInfo: HardwareInfo | null;
    hardware: SetupSettings["hardware"];
    notifications: SetupSettings["notifications"];
    schedule: SetupSettings["schedule"];
    scheduleDraft: ScheduleWindowConfig;
    notificationDraft: NotificationTargetConfig;
    onHardwareChange: (value: SetupSettings["hardware"]) => void;
    onNotificationsChange: (value: SetupSettings["notifications"]) => void;
    onScheduleChange: (value: SetupSettings["schedule"]) => void;
    onScheduleDraftChange: (value: ScheduleWindowConfig) => void;
    onNotificationDraftChange: (value: NotificationTargetConfig) => void;
}

export default function RuntimeStep({
    hardwareInfo,
    hardware,
    notifications,
    schedule,
    scheduleDraft,
    notificationDraft,
    onHardwareChange,
    onNotificationsChange,
    onScheduleChange,
    onScheduleDraftChange,
    onNotificationDraftChange,
}: RuntimeStepProps) {
    const updateHardware = (patch: Partial<SetupSettings["hardware"]>) => onHardwareChange({ ...hardware, ...patch });
    const vendorLabel = (vendor: string): string => {
        const map: Record<string, string> = {
            nvidia: "NVIDIA",
            amd: "AMD",
            intel: "Intel",
            apple: "Apple",
            cpu: "CPU",
        };
        return map[vendor.toLowerCase()] ?? vendor;
    };
    const addScheduleWindow = () => {
        if (!scheduleDraft.start_time || !scheduleDraft.end_time || scheduleDraft.days_of_week.length === 0) return;
        onScheduleChange({ windows: [...schedule.windows, { ...scheduleDraft }] });
    };
    const addNotificationTarget = () => {
        if (!notificationDraft.name.trim() || !notificationDraft.endpoint_url.trim()) return;
        onNotificationsChange({ ...notifications, targets: [...notifications.targets, { ...notificationDraft }] });
        onNotificationDraftChange(DEFAULT_NOTIFICATION_DRAFT);
    };

    return (
        <motion.div key="runtime" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
            <div className="space-y-2">
                <h2 className="text-xl font-semibold text-helios-ink flex items-center gap-2"><ShieldCheck size={20} className="text-helios-solar" />Hardware, Notifications & Automation</h2>
                <p className="text-sm text-helios-slate">Finish the long-term operating profile: hardware policy, alerts, and allowed runtime windows.</p>
            </div>

            <div className="grid grid-cols-1 xl:grid-cols-[1fr_1fr] gap-6">
                <div className="space-y-6">
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><Cpu size={16} className="text-helios-solar" />Hardware Policy</div>
                        {hardwareInfo && <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-4 py-3 text-sm text-helios-ink">Detected{" "}<span className="font-bold">{vendorLabel(hardwareInfo.vendor)}</span>{" "}with{" "}{hardwareInfo.supported_codecs.map((c) => (c === "h264" ? "H.264" : c.toUpperCase())).join(", ")}{" "}support.</div>}
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <LabeledSelect label="Preferred Vendor" value={hardware.preferred_vendor ?? ""} onChange={(preferred_vendor) => updateHardware({ preferred_vendor: preferred_vendor || null })} options={[{ value: "", label: "Auto detect" }, { value: "nvidia", label: "NVIDIA" }, { value: "amd", label: "AMD" }, { value: "intel", label: "Intel" }, { value: "apple", label: "Apple" }, { value: "cpu", label: "CPU" }]} />
                            <div>
                                <LabeledSelect label="CPU Preset" value={hardware.cpu_preset} onChange={(cpu_preset) => updateHardware({ cpu_preset: cpu_preset as SetupSettings["hardware"]["cpu_preset"] })} options={[{ value: "slow", label: "Slow" }, { value: "medium", label: "Medium" }, { value: "fast", label: "Fast" }, { value: "faster", label: "Faster" }]} />
                                <p className="text-xs text-helios-slate mt-1 leading-relaxed">
                                    How much effort your CPU puts into each encode. Slower presets produce smaller, better-quality files but take longer. 'Medium' is a sensible default for most setups.
                                </p>
                            </div>
                        </div>
                        <LabeledInput label="Explicit Device Path" value={hardware.device_path ?? ""} onChange={(device_path) => updateHardware({ device_path: device_path || null })} placeholder="Optional — Linux only (e.g. /dev/dri/renderD128)" />
                        <ToggleRow title="Allow CPU Fallback" body="Use software encoding if the preferred GPU path is unavailable." checked={hardware.allow_cpu_fallback} onChange={(allow_cpu_fallback) => updateHardware({ allow_cpu_fallback })} />
                        <ToggleRow title="Allow CPU Encoding" body="Permit CPU encoders even when GPU options exist." checked={hardware.allow_cpu_encoding} onChange={(allow_cpu_encoding) => updateHardware({ allow_cpu_encoding })} />
                    </div>

                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><Calendar size={16} className="text-helios-solar" />Schedule Windows</div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            <LabeledInput label="Start" type="time" value={scheduleDraft.start_time} onChange={(start_time) => onScheduleDraftChange({ ...scheduleDraft, start_time })} />
                            <LabeledInput label="End" type="time" value={scheduleDraft.end_time} onChange={(end_time) => onScheduleDraftChange({ ...scheduleDraft, end_time })} />
                        </div>
                        <div className="flex flex-wrap gap-2">
                            {WEEKDAY_OPTIONS.map((day, index) => {
                                const selected = scheduleDraft.days_of_week.includes(index);
                                return (
                                    <button key={day} type="button" onClick={() => onScheduleDraftChange({ ...scheduleDraft, days_of_week: selected ? scheduleDraft.days_of_week.filter((value) => value !== index) : [...scheduleDraft.days_of_week, index].sort() })} className={clsx("rounded-full border px-3 py-2 text-xs font-semibold transition-all", selected ? "border-helios-solar bg-helios-solar/10 text-helios-ink" : "border-helios-line/20 text-helios-slate")}>
                                        {day}
                                    </button>
                                );
                            })}
                        </div>
                        <button type="button" onClick={addScheduleWindow} className="rounded-md border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink">Add Schedule Window</button>
                        <div className="space-y-2">
                            {schedule.windows.map((window, index) => (
                                <div key={`${window.start_time}-${window.end_time}-${index}`} className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 flex items-center justify-between gap-4">
                                    <div>
                                        <div className="text-sm font-semibold text-helios-ink">{window.start_time} - {window.end_time}</div>
                                        <div className="text-xs text-helios-slate mt-1">{window.days_of_week.map((day) => WEEKDAY_OPTIONS[day]).join(", ")}</div>
                                    </div>
                                    <button type="button" onClick={() => onScheduleChange({ windows: schedule.windows.filter((_, current) => current !== index) })} className="rounded-md border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-500 hover:bg-red-500/10">Remove</button>
                                </div>
                            ))}
                            {schedule.windows.length === 0 && <p className="text-sm text-helios-slate">No restricted schedule windows configured. Processing will run whenever work is available.</p>}
                        </div>
                    </div>
                </div>

                <div className="space-y-6">
                    <div className="rounded-lg border border-helios-line/20 bg-helios-surface p-5 space-y-4">
                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink"><Bell size={16} className="text-helios-solar" />Notifications</div>
                        <ToggleRow title="Enable Notifications" body="Send alerts when jobs succeed or fail." checked={notifications.enabled} onChange={(enabled) => onNotificationsChange({ ...notifications, enabled })} />
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                            <LabeledInput label="Target Name" value={notificationDraft.name} onChange={(name) => onNotificationDraftChange({ ...notificationDraft, name })} placeholder="Discord" />
                            <LabeledSelect label="Type" value={notificationDraft.target_type} onChange={(target_type) => onNotificationDraftChange({ ...notificationDraft, target_type })} options={[{ value: "discord", label: "Discord" }, { value: "webhook", label: "Webhook" }, { value: "gotify", label: "Gotify" }]} />
                        </div>
                        <LabeledInput label="Endpoint URL" value={notificationDraft.endpoint_url} onChange={(endpoint_url) => onNotificationDraftChange({ ...notificationDraft, endpoint_url })} placeholder="https://example.com/webhook" />
                        <div className="flex flex-wrap gap-2">
                            {EVENT_OPTIONS.map((eventName) => {
                                const selected = notificationDraft.events.includes(eventName);
                                return (
                                    <button key={eventName} type="button" onClick={() => onNotificationDraftChange({ ...notificationDraft, events: selected ? notificationDraft.events.filter((candidate) => candidate !== eventName) : [...notificationDraft.events, eventName] })} className={clsx("rounded-full border px-3 py-2 text-xs font-semibold transition-all", selected ? "border-helios-solar bg-helios-solar/10 text-helios-ink" : "border-helios-line/20 text-helios-slate")}>
                                        {eventName}
                                    </button>
                                );
                            })}
                        </div>
                        <button type="button" onClick={addNotificationTarget} className="rounded-md border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink">Add Notification Target</button>
                        <div className="space-y-2">
                            {notifications.targets.map((target, index) => (
                                <div key={`${target.name}-${target.endpoint_url}-${index}`} className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 flex items-center justify-between gap-4">
                                    <div className="min-w-0">
                                        <div className="text-sm font-semibold text-helios-ink">{target.name}</div>
                                        <div className="text-xs text-helios-slate mt-1 truncate" title={target.endpoint_url}>{target.endpoint_url}</div>
                                    </div>
                                    <button type="button" onClick={() => onNotificationsChange({ ...notifications, targets: notifications.targets.filter((_, current) => current !== index) })} className="rounded-md border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-500 hover:bg-red-500/10">Remove</button>
                                </div>
                            ))}
                            {notifications.targets.length === 0 && <p className="text-sm text-helios-slate">No notification targets configured yet.</p>}
                        </div>
                    </div>
                </div>
            </div>
        </motion.div>
    );
}
