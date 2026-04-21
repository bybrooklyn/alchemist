# Alchemist SEO Plan

## Strategic framing

Three positions to own, in order:

1. **The genuinely FOSS alternative** — GPLv3, no pro tier, no paywalled features. This is the strongest wedge against FileFlows (commercial tiers) and a real differentiator vs Tdarr (which pushes "Tdarr 2.0" increasingly).
2. **The Jellyfin-native transcoder** — Tdarr and FileFlows treat Jellyfin as one of many targets. Alchemist can own the "pre-transcode for Jellyfin" search pool, which is high-intent and growing.
3. **The AV1-first automated pipeline** — AV1 enthusiasm is rising fast (RTX 40-series NVENC, Intel Arc, Alder Lake+ QSV). Most competitors bolted AV1 on; Alchemist should be the "AV1 by default" pick.

The site is a single Docusaurus instance at `alchemist-project.org` with `routeBasePath: '/'`, so marketing pages and docs live in one tree. This is fine — it concentrates PageRank — but it means comparison/landing pages need to be written in a marketing voice while still fitting the docs sidebar. Treat them as first-class docs entries.

The domain is young. For 6–12 months, SEO gains will be bottlenecked by backlinks, not content. Every phase below should pair content with a distribution channel (AlternativeTo listing, awesome-selfhosted PR, r/selfhosted/r/jellyfin/r/homelab organic posts) — otherwise the pages will exist unranked.

---

## 1. Prioritized roadmap

### Phase 0 — Foundation (weeks 0–2)
- Add per-page `title` + `description` frontmatter to every existing doc (Docusaurus emits these into `<title>` and `<meta>`).
- Add `SoftwareApplication` JSON-LD to homepage, `BreadcrumbList` schema site-wide (Docusaurus plugin or manual).
- Generate per-page OG images from a template (title + Alchemist mark); huge CTR lift vs generic social card.
- Verify `sitemap.xml` reaches Google Search Console and Bing Webmaster. Submit.
- Install Plausible or GoatCounter (privacy-respecting fits audience).
- Add a `/llms.txt` summary of the product for LLM-crawler discovery — a rising vector for "open source transcoding" queries answered by assistants.

### Phase 1 — High-intent capture (weeks 2–6)
- Comparison pages: Tdarr, FileFlows, Unmanic, Handbrake (the first three ship with schema + table + migration CTA).
- `/alternatives/` index page.
- Homepage rewrite (see §6).
- Open-source positioning page `/open-source`.
- Submit to AlternativeTo, awesome-selfhosted, awesome-docker, awesome-jellyfin. These submissions ARE the link-building plan for Phase 1.

### Phase 2 — Jellyfin/*arr ecosystem (weeks 6–10)
- `/jellyfin` pillar page.
- `/jellyfin/direct-play` troubleshooter (troubleshooting pulls the best long-tail traffic).
- `/plex` sibling page.
- `/sonarr-radarr-workflow` integration guide.
- One guest post / cross-link attempt per week into r/jellyfin, r/PleX, r/selfhosted.

### Phase 3 — Hardware long-tail (weeks 10–16)
- Per-platform hardware pages (NVENC, QSV, VAAPI, AMF, VideoToolbox) — the current `/hardware` page is one page; split.
- Per-SKU buyer-intent pages: N100, Arc A380, RTX 4060, Apple Silicon.
- "AV1 hardware support in 2026" round-up.

### Phase 4 — Cookbook + migration (months 4–6)
- Task-oriented pages: "Convert library to AV1", "Reduce library size by 50%", "Strip commentary tracks".
- Migration guides from Tdarr/FileFlows/Unmanic — these convert the highest-intent traffic of all.
- Release-notes-as-pages for backlink bait.

### Phase 5 — Content compound (ongoing)
- Mine GSC weekly for queries with impressions but no matching page; build that page.
- Power-consumption, HDR/Dolby Vision, subtitle handling — fill obvious gaps as GSC surfaces them.

---

## 2. Keyword clusters by search intent

### Cluster A — Commercial comparison (transactional)
**Intent:** user is actively evaluating; closest to conversion.
- `tdarr alternative`, `free tdarr alternative`, `open source tdarr`, `tdarr vs fileflows`, `tdarr vs unmanic`
- `fileflows alternative`, `fileflows free`, `fileflows open source`, `fileflows pricing`
- `unmanic alternative`, `handbrake automation`, `automate handbrake`
- `best self hosted transcoder`, `best transcoding automation 2026`

### Cluster B — Category definition (informational→transactional)
**Intent:** user hasn't named a tool yet.
- `self hosted video transcoding`, `self hosted media transcoder`
- `automatic video transcoding`, `batch video transcoder`
- `ffmpeg automation`, `automate ffmpeg`, `ffmpeg batch script replacement`
- `media library transcoder`

### Cluster C — Jellyfin ecosystem (niche, warm, high-intent)
- `jellyfin transcoding automation`, `pre transcode jellyfin`, `jellyfin direct play`
- `jellyfin av1 support`, `jellyfin hevc browser`, `jellyfin codec compatibility`
- `jellyfin cpu 100 percent`, `jellyfin buffering fix`
- Parallel Plex set: `plex pre transcode`, `plex direct play forcing transcode`

### Cluster D — Codec long-tail (informational)
- `av1 transcoding`, `av1 vs hevc file size`, `convert library to av1`
- `hevc vs h264 storage savings`, `convert mkv to hevc`, `re-encode library to save space`

### Cluster E — Hardware acceleration (high intent, technical)
- `nvenc automation`, `nvenc batch encode`, `nvenc av1 rtx 4060`
- `intel quick sync transcoding`, `qsv av1`, `arc a380 transcoding`, `n100 transcoding`
- `vaapi transcoding linux`, `vaapi av1`, `amd amf vs vaapi`
- `apple silicon videotoolbox ffmpeg`, `m1 m2 transcoding`
- `docker gpu passthrough nvenc`, `docker vaapi`, `unraid nvenc container`

### Cluster F — Pain-point troubleshooting (long-tail, very high intent)
- `hevc plays on tv not browser`, `jellyfin transcoding cpu high`
- `ffmpeg crashes av1`, `nvenc not detected docker`, `intel gpu not showing ffmpeg`
- `how to batch transcode media library`, `convert library without breaking sonarr`

### Cluster G — FOSS / ethos (affinity, builds brand searches over time)
- `open source transcoding software`, `gpl transcoder`, `foss media tools`
- `no cloud transcoding`, `homelab transcoder`, `privacy respecting transcoder`

**Cluster priority order for effort:** A → C → F → E → B → D → G.
A converts. C is warm and under-served. F is cheap to rank and hyper-qualified. E is the long-tail engine. B is competitive and slower. D is supporting content. G is brand.

---

## 3. Landing page plan (marketing + evergreen)

All slugs are final URLs (site is mounted at `/`).

| Slug | Page | Primary query | Cluster |
|---|---|---|---|
| `/` | Homepage / overview | alchemist, self-hosted video transcoding | B, G |
| `/alternatives` | Alternatives hub | tdarr alternative, fileflows alternative | A |
| `/alternatives/tdarr` | Alchemist vs Tdarr | tdarr alternative, tdarr vs | A |
| `/alternatives/fileflows` | Alchemist vs FileFlows | fileflows alternative, fileflows open source | A |
| `/alternatives/unmanic` | Alchemist vs Unmanic | unmanic alternative | A |
| `/alternatives/handbrake` | Automating Handbrake | automate handbrake, handbrake batch | A |
| `/jellyfin` | Alchemist for Jellyfin | jellyfin transcoding automation | C |
| `/plex` | Alchemist for Plex | plex pre-transcode | C |
| `/open-source` | FOSS positioning | open source transcoding, gpl transcoder | G |
| `/av1` | AV1 automation | av1 transcoding automation | D, E |
| `/homelab` | Homelab transcoding pipeline | homelab transcoder | B, G |
| `/migrate-from-tdarr` | Tdarr migration guide | migrate from tdarr | A |
| `/migrate-from-fileflows` | FileFlows migration guide | leave fileflows | A |
| `/reduce-library-size` | Library size reduction cookbook | reduce media library size | D |

Install-decision pages (should also rank for platform queries):
- `/install/docker`, `/install/unraid`, `/install/synology`, `/install/truenas`, `/install/proxmox`, `/install/linux`, `/install/windows`, `/install/macos`

Hardware sub-pages (split from current `/hardware`):
- `/hardware/nvidia-nvenc`, `/hardware/intel-quick-sync`, `/hardware/intel-arc`, `/hardware/amd-vaapi`, `/hardware/amd-amf`, `/hardware/apple-videotoolbox`
- `/hardware/av1-support-matrix` — which GPUs encode AV1, updated yearly — strong backlink magnet.

---

## 4. Docs pages that can rank

These exist or should exist; most need a title/meta/intro rewrite to hit the query:

| Slug | Target query | Change needed |
|---|---|---|
| `/first-run` | how to set up automatic transcoding | Add "for Jellyfin / Plex libraries" framing, screenshots |
| `/docker` | docker transcoding automation, docker ffmpeg | Add "unraid/synology/truenas" signal words |
| `/gpu-passthrough` | docker gpu passthrough nvenc/vaapi | Per-runtime headings (Docker, Podman, Unraid) |
| `/hardware/nvidia-nvenc` (new, split) | nvenc automation, nvenc batch | Add supported SKUs + AV1 NVENC note |
| `/hardware/intel-quick-sync` (new) | qsv transcoding, n100 transcoding | Name SKUs: N100, N305, Arc, Alder Lake+ |
| `/hardware/amd-vaapi` (new) | amd vaapi ffmpeg linux | Include specific driver setup |
| `/hardware/apple-videotoolbox` (new) | apple silicon transcoding | M1/M2/M3/M4 naming |
| `/codecs/av1` (promote) | av1 transcoding, av1 hardware | Pair with `/av1` marketing page |
| `/profiles` | video transcoding profiles | Add query in H1 |
| `/stream-rules` | strip audio track ffmpeg automatically | Long-tail `how to remove commentary track` |
| `/scheduling` | off-peak transcoding cron | Add examples |
| `/planner` | ffmpeg remux vs transcode | Rename/subtitle: "When Alchemist skips, remuxes, or transcodes" |
| `/skip-decisions` | ffmpeg skip already compressed | Rewrite intro as Q-shaped snippet |
| `/troubleshooting` | split into sub-pages | See below |
| `/faq` | add query-phrased questions | Each Q gets `FAQPage` schema |

Troubleshooting split — each of these is a long-tail goldmine:
- `/troubleshooting/nvenc-not-detected`
- `/troubleshooting/vaapi-not-detected`
- `/troubleshooting/docker-gpu-not-working`
- `/troubleshooting/jellyfin-direct-play-failing`
- `/troubleshooting/cpu-pegged-during-transcode`
- `/troubleshooting/av1-playback-broken`
- `/troubleshooting/hdr-looks-washed-out`

---

## 5. Comparison-page strategy

### Standard template (every comparison page uses this)

1. **H1:** `Alchemist vs {Competitor}: {honest one-liner}`.
2. **At-a-glance table** (first fold, before any prose) — featured-snippet bait. Rows: License, Platforms, Hardware support, AV1 support, Deployment model, Non-destructive defaults, Config style, Extensibility, Price.
3. **Short "Choose X if… Choose Alchemist if…"** section. This paradoxically improves rankings because it signals trust and reduces bounce from mismatched intent.
4. **Feature-by-feature walkthrough** with screenshots from both tools. Two-column where space allows.
5. **Migration CTA** linking to `/migrate-from-{competitor}`.
6. **FAQ block** with 4–6 questions (`FAQPage` JSON-LD).
7. **Schema:** `SoftwareApplication` for Alchemist; do **not** mark up competitor as a product you review with `Review` schema — Google's guidelines penalize self-serving reviews. Plain prose comparisons only.

### Competitor-specific angles

- **Tdarr:** lead with licensing ambiguity (Tdarr has proprietary Pro components), single-binary vs server/node split, AV1 pipeline maturity, Rust performance. Avoid disparaging tone — Tdarr's community is large and hostile comparison content gets dragged on Reddit.
- **FileFlows:** lead hard with "GPLv3 with no paid tier vs FileFlows' commercial tiers." This is the angle that wins. Frame as "FOSS alternative to FileFlows" because that's literally what people search.
- **Unmanic:** closer philosophically (both FOSS). Differentiate on Rust vs Python, AV1 defaults, Jellyfin integration, hardware breadth, pipeline clarity.
- **Handbrake:** different category — Handbrake is manual GUI. Frame as "Automate what Handbrake does manually." Captures a large query pool that isn't even looking for Alchemist's category yet.

### Long-tail comparison pages to add later
- `tdarr-vs-fileflows` (rank for third-party comparison intent; subtly position Alchemist as the third option you'd actually pick)
- `nvenc-vs-qsv-vs-vaapi` (hardware comparison, not tool comparison — massive query pool)

---

## 6. Homepage messaging and differentiation

### Above the fold

- **H1:** `Self-hosted video transcoding that just works.`
- **Subhead:** `Point Alchemist at your media library. It scans, analyzes, and transcodes only what's worth transcoding — using your GPU, never touching your originals. GPLv3. Single binary. AV1, HEVC, and H.264 on Linux, macOS, Windows, and Docker.`
- **Trust pill row:** `GPLv3` · `NVENC / QSV / VAAPI / AMF / VideoToolbox` · `AV1-first` · `Jellyfin & Plex ready` · `No cloud. No accounts.`
- **Primary CTA:** `Install in 2 minutes` → `/quick-start`
- **Secondary CTA:** `Considering Tdarr or FileFlows?` → `/alternatives`

### Three-up differentiation block

1. **Non-destructive by design.** Your originals are never overwritten by default. Every action is reversible. (Links `/planner`, `/skip-decisions`.)
2. **Hardware acceleration on every platform.** NVENC, Intel Quick Sync, VAAPI, AMF, and Apple VideoToolbox — auto-detected, with a CPU fallback that actually works. (Links `/hardware`.)
3. **Actually open source.** GPLv3, no "Pro" tier, no feature paywalls, no telemetry. (Links `/open-source`.)

### Social-proof strip
GitHub stars, latest release, "Used in {N}+ homelabs" once measurable.

### Conversion-optimized footer block
Three tasks users arrive wanting to do — make them one click each:
- "I'm migrating from Tdarr" → `/migrate-from-tdarr`
- "I want to transcode my Jellyfin library to AV1" → `/jellyfin` then `/av1`
- "I just want it running on Docker" → `/install/docker`

### What NOT to do on the homepage
- Don't lead with the engine architecture. Users want outcomes, not internals.
- Don't bury the GPLv3 claim. It's the strongest differentiator from FileFlows, and "open source transcoding" is a real query.
- Don't hide comparisons in a sub-menu. They convert; surface them.

---

## 7. Internal linking strategy

The single biggest underused lever on a Docusaurus site:

1. **Hub-and-spoke by cluster.** Each pillar (`/jellyfin`, `/av1`, `/hardware`, `/alternatives`, `/open-source`) is a hub. Every related doc links **up** to the hub. The hub links **down** to 6–10 specific pages. This concentrates authority on hubs, which are the pages you want to rank.
2. **Every docs page ends with 2–3 curated "Next" links** (manual, not algorithmic). Example: `/gpu-passthrough` ends with links to `/install/docker`, `/hardware/nvidia-nvenc`, `/troubleshooting/nvenc-not-detected`.
3. **Every comparison page deep-links into docs** for every feature it claims. "Non-destructive by default" → `/planner`. "GPU auto-detection" → `/hardware`. Do not just assert — prove, and pass PageRank while you're at it.
4. **Every troubleshooter links to the relevant install page and the relevant hardware page.**
5. **Glossary terms.** Create micro-pages for AV1, HEVC, NVENC, QSV, VAAPI, AMF, VideoToolbox, VMAF, Direct Play. Every first mention on any page auto-links to these. Docusaurus remark plugin can handle this. Low-effort, captures long-tail, improves topical authority.
6. **Anchor-text discipline.** Use the target query as the anchor, not "click here." "See the [Tdarr comparison]" not "see [here]".
7. **Breadcrumbs** — keep them on, they're Docusaurus default but double-check JSON-LD is emitted.
8. **Orphan audit monthly.** Any page with <2 inbound internal links gets surfaced and linked from somewhere relevant.

---

## 8. Title + meta for priority pages

| Slug | Title (≤60 chars ideal, ≤70 hard cap) | Meta description (≤155 chars) |
|---|---|---|
| `/` | Alchemist — Self-Hosted Video Transcoding Automation | Point Alchemist at your library and walk away. Open-source (GPLv3), Rust-powered transcoding with NVENC, QSV, VAAPI, AMF, VideoToolbox. AV1 & HEVC. |
| `/alternatives/tdarr` | Alchemist vs Tdarr: Open-Source Transcoding Compared | How Alchemist compares to Tdarr — fully GPLv3, single binary, native AV1 pipeline, non-destructive defaults. Honest side-by-side for homelabs. |
| `/alternatives/fileflows` | FileFlows Alternative: Alchemist is Fully Open Source | Free, GPLv3 alternative to FileFlows. No paid tiers, no license gates — automated transcoding for Jellyfin, Plex, and self-hosted libraries. |
| `/alternatives/unmanic` | Alchemist vs Unmanic: FOSS Transcoders Compared | Two open-source transcoders, different philosophies. Rust vs Python, AV1-first defaults, Jellyfin-ready, cross-platform hardware acceleration. |
| `/alternatives/handbrake` | Automate Handbrake: Batch Transcode a Media Library | Handbrake is a great manual tool. Alchemist is what you want when the library is too big to click through. GPU-accelerated, rules-based, GPLv3. |
| `/jellyfin` | Jellyfin Transcoding Automation — Pre-Encode with Alchemist | Stop transcoding on the fly. Pre-transcode your Jellyfin library to AV1 or HEVC with NVENC, QSV, or VAAPI. Non-destructive, self-hosted, GPLv3. |
| `/av1` | Automated AV1 Transcoding for Your Media Library | Batch-convert to AV1 using RTX 40-series NVENC, Intel Arc QSV, or SVT-AV1 on CPU. VMAF quality gates, non-destructive, open source. |
| `/open-source` | Alchemist is GPLv3 — Actually Open-Source Transcoding | No "Pro" tier. No paywalled features. No telemetry. Alchemist is GPLv3 from day one, and it will stay that way. |
| `/install/docker` | Alchemist on Docker — Self-Hosted Transcoder in One Container | Run Alchemist on Docker with GPU passthrough for NVENC, QSV, VAAPI. Works on Unraid, Synology, TrueNAS, Proxmox. docker-compose included. |
| `/hardware/nvidia-nvenc` | NVENC Transcoding Automation — RTX, Quadro, Tesla | Automate NVENC batch encoding with Alchemist. AV1 on RTX 40-series, HEVC everywhere else. Docker GPU passthrough and multi-GPU scheduling. |
| `/hardware/intel-quick-sync` | Intel Quick Sync Transcoding — N100, Arc, Alder Lake | Use Intel QSV to batch-transcode your library. AV1 on Arc and 13th-gen+. N100 miniPCs, desktop Arc, and server iGPUs — all supported. |
| `/hardware/amd-vaapi` | AMD VAAPI Transcoding on Linux | Automated VAAPI transcoding for AMD GPUs on Linux. Driver setup, permissions, Docker passthrough, and when to prefer AMF on Windows. |
| `/migrate-from-tdarr` | Migrate from Tdarr to Alchemist (Step-by-Step) | Moving off Tdarr? Map flows to Alchemist profiles, preserve your library structure, and switch without re-scanning. |
| `/troubleshooting/nvenc-not-detected` | NVENC Not Detected in Docker — How to Fix It | Walk through every reason NVENC fails to appear: driver mismatch, runtime, cgroup, device mapping, and container image. With fixes. |

---

## 9. Additional opportunities you may be missing

1. **"What GPU should I buy for transcoding?"** — commercial intent, converts. Annual "Best GPU for AV1 transcoding in {year}" page drives backlinks every December.
2. **Power-consumption angle** — "How much does transcoding my library cost in electricity?" This is a top r/homelab topic, almost no one writes about it with real numbers.
3. **Sonarr/Radarr/Bazarr integration** — warm, under-served. A `/sonarr-radarr-workflow` page captures intent no competitor page explicitly owns.
4. **N100 / mini PC content** — exploding search volume. `hardware/intel-n100` deserves its own page, not a bullet.
5. **HDR & Dolby Vision handling** — rapidly searched, barely served. Specific terms: `tonemap hdr to sdr ffmpeg`, `dolby vision profile 5 jellyfin`, `hdr10+ metadata preservation`.
6. **Subtitle handling** — `extract subtitles ffmpeg`, `burn subtitles automatically`, `pgs to srt`. All long-tail, all relevant to your stream-rules feature.
7. **YouTube surface area** — a 4-minute "Tdarr alternative in 2026" setup video usually sits above text results on the same query. Even one video matters.
8. **Awesome-lists submissions** — awesome-selfhosted, awesome-jellyfin, awesome-docker, awesome-rust. These are link sources *and* direct traffic. They convert.
9. **AlternativeTo.net** — ranks for virtually every "X alternative" query. Getting listed with screenshots is a weekend's work and a multi-year dividend.
10. **GitHub README SEO** — the repo itself ranks for brand + category terms. Put target keywords naturally into the README's first 200 words.
11. **Release-notes-as-pages** — `/releases/0.3.1` etc. Attract version-specific backlinks from forums and changelog aggregators.
12. **Mine Search Console monthly** for queries with impressions but no landing page, build the page. The site will tell you what to write six months in.
13. **Glossary micro-pages** (AV1, HEVC, VAAPI, NVENC, QSV, VMAF, Direct Play) — cheap to write, boost topical authority, resolve long-tail.
14. **llms.txt** — an emerging signal for LLM crawlers. `llms.txt` + `llms-full.txt` with clean product summary captures the answers-in-chat traffic that bypasses search entirely.
15. **Structured FAQ** — every pillar page gets 4–6 `FAQPage`-schema Q&A items. Consistently wins rich snippets for "is alchemist free", "does alchemist support av1", etc.
16. **Reddit AMA in r/selfhosted** once you hit a milestone release. This is the single highest-leverage backlink event a project in this niche can produce.
17. **Comparison against non-obvious tools**: `nvidia patch alternative`, `nvencc batch gui`, `staxrip automation`. Low volume but effectively unopposed.
18. **Homelab newsletters** — Noted.lol, Selfh.st, Awesome Self-Hosted Weekly. Submissions are usually free and drive both traffic and referring domains.
19. **Do NOT chase** — `plex transcoding` (too competitive, Plex dominates), `handbrake download` (wrong intent), generic `video converter` (consumer intent, unqualified traffic).

---

## Quick-start checklist (next 14 days)

- [ ] Frontmatter pass: title + description on every existing doc.
- [ ] Homepage rewrite per §6.
- [ ] `/alternatives/tdarr` and `/alternatives/fileflows` drafted.
- [ ] `/open-source` page published.
- [ ] AlternativeTo listing submitted.
- [ ] awesome-selfhosted PR opened.
- [ ] Google Search Console + Bing Webmaster verified, sitemap submitted.
- [ ] `SoftwareApplication` JSON-LD on homepage.
- [ ] OG image template producing per-page social cards.
- [ ] `llms.txt` in site root.

Everything after those 14 days compounds. Before those 14 days, you are invisible no matter how much content exists.
