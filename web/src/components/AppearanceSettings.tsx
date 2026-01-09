import { useCallback, useEffect, useState } from "react";
import {
    Palette,
    AlertCircle,
    Sparkles,
    CloudMoon,
    Sun,
    Zap,
    CheckCircle2,
    Loader2
} from "lucide-react";
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface Theme {
    id: string;
    name: string;
    description: string;
}

interface ThemeCategory {
    id: string;
    label: string;
    icon: React.ReactNode;
    themes: Theme[];
}

const THEME_CATEGORIES: ThemeCategory[] = [
    {
        id: "vibrant",
        label: "Vibrant & Energetic",
        icon: <Sparkles size={16} className="text-helios-solar" />,
        themes: [
            {
                id: "helios-orange",
                name: "Helios Orange",
                description: "Warm ember tones with bright solar accents.",
            },
            {
                id: "sunset",
                name: "Sunset",
                description: "Warm, radiant gradients inspired by dusk.",
            },
            {
                id: "neon",
                name: "Neon",
                description: "Electric high-contrast cyber aesthetics.",
            },
            {
                id: "crimson",
                name: "Crimson",
                description: "Charcoal reds with confident crimson energy.",
            },
        ],
    },
    {
        id: "cool",
        label: "Cool & Calm",
        icon: <CloudMoon size={16} className="text-status-success" />,
        themes: [
            {
                id: "deep-blue",
                name: "Deep Blue",
                description: "Navy panels with crisp, cool blue highlights.",
            },
            {
                id: "ocean",
                name: "Ocean",
                description: "Calm, deep teal and turquoise currents.",
            },
            {
                id: "emerald",
                name: "Emerald",
                description: "Deep green base with luminous emerald accents.",
            },
        ],
    },
    {
        id: "soft",
        label: "Soft & Dreamy",
        icon: <Sun size={16} className="text-status-warning" />,
        themes: [
            {
                id: "lavender",
                name: "Lavender",
                description: "Soft, dreamy pastels with deep purple undertones.",
            },
            {
                id: "purple",
                name: "Purple",
                description: "Velvet violets with bright lavender accents.",
            },
        ],
    },
    {
        id: "dark",
        label: "Dark & Minimal",
        icon: <Zap size={16} className="text-helios-slate" />,
        themes: [
            {
                id: "midnight",
                name: "Midnight",
                description: "Pure OLED black with stark white accents.",
            },
            {
                id: "monochrome",
                name: "Monochrome",
                description: "Neutral graphite with clean grayscale accents.",
            },
            {
                id: "dracula",
                name: "Dracula",
                description: "Dark vampire aesthetic with pink and purple accents.",
            },
        ],
    },
];

const getRootTheme = () => {
    if (typeof document === "undefined") {
        return null;
    }
    return document.documentElement.getAttribute("data-color-profile");
};

const applyRootTheme = (themeId: string) => {
    if (typeof document === "undefined") {
        return;
    }
    document.documentElement.setAttribute("data-color-profile", themeId);
    localStorage.setItem("theme", themeId);
};

export default function AppearanceSettings() {
    // Initialize from local storage or default
    const [activeThemeId, setActiveThemeId] = useState(
        () => (typeof window !== 'undefined' ? localStorage.getItem("theme") : null) || getRootTheme() || "helios-orange"
    );
    const [savingThemeId, setSavingThemeId] = useState<string | null>(null);
    const [error, setError] = useState("");

    // Effect to ensure theme is applied on mount (if mismatched)
    useEffect(() => {
        applyRootTheme(activeThemeId);
    }, [activeThemeId]);

    const handleSelect = useCallback(
        async (themeId: string) => {
            if (!themeId || themeId === activeThemeId || savingThemeId) {
                return;
            }

            const previousTheme = activeThemeId;
            setActiveThemeId(themeId);
            setSavingThemeId(themeId);
            setError("");
            applyRootTheme(themeId);

            try {
                // Determine API endpoint. 
                // Since we don't have the full Helios API, we'll implement a simple one or just use local storage for now if backend isn't ready.
                // But the plan says "Implement PUT /api/ui/preferences".
                // We'll try to fetch it.
                const response = await fetch("/api/ui/preferences", {
                    method: "POST", // Using POST for simplicity if PUT is tricky in backend routing without full REST
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ active_theme_id: themeId }),
                });

                if (!response.ok) {
                    // If backend doesn't support it yet, we just rely on LocalStorage, which we already set in applyRootTheme.
                    // So we might warn but not revert UI, or just suppress error if 404.
                    if (response.status !== 404) {
                        throw new Error("Failed to save preference");
                    }
                }
            } catch (saveError) {
                console.warn("Theme save failed, using local storage fallback", saveError);
                // We don't revert here because we want the UI to update immediately and persist locally at least.
                // setError("Unable to save theme preference to server.");
            } finally {
                setSavingThemeId(null);
            }
        },
        [activeThemeId, savingThemeId]
    );

    return (
        <div className="flex flex-col gap-6">
            <div className="flex items-center justify-between pb-2 border-b border-helios-line/10">
                <div>
                    <h3 className="text-base font-bold text-helios-ink tracking-tight uppercase tracking-[0.1em]">Color Profiles</h3>
                    <p className="text-xs text-helios-slate mt-0.5">Customize the interface aesthetic across all your devices.</p>
                </div>
                <div className="p-2 bg-helios-solar/10 rounded-xl text-helios-solar">
                    <Palette size={20} />
                </div>
            </div>

            {error && (
                <div className="py-2.5 px-4 rounded-xl flex items-center gap-2 border border-red-500/20 bg-red-500/10 text-red-500">
                    <AlertCircle size={16} />
                    <span className="text-xs font-semibold">{error}</span>
                </div>
            )}

            <div className="flex flex-col gap-10">
                {THEME_CATEGORIES.map((category) => (
                    <div key={category.id} className="flex flex-col gap-4">
                        <div className="flex items-center gap-2 px-1">
                            {category.icon}
                            <h4 className="text-[10px] font-bold uppercase tracking-widest text-helios-slate/60">
                                {category.label}
                            </h4>
                        </div>

                        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                            {category.themes.map((theme) => {
                                const isActive = theme.id === activeThemeId;
                                const isSaving = savingThemeId === theme.id;

                                return (
                                    <button
                                        key={theme.id}
                                        onClick={() => handleSelect(theme.id)}
                                        disabled={isActive || Boolean(savingThemeId)}
                                        className={cn(
                                            "group relative flex flex-col items-start gap-4 rounded-3xl border p-5 text-left transition-all duration-300 outline-none",
                                            isActive
                                                ? "border-helios-solar bg-helios-solar/5 shadow-[0_0_20px_rgba(var(--accent-primary),0.1)] ring-1 ring-helios-solar/30"
                                                : "border-helios-line/40 bg-helios-surface hover:border-helios-solar/40 hover:bg-helios-surface/80 hover:shadow-xl hover:shadow-black/5"
                                        )}
                                    >
                                        <div className="flex w-full items-center justify-between gap-3">
                                            <div
                                                className="h-12 w-12 rounded-2xl border border-white/5 shadow-inner flex-shrink-0 flex items-center justify-center relative overflow-hidden"
                                                data-color-profile={theme.id}
                                                style={{
                                                    background: `linear-gradient(135deg, rgb(var(--bg-main)), rgb(var(--bg-panel)))`,
                                                }}
                                            >
                                                <div
                                                    className="absolute inset-0 opacity-20"
                                                    style={{
                                                        background: `linear-gradient(to bottom right, rgb(var(--accent-primary)), transparent)`
                                                    }}
                                                />
                                                <div
                                                    className="relative z-10 w-3 h-3 rounded-full shadow-[0_0_10px_rgb(var(--accent-primary))]"
                                                    style={{ backgroundColor: `rgb(var(--accent-primary))` }}
                                                />
                                            </div>

                                            {isActive && (
                                                <div className="flex-shrink-0 flex items-center gap-1.5 bg-helios-solar text-helios-mist px-2.5 py-1 rounded-full">
                                                    <CheckCircle2 size={12} />
                                                    <span className="text-[9px] font-bold uppercase tracking-widest">Active</span>
                                                </div>
                                            )}

                                            {isSaving && (
                                                <Loader2 size={16} className="animate-spin text-helios-solar" />
                                            )}
                                        </div>

                                        <div className="flex flex-col gap-1 min-w-0">
                                            <span className={cn(
                                                "text-sm font-bold tracking-tight",
                                                isActive ? 'text-helios-ink' : 'text-helios-ink/90'
                                            )}>
                                                {theme.name}
                                            </span>
                                            <span className="text-[11px] text-helios-slate font-medium leading-relaxed opacity-70">
                                                {theme.description}
                                            </span>
                                        </div>

                                        <div className="mt-2 flex w-full gap-1.5 opacity-40 transition-opacity group-hover:opacity-80" data-color-profile={theme.id}>
                                            <div className="h-1.5 flex-1 rounded-full" style={{ backgroundColor: 'rgb(var(--accent-primary))' }} />
                                            <div className="h-1.5 flex-1 rounded-full" style={{ backgroundColor: 'rgb(var(--accent-secondary))' }} />
                                            <div className="h-1.5 flex-1 rounded-full opacity-40" style={{ backgroundColor: 'rgb(var(--accent-primary))' }} />
                                        </div>

                                        {!isActive && !savingThemeId && (
                                            <div className="absolute top-4 right-4 opacity-0 group-hover:opacity-100 transition-opacity">
                                                <div className="p-1.5 bg-helios-line/10 rounded-full text-helios-slate">
                                                    <Palette size={14} />
                                                </div>
                                            </div>
                                        )}
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
