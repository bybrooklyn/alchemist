import { useEffect, useState } from "react";
import { Bell, Plus, Trash2, Zap } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";

type NotificationTargetType =
    | "discord_webhook"
    | "discord_bot"
    | "gotify"
    | "webhook"
    | "telegram"
    | "email";

interface NotificationTarget {
    id: number;
    name: string;
    target_type: NotificationTargetType;
    config_json: Record<string, unknown>;
    events: string[];
    enabled: boolean;
    created_at: string;
}

interface NotificationsSettingsResponse {
    daily_summary_time_local: string;
    quiet_hours_enabled?: boolean;
    quiet_hours_start_local?: string;
    quiet_hours_end_local?: string;
    targets: NotificationTarget[];
}

interface LegacyNotificationTarget {
    id: number;
    name: string;
    target_type: "discord" | "gotify" | "webhook";
    endpoint_url: string;
    auth_token: string | null;
    events: string;
    enabled: boolean;
    created_at?: string;
}

const TARGET_TYPES: Array<{ value: NotificationTargetType; label: string }> = [
    { value: "discord_webhook", label: "Discord Webhook" },
    { value: "discord_bot", label: "Discord Bot" },
    { value: "gotify", label: "Gotify" },
    { value: "webhook", label: "Generic Webhook" },
    { value: "telegram", label: "Telegram" },
    { value: "email", label: "Email" },
];

const EVENT_OPTIONS = [
    "encode.queued",
    "encode.started",
    "encode.completed",
    "encode.failed",
    "scan.completed",
    "engine.idle",
    "daily.summary",
];

function targetSummary(target: NotificationTarget): string {
    const config = target.config_json;
    switch (target.target_type) {
        case "discord_webhook":
            return String(config.webhook_url ?? "");
        case "discord_bot":
            return `channel ${String(config.channel_id ?? "")}`;
        case "gotify":
            return String(config.server_url ?? "");
        case "webhook":
            return String(config.url ?? "");
        case "telegram":
            return `chat ${String(config.chat_id ?? "")}`;
        case "email":
            return String((config.to_addresses as string[] | undefined)?.join(", ") ?? "");
        default:
            return "";
    }
}

function normalizeTarget(target: NotificationTarget | LegacyNotificationTarget): NotificationTarget {
    if ("config_json" in target) {
        return target;
    }

    const normalizedType: NotificationTargetType =
        target.target_type === "discord" ? "discord_webhook" : target.target_type;
    const config_json =
        normalizedType === "discord_webhook"
            ? { webhook_url: target.endpoint_url }
            : normalizedType === "gotify"
              ? { server_url: target.endpoint_url, app_token: target.auth_token ?? "" }
              : { url: target.endpoint_url, auth_token: target.auth_token ?? "" };

    return {
        id: target.id,
        name: target.name,
        target_type: normalizedType,
        config_json,
        events: JSON.parse(target.events),
        enabled: target.enabled,
        created_at: target.created_at ?? new Date().toISOString(),
    };
}

function defaultConfigForType(type: NotificationTargetType): Record<string, unknown> {
    switch (type) {
        case "discord_webhook":
            return { webhook_url: "" };
        case "discord_bot":
            return { bot_token: "", channel_id: "" };
        case "gotify":
            return { server_url: "", app_token: "" };
        case "webhook":
            return { url: "", auth_token: "" };
        case "telegram":
            return { bot_token: "", chat_id: "" };
        case "email":
            return {
                smtp_host: "",
                smtp_port: 587,
                username: "",
                password: "",
                from_address: "",
                to_addresses: [""],
                security: "starttls",
            };
    }
}

export default function NotificationSettings() {
    const [targets, setTargets] = useState<NotificationTarget[]>([]);
    const [dailySummaryTime, setDailySummaryTime] = useState("09:00");
    const [quietHoursEnabled, setQuietHoursEnabled] = useState(false);
    const [quietHoursStart, setQuietHoursStart] = useState("22:00");
    const [quietHoursEnd, setQuietHoursEnd] = useState("08:00");
    const [loading, setLoading] = useState(true);
    const [savingSchedule, setSavingSchedule] = useState(false);
    const [testingId, setTestingId] = useState<number | null>(null);
    const [error, setError] = useState<string | null>(null);

    const [showForm, setShowForm] = useState(false);
    const [draftName, setDraftName] = useState("");
    const [draftType, setDraftType] = useState<NotificationTargetType>("discord_webhook");
    const [draftConfig, setDraftConfig] = useState<Record<string, unknown>>(defaultConfigForType("discord_webhook"));
    const [draftEvents, setDraftEvents] = useState<string[]>(["encode.completed", "encode.failed"]);
    const [pendingDeleteId, setPendingDeleteId] = useState<number | null>(null);

    useEffect(() => {
        void fetchTargets();
    }, []);

    const fetchTargets = async () => {
        try {
            const data = await apiJson<NotificationsSettingsResponse | LegacyNotificationTarget[]>(
                "/api/settings/notifications",
            );
            if (Array.isArray(data)) {
                setTargets(data.map(normalizeTarget));
                setDailySummaryTime("09:00");
                setQuietHoursEnabled(false);
                setQuietHoursStart("22:00");
                setQuietHoursEnd("08:00");
            } else {
                setTargets(data.targets.map(normalizeTarget));
                setDailySummaryTime(data.daily_summary_time_local);
                setQuietHoursEnabled(data.quiet_hours_enabled ?? false);
                setQuietHoursStart(data.quiet_hours_start_local ?? "22:00");
                setQuietHoursEnd(data.quiet_hours_end_local ?? "08:00");
            }
            setError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to load notification targets";
            setError(message);
        } finally {
            setLoading(false);
        }
    };

    const saveNotificationSettings = async () => {
        setSavingSchedule(true);
        try {
            await apiAction("/api/settings/notifications", {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    daily_summary_time_local: dailySummaryTime,
                    quiet_hours_enabled: quietHoursEnabled,
                    quiet_hours_start_local: quietHoursStart,
                    quiet_hours_end_local: quietHoursEnd,
                }),
            });
            await fetchTargets();
            showToast({
                kind: "success",
                title: "Notifications",
                message: "Notification schedule settings saved.",
            });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to save notification schedule settings";
            setError(message);
            showToast({ kind: "error", title: "Notifications", message });
        } finally {
            setSavingSchedule(false);
        }
    };

    const resetDraft = (type: NotificationTargetType = "discord_webhook") => {
        setDraftName("");
        setDraftType(type);
        setDraftConfig(defaultConfigForType(type));
        setDraftEvents(["encode.completed", "encode.failed"]);
    };

    const handleAdd = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            await apiAction("/api/settings/notifications", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name: draftName,
                    target_type: draftType,
                    config_json: draftConfig,
                    events: draftEvents,
                    enabled: true,
                }),
            });
            setShowForm(false);
            resetDraft();
            setError(null);
            await fetchTargets();
            showToast({ kind: "success", title: "Notifications", message: "Target added." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to add notification target";
            setError(message);
            showToast({ kind: "error", title: "Notifications", message });
        }
    };

    const handleDelete = async (id: number) => {
        try {
            await apiAction(`/api/settings/notifications/${id}`, { method: "DELETE" });
            setError(null);
            await fetchTargets();
            showToast({ kind: "success", title: "Notifications", message: "Target removed." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to remove target";
            setError(message);
            showToast({ kind: "error", title: "Notifications", message });
        }
    };

    const handleTest = async (target: NotificationTarget) => {
        setTestingId(target.id);
        try {
            await apiAction("/api/settings/notifications/test", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name: target.name,
                    target_type: target.target_type,
                    config_json: target.config_json,
                    events: target.events,
                    enabled: target.enabled,
                }),
            });
            showToast({ kind: "success", title: "Notifications", message: "Test notification sent." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Test notification failed";
            setError(message);
            showToast({ kind: "error", title: "Notifications", message });
        } finally {
            setTestingId(null);
        }
    };

    const toggleEvent = (evt: string) => {
        setDraftEvents((current) =>
            current.includes(evt)
                ? current.filter((candidate) => candidate !== evt)
                : [...current, evt],
        );
    };

    const setConfigField = (key: string, value: unknown) => {
        setDraftConfig((current) => ({ ...current, [key]: value }));
    };

    return (
        <div className="space-y-6" aria-live="polite">
            <div className="grid gap-4 md:grid-cols-[1fr_auto] items-end">
                <div className="space-y-4">
                    <div className="rounded-xl border border-helios-line/20 bg-helios-surface-soft p-4">
                        <div className="flex items-center gap-2 text-sm font-semibold text-helios-ink">
                            <Bell size={16} className="text-helios-solar" />
                            Daily Summary Time
                        </div>
                        <p className="mt-1 text-xs text-helios-slate">
                            Daily summaries are opt-in per target, but they all use one global local-time send window.
                        </p>
                        <input
                            type="time"
                            value={dailySummaryTime}
                            onChange={(event) => setDailySummaryTime(event.target.value)}
                            className="mt-3 w-full max-w-xs bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                        />
                    </div>
                    <div className="rounded-xl border border-helios-line/20 bg-helios-surface-soft p-4">
                        <div className="flex items-center justify-between gap-4">
                            <div>
                                <div className="text-sm font-semibold text-helios-ink">Quiet Hours</div>
                                <p className="mt-1 text-xs text-helios-slate">
                                    Suppress non-critical notifications inside a local-time window.
                                </p>
                            </div>
                            <label className="inline-flex items-center gap-2 text-sm text-helios-ink">
                                <input
                                    type="checkbox"
                                    checked={quietHoursEnabled}
                                    onChange={(event) => setQuietHoursEnabled(event.target.checked)}
                                />
                                Enabled
                            </label>
                        </div>
                        <div className="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-3 max-w-md">
                            <div>
                                <label className="block text-xs font-medium text-helios-slate mb-1">Start</label>
                                <input
                                    type="time"
                                    value={quietHoursStart}
                                    onChange={(event) => setQuietHoursStart(event.target.value)}
                                    disabled={!quietHoursEnabled}
                                    className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink disabled:opacity-60"
                                />
                            </div>
                            <div>
                                <label className="block text-xs font-medium text-helios-slate mb-1">End</label>
                                <input
                                    type="time"
                                    value={quietHoursEnd}
                                    onChange={(event) => setQuietHoursEnd(event.target.value)}
                                    disabled={!quietHoursEnabled}
                                    className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink disabled:opacity-60"
                                />
                            </div>
                        </div>
                    </div>
                </div>
                <button
                    onClick={() => void saveNotificationSettings()}
                    disabled={savingSchedule}
                    className="rounded-lg border border-helios-line/20 px-4 py-2 text-sm font-semibold text-helios-ink hover:bg-helios-surface-soft transition-colors"
                >
                    {savingSchedule ? "Saving..." : "Save Schedule Settings"}
                </button>
            </div>

            <div className="flex justify-end">
                <button
                    onClick={() => setShowForm((current) => !current)}
                    className="flex items-center gap-2 px-3 py-1.5 bg-helios-surface border border-helios-line/30 hover:bg-helios-surface-soft text-helios-ink rounded-lg text-xs font-medium transition-colors"
                >
                    <Plus size={14} />
                    {showForm ? "Cancel" : "Add Target"}
                </button>
            </div>

            {error && (
                <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error text-sm">
                    {error}
                </div>
            )}

            {showForm && (
                <form onSubmit={handleAdd} className="bg-helios-surface-soft p-4 rounded-xl space-y-4 border border-helios-line/20 mb-6">
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <div>
                            <label className="block text-xs font-medium text-helios-slate mb-1">Name</label>
                            <input
                                value={draftName}
                                onChange={(event) => setDraftName(event.target.value)}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                                placeholder="My Discord"
                                required
                            />
                        </div>
                        <div>
                            <label className="block text-xs font-medium text-helios-slate mb-1">Type</label>
                            <select
                                value={draftType}
                                onChange={(event) => {
                                    const nextType = event.target.value as NotificationTargetType;
                                    setDraftType(nextType);
                                    setDraftConfig(defaultConfigForType(nextType));
                                }}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                            >
                                {TARGET_TYPES.map((type) => (
                                    <option key={type.value} value={type.value}>
                                        {type.label}
                                    </option>
                                ))}
                            </select>
                        </div>
                    </div>

                    {draftType === "discord_webhook" && (
                        <TextField
                            label="Webhook URL"
                            value={String(draftConfig.webhook_url ?? "")}
                            onChange={(value) => setConfigField("webhook_url", value)}
                            placeholder="https://discord.com/api/webhooks/..."
                        />
                    )}

                    {draftType === "discord_bot" && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <TextField
                                label="Bot Token"
                                value={String(draftConfig.bot_token ?? "")}
                                onChange={(value) => setConfigField("bot_token", value)}
                                placeholder="Discord bot token"
                            />
                            <TextField
                                label="Channel ID"
                                value={String(draftConfig.channel_id ?? "")}
                                onChange={(value) => setConfigField("channel_id", value)}
                                placeholder="123456789012345678"
                            />
                        </div>
                    )}

                    {draftType === "gotify" && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <TextField
                                label="Server URL"
                                value={String(draftConfig.server_url ?? "")}
                                onChange={(value) => setConfigField("server_url", value)}
                                placeholder="https://gotify.example.com/message"
                            />
                            <TextField
                                label="App Token"
                                value={String(draftConfig.app_token ?? "")}
                                onChange={(value) => setConfigField("app_token", value)}
                                placeholder="Gotify app token"
                            />
                        </div>
                    )}

                    {draftType === "webhook" && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <TextField
                                label="Endpoint URL"
                                value={String(draftConfig.url ?? "")}
                                onChange={(value) => setConfigField("url", value)}
                                placeholder="https://example.com/webhook"
                            />
                            <TextField
                                label="Bearer Token (Optional)"
                                value={String(draftConfig.auth_token ?? "")}
                                onChange={(value) => setConfigField("auth_token", value)}
                                placeholder="Bearer token"
                            />
                        </div>
                    )}

                    {draftType === "telegram" && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <TextField
                                label="Bot Token"
                                value={String(draftConfig.bot_token ?? "")}
                                onChange={(value) => setConfigField("bot_token", value)}
                                placeholder="Telegram bot token"
                            />
                            <TextField
                                label="Chat ID"
                                value={String(draftConfig.chat_id ?? "")}
                                onChange={(value) => setConfigField("chat_id", value)}
                                placeholder="Telegram chat ID"
                            />
                        </div>
                    )}

                    {draftType === "email" && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                            <TextField
                                label="SMTP Host"
                                value={String(draftConfig.smtp_host ?? "")}
                                onChange={(value) => setConfigField("smtp_host", value)}
                                placeholder="smtp.example.com"
                            />
                            <TextField
                                label="SMTP Port"
                                value={String(draftConfig.smtp_port ?? 587)}
                                onChange={(value) => setConfigField("smtp_port", Number(value))}
                                placeholder="587"
                            />
                            <TextField
                                label="Username"
                                value={String(draftConfig.username ?? "")}
                                onChange={(value) => setConfigField("username", value)}
                                placeholder="Optional"
                            />
                            <TextField
                                label="Password"
                                value={String(draftConfig.password ?? "")}
                                onChange={(value) => setConfigField("password", value)}
                                placeholder="Optional"
                            />
                            <TextField
                                label="From Address"
                                value={String(draftConfig.from_address ?? "")}
                                onChange={(value) => setConfigField("from_address", value)}
                                placeholder="alchemist@example.com"
                            />
                            <TextField
                                label="To Addresses"
                                value={Array.isArray(draftConfig.to_addresses) ? String((draftConfig.to_addresses as string[]).join(", ")) : ""}
                                onChange={(value) =>
                                    setConfigField(
                                        "to_addresses",
                                        value
                                            .split(",")
                                            .map((candidate) => candidate.trim())
                                            .filter(Boolean),
                                    )
                                }
                                placeholder="ops@example.com, alerts@example.com"
                            />
                            <div>
                                <label className="block text-xs font-medium text-helios-slate mb-1">Security</label>
                                <select
                                    value={String(draftConfig.security ?? "starttls")}
                                    onChange={(event) => setConfigField("security", event.target.value)}
                                    className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                                >
                                    <option value="starttls">STARTTLS</option>
                                    <option value="tls">TLS / SMTPS</option>
                                    <option value="none">None</option>
                                </select>
                            </div>
                        </div>
                    )}

                    <div>
                        <label className="block text-xs font-medium text-helios-slate mb-2">Events</label>
                        <div className="flex gap-2 flex-wrap">
                            {EVENT_OPTIONS.map((evt) => (
                                <button
                                    key={evt}
                                    type="button"
                                    onClick={() => toggleEvent(evt)}
                                    className={`rounded-full border px-3 py-2 text-xs font-semibold transition-all ${
                                        draftEvents.includes(evt)
                                            ? "border-helios-solar bg-helios-solar/10 text-helios-ink"
                                            : "border-helios-line/20 text-helios-slate"
                                    }`}
                                >
                                    {evt}
                                </button>
                            ))}
                        </div>
                    </div>

                    <button type="submit" className="w-full bg-helios-solar text-helios-main font-bold py-2 rounded-lg hover:opacity-90 transition-opacity">
                        Save Target
                    </button>
                </form>
            )}

            {loading ? (
                <div className="text-sm text-helios-slate animate-pulse">Loading targets…</div>
            ) : (
                <div className="space-y-3">
                    {targets.map((target) => (
                        <div key={target.id} className="flex items-center justify-between p-4 bg-helios-surface border border-helios-line/10 rounded-xl group/item">
                            <div className="flex items-center gap-4">
                                <div className="p-2 bg-helios-surface-soft rounded-lg text-helios-slate">
                                    <Bell size={18} />
                                </div>
                                <div className="min-w-0">
                                    <h3 className="font-bold text-sm text-helios-ink">{target.name}</h3>
                                    <div className="flex items-center gap-2 mt-0.5 flex-wrap">
                                        <span className="text-xs font-medium text-helios-slate bg-helios-surface-soft px-1.5 rounded">
                                            {target.target_type}
                                        </span>
                                        <span className="text-xs text-helios-slate break-all">
                                            {targetSummary(target)}
                                        </span>
                                    </div>
                                    <div className="mt-2 flex flex-wrap gap-2">
                                        {target.events.map((eventName) => (
                                            <span key={eventName} className="rounded-full border border-helios-line/20 px-2 py-0.5 text-[11px] text-helios-slate">
                                                {eventName}
                                            </span>
                                        ))}
                                    </div>
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                <button
                                    onClick={() => void handleTest(target)}
                                    disabled={testingId === target.id}
                                    className="p-2 text-helios-slate hover:text-helios-solar hover:bg-helios-solar/10 rounded-lg transition-colors"
                                    title="Test Notification"
                                >
                                    <Zap size={16} className={testingId === target.id ? "animate-pulse" : ""} />
                                </button>
                                <button
                                    onClick={() => setPendingDeleteId(target.id)}
                                    className="p-2 text-helios-slate hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors"
                                    aria-label={`Delete notification target ${target.name}`}
                                >
                                    <Trash2 size={16} />
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
            )}

            <ConfirmDialog
                open={pendingDeleteId !== null}
                title="Remove notification target"
                description="Remove this notification target?"
                confirmLabel="Remove"
                tone="danger"
                onClose={() => setPendingDeleteId(null)}
                onConfirm={async () => {
                    if (pendingDeleteId === null) return;
                    await handleDelete(pendingDeleteId);
                }}
            />
        </div>
    );
}

function TextField({
    label,
    value,
    onChange,
    placeholder,
}: {
    label: string;
    value: string;
    onChange: (value: string) => void;
    placeholder: string;
}) {
    return (
        <div>
            <label className="block text-xs font-medium text-helios-slate mb-1">{label}</label>
            <input
                value={value}
                onChange={(event) => onChange(event.target.value)}
                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                placeholder={placeholder}
            />
        </div>
    );
}
