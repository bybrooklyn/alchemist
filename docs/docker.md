---
title: Docker
description: Docker Compose configuration, volumes, environment variables, and updates.
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
      - /path/to/config:/app/config
      - /path/to/data:/app/data
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
| `/app/config` | `config.toml` — persists across restarts |
| `/app/data` | `alchemist.db` (SQLite) — persists across restarts |
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
Docker configuration.

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
