# Alchemist macOS SwiftUI Client Specification

**Document:** `swift.md`
**Project:** Alchemist
**Target:** Native macOS client
**Primary UI framework:** SwiftUI
**Design direction:** Modern Apple-native, Liquid Glass-first, Alchemist-branded
**Backend assumption:** Alchemist core already handles transcoding, scanning, queues, configuration, ffmpeg orchestration, and API control. The SwiftUI app is a native client for the existing core/API.

---

## 0. Locked Product Decisions

These decisions are current as of 2026-04-30.

```text
Primary user
- Normal Mac user first.

Repo location
- Native app source lives under native/mac.
- Keep native app build products, Swift package files, docs, and tests separate
  from Rust backend, web UI, docs site, and e2e tests.

Runtime
- Bundled mode is the default: Alchemist.app ships with the Rust daemon.
- The Swift app starts the bundled daemon and talks to it over the current API.
- Remote/API-only mode remains supported by design, but is later work.

Product role
- Premium native companion, not a WebUI replacement on day one.
- WebUI parity should land feature area by feature area, starting with Jobs.

API
- The Swift prototype targets the locked `/api/v1/...` routes.
- The Swift API client must centralize route paths so compatibility changes do
  not touch view code.

Storage
- Bundled mode stores config, database, logs, temp files, and support files in:
  `~/Library/Application Support/Alchemist/`
- Remote mode does not own server storage; it is just a client.

Auth
- Require normal Alchemist login.
- Do not silently bypass auth for local bundled mode.

Visual priority
- Beautiful Dashboard first.
- Dense WebUI parity features must stay native and readable, with Liquid Glass
  reserved for command/navigation surfaces.

Add Media
- V1 design must account for all three flows:
  1. enqueue existing local files,
  2. add watched folders,
  3. upload/stage a single-file conversion/remux.

Settings
- Basics first: connection, Appearance, API/WebUI basics.
- Full settings parity remains the long-term target.

macOS support
- Target the newest macOS for the native design.
- If Liquid Glass APIs are unavailable on an older macOS, skip those effects
  instead of replacing the whole design.

First artifacts
- Refined spec.
- SwiftUI prototype.
- API contract checklist.
- `just` recipes for native app build/run/check plus whole-project flows.
```

---

## 1. Product Goal

The Alchemist macOS app should feel like a first-class Apple platform app, not a web dashboard wrapped in a window.

The goal is to build a beautiful, modern, deeply native macOS client that controls Alchemist through the standardized API while preserving Alchemist’s own identity: transformation, speed, media optimization, local control, and FOSS power-user energy.

The app should feel like:

- A native macOS utility
- A premium media-processing control center
- A modern Apple app using Liquid Glass where appropriate
- A serious alternative to clunky transcoding dashboards
- A polished front-end for Alchemist’s Rust backend

The app should **not** feel like:

- A browser app pretending to be native
- A generic admin dashboard
- A gamer RGB utility
- A web UI port with native buttons slapped on
- A settings-heavy nightmare with no visual hierarchy

---

## 2. Core Design Principle

> **SwiftUI is the face. Alchemist core is the brain.**

The SwiftUI app must not duplicate transcoding logic, queue logic, ffmpeg orchestration, scan rules, codec decision-making, or configuration behavior.

The SwiftUI app should:

- Launch or connect to the Alchemist daemon/core
- Authenticate if needed
- Display app state
- Send user actions to the API
- Subscribe to live updates
- Edit configuration through official API/config models
- Provide a native macOS experience around the existing engine

The SwiftUI app should not:

- Directly shell out to ffmpeg for normal jobs
- Maintain a separate job queue
- Maintain a separate configuration format
- Reimplement backend validation rules
- Store theme state in `UserDefaults` or local-only storage if the canonical config belongs to Alchemist
- Treat the WebUI as the source of truth

---

## 3. Architecture

### 3.1 Preferred App Structure

```text
Alchemist.app
├── SwiftUI frontend
├── bundled alchemistd / Rust core binary
├── local API client
├── config bridge
├── update/about system
├── native settings window
├── optional WebUI launcher/toggle
└── native macOS integration layer
```

### 3.2 Runtime Model

The app should support two runtime modes:

```text
Bundled Mode
- Alchemist.app ships with alchemistd
- App starts/stops daemon automatically
- Talks to the current Alchemist API on localhost
- Uses ~/Library/Application Support/Alchemist for bundled app state
- Best for normal macOS users

Remote Mode
- App connects to an existing Alchemist server
- Useful for homelabs, servers, and remote machines
- Later than bundled v1
```

### 3.3 Communication

Preferred local communication options, in order:

1. Unix domain socket
2. Localhost HTTP API
3. Localhost HTTPS API with local certs, if auth/security requirements demand it

For first implementation, localhost HTTP is acceptable if bound only to `127.0.0.1` and protected appropriately when needed.

Example:

```text
http://127.0.0.1:3000/api
```

Long-term local transport:

```text
~/Library/Application Support/Alchemist/alchemist.sock
```

### 3.4 API Contract and Versioning

The SwiftUI prototype targets the locked `/api/v1` API:

```text
/api/v1/system/info
/api/v1/stats
/api/v1/stats/savings
/api/v1/engine/status
/api/v1/jobs
/api/v1/jobs/enqueue
/api/v1/settings/watch-dirs
/api/v1/conversion/uploads
```

All current paths must be centralized in the Swift API client.

The app should display compatibility errors clearly:

```text
This version of Alchemist.app requires Alchemist API v1.2 or newer.
Detected API version: v1.0.
```

---

## 4. Apple Design Direction

### 4.1 Design Philosophy

The app should follow Apple platform conventions first, then layer Alchemist’s branding on top.

Alchemist should look native because native apps feel more trustworthy for system-level tasks like file processing, media optimization, background services, GPU usage, and local configuration.

Design priorities:

1. Clarity
2. Native structure
3. Strong hierarchy
4. Fast comprehension
5. Beautiful materials
6. Alchemist identity
7. Accessibility
8. Power-user depth without clutter

### 4.2 Modern Apple Look

Use:

- Liquid Glass materials
- Native sidebars
- Native toolbars
- Native inspectors
- Native sheets
- Native settings windows
- Native file pickers
- Native notifications
- Native menu bar commands
- Native keyboard shortcuts
- Native window restoration
- Native system colors
- Native typography

Avoid:

- Fake custom chrome unless necessary
- Over-customized controls
- Web-dashboard style cards everywhere
- Large random gradients
- Excessive blur behind dense text
- Tiny text in translucent panels
- Nonstandard window controls
- Custom scroll behavior

---

## 5. Liquid Glass Design System

### 5.1 Purpose

Liquid Glass should make Alchemist feel modern, alive, and Apple-native. It should not make the app unreadable.

Use Liquid Glass for structural and interactive surfaces, not dense information surfaces. Current Apple guidance says Liquid Glass is a functional layer for controls and navigation, while the content layer should remain readable and structurally clear.

Good places for Liquid Glass:

- Sidebar background
- Toolbar background
- Floating action controls
- Inspector background
- Empty states
- About screen hero area
- Modal sheets
- Encoding summary cards
- Mini progress overlays
- App icon and branding surfaces

Bad places for Liquid Glass:

- Large log viewers
- Dense job tables
- Long settings forms
- File path lists
- Error details
- ffmpeg command output
- Anything where contrast matters more than beauty

Implementation rules:

- Prefer standard SwiftUI containers and controls first: `NavigationSplitView`,
  `toolbar`, sheets, popovers, menus, `Form`, and native controls.
- Use `.buttonStyle(.glass)` and `.buttonStyle(.glassProminent)` for important
  command surfaces before inventing custom effects.
- Use `glassEffect(_:in:)` only on custom controls that need a distinct
  functional glass layer.
- Wrap nearby custom glass controls in `GlassEffectContainer`.
- Group toolbar actions with `ToolbarItemGroup` and `ToolbarSpacer`.
- Avoid custom backgrounds on sidebars and toolbars that would fight system
  Liquid Glass or scroll-edge effects.
- Use `.searchable` on navigation surfaces for Queue and Library search.
- Keep dense lists/tables on solid or standard material surfaces.

### 5.2 Liquid Glass Rule

> **Glass frames the content. It should not fight the content.**

Dense content should live on solid or near-solid surfaces. Glass should be used around navigation, context, and focus.

### 5.3 Material Layers

Suggested visual layering:

```text
Window background
└── subtle system material / native background
    ├── glass sidebar
    ├── glass toolbar
    ├── solid main content panels
    └── glass inspector / floating details
```

### 5.4 Liquid Glass Intensity

Support a material preference in Alchemist config:

```toml
[ui]
material = "adaptive" # adaptive | reduced | solid
```

Behavior:

```text
adaptive
- Uses Liquid Glass where appropriate
- Default modern appearance

reduced
- Less blur/transparency
- Better contrast
- Good for accessibility and older hardware

solid
- Minimal glass
- Mostly opaque native surfaces
- Best for users who dislike transparency
```

### 5.5 Accessibility Requirements for Glass

The app must respect:

- Reduce Transparency
- Increase Contrast
- Reduce Motion
- Dark Mode
- Light Mode
- System accent preferences where applicable
- Dynamic Type where practical
- VoiceOver labels

Glass-heavy UI should degrade gracefully.

If Reduce Transparency is enabled, Liquid Glass surfaces should become more opaque.

If Increase Contrast is enabled, text and separator contrast must increase.

If Reduce Motion is enabled, morphing transitions and animated background effects must be reduced or disabled.

---

## 6. Alchemist Theme System

### 6.1 Theme Storage

Theme must be stored in Alchemist’s canonical config, not local-only client storage.

```toml
[ui]
theme = "system" # system | light | dark
accent = "helios-orange" # helios-orange | alchemist-green | system | custom
material = "adaptive" # adaptive | reduced | solid
density = "comfortable" # compact | comfortable | spacious
```

The SwiftUI app may cache the current theme temporarily in memory, but the canonical value lives in config/API.

### 6.2 Theme Responsibilities

Theme controls:

- App appearance preference
- Accent color
- Logo variant
- Progress highlights
- Status accents
- Material intensity
- Density

Theme should not override every native system color.

### 6.3 Alchemist Accent Colors

Recommended default accents:

```text
Helios Orange
- Primary brand accent
- Used for primary actions, delta arrow, active queue state

Alchemist Green
- Success, optimized, completed, savings

System Blue
- Optional native fallback

Graphite
- Low-saturation professional mode
```

### 6.4 Color Usage

Use accent color sparingly.

Good uses:

- Primary buttons
- Selected navigation item accent
- Progress ring highlights
- Logo mark
- Active job indicator
- Small charts

Bad uses:

- Entire backgrounds
- Whole sidebars
- Every icon
- Logs
- Huge gradients

---

## 7. Brand Identity

### 7.1 Logo Direction

Alchemist needs a native-friendly logo.

Concept:

```text
Stylized Delta ∆
Top-right stroke becomes a Helios-orange arrow
Sharp geometric silhouette
Readable at 16px
Works monochrome
Works in light/dark modes
Works as app icon glyph
```

Symbol meaning:

```text
∆ = transformation, change, conversion
Arrow = forward motion, optimization, queue progress
Orange = Helios lineage / energy / speed
Alchemist = turning media into a better form
```

### 7.2 App Icon

The app icon should follow modern macOS icon conventions:

- Rounded square base
- Dimensional but not skeuomorphic-heavy
- Delta glyph centered
- Orange arrow integrated into top-right stroke
- Subtle depth/glass reflection compatible with modern macOS
- Distinct silhouette at small sizes

Icon variants:

```text
Default
Dark mode
Monochrome template
Menu bar template
Document/file association icon, optional later
```

---

## 8. App Navigation

### 8.1 Primary Layout

Use a three-column app structure where appropriate:

```text
Sidebar        Main Content             Inspector
Navigation     List / Dashboard          Details / Logs / Actions
```

SwiftUI structure:

```swift
NavigationSplitView {
    SidebarView()
} content: {
    PrimaryContentView()
} detail: {
    DetailView()
}
```

### 8.2 Sidebar Sections

Recommended sidebar:

```text
Dashboard
Queue
Library
Profiles
Activity
System
Settings
About
```

Optional future sections:

```text
Plugins
Integrations
Remote Servers
MCP
Jellyfin
```

### 8.3 Sidebar Behavior

Sidebar should:

- Use native source-list styling
- Show active section clearly
- Use SF Symbols
- Support keyboard navigation
- Collapse naturally on smaller windows
- Preserve selected section between launches
- Use glass/material background when appropriate

### 8.4 Main Content Behavior

Main content changes based on selected section.

Main content should usually be solid or lightly material-backed, because it contains dense information.

### 8.5 Inspector Behavior

Inspector should show contextual details for the selected object:

```text
Selected job
Selected media file
Selected encoding profile
Selected watched folder
Selected system diagnostic item
```

Inspector should be toggleable from toolbar and keyboard shortcut.

Suggested shortcut:

```text
⌘⌥I = Toggle Inspector
```

---

## 9. Screens

## 9.1 Dashboard

### Purpose

High-level overview of Alchemist state.

### Content

```text
Current active job
Queue summary
Files optimized today
Storage saved
Average encode speed
GPU/CPU acceleration state
Recent completed jobs
System health
Update status
```

### Design

Use beautiful summary cards with restrained Liquid Glass.

Suggested cards:

```text
Active Encode
Queue
Storage Saved
Hardware Acceleration
Recent Activity
```

### Actions

```text
Add Media
Start Queue
Pause Queue
Open Logs
Open Settings
```

### Empty State

If no jobs exist:

```text
No media queued yet.
Drop files here or add a watched folder to begin optimizing your library.
```

Use a large delta glyph with subtle glass treatment.

---

## 9.2 Queue

### Purpose

Manage all encoding jobs.

### Content

```text
Active jobs
Pending jobs
Paused jobs
Failed jobs
Completed jobs
```

### Job Row Fields

```text
Filename
Source codec
Target codec
Container
Progress
ETA
Speed
Hardware encoder
Output size estimate
Status
```

### Job States

```text
Queued
Scanning
Encoding
Muxing
Verifying
Completed
Failed
Paused
Cancelled
```

### Design

Use a native list/table with optional card-style rows for active jobs.

Active job should have a more visual progress display:

```text
Circular progress or horizontal progress bar
Live speed
ETA
Current ffmpeg stage
```

### Actions

```text
Start
Pause
Resume
Cancel
Retry
Reveal in Finder
Show Logs
Show ffmpeg Command
Remove from Queue
```

### Inspector for Selected Job

```text
Input path
Output path
Profile
Codec settings
ffmpeg command
Progress events
Logs
Errors
Retry action
```

Logs should be displayed on a solid surface, not glass.

---

## 9.3 Library

### Purpose

Manage watched folders and scanned media.

### Content

```text
Watched folders
Scan status
Detected media
Optimization suggestions
Ignored files
Recently changed files
```

### Actions

```text
Add watched folder
Remove watched folder
Scan now
Pause scan
Reveal in Finder
Ignore file
Queue selected files
```

### Design

Use split layout:

```text
Watched folders list
↓
Media results table/list
↓
Inspector with metadata and optimization recommendation
```

### Drag and Drop

Support dragging files/folders into:

- Dashboard
- Queue
- Library
- Dock icon, if feasible

Dropping media should open an import/queue sheet.

---

## 9.4 Profiles

### Purpose

Create and manage encoding profiles.

### Default Profiles

```text
AV1 Balanced
AV1 Quality
HEVC Balanced
HEVC Compatibility
H.264 Compatibility
Remux Only
Audio Normalize, optional future
```

### Profile Fields

```text
Name
Description
Video codec
Audio codec behavior
Container
Quality mode
CRF/CQ value
Preset
Hardware acceleration preference
CPU fallback policy
Subtitle behavior
Metadata behavior
Output naming rule
Append string
Replace original behavior
```

### Design

Profiles should feel like native Settings + document editor.

Use:

- List of profiles on left
- Profile editor on right
- Inspector/preview for expected ffmpeg behavior

### Actions

```text
Create Profile
Duplicate Profile
Delete Profile
Reset Default Profiles
Export Profile
Import Profile
```

---

## 9.5 Activity

### Purpose

Historical timeline of what Alchemist has done.

### Content

```text
Completed jobs
Failed jobs
Skipped files
Scans
Config changes
Updates
Warnings
```

### Design

Timeline/list hybrid.

Group by:

```text
Today
Yesterday
This Week
Older
```

### Actions

```text
Filter by type
Search activity
Open related file
Copy log excerpt
Retry failed job
```

---

## 9.6 System

### Purpose

Diagnostics and environment visibility.

### Content

```text
Alchemist version
API version
Daemon status
ffmpeg path
ffmpeg version
Detected hardware encoders
CPU info
GPU info
Config path
Database path
Log path
WebUI status
Port
Update channel
```

### Actions

```text
Run Doctor
Copy Diagnostics
Open Config Folder
Open Logs Folder
Restart Daemon
Check for Updates
```

### Doctor Checks

```text
ffmpeg available
ffprobe available
config readable
database writable
output directory writable
hardware encoder available
API reachable
daemon healthy
permissions valid
```

### Design

System page should be clean and utilitarian.

Use glass only for high-level status cards. Use solid surfaces for diagnostic tables.

---

## 9.7 Settings

### Purpose

Native settings for Alchemist.

Use a real macOS Settings window where appropriate, not a fake web settings panel.

Suggested sections:

```text
General
Appearance
Encoding
Library
Network/API
WebUI
Updates
Advanced
```

### General

```text
Launch at login
Start daemon when app opens
Keep daemon running after window closes
Show menu bar item
Default output folder
Default behavior after encode
```

### Appearance

```text
Theme: System / Light / Dark
Accent: Helios Orange / Alchemist Green / System / Custom
Material: Adaptive / Reduced / Solid
Density: Compact / Comfortable / Spacious
Show advanced metrics on dashboard
```

### Encoding

```text
Default profile
Allow CPU fallback
Max concurrent jobs
Priority
Output naming
Replace original behavior
Verification behavior
```

### Library

```text
Watched folders
Ignored paths
Allowed extensions
Scan interval
Auto-queue rules
```

### Network/API

```text
API host
API port
Local-only mode
Authentication
Token management
Remote server connections, future
```

### WebUI

```text
Enable WebUI
WebUI port
Open WebUI
Disable WebUI entirely
```

### Updates

```text
Update channel: Stable / Release Candidate / Nightly
Check automatically
Download automatically
Install updates manually
Show release notes
```

### Advanced

```text
ffmpeg path
ffprobe path
Database maintenance
Export config
Import config
Reset app state
Factory reset
```

---

## 9.8 About Screen

### Purpose

Beautiful branding + useful technical info.

### Design

The About screen should be one of the prettiest parts of the app.

Use:

- Large Liquid Glass hero surface
- Delta logo
- Version/build info
- License info
- ffmpeg status
- API status
- Update check

### Fields

```text
Alchemist version
Build commit
Build date
API version
Daemon version
Swift app version
ffmpeg version
License
Website/docs link
GitHub link
Credits
```

### Interaction

The About screen could morph similarly to a system modal: compact at first, expandable for technical diagnostics.

Modes:

```text
Simple
Technical
Licenses
Diagnostics
```

---

## 10. Menu Bar

### 10.1 App Menu

```text
Alchemist
├── About Alchemist
├── Settings…
├── Check for Updates…
├── Services
├── Hide Alchemist
├── Hide Others
├── Show All
└── Quit Alchemist
```

### 10.2 File Menu

```text
File
├── Add Media…
├── Add Folder…
├── New Profile…
├── Import Profile…
├── Export Profile…
└── Reveal Output Folder
```

### 10.3 Queue Menu

```text
Queue
├── Start Queue
├── Pause Queue
├── Resume Queue
├── Cancel Active Job
├── Retry Failed Jobs
└── Clear Completed Jobs
```

### 10.4 View Menu

```text
View
├── Dashboard
├── Queue
├── Library
├── Profiles
├── Activity
├── System
├── Toggle Sidebar
├── Toggle Inspector
└── Customize Toolbar…
```

### 10.5 Window Menu

Use standard macOS behavior.

### 10.6 Help Menu

```text
Help
├── Alchemist Help
├── Documentation
├── Report an Issue
├── Copy Diagnostics
└── Open Logs Folder
```

---

## 11. Toolbar

### 11.1 Main Toolbar Actions

Recommended toolbar:

```text
Add Media
Start/Pause Queue
Search
Filter
Inspector Toggle
System Status
```

Toolbar should be native and grouped logically.

Avoid cramming too many icons into the toolbar. Important actions only.

### 11.2 Contextual Toolbar

Toolbar can change subtly by section:

Dashboard:

```text
Add Media, Start Queue, Check Updates
```

Queue:

```text
Start, Pause, Retry Failed, Clear Completed, Filter
```

Library:

```text
Add Folder, Scan Now, Queue Selected
```

Profiles:

```text
New Profile, Duplicate, Import, Export
```

System:

```text
Run Doctor, Copy Diagnostics, Restart Daemon
```

---

## 12. Real-Time Updates

The SwiftUI app needs live state updates.

Preferred mechanisms:

1. Server-Sent Events
2. WebSockets
3. Polling fallback

Events needed:

```text
job.created
job.updated
job.progress
job.completed
job.failed
queue.started
queue.paused
scan.started
scan.updated
scan.completed
config.updated
system.updated
log.appended
```

SwiftUI should update reactively using observable state models.

---

## 13. Data Models

### 13.1 Job Model

```json
{
  "id": "job_123",
  "input_path": "/Movies/Input.mkv",
  "output_path": "/Movies/Input.alchemist.mkv",
  "status": "encoding",
  "progress": 0.42,
  "stage": "video_encode",
  "source_codec": "h264",
  "target_codec": "av1",
  "container": "mkv",
  "profile_id": "av1-balanced",
  "speed": "2.4x",
  "eta_seconds": 912,
  "hardware_encoder": "videotoolbox",
  "created_at": "2026-04-30T00:00:00Z",
  "updated_at": "2026-04-30T00:12:00Z"
}
```

### 13.2 Profile Model

```json
{
  "id": "av1-balanced",
  "name": "AV1 Balanced",
  "video_codec": "av1",
  "audio_mode": "copy_when_possible",
  "container": "mkv",
  "quality_mode": "cq",
  "quality_value": 30,
  "preset": "medium",
  "allow_cpu_fallback": true,
  "append": "alchemist"
}
```

### 13.3 System Model

```json
{
  "app_version": "0.4.0",
  "api_version": "1.0",
  "daemon_status": "running",
  "ffmpeg_path": "/Users/brook/.alchemist/ffmpeg/ffmpeg",
  "ffmpeg_version": "7.x",
  "hardware_acceleration": ["videotoolbox"],
  "config_path": "/Users/brook/Library/Application Support/Alchemist/config.toml",
  "database_path": "/Users/brook/Library/Application Support/Alchemist/alchemist.db"
}
```

---

## 14. SwiftUI Implementation Notes

### 14.1 App Entry

```swift
@main
struct AlchemistApp: App {
    @StateObject private var appModel = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(appModel)
        }

        Settings {
            SettingsView()
                .environmentObject(appModel)
        }
    }
}
```

### 14.2 State Architecture

Recommended model:

```text
AppModel
├── ConnectionModel
├── QueueModel
├── LibraryModel
├── ProfileModel
├── SettingsModel
├── SystemModel
└── ThemeModel
```

Use Swift Concurrency:

```swift
@MainActor
final class QueueModel: ObservableObject {
    @Published var jobs: [Job] = []
    @Published var activeJob: Job?
    @Published var isQueueRunning = false
}
```

### 14.3 API Client

```swift
actor AlchemistAPIClient {
    let baseURL: URL

    func getJobs() async throws -> [Job] { ... }
    func startQueue() async throws { ... }
    func pauseQueue() async throws { ... }
    func getSystemInfo() async throws -> SystemInfo { ... }
}
```

Use typed errors.

Do not throw random strings into the UI.

### 14.4 Error Model

```swift
struct AlchemistError: Identifiable, Codable, Error {
    let id: String
    let code: String
    let message: String
    let detail: String?
    let recoverable: Bool
}
```

UI should show:

```text
Human-readable message
Technical detail disclosure
Suggested action
Copy diagnostics button
```

---

## 15. Native macOS Integration

### 15.1 Finder Integration

Support:

```text
Reveal in Finder
Open output folder
Drag files into queue
Share/export profiles
```

### 15.2 Notifications

Send native notifications for:

```text
Queue completed
Job failed
Scan completed
Update available
Doctor found problem
```

Notification actions, future:

```text
Reveal File
Open Alchemist
Retry Job
```

### 15.3 Menu Bar Item

Optional menu bar item:

```text
Alchemist ∆
├── Queue Running / Paused
├── Active job progress
├── Start/Pause Queue
├── Open Alchemist
├── Open WebUI
└── Quit
```

Menu bar icon should be template-compatible.

### 15.4 Launch at Login

Add setting:

```text
Launch Alchemist at login
Start daemon automatically
Keep daemon alive in background
```

---

## 16. WebUI Toggle

The SwiftUI app should expose a clear setting:

```text
Enable WebUI
```

Options:

```toml
[webui]
enabled = true
port = 3067
```

The SwiftUI app should be able to:

```text
Enable WebUI
Disable WebUI
Open WebUI in browser
Change WebUI port
Show WebUI status
```

Important: the SwiftUI app should not depend on the WebUI.

---

## 17. Updates

### 17.1 Update UX

The update flow should feel native:

```text
Check for Updates…
Update Available
View Release Notes
Download
Install and Relaunch
```

### 17.2 Channels

```text
Stable
Release Candidate
Nightly, optional
```

### 17.3 Safety

Updates must verify:

```text
Version
Platform
Architecture
Checksum
Signature
```

Never silently replace binaries without verification.

---

## 18. Performance Expectations

The app should feel instant.

Targets:

```text
Cold launch: under 1 second to first window where possible
Dashboard visible immediately
Daemon connection status shown quickly
No blocking API calls on main thread
Large job lists virtualized/lazy-loaded
Logs streamed incrementally
```

SwiftUI should avoid unnecessary redraws for high-frequency progress updates.

Progress updates should be throttled if needed:

```text
Backend may emit frequently
UI updates at sane visual cadence
```

---

## 19. Accessibility

Accessibility is mandatory, not polish.

Requirements:

```text
VoiceOver labels for controls
Keyboard navigation for all major actions
Native focus handling
Readable contrast in glass mode
Reduce Motion support
Reduce Transparency support
Increase Contrast support
Color is never the only status indicator
Progress has textual values
Errors are readable and copyable
```

Job status should use both icon and text.

Bad:

```text
Only a green dot
```

Good:

```text
✓ Completed
⚠ Failed
⏸ Paused
```

---

## 20. Keyboard Shortcuts

Recommended shortcuts:

```text
⌘N        Add Media
⌘O        Add Folder / Open Media
⌘,        Settings
⌘R        Scan / Refresh
⌘F        Search
Space     Start/Pause selected queue item, context dependent
⌘⌥I       Toggle Inspector
⌘L        Open Logs
⌘K        Command palette, optional but awesome
⌘1        Dashboard
⌘2        Queue
⌘3        Library
⌘4        Profiles
⌘5        Activity
⌘6        System
```

A command palette would be excellent for power users.

---

## 21. Command Palette

Optional but highly recommended.

Shortcut:

```text
⌘K
```

Commands:

```text
Add Media
Start Queue
Pause Queue
Run Doctor
Open Config Folder
Open Logs Folder
Switch Theme
Create Profile
Check for Updates
Open WebUI
Copy Diagnostics
```

This would make Alchemist feel modern and power-user friendly.

---

## 22. Visual Components

### 22.1 Job Card

Use for active/recent jobs.

Fields:

```text
Filename
Progress
Codec conversion
Speed
ETA
Hardware encoder
Status
```

Design:

```text
Rounded rectangle
Subtle material
Strong progress indicator
Minimal accent color
Context menu
```

### 22.2 Status Badge

States:

```text
Running
Paused
Completed
Failed
Scanning
Warning
Idle
```

Each badge uses:

```text
Icon + label + optional accent
```

### 22.3 Diagnostic Row

```text
Check name
Status
Details
Fix button, when available
```

### 22.4 Empty State

Beautiful empty states matter.

Example:

```text
No jobs yet
Drop media here or add a watched folder to start optimizing your library.
[Add Media] [Add Folder]
```

Use large subtle delta glyph with glass/blur treatment.

---

## 23. File Import Flow

When user adds files:

```text
1. User drops/selects media
2. App validates file list through API
3. App shows import sheet
4. User chooses profile/output behavior
5. App queues jobs
6. Queue screen shows newly added jobs
```

Import sheet fields:

```text
Selected files count
Profile
Output directory
Append naming
Replace original toggle
Allow CPU fallback
Start immediately toggle
```

---

## 24. Error UX

Errors must be useful.

Bad:

```text
Something went wrong.
```

Good:

```text
ffmpeg could not access the output folder.
Check that Alchemist has permission to write to /Movies/Optimized.

[Open Folder] [Choose Another Folder] [Copy Details]
```

Error panels should have:

```text
Human explanation
Technical disclosure
Action buttons
Copy details
```

---

## 25. Security and Privacy

### 25.1 Local Mode

Default mode should bind API locally only.

```text
127.0.0.1 only
No LAN exposure unless user enables it
```

### 25.2 Remote Mode

Remote mode should require authentication.

```text
API token
HTTPS strongly recommended
Clear warning for insecure remote connections
```

### 25.3 File Access

The app should clearly communicate when it needs access to folders.

Use native file picker permissions.

### 25.4 Sensitive Data

Do not expose tokens/passwords in:

```text
logs
diagnostics bundle
screenshots
error reports
process arguments
```

---

## 26. Packaging

### 26.1 Bundle Layout

```text
Alchemist.app/
├── Contents/
│   ├── MacOS/
│   │   ├── Alchemist
│   │   └── alchemistd
│   ├── Resources/
│   │   ├── AppIcon.icns
│   │   └── default-config.toml
│   └── Info.plist
```

### 26.2 Support Paths

Use native macOS paths:

```text
~/Library/Application Support/Alchemist/
~/Library/Logs/Alchemist/
~/Library/Caches/Alchemist/
```

Do not dump random files in the app bundle after install.

For the prototype, bundled daemon launch passes:

```text
ALCHEMIST_CONFIG_PATH=~/Library/Application Support/Alchemist/config.toml
ALCHEMIST_DB_PATH=~/Library/Application Support/Alchemist/alchemist.db
ALCHEMIST_TEMP_DIR=~/Library/Application Support/Alchemist/temp
```

Remote mode does not set these paths because the Swift app is only a client in
that mode.

### 26.3 CLI Integration

The app can optionally install a CLI shim:

```text
/usr/local/bin/alchemist
```

CLI should communicate with the same daemon/core.

---

## 27. Minimum Viable SwiftUI Client

The first useful version should include:

```text
Native app shell
Sidebar navigation
Dashboard
Queue view
Job progress
Add media flow
- Enqueue local files
- Add watched folders
- Upload/stage single-file conversion
Settings: Appearance + API/WebUI basics
System diagnostics
About screen
Daemon start/connect handling
Theme from config
```

Do not start with every feature.

Start with the parts that prove the app is native, beautiful, and useful.

---

## 28. Phase Plan

### Phase 0: API Contract Lock

```text
Inventory current /api routes used by native/mac
Document job/config/system schemas
Add API capability/version endpoint
Keep typed error responses on all native-client routes
Define SSE event names and payloads
Keep Swift route paths centralized for /api/v1 migration
```

### Phase 1: App Shell

```text
Create SwiftUI app under native/mac
NavigationSplitView
Sidebar
Toolbar
Theme bridge
API connection status
Daemon launch/connect
Just recipes for build/run/check/stage-daemon
```

### Phase 2: Jobs + Dashboard

Implemented Jobs parity milestone on 2026-05-01 for the native prototype.
Follow-up on 2026-05-01 aligned the native sidebar to the WebUI surface,
fixed session-cookie propagation so protected buttons authorize correctly,
started bundled daemon mode by default, and moved Dashboard toward the WebUI
layout/data model.

```text
Jobs workspace
Tabs, search, sort, pagination, saved views
Batch cancel/restart/delete
Priority controls
Job details inspector
Queue position and processor status
Decision/failure explanations
Attempt history and logs
Active job card
Progress updates
Pause/resume/cancel
Dashboard summary cards
WebUI sidebar parity: Dashboard, Jobs, Logs, Statistics, Intelligence, Convert, Settings
```

### Phase 3: Import + Profiles

```text
Add media
Drag and drop
Profile selection
Profile list/editor
Queue selected files
```

### Phase 4: Settings + System

```text
Native Settings window
Appearance config
WebUI toggle
System diagnostics
Doctor integration
About screen
```

### Phase 5: Polish

```text
Liquid Glass refinement
Animations
Command palette
Menu bar item
Notifications
Update UI
Accessibility pass
Performance pass
```

---

## 29. Non-Negotiables

```text
Theme stored in Alchemist config
SwiftUI does not own transcoding logic
No localStorage-style UI state as source of truth
No fake web dashboard UI
Native menus and settings
Native file picking
Native notifications
Readable logs
Accessible glass
Current API paths centralized for later versioned API migration
Beautiful About screen
Delta logo direction
```

---

## 30. Final Design Target

Alchemist for macOS should feel like Apple built a native media optimization tool, then Alchemist gave it a sharper identity.

It should be:

```text
Native
Fast
Glass-modern
Readable
Powerful
Beautiful
Config-driven
API-backed
Rust-powered
Mac-ass Mac app
```

The app’s personality should be:

> **Elegant control over a brutally capable transcoding engine.**

That is the north star.

---

## 31. Reference Notes

This spec is based on Apple’s current direction for modern platform UI, including Human Interface Guidelines concepts such as sidebars, toolbars, panels/inspectors, native layout conventions, adaptive colors/materials, and the newer Liquid Glass visual language introduced across Apple platforms.

Key implementation references to consult while building:

- Apple Human Interface Guidelines
- Apple SwiftUI documentation
- `NavigationSplitView`
- `MenuBarExtra`
- SwiftUI inspectors
- SwiftUI materials and Liquid Glass APIs
- `GlassEffectContainer`
- `glassEffect(_:in:)`
- `.buttonStyle(.glass)` / `.buttonStyle(.glassProminent)`
- `ToolbarSpacer`
- WWDC25 “Meet Liquid Glass”
- WWDC25 “Build a SwiftUI app with the new design”
- WWDC25 “What’s new in SwiftUI”
