# Feedback Investigation

Date: 2026-05-13
Updated: 2026-06-04
Version context: stable 0.3.3, active 0.3.4

This file records the clarified reports, what was found in the current codebase, likely causes, suggested fixes, and follow-up questions.

## 1. Appearance theme resets after leaving Settings

**Status: RESOLVED in 0.3.3.** Global theme bootstrap now preserves the selected
profile across pages while keeping the server preference authoritative.

### Clarified report
When a user changes the color profile in Settings, the Settings page reflects the selected theme. After navigating back to Dashboard or any other non-Settings page, the UI returns to the default Helios Orange theme. Returning to Settings reapplies the saved user-selected theme.

### Investigation notes
- The global layout only ensures a `data-color-profile` attribute exists and defaults it to `helios-orange`; it does not fetch or restore the saved theme itself.
- The Appearance settings component fetches `/api/ui/preferences` and applies `active_theme_id` after the React settings island loads.
- The backend preference endpoint reads and writes `config.appearance.active_theme_id`, so the saved value can exist, but non-Settings pages never apply it.
- Astro view transitions call the same defaulting logic after swaps, which can leave pages on Helios Orange until a component explicitly applies the user preference.

### Likely cause
Theme application is scoped to `AppearanceSettings` instead of being a global boot-time concern. The layout falls back to Helios Orange and has no persisted client-side cache or global preferences bootstrap.

### Decisions from follow-up
- Server preference should always be the source of truth. A local cache is acceptable only as a temporary first-paint optimization, and must be overwritten by the server value as soon as `/api/ui/preferences` returns.
- Login should honor the saved theme. Setup should not, because a first-run setup user clearly does not have a completed saved appearance preference yet.

### Ways to fix
1. Add a global theme bootstrap in `Layout.astro` that can use a fast cached value only to avoid first-paint flash, then fetch `/api/ui/preferences` and always apply the server preference when it returns.
2. After boot, fetch `/api/ui/preferences` globally on authenticated app pages and on the login page, apply the server value, and update the local cache from the server response.
3. When `AppearanceSettings` saves a theme, write the same theme id to local storage and optionally dispatch a small custom event like `alchemist:theme-changed` so already-mounted header/sidebar islands can react without waiting for navigation. Treat this as immediate UI feedback only; server preference remains authoritative.
4. Keep Helios Orange as the fallback only when there is no cached value and no server preference.
5. Do not add saved-theme bootstrapping to setup until after setup completion; setup should stay deterministic for first-run users.

### Follow-up questions
- Resolved: server preference should always win across devices.
- Resolved: login should honor saved theme; setup should not.

## 2. About menu should match the System modal motion

**Status: RESOLVED in 0.3.3.** About motion now follows the System modal style.

### Clarified report
The About dialog currently feels like it fades in separately. It should visually fit the System modal pattern, including the same “morph into the screen” feel rather than a plain fade.

### Investigation notes
- `AboutDialog` uses an overlay opacity transition plus a content animation from `opacity: 0`, `scale: 0.95`, and `y: 20`.
- `SystemStatus` uses an overlay opacity transition plus content animation from `opacity: 0`, `scale: 0.96`, and `y: 8`, with an explicit short transition: `duration: 0.18`, `ease: [0.22, 1, 0.36, 1]`.
- Both dialogs share similar modal styling: centered fixed overlay, `bg-helios-surface`, `border-helios-line/30`, rounded corners, shadow, and the solar top gradient.
- The main mismatch is the larger About vertical offset, missing explicit transition curve, and less structured header layout.

### Likely cause
About and System modal animations were implemented independently instead of sharing a common modal motion preset/component.

### Decisions from follow-up
- The About dialog should match the System modal motion style.
- The modal should originate from the About button position, then settle into the centered modal, rather than only scaling from the center.

### Ways to fix
1. Capture the About button bounding rectangle in `HeaderActions` before opening the dialog and pass it into `AboutDialog` as the animation origin.
2. Change `AboutDialog` panel animation to start near that button position, using the same final System modal feel: `scale: 0.96`, small vertical motion, and transition `{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }`.
3. Match overlay blur and modal outline details with the System modal (`backdrop-blur-md`, `outline-none`).
4. Rework the About header to follow the System modal structure: icon/title/status cluster on the left and close button on the right.
5. Longer-term, extract a shared modal shell/motion preset so About, System Status, and future dialogs cannot drift apart.

### Follow-up questions
- Resolved: About should match System modal motion and come from the About button position.
- Still open: Should About remain `max-w-lg`, or should it match the System modal size exactly?

## 3. Statistics page loads indefinitely

**Status: RESOLVED in 0.3.3.** Stats loading now tolerates partial endpoint
failures and the supporting queries are indexed.

### Clarified report
The Statistics page shows a loading spinner indefinitely and never displays statistics content.

### Investigation notes
- `StatsCharts` sets `loading` to true initially and clears it in a `finally` block after `Promise.all` resolves or rejects for aggregated, daily, and detailed stats.
- The page also renders `SavingsOverview` before `StatsCharts`, so an issue in either island may be perceived as “Statistics page loading.”
- `StatsCharts` has an `error` state, but after a fetch failure it still falls through to the “No statistics available yet” empty state if `stats` is null, hiding the error message from the user.
- `fetchAllStats` has no timeout or abort path. If any of the three API requests hangs, `Promise.all` never resolves and the spinner never clears.
- Backend routes exist for `/api/stats/aggregated`, `/api/stats/daily`, and `/api/stats/detailed`, and the handlers call database aggregation queries.
- The daily stats query binds `days_str = "30 days"` into `datetime('now', 'start of day', '-' || ?)`, producing a modifier like `-30 days`. This looks valid for SQLite, but it should be checked against the live database and SQLx behavior.

### Likely cause
Most likely one of the stats requests is never completing, and the frontend waits indefinitely because it has no timeout or partial-result rendering. A secondary UX bug hides errors once loading ends with `stats === null`.

### Ways to fix
1. Reproduce with browser DevTools or direct API calls and identify which endpoint hangs: aggregated, daily, detailed, savings, or top-reason-codes.
2. Add request timeouts/abort controllers to the Stats page so the spinner always resolves into a clear error state.
3. Render the `error` state before the `!stats || stats.total_jobs === 0` empty state so failed requests are visible.
4. Avoid all-or-nothing `Promise.all` for non-critical panels. Load aggregated stats first, then daily/detailed/top reasons independently with section-level skeletons/errors.
5. Add backend tracing or timing around the stats DB queries if a specific endpoint is slow or blocked.
6. Consider adding limits/indexes if detailed or reason-code queries are slow on large libraries.

### Follow-up questions
- Does the spinner appear in the main charts area, the savings overview area, or both?
- Can you confirm whether any `/api/stats/...` request stays pending in the browser Network panel?
- Approximately how many jobs/decisions are in your database?

## 4a. Setup screen is poor and needs research/interviews

**Status: OPEN RESEARCH.** Do not begin a broad setup redesign until interviews
cover first-time self-hosters, experienced operators, and Docker/NAS users.

### Clarified report
The first-run setup experience is not good enough. More research and user interviews are needed before deciding the right redesign.

### Investigation notes
- The setup flow is a multi-step React wizard: welcome, admin account, library selection, processing/output/quality, runtime, review.
- The welcome step is very minimal and does not explain what will be configured, how long setup takes, or what information the user should have ready.
- The Library step asks for server filesystem paths and includes a browser, but it may still be unclear for Docker/container users who think in host paths rather than container paths.
- The Processing step combines codec, quality profile, concurrency, skip thresholds, output behavior, subtitle behavior, VMAF, fallback, and deletion into one dense step.
- The flow has validation, but some high-risk choices may not be guided by user intent or plain-language presets.

### Suggested research plan
1. Interview at least three user groups: first-time self-hosters, experienced media-server operators, and Docker/NAS users.
2. Ask users to complete setup from a fresh install while narrating confusion points.
3. Track where users hesitate: choosing library paths, understanding container paths, selecting codec, output location, delete-source behavior, hardware fallback, and schedule/notifications.
4. Collect desired defaults for common goals: “save space safely,” “maximum compatibility,” “best quality,” and “hands-off automation.”
5. Convert findings into a setup design brief before implementing major changes.

### Ways to fix after research
1. Replace the minimal welcome screen with a setup overview and checklist.
2. Split the dense Processing step into goal-based presets first, advanced controls second.
3. Add Docker/NAS-specific copy and path examples in Library selection.
4. Add a safer “dry run / preview first” mental model before anything touches real media.
5. Add progressive disclosure for dangerous settings like source deletion.
6. Improve the review step so it explains consequences in plain language, not just summarizing values.

### Follow-up questions
- Who is the primary setup user: a beginner self-hoster, an advanced media admin, or both?
- Which install path should research prioritize first: Docker, native binary, NAS, or source build?
- What does “terrible” mean most here: confusing wording, visual design, too many choices, broken flow, bad defaults, or missing safety explanations?

## 4b. Settings page is unclear and inconsistent with AlchemistUI

**Status: OPEN RESEARCH.** The current surface works, but its information
architecture and visual consistency still need user-backed direction.

### Clarified report
The Settings page mostly works, but it is unclear and not consistent enough with the rest of AlchemistUI.

### Investigation notes
- Settings uses a vertical tab list and a large content card. The active tab defaults to `watch` after hydration, while the Appearance tab is first in the list.
- The Appearance tab is reachable via `/settings?tab=appearance`, and `/appearance` redirects there.
- Settings labels mix categories and implementation concepts: “Runtime,” “Config,” “Output & Files,” “Library & Intake,” “Hardware,” etc.
- The page includes many distinct settings areas with different internal component styles, which can create inconsistent density and hierarchy.
- Unlike some app surfaces, Settings does not clearly present task-oriented grouping or guided descriptions at the top level.

### Likely cause
Settings has grown organically as more configuration surfaces were added. The tab list is functional but not a cohesive information architecture or visual system.

### Ways to fix
1. Audit each Settings tab for: user goal, primary action, danger level, save behavior, validation behavior, and help text.
2. Rename/group tabs around user tasks rather than backend concepts. For example: Appearance, Library, Encoding, Quality Guardrails, Output Safety, Automation, Notifications, Hardware, Access, Advanced.
3. Add short tab descriptions and/or a top-level settings overview so users know where to go.
4. Standardize section headers, card spacing, field labels, helper text, warning styles, toggles, and save states across all Settings components.
5. Consider moving raw config editing into a clearly marked Advanced area, separated from everyday settings.
6. Decide whether the default tab should be Appearance, Library, or the last visited tab, then make it consistent with user expectations.

### Follow-up questions
- Which Settings areas feel most unclear right now: library paths, encoding, quality, output, automation, notifications, hardware, API tokens, runtime, or config editor?
- Should Settings prioritize “simple mode by default” with advanced controls hidden?
- Should Settings remember the last visited tab across sessions?
