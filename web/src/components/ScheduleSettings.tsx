import { useState, useEffect } from "react";
import { Clock, Plus, Trash2, Calendar } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";

interface ScheduleWindow {
    id: number;
    start_time: string;
    end_time: string;
    days_of_week: string;
    enabled: boolean;
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

export default function ScheduleSettings() {
    const [windows, setWindows] = useState<ScheduleWindow[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const [newStart, setNewStart] = useState("00:00");
    const [newEnd, setNewEnd] = useState("08:00");
    const [selectedDays, setSelectedDays] = useState<number[]>([0, 1, 2, 3, 4, 5, 6]);
    const [showForm, setShowForm] = useState(false);
    const [pendingDeleteId, setPendingDeleteId] = useState<number | null>(null);

    useEffect(() => {
        void fetchSchedule();
    }, []);

    const fetchSchedule = async () => {
        try {
            const data = await apiJson<ScheduleWindow[]>("/api/settings/schedule");
            setWindows(data);
            setError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to load schedule windows";
            setError(message);
        } finally {
            setLoading(false);
        }
    };

    const handleAdd = async (e: React.FormEvent) => {
        e.preventDefault();
        try {
            await apiAction("/api/settings/schedule", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    start_time: newStart,
                    end_time: newEnd,
                    days_of_week: selectedDays,
                    enabled: true,
                }),
            });
            setShowForm(false);
            setError(null);
            await fetchSchedule();
            showToast({ kind: "success", title: "Schedule", message: "Schedule added." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to add schedule";
            setError(message);
            showToast({ kind: "error", title: "Schedule", message });
        }
    };

    const handleDelete = async (id: number) => {
        try {
            await apiAction(`/api/settings/schedule/${id}`, { method: "DELETE" });
            setError(null);
            await fetchSchedule();
            showToast({ kind: "success", title: "Schedule", message: "Schedule removed." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to remove schedule";
            setError(message);
            showToast({ kind: "error", title: "Schedule", message });
        }
    };

    const toggleDay = (dayIndex: number) => {
        if (selectedDays.includes(dayIndex)) {
            setSelectedDays(selectedDays.filter(d => d !== dayIndex));
        } else {
            setSelectedDays([...selectedDays, dayIndex].sort());
        }
    };

    const parseDays = (json: string) => {
        try {
            return JSON.parse(json) as number[];
        } catch {
            return [];
        }
    };

    return (
        <div className="space-y-6" aria-live="polite">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <div className="p-2 bg-helios-solar/10 rounded-lg">
                        <Clock className="text-helios-solar" size={20} />
                    </div>
                    <div>
                        <h2 className="text-lg font-semibold text-helios-ink">Active Hours</h2>
                        <p className="text-xs text-helios-slate">Restrict processing to specific times (e.g. overnight).</p>
                    </div>
                </div>
                <button
                    onClick={() => setShowForm(!showForm)}
                    className="flex items-center gap-2 px-3 py-1.5 bg-helios-surface border border-helios-line/30 hover:bg-helios-surface-soft text-helios-ink rounded-lg text-xs font-bold uppercase tracking-wider transition-colors"
                >
                    <Plus size={14} />
                    {showForm ? "Cancel" : "Add Schedule"}
                </button>
            </div>

            {error && (
                <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error text-sm">
                    {error}
                </div>
            )}

            {loading ? (
                <div className="text-sm text-helios-slate animate-pulse">Loading schedules…</div>
            ) : windows.length > 0 ? (
                <div className="p-4 bg-yellow-500/10 border border-yellow-500/20 rounded-xl mb-4">
                    <p className="text-xs text-yellow-600 dark:text-yellow-400 font-medium flex items-center gap-2">
                        <Calendar size={14} />
                        Processing is restricted to the windows below. Outside these times, the engine will pause automatically.
                    </p>
                </div>
            ) : (
                <div className="p-4 bg-green-500/10 border border-green-500/20 rounded-xl mb-4">
                    <p className="text-xs text-green-600 dark:text-green-400 font-medium flex items-center gap-2">
                        <Clock size={14} />
                        No schedules active. Processing is allowed 24/7.
                    </p>
                </div>
            )}

            {showForm && (
                <form onSubmit={handleAdd} className="bg-helios-surface-soft p-4 rounded-xl space-y-4 border border-helios-line/20 mb-6">
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <div>
                            <label className="block text-xs font-bold uppercase text-helios-slate mb-1">Start Time</label>
                            <input
                                type="time"
                                value={newStart}
                                onChange={e => setNewStart(e.target.value)}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                                required
                            />
                        </div>
                        <div>
                            <label className="block text-xs font-bold uppercase text-helios-slate mb-1">End Time</label>
                            <input
                                type="time"
                                value={newEnd}
                                onChange={e => setNewEnd(e.target.value)}
                                className="w-full bg-helios-surface border border-helios-line/20 rounded p-2 text-sm text-helios-ink font-mono"
                                required
                            />
                        </div>
                    </div>

                    <div>
                        <label className="block text-xs font-bold uppercase text-helios-slate mb-2">Days</label>
                        <div className="flex gap-2 flex-wrap">
                            {DAYS.map((day, idx) => (
                                <button
                                    key={day}
                                    type="button"
                                    onClick={() => toggleDay(idx)}
                                    className={`px-3 py-1.5 rounded-lg text-xs font-bold transition-colors ${
                                        selectedDays.includes(idx)
                                            ? "bg-helios-solar text-helios-main"
                                            : "bg-helios-surface border border-helios-line/30 text-helios-slate hover:bg-helios-surface-soft"
                                    }`}
                                >
                                    {day}
                                </button>
                            ))}
                        </div>
                    </div>

                    <button type="submit" className="w-full bg-helios-solar text-helios-main font-bold py-2 rounded-lg hover:opacity-90 transition-opacity">
                        Save Schedule
                    </button>
                </form>
            )}

            <div className="space-y-3">
                {windows.map(win => (
                    <div key={win.id} className="flex items-center justify-between p-4 bg-helios-surface border border-helios-line/10 rounded-xl">
                        <div>
                            <div className="flex items-center gap-3">
                                <span className="text-xl font-mono font-bold text-helios-ink">
                                    {win.start_time} - {win.end_time}
                                </span>
                                {win.enabled ? (
                                    <span className="text-[10px] uppercase font-bold text-green-500 bg-green-500/10 px-2 py-0.5 rounded-full">Active</span>
                                ) : (
                                    <span className="text-[10px] uppercase font-bold text-red-500 bg-red-500/10 px-2 py-0.5 rounded-full">Disabled</span>
                                )}
                            </div>
                            <div className="flex gap-1 mt-2">
                                {DAYS.map((day, idx) => {
                                    const active = parseDays(win.days_of_week).includes(idx);
                                    return (
                                        <span key={day} className={`text-[10px] font-bold px-1.5 rounded ${active ? "text-helios-solar bg-helios-solar/10" : "text-helios-slate/30"}`}>
                                            {day}
                                        </span>
                                    );
                                })}
                            </div>
                        </div>
                        <button
                            onClick={() => setPendingDeleteId(win.id)}
                            className="p-2 text-helios-slate hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors"
                            aria-label={`Delete schedule ${win.start_time}-${win.end_time}`}
                        >
                            <Trash2 size={16} />
                        </button>
                    </div>
                ))}
            </div>

            <ConfirmDialog
                open={pendingDeleteId !== null}
                title="Remove schedule"
                description="Remove this schedule window?"
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
