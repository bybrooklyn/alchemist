---
title: FAQ
description: Common questions from self-hosters running Alchemist.
---

## 1. Is Alchemist free?

Yes. Alchemist is GPLv3 open source. There is no paid tier,
license key, private pro build, or commercial-use unlock.

## 2. Will it ruin my quality?

No by default. The planner uses BPP analysis and optional
VMAF gating to avoid pointless or low-quality transcodes.

## 3. How much space will I save?

Typical results are 30–70%, depending on source quality,
codec choice, and how much waste is in the original file.

## 4. Do I need a GPU?

No. CPU encoding works. GPU encoding is much faster and more
power-efficient.

## 5. Does it work with Plex, Jellyfin, or Emby?

Yes. Point Alchemist at the same media directories your
server already uses.

## 6. What happens to my originals?

By default they stay in place. Nothing is deleted unless you
enable `delete_source`.

## 7. Does it support 4K and HDR?

Yes. It can preserve HDR metadata or tonemap to SDR.

## 8. Can I run multiple instances?

Not against the same SQLite database. Use one Alchemist
instance per DB.

## 9. What is BPP?

Bits-per-pixel. It is the compression density measurement
Alchemist uses to decide whether a file is already efficient.

## 10. What is VMAF?

Netflix’s perceptual quality metric. It is optional and
slows down post-encode validation.

## 11. How do I update?

```bash
docker compose pull && docker compose up -d
```

## 12. What if hardware is not detected?

CPU fallback is automatic unless you disabled it. Check the
hardware probe log to fix the GPU path.

On repeat boots, Alchemist may show a cached hardware result
immediately if the runtime fingerprint still matches.

## 13. Can I control when it runs?

Yes. Use **Settings → Schedule** to define allowed windows.

## 14. Does it handle subtitles?

Yes. Subtitle mode supports copy, burn, extract, and drop.

## 15. Why did it skip my file?

Open the skipped job and read the reason, or start with
[Skip Decisions](/skip-decisions).
