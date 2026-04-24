---
title: "VMAF quality gate"
summary: "Explains VMAF scoring, minimum thresholds, and safe revert behavior for encoded files."
area: "quality"
order: 10
---

VMAF is an optional post-encode quality check. When enabled, Alchemist compares the encoded output against the source and records a score from 0 to 100.

Higher scores mean the output is more visually similar to the source. A minimum score around 90 is conservative for most space-saving profiles.

If **Revert on Low Quality** is enabled, Alchemist keeps the source when the encoded output falls below the configured threshold. This preserves user media at the cost of losing the expected storage savings for that job.
