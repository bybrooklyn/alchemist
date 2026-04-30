import type { ReactNode } from "react";
import { motion } from "framer-motion";
import { AlertCircle, Bell, Calendar, Cpu, Plus, ShieldCheck, Trash2 } from "lucide-react";
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

interface SectionHeaderProps {
    icon: ReactNode;
    eyebrow: string;
    title: string;
    body: string;
}

function SectionHeader({ icon, eyebrow, title, body }: SectionHeaderProps) {
    return (
        <div className="flex items-start gap-3">
            <div className="rounded-lg bg-helios-solar/10 p-2 text-helios-solar">
                {icon}
            </div>
            <div>
                <p className="text-xs font-semibold uppercase text-helios-slate/70">{eyebrow}</p>
                <h3 className="mt-1 text-base font-semibold text-helios-ink">{title}</h3>
                <p className="mt-1 text-sm leading-relaxed text-helios-slate">{body}</p>
            </div>
        </div>
    );
}

const formatCodec = (codec: string) => (codec === "h264" ? "H.264" : codec.toUpperCase());

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
    const canAddScheduleWindow = scheduleDraft.start_time.length > 0 && scheduleDraft.end_time.length > 0 && scheduleDraft.days_of_week.length > 0;
    const canAddNotificationTarget = notifications.enabled && notificationDraft.name.trim().length > 0 && notificationDraft.endpoint_url.trim().length > 0;
    const addScheduleWindow = () => {
        if (!canAddScheduleWindow) return;
        onScheduleChange({ windows: [...schedule.windows, { ...scheduleDraft }] });
    };
    const addNotificationTarget = () => {
        if (!canAddNotificationTarget) return;
        onNotificationsChange({ ...notifications, targets: [...notifications.targets, { ...notificationDraft }] });
        onNotificationDraftChange(DEFAULT_NOTIFICATION_DRAFT);
    };

    return (
        <motion.div key="runtime" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-8">
            <div className="space-y-2">
                <h2 className="flex items-center gap-2 text-xl font-semibold text-helios-ink">
                    <ShieldCheck size={20} className="text-helios-solar" />
                    Hardware, Notifications & Automation
                </h2>
                <p className="max-w-3xl text-sm leading-relaxed text-helios-slate">
                    Finish the long-term operating profile: which encoders may run, when work is allowed, and where alerts should go.
                </p>
            </div>

            <div className="grid grid-cols-1 gap-6 xl:grid-cols-[1fr_1fr]">
                <section className="space-y-5 rounded-lg border border-helios-line/20 bg-helios-surface p-5">
                    <SectionHeader
                        icon={<Cpu size={18} />}
                        eyebrow="Hardware policy"
                        title="Prefer the detected encoder, then choose fallbacks"
                        body="Auto-detection stays the safest default. Use overrides only when the host exposes multiple GPU paths or a container needs an explicit device."
                    />

                    {hardwareInfo ? (
                        <div className="grid gap-3 sm:grid-cols-2">
                            <div className="rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-4 py-3">
                                <p className="text-xs font-medium text-emerald-300">Detected vendor</p>
                                <p className="mt-1 text-sm font-semibold text-helios-ink">{vendorLabel(hardwareInfo.vendor)}</p>
                            </div>
                            <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                                <p className="text-xs font-medium text-helios-slate">Codec support</p>
                                <p className="mt-1 text-sm font-semibold text-helios-ink">
                                    {hardwareInfo.supported_codecs.length > 0 ? hardwareInfo.supported_codecs.map(formatCodec).join(", ") : "Pending"}
                                </p>
                            </div>
                        </div>
                    ) : (
                        <div className="flex items-start gap-3 rounded-lg border border-blue-500/25 bg-blue-500/10 px-4 py-3 text-sm text-blue-300">
                            <AlertCircle size={16} className="mt-0.5 shrink-0" />
                            <div>
                                <p className="font-semibold">Hardware detection pending.</p>
                                <p className="mt-1 leading-relaxed text-blue-200/80">Setup can continue; Alchemist will update the hardware state when detection completes.</p>
                            </div>
                        </div>
                    )}

                    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        <LabeledSelect label="Preferred Vendor" value={hardware.preferred_vendor ?? ""} onChange={(preferred_vendor) => updateHardware({ preferred_vendor: preferred_vendor || null })} options={[{ value: "", label: "Auto detect" }, { value: "nvidia", label: "NVIDIA" }, { value: "amd", label: "AMD" }, { value: "intel", label: "Intel" }, { value: "apple", label: "Apple" }, { value: "cpu", label: "CPU" }]} />
                        <LabeledSelect
                            label="CPU Preset"
                            value={hardware.cpu_preset}
                            onChange={(cpu_preset) => updateHardware({ cpu_preset: cpu_preset as SetupSettings["hardware"]["cpu_preset"] })}
                            options={[{ value: "slow", label: "Slow" }, { value: "medium", label: "Medium" }, { value: "fast", label: "Fast" }, { value: "faster", label: "Faster" }]}
                            helperText="Used when CPU encoding or CPU fallback is allowed."
                        />
                    </div>

                    <div className="grid gap-3 lg:grid-cols-2">
                        <ToggleRow title="Allow CPU Fallback" body="Use software encoding if the preferred GPU path is unavailable." checked={hardware.allow_cpu_fallback} onChange={(allow_cpu_fallback) => updateHardware({ allow_cpu_fallback })} />
                        <ToggleRow title="Allow CPU Encoding" body="Permit CPU encoders even when GPU options exist." checked={hardware.allow_cpu_encoding} onChange={(allow_cpu_encoding) => updateHardware({ allow_cpu_encoding })} />
                    </div>

                    <details className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                        <summary className="cursor-pointer list-none text-sm font-semibold text-helios-ink">Advanced hardware override</summary>
                        <div className="mt-4 border-t border-helios-line/10 pt-4">
                            <LabeledInput
                                label="Explicit Device Path"
                                value={hardware.device_path ?? ""}
                                onChange={(device_path) => updateHardware({ device_path: device_path || null })}
                                placeholder="Optional - Linux only (for example /dev/dri/renderD128)"
                                helperText="Leave blank unless container passthrough or multi-GPU routing needs a specific render node."
                            />
                        </div>
                    </details>
                </section>

                <section className="space-y-5 rounded-lg border border-helios-line/20 bg-helios-surface p-5">
                    <SectionHeader
                        icon={<Calendar size={18} />}
                        eyebrow="Automation windows"
                        title="Let processing run always, or restrict it"
                        body="No windows means the engine can work whenever jobs are available. Add windows only when you want a controlled operating schedule."
                    />

                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                        <LabeledInput label="Start" type="time" value={scheduleDraft.start_time} onChange={(start_time) => onScheduleDraftChange({ ...scheduleDraft, start_time })} />
                        <LabeledInput label="End" type="time" value={scheduleDraft.end_time} onChange={(end_time) => onScheduleDraftChange({ ...scheduleDraft, end_time })} />
                    </div>
                    <div className="flex flex-wrap gap-2">
                        {WEEKDAY_OPTIONS.map((day, index) => {
                            const selected = scheduleDraft.days_of_week.includes(index);
                            return (
                                <button
                                    key={day}
                                    type="button"
                                    onClick={() => onScheduleDraftChange({ ...scheduleDraft, days_of_week: selected ? scheduleDraft.days_of_week.filter((value) => value !== index) : [...scheduleDraft.days_of_week, index].sort() })}
                                    className={clsx(
                                        "rounded-md border px-3 py-2 text-xs font-semibold transition-all",
                                        selected ? "border-helios-solar bg-helios-solar/10 text-helios-ink" : "border-helios-line/20 text-helios-slate hover:border-helios-solar/40 hover:text-helios-ink"
                                    )}
                                >
                                    {day}
                                </button>
                            );
                        })}
                    </div>
                    <button
                        type="button"
                        onClick={addScheduleWindow}
                        disabled={!canAddScheduleWindow}
                        className="inline-flex items-center gap-2 rounded-md border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink transition-colors hover:border-helios-solar/40 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                        <Plus size={15} />
                        Add Schedule Window
                    </button>
                    <div className="space-y-2">
                        {schedule.windows.map((window, index) => (
                            <div key={`${window.start_time}-${window.end_time}-${index}`} className="flex items-center justify-between gap-4 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                                <div>
                                    <div className="text-sm font-semibold text-helios-ink">{window.start_time} - {window.end_time}</div>
                                    <div className="mt-1 text-xs text-helios-slate">{window.days_of_week.map((day) => WEEKDAY_OPTIONS[day]).join(", ")}</div>
                                </div>
                                <button type="button" onClick={() => onScheduleChange({ windows: schedule.windows.filter((_, current) => current !== index) })} className="inline-flex items-center gap-1.5 rounded-md border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-400 hover:bg-red-500/10">
                                    <Trash2 size={13} />
                                    Remove
                                </button>
                            </div>
                        ))}
                        {schedule.windows.length === 0 && (
                            <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 text-sm text-helios-slate">
                                No schedule windows configured. Processing will run whenever work is available.
                            </div>
                        )}
                    </div>
                </section>

                <section className="space-y-5 rounded-lg border border-helios-line/20 bg-helios-surface p-5 xl:col-span-2">
                    <SectionHeader
                        icon={<Bell size={18} />}
                        eyebrow="Notifications"
                        title="Send only the events you care about"
                        body="Targets stay optional during setup. You can leave this off and configure alerts later from Settings."
                    />
                    <ToggleRow title="Enable Notifications" body="Send configured alerts when selected queue events occur." checked={notifications.enabled} onChange={(enabled) => onNotificationsChange({ ...notifications, enabled })} />

                    {!notifications.enabled && (
                        <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 text-sm text-helios-slate">
                            Notification setup is paused. Existing fields stay visible so the flow is understandable, but targets will not be added until notifications are enabled.
                        </div>
                    )}

                    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_180px]">
                        <LabeledInput label="Target Name" value={notificationDraft.name} onChange={(name) => onNotificationDraftChange({ ...notificationDraft, name })} placeholder="Discord" disabled={!notifications.enabled} />
                        <LabeledSelect label="Type" value={notificationDraft.target_type} onChange={(target_type) => onNotificationDraftChange({ ...notificationDraft, target_type })} options={[{ value: "discord", label: "Discord" }, { value: "webhook", label: "Webhook" }, { value: "gotify", label: "Gotify" }]} disabled={!notifications.enabled} />
                    </div>
                    <LabeledInput label="Endpoint URL" value={notificationDraft.endpoint_url} onChange={(endpoint_url) => onNotificationDraftChange({ ...notificationDraft, endpoint_url })} placeholder="https://example.com/webhook" disabled={!notifications.enabled} />
                    <div className="flex flex-wrap gap-2">
                        {EVENT_OPTIONS.map((eventName) => {
                            const selected = notificationDraft.events.includes(eventName);
                            return (
                                <button
                                    key={eventName}
                                    type="button"
                                    disabled={!notifications.enabled}
                                    onClick={() => onNotificationDraftChange({ ...notificationDraft, events: selected ? notificationDraft.events.filter((candidate) => candidate !== eventName) : [...notificationDraft.events, eventName] })}
                                    className={clsx(
                                        "rounded-md border px-3 py-2 text-xs font-semibold transition-all disabled:cursor-not-allowed disabled:opacity-50",
                                        selected ? "border-helios-solar bg-helios-solar/10 text-helios-ink" : "border-helios-line/20 text-helios-slate hover:border-helios-solar/40 hover:text-helios-ink"
                                    )}
                                >
                                    {eventName}
                                </button>
                            );
                        })}
                    </div>
                    <button
                        type="button"
                        onClick={addNotificationTarget}
                        disabled={!canAddNotificationTarget}
                        className="inline-flex items-center gap-2 rounded-md border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink transition-colors hover:border-helios-solar/40 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                        <Plus size={15} />
                        Add Notification Target
                    </button>
                    <div className="space-y-2">
                        {notifications.targets.map((target, index) => (
                            <div key={`${target.name}-${target.endpoint_url}-${index}`} className="flex items-center justify-between gap-4 rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3">
                                <div className="min-w-0">
                                    <div className="text-sm font-semibold text-helios-ink">{target.name}</div>
                                    <div className="mt-1 truncate text-xs text-helios-slate" title={target.endpoint_url}>{target.endpoint_url}</div>
                                </div>
                                <button type="button" onClick={() => onNotificationsChange({ ...notifications, targets: notifications.targets.filter((_, current) => current !== index) })} className="inline-flex items-center gap-1.5 rounded-md border border-red-500/20 px-3 py-2 text-xs font-semibold text-red-400 hover:bg-red-500/10">
                                    <Trash2 size={13} />
                                    Remove
                                </button>
                            </div>
                        ))}
                        {notifications.targets.length === 0 && (
                            <div className="rounded-lg border border-helios-line/20 bg-helios-surface-soft/40 px-4 py-3 text-sm text-helios-slate">
                                No notification targets configured yet.
                            </div>
                        )}
                    </div>
                </section>
            </div>
        </motion.div>
    );
}
