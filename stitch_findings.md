# Alchemist Project Audit & Findings

This document provides a comprehensive audit of the Alchemist media transcoding project (v0.3.0-rc.3), covering backend architecture, frontend design, database schema, and operational workflows.

---

## 1. Project Architecture & Pipeline

Alchemist implements a robust, asynchronous media transcoding pipeline managed by a central `Agent`. The pipeline follows a strictly ordered lifecycle:

1.  **Scanner (`src/media/scanner.rs`):** Performs a high-speed traversal of watch folders. It uses `mtime_hash` (seconds + nanoseconds) to detect changes without full file analysis, efficiently handling re-scans and minimizing DB writes.
2.  **Analyzer (`src/media/analyzer.rs`):** Executes `ffprobe` to extract normalized media metadata (codecs, bit depth, BPP, bitrate). Analysis results are used to populate the `DetailedEncodeStats` and `Decision` tables.
3.  **Planner (`src/media/planner.rs`):** A complex decision engine that evaluates whether to **Skip**, **Remux**, or **Transcode** a file based on user profiles. 
    *   *Finding:* The planning logic is heavily hardcoded with "magic thresholds" (e.g., Bits-per-pixel thresholds). While effective, these could be more exposed as "Advanced Settings" in the UI.
4.  **Executor (`src/media/executor.rs`):** Orchestrates the `ffmpeg` process. It dynamically selects encoders (NVENC, VAAPI, QSV, ProRes, or CPU fallback) based on the target profile and host hardware capabilities detected in `src/system/hardware.rs`.

---

## 2. Backend & API Design (Rust/Axum)

*   **Concurrency:** Utilizes `tokio` for async orchestration and `rayon` for CPU-intensive tasks (like file hashing or list processing). The scheduler supports multiple concurrency modes: `Background` (1 job), `Balanced` (capped), and `Throughput` (uncapped).
*   **State Management:** The backend uses `broadcast` channels to separate high-volume events (Progress, Logs) from low-volume system events (Config updates). This prevents UI "flicker" and unnecessary re-renders in the frontend.
*   **API Structure:** 
    *   **RESTful endpoints** for jobs, settings, and stats.
    *   **SSE (`src/server/sse.rs`)** for real-time progress updates, ensuring a reactive UI without high-frequency polling.
    *   **Auth (`src/server/auth.rs`):** Implements JWT-based authentication with Argon2 hashing for the initial setup.

---

## 3. Database Schema (SQLite/SQLx)

*   **Stability:** The project uses 16+ migrations, showing a mature evolution from a simple schema to a sophisticated job-tracking system.
*   **Decision Logging:** The `decisions` and `job_failure_explanations` tables are a standout feature. They store the "why" behind every action as structured JSON, which is then humanized in the UI (e.g., explaining exactly why a file was skipped).
*   **Data Integrity:** Foreign keys and WAL (Write-Ahead Logging) mode ensure database stability even during heavy concurrent I/O.

---

## 4. Frontend Design (Astro/React/Helios)

*   **Stack:** Astro 5 provides a fast, static-first framework with React 18 handles the complex stateful dashboards.
*   **Design System ("Helios"):** 
    *   *Identity:* A dark-themed, data-dense industrial aesthetic.
    *   *Findings:* While functional, the system suffers from "component bloat." `JobManager.tsx` (~2,000 lines) is a significant maintainability risk. It contains UI logic, filtering logic, and data transformation logic mixed together.
*   **Data Visualization:** Uses `recharts` for historical trends and performance metrics. 
    *   *Improvement:* The charts are currently static snapshots. Adding real-time interactivity (brushing, zooming) would improve the exploration of large datasets.

---

## 5. System & Hardware Integration

*   **Hardware Discovery:** `src/system/hardware.rs` is extensive, detecting NVIDIA, Intel, AMD, and Apple Silicon capabilities. It correctly maps these to `ffmpeg` encoder flags.
*   **FS Browser:** A custom filesystem browser (`src/system/fs_browser.rs`) allows for secure directory selection during setup, preventing path injection and ensuring platform-agnostic path handling.

---

## 6. Critical Areas for Improvement

### **Maintainability (High Priority)**
*   **Decouple `JobManager.tsx`:** Refactor into functional hooks (`useJobs`, `useFilters`) and smaller, presentation-only components.
*   **Standardize Formatters:** Move `formatBytes`, `formatTime`, and `formatReduction` into a centralized `lib/formatters.ts` to reduce code duplication across the Dashboard and Stats pages.

### **UX & Performance (Medium Priority)**
*   **Polling vs. SSE:** Ensure all real-time metrics (like GPU temperature) are delivered via SSE rather than periodic polling to reduce backend load and improve UI responsiveness.
*   **Interactive Decision Explanations:** The current skip reasons are helpful but static. Adding links to the relevant settings (e.g., "Change this threshold in Transcoding Settings") would close the loop for users.

### **Reliability (Low Priority)**
*   **E2E Testing:** While Playwright tests exist, they focus on "reliability." Expanding these to cover complex "edge cases" (like network-attached storage disconnects during a scan) would improve long-term stability.

---

## 7. Stitch Recommendation
Use Stitch to generate **atomic component refinements** based on this audit. 
*   *Prompt Example:* "Refine the JobTable row to use iconic status indicators with tooltips for skip reasons, as outlined in the Alchemist Audit."
*   *Prompt Example:* "Create a unified `Formatter` utility library in TypeScript that handles bytes, time, and percentage formatting for the Helios design system."
