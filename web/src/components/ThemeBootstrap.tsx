import { useEffect } from "react";
import { apiJson } from "../lib/api";
import { applyRootTheme, cacheTheme } from "../lib/theme";

// Mounts globally from Layout.astro. Fetches the server-side theme
// preference once on mount and applies it, then writes it to the cache
// the first-paint Layout script reads. Skips first-run setup, where no
// saved preference exists yet.
export default function ThemeBootstrap() {
    useEffect(() => {
        if (typeof window === "undefined") return;
        if (window.location.pathname.startsWith("/setup")) return;

        let cancelled = false;
        void apiJson<{ active_theme_id?: string | null }>("/api/ui/preferences")
            .then((preferences) => {
                if (cancelled) return;
                const themeId = preferences.active_theme_id;
                if (themeId) {
                    applyRootTheme(themeId);
                    cacheTheme(themeId);
                }
            })
            .catch(() => undefined);

        return () => {
            cancelled = true;
        };
    }, []);

    return null;
}
