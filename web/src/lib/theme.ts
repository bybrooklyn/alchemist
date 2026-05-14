// Shared theme helpers. The data-color-profile attribute on <html> drives
// the CSS theme; the localStorage cache backs the first-paint script in
// Layout.astro so cross-page navigation doesn't flash the default.

export const THEME_STORAGE_KEY = "theme";
export const DEFAULT_THEME_ID = "helios-orange";

export function applyRootTheme(themeId: string) {
    if (typeof document === "undefined") return;
    document.documentElement.setAttribute("data-color-profile", themeId);
}

export function getRootTheme(): string | null {
    if (typeof document === "undefined") return null;
    return document.documentElement.getAttribute("data-color-profile");
}

export function cacheTheme(themeId: string) {
    try {
        localStorage.setItem(THEME_STORAGE_KEY, themeId);
    } catch {
        // Storage may be unavailable (private mode, quota); first-paint then
        // falls back to the default, which is acceptable.
    }
}
