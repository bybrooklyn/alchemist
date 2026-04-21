---
title: VAAPI Not Detected
description: Fix "VAAPI not detected" in Alchemist on Linux. Verify /dev/dri passthrough, vainfo output, the render group, LIBVA_DRIVER_NAME, and the startup probe log.
keywords:
  - vaapi not detected
  - vaapi docker not working
  - /dev/dri not visible
  - libva driver name
  - vainfo no profiles
slug: /troubleshooting/vaapi-not-detected
---

VAAPI is Alchemist's preferred path for both Intel and AMD
GPUs on Linux. When it isn't being used, the issue is almost
always one of:

- `/dev/dri` isn't exposed to the process (common in
  Docker).
- The container user lacks access to the `video` / `render`
  groups.
- `vainfo` reports no usable profiles — the VAAPI driver is
  missing, wrong, or broken.
- The startup probe failed with a codec-specific error.

Start with **Settings → Hardware → Probe Log**. Each VAAPI
probe records the full FFmpeg stderr when it fails, and
that's the authoritative answer to "why didn't VAAPI work?".
The steps below narrow down what the probe log already says.

## 1. Is `/dev/dri` exposed?

On the host:

```bash
ls -l /dev/dri
```

You should see at least one `renderD128` (or similar) device
node. If the directory is empty or missing, the GPU driver
isn't loaded on the host — fix that before worrying about
the container.

In Docker, the device must be passed in:

```yaml
devices:
  - /dev/dri:/dev/dri
group_add:
  - video
  - render
```

The `group_add` entries matter — without them, the
container user can see the device nodes but not open them.

See [GPU Passthrough](/gpu-passthrough) for full examples.

## 2. Does `vainfo` report profiles?

Inside the container (or on the host for binary installs):

```bash
vainfo --display drm --device /dev/dri/renderD128
```

On newer systems, Intel may use `renderD129`. Check
`ls -l /dev/dri` to identify which render node is which.

Expected output includes a block like `VAProfileH264...`,
`VAProfileHEVCMain...`, and so on. If `vainfo` errors with
"no valid VA driver", the VAAPI driver isn't present or the
wrong one is selected.

Set `LIBVA_DRIVER_NAME` explicitly:

- `iHD` — modern Intel iGPUs (8th gen+)
- `i965` — older Intel hardware
- `radeonsi` — AMD on Mesa

Pass it in the container environment:

```yaml
environment:
  - LIBVA_DRIVER_NAME=iHD   # or radeonsi for AMD
```

## 3. Does FFmpeg expose VAAPI encoders?

```bash
ffmpeg -encoders | grep vaapi
```

You should see at least `h264_vaapi`, `hevc_vaapi`, and — on
sufficiently new Intel/AMD hardware with a compatible
driver/FFmpeg stack — `av1_vaapi`. If the list is empty,
FFmpeg wasn't built with VAAPI. Use the Alchemist Docker
image or install an FFmpeg package that enables VAAPI.

## 4. Read the probe log

**Settings → Hardware → Probe Log** shows each probed
backend with the full FFmpeg stderr for failures. Common
patterns:

- **"Failed to get any profile / no valid profile"** —
  `vainfo` would show the same thing. Driver is missing or
  mis-selected (step 2).
- **"Permission denied" on `/dev/dri/renderD128`** — the
  container user isn't in the `render` group (step 1).
- **`av1_vaapi` fails but `hevc_vaapi` works** — the driver
  or FFmpeg version lacks AV1 VAAPI encode. This is driver
  and FFmpeg-stack sensitive. Either update the stack or
  target HEVC. See [AV1](/av1).
- **"Probe succeeded" but Alchemist still uses CPU** — a
  different vendor scored higher, or **Preferred Vendor**
  is pinned elsewhere. See
  [Hardware Acceleration](/hardware).

## 5. Multi-GPU Linux hosts

If the host has multiple GPUs (iGPU + dGPU, or two dGPUs),
VAAPI auto-selection may pick the wrong one. Override:

- **Settings → Hardware → Device Path** → set to the render
  node you want (e.g. `/dev/dri/renderD128`).
- Verify with `vainfo --display drm --device <path>` that
  the node has the profiles you expect.

## Vendor-specific notes

- **Intel Arc** — uses VAAPI, not QSV. See the
  [Intel hardware guide](/hardware/intel). Forcing QSV on
  Arc is almost always wrong.
- **AMD on Linux** — uses `radeonsi` via VAAPI.
  `hevc_vaapi` and `h264_vaapi` are the validated paths;
  `av1_vaapi` support depends on GPU generation and
  driver/FFmpeg version. See the
  [AMD hardware guide](/hardware/amd).
- **AMD on Windows** — uses AMF, not VAAPI. This page does
  not apply; see the AMD guide for the AMF path.

## Related

- [Intel hardware guide](/hardware/intel)
- [AMD hardware guide](/hardware/amd)
- [GPU Passthrough](/gpu-passthrough)
- [Troubleshooting overview](/troubleshooting)
- [AV1 transcoding](/av1) — if the failing probe is
  specifically `av1_vaapi`.
