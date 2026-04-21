---
title: Run Alchemist on Docker — Self-Hosted Transcoding Container
description: Docker Compose configuration, volumes, environment variables, and updates for Alchemist. Works on standard Docker, Unraid, Synology, TrueNAS, and Proxmox hosts.
keywords:
  - docker transcoding
  - docker ffmpeg automation
  - alchemist docker
  - self-hosted transcoder docker
---

## Recommended Compose file

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    container_name: alchemist
    ports:
      - "3000:3000"
    volumes:
      - ~/.config/alchemist:/app/config
      - ~/.config/alchemist:/app/data
      - /path/to/media:/media
      - /tmp/alchemist:/tmp   # optional: fast SSD for temp files
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
    restart: unless-stopped
```

## Volumes

| Mount | Purpose |
|-------|---------|
| `~/.config/alchemist` on the host | Mounted into `/app/config` and `/app/data` so `config.toml` and `alchemist.db` persist across restarts |
| `/media` | Your media library — mount read-write |
| `/tmp` (optional) | Temp dir for in-progress encodes — use a fast SSD |

## Environment variables

| Variable | Description |
|----------|-------------|
| `ALCHEMIST_CONFIG_PATH` | Path to `config.toml` inside the container |
| `ALCHEMIST_DB_PATH` | Path to the SQLite database inside the container |
| `ALCHEMIST_CONFIG_MUTABLE` | Set `false` to block runtime config writes |
| `RUST_LOG` | Log verbosity: `info`, `debug`, `alchemist=trace` |

Alchemist does not use `PUID`/`PGID`. Handle permissions
at the host level.

## Hardware acceleration

See [GPU Passthrough](/gpu-passthrough) for vendor-specific
Docker configuration. If a GPU is present but Alchemist falls
back to CPU, the
[Troubleshooting](/troubleshooting#cpu-fallback-despite-gpu)
page walks through the common causes.

## Updating

```bash
docker compose pull && docker compose up -d
```

Migrations run automatically on startup. Config and database
are preserved in mounted volumes.

## Nightly builds

```yaml
image: ghcr.io/bybrooklyn/alchemist:nightly
```

Published on every push to `main` that passes Rust checks.

## See also

- [GPU Passthrough](/gpu-passthrough) — NVIDIA, Intel, AMD
  device setup in Docker.
- [First Run](/first-run) — setup wizard after the container
  is up.
- [Alchemist for Jellyfin](/jellyfin) — pointing Alchemist
  at a Jellyfin library.
- [Troubleshooting](/troubleshooting) — CPU fallback,
  permissions, missing encoders.
