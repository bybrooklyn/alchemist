import { useEffect, useMemo, useState } from "react";
import { X, Play, Pencil } from "lucide-react";
import { apiAction, apiJson, isApiError } from "../lib/api";
import { showToast } from "../lib/toast";
import ConfirmDialog from "./ui/ConfirmDialog";
import ServerDirectoryPicker from "./ui/ServerDirectoryPicker";

interface WatchDir {
    id: number;
    path: string;
    is_recursive: boolean;
    profile_id: number | null;
}

interface LibraryProfile {
    id: number;
    name: string;
    preset: string;
    codec: "av1" | "hevc" | "h264";
    quality_profile: "speed" | "balanced" | "quality";
    hdr_mode: "preserve" | "tonemap";
    audio_mode: "copy" | "aac" | "aac_stereo";
    crf_override: number | null;
    notes: string | null;
    builtin: boolean;
}

interface ProfileDraft {
    name: string;
    preset: string;
    codec: "av1" | "hevc" | "h264";
    quality_profile: "speed" | "balanced" | "quality";
    hdr_mode: "preserve" | "tonemap";
    audio_mode: "copy" | "aac" | "aac_stereo";
    crf_override: string;
    notes: string;
}

interface SettingsBundleResponse {
    settings: {
        scanner: {
            directories: string[];
        };
        [key: string]: unknown;
    };
}

function draftFromProfile(profile: LibraryProfile): ProfileDraft {
    return {
        name: profile.builtin ? `${profile.name} Custom` : profile.name,
        preset: profile.preset,
        codec: profile.codec,
        quality_profile: profile.quality_profile,
        hdr_mode: profile.hdr_mode,
        audio_mode: profile.audio_mode,
        crf_override: profile.crf_override === null ? "" : String(profile.crf_override),
        notes: profile.notes ?? "",
    };
}

export default function WatchFolders() {
    const [dirs, setDirs] = useState<WatchDir[]>([]);
    const [profiles, setProfiles] = useState<LibraryProfile[]>([]);
    const [presets, setPresets] = useState<LibraryProfile[]>([]);
    const [dirInput, setDirInput] = useState("");
    const [loading, setLoading] = useState(true);
    const [scanning, setScanning] = useState(false);
    const [assigningDirId, setAssigningDirId] = useState<number | null>(null);
    const [savingProfile, setSavingProfile] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [pendingRemovePath, setPendingRemovePath] = useState<string | null>(null);
    const [pickerOpen, setPickerOpen] = useState<boolean>(false);
    const [customizeDir, setCustomizeDir] = useState<WatchDir | null>(null);
    const [profileDraft, setProfileDraft] = useState<ProfileDraft | null>(null);

    const builtinProfiles = useMemo(
        () => profiles.filter((profile) => profile.builtin),
        [profiles]
    );
    const customProfiles = useMemo(
        () => profiles.filter((profile) => !profile.builtin),
        [profiles]
    );

    const fetchDirs = async () => {
        // Fetch both canonical library dirs and extra watch dirs, merge them for the UI
        const [bundle, watchDirs] = await Promise.all([
            apiJson<SettingsBundleResponse>("/api/settings/bundle"),
            apiJson<WatchDir[]>("/api/settings/watch-dirs")
        ]);

        const merged: WatchDir[] = [];
        const seen = new Set<string>();

        // Canonical roots get mapped to WatchDir structure (id is synthetic/negative, profile_id is null)
        bundle.settings.scanner.directories.forEach((dir, idx) => {
            if (!seen.has(dir)) {
                seen.add(dir);
                merged.push({ id: -(idx + 1), path: dir, is_recursive: true, profile_id: null });
            }
        });

        // Extra watch dirs append (usually they would be stored in the DB)
        watchDirs.forEach(wd => {
            if (!seen.has(wd.path)) {
                seen.add(wd.path);
                merged.push(wd);
            } else {
                // If it exists in both, prefer the DB version so we have a real ID for profiles
                const existing = merged.find(m => m.path === wd.path);
                if (existing) {
                    existing.id = wd.id;
                    existing.profile_id = wd.profile_id;
                }
            }
        });

        setDirs(merged);
    };

    const fetchProfiles = async () => {
        const data = await apiJson<LibraryProfile[]>("/api/profiles");
        setProfiles(data);
    };

    const fetchPresets = async () => {
        const data = await apiJson<LibraryProfile[]>("/api/profiles/presets");
        setPresets(data);
    };

    const refreshAll = async () => {
        try {
            await Promise.all([fetchDirs(), fetchProfiles(), fetchPresets()]);
            setError(null);
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to load watch folders";
            setError(message);
            showToast({ kind: "error", title: "Watch Folders", message });
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        void refreshAll();
    }, []);

    const triggerScan = async () => {
        setScanning(true);
        setError(null);
        try {
            await apiAction("/api/scan/start", { method: "POST" });
            showToast({ kind: "success", title: "Scan", message: "Library scan started." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to start scan";
            setError(message);
            showToast({ kind: "error", title: "Scan", message });
        } finally {
            window.setTimeout(() => setScanning(false), 1200);
        }
    };

    const addDirectory = async (targetPath: string) => {
        const normalized = targetPath.trim();
        if (!normalized) return;
        if (dirs.some((d) => d.path === normalized)) {
            showToast({ kind: "error", title: "Watch Folders", message: "Folder already exists." });
            return;
        }

        try {
            // Add to BOTH config (canonical) and DB (profiles)
            const bundle = await apiJson<SettingsBundleResponse>("/api/settings/bundle");
            if (!bundle.settings.scanner.directories.includes(normalized)) {
                await apiAction("/api/settings/bundle", {
                    method: "PUT",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        ...bundle.settings,
                        scanner: {
                            ...bundle.settings.scanner,
                            directories: [...bundle.settings.scanner.directories, normalized],
                        },
                    }),
                });
            }

            try {
                await apiAction("/api/settings/watch-dirs", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ path: normalized, is_recursive: true }),
                });
            } catch (innerE) {
                // If it's just a duplicate DB error we can ignore it since we successfully added to canonical
                if (!(isApiError(innerE) && innerE.status === 409)) {
                    throw innerE;
                }
            }

            setDirInput("");
            setError(null);
            await fetchDirs();
            showToast({ kind: "success", title: "Watch Folders", message: "Folder added." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to add directory";
            setError(message);
            showToast({ kind: "error", title: "Watch Folders", message });
        }
    };

    const removeDirectory = async (dirPath: string) => {
        const dir = dirs.find((d) => d.path === dirPath);
        if (!dir) return;

        try {
            // Remove from canonical config if present
            const bundle = await apiJson<SettingsBundleResponse>("/api/settings/bundle");
            const filteredDirs = bundle.settings.scanner.directories.filter(candidate => candidate !== dir.path);
            
            if (filteredDirs.length !== bundle.settings.scanner.directories.length) {
                 await apiAction("/api/settings/bundle", {
                    method: "PUT",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                        ...bundle.settings,
                        scanner: {
                            ...bundle.settings.scanner,
                            directories: filteredDirs,
                        },
                    }),
                });
            }

            // Remove from DB if it has a real ID
            if (dir.id > 0) {
                await apiAction(`/api/settings/watch-dirs/${dir.id}`, {
                    method: "DELETE",
                });
            }

            setError(null);
            await fetchDirs();
            showToast({ kind: "success", title: "Watch Folders", message: "Folder removed." });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to remove directory";
            setError(message);
            showToast({ kind: "error", title: "Watch Folders", message });
        }
    };

    const assignProfile = async (dirId: number, profileId: number | null) => {
        // Can only assign profiles to DB-backed rows
        if (dirId < 0) {
            showToast({ kind: "error", title: "Profiles", message: "This directory must be re-added to support profiles." });
            return;
        }

        setAssigningDirId(dirId);
        try {
            await apiAction(`/api/watch-dirs/${dirId}/profile`, {
                method: "PATCH",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ profile_id: profileId }),
            });
            await fetchDirs();
            setError(null);
            showToast({
                kind: "success",
                title: "Profiles",
                message: profileId === null ? "Watch folder now uses global settings." : "Profile assigned.",
            });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to assign profile";
            setError(message);
            showToast({ kind: "error", title: "Profiles", message });
        } finally {
            setAssigningDirId(null);
        }
    };

    const openCustomizeModal = (dir: WatchDir) => {
        if (dir.id < 0) {
             showToast({ kind: "error", title: "Profiles", message: "This directory must be re-added to support custom profiles." });
             return;
        }

        const selectedProfile = profiles.find((profile) => profile.id === dir.profile_id);
        const fallbackPreset =
            presets.find((preset) => preset.preset === "balanced")
            ?? presets[0]
            ?? builtinProfiles[0]
            ?? selectedProfile;

        const baseProfile = selectedProfile ?? fallbackPreset;
        if (!baseProfile) {
            showToast({
                kind: "error",
                title: "Profiles",
                message: "Preset definitions are unavailable right now.",
            });
            return;
        }

        setCustomizeDir(dir);
        setProfileDraft(draftFromProfile(baseProfile));
    };

    const saveCustomProfile = async (event: React.FormEvent) => {
        event.preventDefault();
        if (!customizeDir || !profileDraft) {
            return;
        }

        setSavingProfile(true);
        try {
            const created = await apiJson<LibraryProfile>("/api/profiles", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name: profileDraft.name,
                    preset: profileDraft.preset,
                    codec: profileDraft.codec,
                    quality_profile: profileDraft.quality_profile,
                    hdr_mode: profileDraft.hdr_mode,
                    audio_mode: profileDraft.audio_mode,
                    crf_override: profileDraft.crf_override.trim()
                        ? Number(profileDraft.crf_override)
                        : null,
                    notes: profileDraft.notes.trim() || null,
                }),
            });

            await apiAction(`/api/watch-dirs/${customizeDir.id}/profile`, {
                method: "PATCH",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ profile_id: created.id }),
            });

            await Promise.all([fetchProfiles(), fetchDirs()]);
            setCustomizeDir(null);
            setProfileDraft(null);
            setError(null);
            showToast({
                kind: "success",
                title: "Profiles",
                message: "Custom profile created and assigned.",
            });
        } catch (e) {
            const message = isApiError(e) ? e.message : "Failed to save custom profile";
            setError(message);
            showToast({ kind: "error", title: "Profiles", message });
        } finally {
            setSavingProfile(false);
        }
    };

    return (
        <div className="space-y-6" aria-live="polite">
            <div className="flex items-center justify-between gap-4">
                <div className="space-y-1">
                    <p className="text-sm text-helios-slate">
                        Folders Alchemist scans and watches for new media.
                    </p>
                </div>
                <button
                    onClick={() => void triggerScan()}
                    disabled={scanning}
                    className="flex items-center gap-2 px-3 py-1.5 bg-helios-solar/10 hover:bg-helios-solar/20 text-helios-solar rounded-lg text-xs font-bold transition-colors disabled:opacity-50"
                >
                    <Play size={14} className={scanning ? "animate-spin" : ""} />
                    {scanning ? "Scanning..." : "Scan Now"}
                </button>
            </div>

            {error && (
                <div className="p-3 rounded-lg bg-status-error/10 border border-status-error/30 text-status-error text-sm">
                    {error}
                </div>
            )}

            <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
                <input
                    type="text"
                    value={dirInput}
                    onChange={(e) => setDirInput(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter") {
                            e.preventDefault();
                            void addDirectory(dirInput);
                        }
                    }}
                    placeholder="/path/to/media"
                    className="flex-1 rounded-lg border border-helios-line/40 bg-helios-surface px-4 py-2.5 font-mono text-sm text-helios-ink outline-none transition-colors focus:border-helios-solar"
                />
                <button
                    type="button"
                    onClick={() => setPickerOpen(true)}
                    className="rounded-lg border border-helios-line/30 bg-helios-surface px-4 py-2.5 text-sm font-medium text-helios-slate transition-colors hover:border-helios-solar/40 hover:text-helios-ink"
                >
                    Browse
                </button>
                <button
                    type="button"
                    onClick={() => void addDirectory(dirInput)}
                    disabled={!dirInput.trim()}
                    className="rounded-lg bg-helios-solar px-4 py-2.5 text-sm font-semibold text-helios-main transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
                >
                    Add
                </button>
            </div>

            {loading ? (
                <div className="text-center py-8 text-helios-slate animate-pulse text-sm">
                    Loading folders...
                </div>
            ) : dirs.length > 0 ? (
                <div className="overflow-hidden rounded-lg border border-helios-line/30 bg-helios-surface">
                    {dirs.map((dir, index) => (
                        <div
                            key={dir.path}
                            className={`flex flex-col gap-3 px-4 py-3 ${
                                index < dirs.length - 1 ? "border-b border-helios-line/10" : ""
                            }`}
                        >
                            <div className="flex items-start gap-4">
                                <p
                                    className="min-w-0 flex-1 break-all font-mono text-sm text-helios-slate"
                                    title={dir.path}
                                >
                                    {dir.path}
                                </p>
                                <button
                                    type="button"
                                    onClick={() => setPendingRemovePath(dir.path)}
                                    className="shrink-0 rounded-lg p-1.5 text-helios-slate transition-colors hover:text-status-error"
                                    aria-label={`Remove ${dir.path}`}
                                >
                                    <X size={15} />
                                </button>
                            </div>
                            <div className="flex flex-col gap-2 md:flex-row md:items-center">
                                <select
                                    value={dir.profile_id === null ? "" : String(dir.profile_id)}
                                    onChange={(event) => {
                                        const value = event.target.value;
                                        void assignProfile(
                                            dir.id,
                                            value === "" ? null : Number(value)
                                        );
                                    }}
                                    disabled={assigningDirId === dir.id || dir.id < 0}
                                    className="w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-2 text-sm text-helios-ink outline-none focus:border-helios-solar disabled:opacity-60"
                                >
                                    <option value="">No profile (use global settings)</option>
                                    {builtinProfiles.map((profile) => (
                                        <option key={profile.id} value={profile.id}>
                                            {profile.name}
                                        </option>
                                    ))}
                                    {customProfiles.length > 0 ? (
                                        <option value="divider" disabled>
                                            ──────────
                                        </option>
                                    ) : null}
                                    {customProfiles.map((profile) => (
                                        <option key={profile.id} value={profile.id}>
                                            {profile.name}
                                        </option>
                                    ))}
                                </select>
                                <button
                                    type="button"
                                    onClick={() => openCustomizeModal(dir)}
                                    disabled={dir.id < 0}
                                    className="inline-flex items-center justify-center rounded-lg border border-helios-line/20 bg-helios-surface px-3 py-2 text-helios-slate hover:text-helios-ink hover:bg-helios-surface-soft disabled:opacity-50"
                                    title="Customize profile"
                                >
                                    <Pencil size={14} />
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
            ) : (
                <div className="py-8 text-center">
                    <p className="text-sm text-helios-slate/60">No folders added yet</p>
                    <p className="mt-1 text-sm text-helios-slate/60">
                        Add a folder above or browse the server filesystem
                    </p>
                </div>
            )}

            <ConfirmDialog
                open={pendingRemovePath !== null}
                title="Remove folder"
                description={`Stop watching ${pendingRemovePath} for new media?`}
                confirmLabel="Remove"
                tone="danger"
                onClose={() => setPendingRemovePath(null)}
                onConfirm={async () => {
                    if (pendingRemovePath === null) return;
                    await removeDirectory(pendingRemovePath);
                    setPendingRemovePath(null);
                }}
            />

            {customizeDir && profileDraft ? (
                <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 px-4 backdrop-blur-sm">
                    <div className="w-full max-w-2xl rounded-lg border border-helios-line/20 bg-helios-surface p-6 shadow-2xl">
                        <div className="flex items-start justify-between gap-4">
                            <div>
                                <h3 className="text-lg font-semibold text-helios-ink">Customize Profile</h3>
                                <p className="text-sm text-helios-slate">
                                    Create a custom profile for <span className="font-mono">{customizeDir.path}</span>.
                                </p>
                            </div>
                            <button
                                type="button"
                                onClick={() => {
                                    setCustomizeDir(null);
                                    setProfileDraft(null);
                                }}
                                className="rounded-lg border border-helios-line/20 px-3 py-2 text-sm text-helios-slate hover:bg-helios-surface-soft"
                            >
                                Close
                            </button>
                        </div>

                        <form onSubmit={saveCustomProfile} className="mt-6 space-y-4">
                            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        Name
                                    </label>
                                    <input
                                        type="text"
                                        value={profileDraft.name}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, name: event.target.value })}
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    />
                                </div>
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        Starting preset
                                    </label>
                                    <select
                                        value={profileDraft.preset}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, preset: event.target.value as ProfileDraft["preset"] })}
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    >
                                        {presets.map((preset) => (
                                            <option key={preset.id} value={preset.preset}>
                                                {preset.name}
                                            </option>
                                        ))}
                                    </select>
                                </div>
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        Codec
                                    </label>
                                    <select
                                        value={profileDraft.codec}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, codec: event.target.value as ProfileDraft["codec"] })}
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    >
                                        <option value="av1">AV1</option>
                                        <option value="hevc">HEVC</option>
                                        <option value="h264">H.264</option>
                                    </select>
                                </div>
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        Quality profile
                                    </label>
                                    <select
                                        value={profileDraft.quality_profile}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, quality_profile: event.target.value as ProfileDraft["quality_profile"] })}
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    >
                                        <option value="speed">Speed</option>
                                        <option value="balanced">Balanced</option>
                                        <option value="quality">Quality</option>
                                    </select>
                                </div>
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        HDR mode
                                    </label>
                                    <select
                                        value={profileDraft.hdr_mode}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, hdr_mode: event.target.value as ProfileDraft["hdr_mode"] })}
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    >
                                        <option value="preserve">Preserve</option>
                                        <option value="tonemap">Tonemap</option>
                                    </select>
                                </div>
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        Audio mode
                                    </label>
                                    <select
                                        value={profileDraft.audio_mode}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, audio_mode: event.target.value as ProfileDraft["audio_mode"] })}
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    >
                                        <option value="copy">Copy</option>
                                        <option value="aac">AAC</option>
                                        <option value="aac_stereo">AAC Stereo</option>
                                    </select>
                                </div>
                                <div>
                                    <label className="text-xs font-bold text-helios-slate">
                                        CRF override
                                    </label>
                                    <input
                                        type="number"
                                        value={profileDraft.crf_override}
                                        onChange={(event) => setProfileDraft({ ...profileDraft, crf_override: event.target.value })}
                                        placeholder="Leave blank to use the preset default"
                                        className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                    />
                                </div>
                            </div>

                            <div>
                                <label className="text-xs font-bold text-helios-slate">
                                    Notes
                                </label>
                                <textarea
                                    value={profileDraft.notes}
                                    onChange={(event) => setProfileDraft({ ...profileDraft, notes: event.target.value })}
                                    rows={3}
                                    className="mt-2 w-full rounded-lg border border-helios-line/20 bg-helios-surface-soft px-4 py-3 text-helios-ink outline-none focus:border-helios-solar"
                                />
                            </div>

                            <div className="flex justify-end">
                                <button
                                    type="submit"
                                    disabled={savingProfile}
                                    className="rounded-lg bg-helios-solar px-5 py-3 text-sm font-semibold text-helios-main disabled:opacity-60"
                                >
                                    {savingProfile ? "Saving..." : "Save Custom Profile"}
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            ) : null}

            <ServerDirectoryPicker
                open={pickerOpen}
                title="Select Folder"
                description="Choose a directory for Alchemist to scan and watch for new media."
                onClose={() => setPickerOpen(false)}
                onSelect={(selectedPath) => {
                    setDirInput(selectedPath);
                    setPickerOpen(false);
                }}
            />
        </div>
    );
}
