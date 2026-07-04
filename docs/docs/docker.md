---
title: Docker
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
      - ./config:/app/config
      - ./data:/app/data
      - /path/to/media:/media
      - /tmp/alchemist:/tmp   # optional: fast SSD for temp files
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
      # - PUID=1000
      # - PGID=1000
    restart: unless-stopped
```

The setup wizard writes `config.toml` into `./config` on first
run. Mount the **directories**, never `config.toml` itself — a
single-file bind mount blocks the wizard from saving (and if the
host file doesn't exist, Docker creates a directory named
`config.toml`), which restarts setup on every boot.

Do not set `ALCHEMIST_CONFIG_MUTABLE=false` until setup has
completed at least once; the wizard needs to write the config.

## Config and data paths

Inside the Docker container, Alchemist reads its config from
`/app/config/config.toml` and its SQLite database from
`/app/data/alchemist.db`. The image sets these defaults with
`ALCHEMIST_CONFIG_PATH=/app/config/config.toml` and
`ALCHEMIST_DB_PATH=/app/data/alchemist.db`.

Your Compose volumes decide where those container directories
are stored on the Docker host. Docker volume entries are
`host_path:container_path`:

```yaml
volumes:
  - ./config:/app/config
```

- `./config` is the directory on the Docker host.
- `/app/config` is the directory inside the Alchemist container.
- `ALCHEMIST_CONFIG_PATH=/app/config/config.toml` is therefore an
  in-container path, but the file persists on the host as
  `./config/config.toml`.

If your Compose file says `/data/alchemist/config:/app/config`,
then `/data/alchemist/config` is the host directory and
`/app/config` is only where the container sees it. Native binary
installs use `~/.config/alchemist/config.toml` by default because
there is no container mount involved.

## Volumes

| Mount | Purpose |
|-------|---------|
| `./config` | `config.toml` — written by the setup wizard, persists across restarts |
| `./data` | `alchemist.db` (SQLite) — jobs, users, history |
| `/media` | Your media library — mount read-write |
| `/tmp` (optional) | Temp dir for in-progress encodes — use a fast SSD |

## Environment variables

| Variable | Description |
|----------|-------------|
| `ALCHEMIST_CONFIG_PATH` | Path to `config.toml` inside the container |
| `ALCHEMIST_DB_PATH` | Path to the SQLite database inside the container |
| `ALCHEMIST_CONFIG_MUTABLE` | Set `false` to block runtime config writes (only after setup) |
| `PUID` / `PGID` | Run as this user/group id; mounted dirs are chowned at start. Unset = root. Supplemental `group_add` ids are preserved for render-node access. |
| `RUST_LOG` | Log verbosity: `info`, `debug`, `alchemist=trace` |

## Setup returns SETUP_ACCESS_FORBIDDEN

First-run setup is only reachable from the local network. On
Docker Desktop (Mac/Windows), the host's own connections to a
`0.0.0.0`-published port can reach the container with a
non-private source IP, so the gate rejects them. Two fixes:

- Bind the port to loopback while setting up:
  `"127.0.0.1:3000:3000"`, or
- Set `ALCHEMIST_SETUP_TOKEN=<secret>` and open
  `http://host:3000/setup?token=<secret>`.

## Health check

The image ships a `HEALTHCHECK` against `/api/health`, so
`docker ps` shows `healthy`/`unhealthy` and orchestrators can
restart a wedged container automatically.

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

The in-app update panel detects Docker and shows this command
instead of replacing files inside the container. Container
updates remain owned by your Docker/Compose workflow.

## Nightly builds

```yaml
image: ghcr.io/bybrooklyn/alchemist:nightly
```

Published on every push to `master` that passes Rust checks.

## See also

- [GPU Passthrough](/gpu-passthrough) — NVIDIA, Intel, AMD
  device setup in Docker.
- [First Run](/first-run) — setup wizard after the container
  is up.
- [Alchemist for Jellyfin](/jellyfin) — pointing Alchemist
  at a Jellyfin library.
- [Troubleshooting](/troubleshooting) — CPU fallback,
  permissions, missing encoders.
