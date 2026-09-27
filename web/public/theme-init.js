/* First-paint theme application. Loaded as a classic (parser-blocking)
 * script from <head> before any body content is parsed so the cached
 * profile is applied before the browser's first paint — placing this in
 * <body> or loading it deferred leaves a visible flash of the default
 * theme. Reads the cached theme id from localStorage so navigating between
 * pages doesn't snap back to the default. Server preference remains
 * authoritative — the ThemeBootstrap island fetches /api/ui/preferences
 * after hydration and overwrites the cache when it returns.
 *
 * Kept as a standalone file (instead of an inline <script>) so the
 * Content-Security-Policy can omit 'unsafe-inline' from script-src.
 * Keep this file free of dynamic imports: it must run synchronously
 * during parsing. If this file changes, no CSP update is needed
 * (script-src 'self' covers it).
 */
function initTheme() {
    try {
        var cached = localStorage.getItem("theme");
        document.documentElement.setAttribute(
            "data-color-profile",
            cached || "helios-orange",
        );
    } catch (e) {
        document.documentElement.setAttribute(
            "data-color-profile",
            "helios-orange",
        );
    }
}

initTheme();
document.addEventListener("astro:after-swap", initTheme);
