/** @type {import('tailwindcss').Config} */
export default {
    content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
    theme: {
        extend: {
            fontFamily: {
                display: ["\"Space Grotesk\"", "ui-sans-serif", "system-ui"],
                mono: ["\"IBM Plex Mono\"", "ui-monospace", "monospace"],
            },
            colors: {
                helios: {
                    ink: "rgb(var(--text-primary) / <alpha-value>)",
                    mist: "rgb(var(--bg-main) / <alpha-value>)",
                    main: "rgb(var(--bg-main) / <alpha-value>)",
                    solar: "rgb(var(--accent-primary) / <alpha-value>)",
                    cyan: "rgb(var(--accent-secondary) / <alpha-value>)",
                    slate: "rgb(var(--text-muted) / <alpha-value>)",
                    surface: "rgb(var(--bg-panel) / <alpha-value>)",
                    "surface-soft": "rgb(var(--bg-elevated) / <alpha-value>)",
                    line: "rgb(var(--border-subtle) / <alpha-value>)",
                },
                status: {
                    success: "rgb(var(--status-success) / <alpha-value>)",
                    warning: "rgb(var(--status-warning) / <alpha-value>)",
                    error: "rgb(var(--status-error) / <alpha-value>)",
                },
            },
        },
    },
    plugins: [],
};
