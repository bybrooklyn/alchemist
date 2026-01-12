import { useState, useEffect } from "react";
import { Bell, Plus, Trash2, Zap } from "lucide-react";
import { apiFetch } from "../lib/api";

interface NotificationTarget {
    id: number;
    name: string;
    target_type: 'gotify' | 'discord' | 'webhook';
    endpoint_url: string;
    auth_token?: string;
    events: string; // JSON string
    enabled: boolean;
}

export default function NotificationSettings() {
    const [targets, setTargets] = useState<NotificationTarget[]>([]);
    const [loading, setLoading] = useState(true);
    const [testingId, setTestingId] = useState<number | null>(null);

    // Form state
    const [showForm, setShowForm] = useState(false);
    const [newName, setNewName] = useState("");
    const [newType, setNewType] = useState<NotificationTarget['target_type']>("discord");
    const [newUrl, setNewUrl] = useState("");
    const [newToken, setNewToken] = useState("");
    const [newEvents, setNewEvents] = useState<string[]>(["completed", "failed"]);

    useEffect(() => {
        fetchTargets();
    }, []);

    const fetchTargets = async () => {
        try {
            const res = await apiFetch("/api/settings/notifications");
            if (res.ok) {
                const data = await res.json();
                setTargets(data);
            }
        } catch (e) {
            console.error(e);
        } finally {
            setLoading(false);
        }
    };

    const handleAdd = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            const res = await apiFetch("/api/settings/notifications", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name: newName,
                    target_type: newType,
                    endpoint_url: newUrl,
                    auth_token: newToken || null,
                    events: newEvents,
                    enabled: true
                })
            });
            if (res.ok) {
                setShowForm(false);
                setNewName("");
                setNewUrl("");
                setNewToken("");
                fetchTargets();
            }
        } catch (e) {
            console.error(e);
        }
    };

    const handleDelete = async (id: number) => {
        if (!confirm("Remove this notification target?")) return;
        try {
            await apiFetch(`/api/settings/notifications/${id}`, { method: "DELETE" });
            fetchTargets();
        } catch (e) {
            console.error(e);
        }
    };

    const handleTest = async (target: NotificationTarget) => {
        setTestingId(target.id);
        try {
            // We reconstruct payload from target to send to test endpoint
            // Or test endpoint could take ID if we implemented that.
            // But we implemented endpoint taking Payload.
            // So we send current target data.
            const events = JSON.parse(target.events);

            const res = await apiFetch("/api/settings/notifications/test", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name: target.name,
                    target_type: target.target_type,
                    endpoint_url: target.endpoint_url,
                    auth_token: target.auth_token,
                    events: events,
                    enabled: target.enabled
                })
            });

            if (res.ok) {
                alert("Test notification sent!");
            } else {
                alert("Test failed.");
            }
        } catch (e) {
            console.error(e);
            alert("Test error");
        } finally {
            setTestingId(null);
        }
    };

    const toggleEvent = (evt: string) => {
        if (newEvents.includes(evt)) {
            setNewEvents(newEvents.filter(e => e !== evt));
        } else {
            setNewEvents([...newEvents, evt]);
        }
    };

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <div className="p-2 bg-helios-solar/10 rounded-lg">
                        <Bell className="text-helios-solar" size={20} />
                    </div>
                    <div>
                        <h2 className="text-lg font-semibold text-helios-ink">Notifications</h2>
                        <p className="text-xs text-helios-slate">Alerts for job events via Discord, Gotify, etc.</p>
                    </div>
                </div>
                <button
                    onClick={() => setShowForm(!showForm)}
                    className="flex items-center gap-2 px-3 py-1.5 bg-helios-surface border border-helios-line/30 hover:bg-helios-surface-soft text-helios-ink rounded-lg text-xs font-bold uppercase tracking-wider transition-colors"
                >
                    <Plus size={14} />
                    {showForm ? "Cancel" : "Add Target"}
                </button>
            </div>

            {showForm && (
                <form onSubmit={handleAdd} className="bg-helios-surface-soft p-4 rounded-xl space-y-4 border border-helios-line/20 mb-6">
                    <div className="grid grid-cols-2 gap-4">
                        <div>
                            <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Name</label>
                            <input
                                value={newName}
                                onChange={e => setNewName(e.target.value)}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                                placeholder="My Discord"
                                required
                            />
                        </div>
                        <div>
                            <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Type</label>
                            <select
                                value={newType}
                                onChange={e => setNewType(e.target.value as any)}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink"
                            >
                                <option value="discord">Discord Webhook</option>
                                <option value="gotify">Gotify</option>
                                <option value="webhook">Generic Webhook</option>
                            </select>
                        </div>
                    </div>

                    <div>
                        <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Endpoint URL</label>
                        <input
                            value={newUrl}
                            onChange={e => setNewUrl(e.target.value)}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                            placeholder="https://discord.com/api/webhooks/..."
                            required
                        />
                    </div>

                    <div>
                        <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Auth Token (Optional)</label>
                        <input
                            value={newToken}
                            onChange={e => setNewToken(e.target.value)}
                            className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                            placeholder="Bearer token or API Key"
                        />
                    </div>

                    <div>
                        <label className="block text-xs font-bold uppercase text-helios-slate mb-2">Events</label>
                        <div className="flex gap-4">
                            {['completed', 'failed', 'queued'].map(evt => (
                                <label key={evt} className="flex items-center gap-2 text-sm text-helios-ink cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={newEvents.includes(evt)}
                                        onChange={() => toggleEvent(evt)}
                                        className="rounded border-helios-line/30 bg-helios-surface accent-helios-solar"
                                    />
                                    <span className="capitalize">{evt}</span>
                                </label>
                            ))}
                        </div>
                    </div>

                    <button type="submit" className="w-full bg-helios-solar text-helios-main font-bold py-2 rounded-lg hover:opacity-90 transition-opacity">
                        Save Target
                    </button>
                </form>
            )}

            <div className="space-y-3">
                {targets.map(target => (
                    <div key={target.id} className="flex items-center justify-between p-4 bg-helios-surface border border-helios-line/10 rounded-xl group/item">
                        <div className="flex items-center gap-4">
                            <div className="p-2 bg-helios-surface-soft rounded-lg text-helios-slate">
                                <Zap size={18} />
                            </div>
                            <div>
                                <h3 className="font-bold text-sm text-helios-ink">{target.name}</h3>
                                <div className="flex items-center gap-2 mt-0.5">
                                    <span className="text-[10px] uppercase font-bold tracking-wider text-helios-slate bg-helios-surface-soft px-1.5 rounded">
                                        {target.target_type}
                                    </span>
                                    <span className="text-xs text-helios-slate truncate max-w-[200px]">
                                        {target.endpoint_url}
                                    </span>
                                </div>
                            </div>
                        </div>
                        <div className="flex items-center gap-2">
                            <button
                                onClick={() => handleTest(target)}
                                disabled={testingId === target.id}
                                className="p-2 text-helios-slate hover:text-helios-solar hover:bg-helios-solar/10 rounded-lg transition-colors"
                                title="Test Notification"
                            >
                                <Zap size={16} className={testingId === target.id ? "animate-pulse" : ""} />
                            </button>
                            <button
                                onClick={() => handleDelete(target.id)}
                                className="p-2 text-helios-slate hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors"
                            >
                                <Trash2 size={16} />
                            </button>
                        </div>
                    </div>
                ))}

                {targets.length === 0 && !loading && (
                    <div className="text-center py-8 text-helios-slate text-sm">
                        No notification targets configured.
                    </div>
                )}
            </div>
        </div>
    );
}
