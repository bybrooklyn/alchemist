---
title: NVENC Not Detected
description: Fix "NVENC not detected" in Alchemist. Verify the NVIDIA driver, nvidia-container-toolkit, FFmpeg NVENC support, and the startup probe log that records why a backend failed.
keywords:
  - nvenc not detected
  - nvenc docker not working
  - nvenc ffmpeg not found
  - nvidia container toolkit not working
slug: /troubleshooting/nvenc-not-detected
---

If Alchemist is running on a host with an NVIDIA GPU and
NVENC isn't being used, the problem is almost always one of
three things: the driver isn't visible to the process,
FFmpeg wasn't built with NVENC support, or the startup probe
failed for a specific codec. This page walks through all
three in order.

Start with **Settings → Hardware → Probe Log**. Alchemist
probes every plausible encoder at startup with a short
FFmpeg test encode and records the stderr for every failure.
That log is the authoritative answer to "why didn't NVENC
work?" — most of the checks below are just narrowing down
what the probe log already says.

## 1. Is the driver visible?

On the host:

```bash
nvidia-smi
```

If that command fails or hangs, the driver is the problem,
not Alchemist. Reinstall or reload the NVIDIA kernel module
before continuing.

In Docker, `nvidia-smi` inside the container is the real
test:

```bash
docker run --rm --gpus all nvidia/cuda:12.0.0-base-ubuntu22.04 nvidia-smi
```

If this fails, the container can't see the GPU. That's
almost always a missing or misconfigured
`nvidia-container-toolkit`. Reinstall it per the
[NVIDIA hardware guide](/hardware/nvidia) and restart
Docker.

## 2. Does FFmpeg expose NVENC?

Inside the container (or on the host for a binary install):

```bash
ffmpeg -encoders | grep nvenc
```

You should see at least:

- `h264_nvenc`
- `hevc_nvenc`

On RTX 30 / 40 series, you should also see `av1_nvenc`. If
the list is empty, FFmpeg in that environment was not
compiled with NVENC. For Docker, use the official Alchemist
image — it ships FFmpeg with NVENC enabled. For a binary
install, install an FFmpeg build that includes NVENC support
(Debian/Ubuntu, Fedora, Arch, and Homebrew FFmpeg packages
all enable it by default, but distro forks differ).

## 3. Does the probe actually succeed?

Open **Settings → Hardware → Probe Log**. Each entry shows:

- The encoder being tested (`h264_nvenc`, `hevc_nvenc`,
  `av1_nvenc`).
- Whether the probe succeeded or failed.
- The full FFmpeg stderr for failures.

Common probe-level failures:

- **"Driver does not support the required NVENC features"**
  — update the NVIDIA driver; FFmpeg's NVENC build is ahead
  of your driver.
- **"No capable devices found"** — GPU isn't visible to the
  process (go back to step 1).
- **`av1_nvenc` fails but `hevc_nvenc` / `h264_nvenc`
  succeed** — your card is pre-Ampere. AV1 NVENC requires
  RTX 30 or RTX 40 series. Target HEVC or H.264 instead, or
  see [Codecs](/codecs).

## 4. Is the right vendor selected?

If the probe succeeded but NVENC still isn't being used,
check **Settings → Hardware → Preferred Vendor**. `auto`
selects based on scoring; setting it to `nvidia` forces
NVENC when it's available.

Leave **Device Path** empty. NVENC is selected from the
driver and `/dev/nvidiactl`, not a render-node path.

## 5. What to check when the probe log is empty

If no probes ran at all for NVENC, Alchemist didn't see the
GPU as a candidate. That usually means:

- The NVIDIA driver is not loaded (step 1).
- Docker's `--gpus all` flag (or Compose `deploy.resources`
  block) isn't set. See
  [GPU Passthrough](/gpu-passthrough#nvidia).
- You're running on a host that truly has no NVIDIA GPU —
  confirm with `lspci | grep -i nvidia`.

## Still stuck?

Check the related pages for the platform-specific paths, and
the broader troubleshooting overview:

- [NVIDIA hardware guide](/hardware/nvidia) — driver,
  generation support, Docker Compose.
- [GPU Passthrough](/gpu-passthrough#nvidia) —
  `nvidia-container-toolkit` install.
- [Troubleshooting overview](/troubleshooting) —
  CPU-fallback-despite-GPU and related issues.
- [AV1 transcoding](/av1) — if the problem is specifically
  `av1_nvenc` failing.

If the probe log points at a specific FFmpeg error that
isn't covered here, that stderr is the right thing to paste
into a GitHub issue:
[github.com/bybrooklyn/alchemist/issues](https://github.com/bybrooklyn/alchemist/issues).
