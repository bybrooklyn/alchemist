# Docker

Docker Compose configuration, volume setup, and advanced options.

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

### Volume explanations

| Volume | Purpose |
|--------|---------|
| `/app/config` | Stores `config.toml`. Survives container restarts. |
| `/app/data` | Stores `alchemist.db` (the SQLite database). Survives restarts. |
| `/media` | Your media library. Mount read-write if Alchemist needs to write encoded files beside the source. |
| `/tmp` (optional) | Temp directory for in-progress encodes. Mounting a fast SSD here speeds things up on large files. |

### Environment variables

| Variable | Description |
|----------|-------------|
| `ALCHEMIST_CONFIG_PATH` | Path to `config.toml` inside the container |
| `ALCHEMIST_DB_PATH` | Path to the SQLite database file inside the container |
| `ALCHEMIST_CONFIG_MUTABLE` | Set to `false` if you want config changes blocked at runtime |
| `RUST_LOG` | Log verbosity, e.g. `info` or `alchemist=debug` |

> Caution: Alchemist does not use LinuxServer-style user/group
> env vars. File permissions are handled at the host level.
> Ensure the user running Docker has read/write access to your
> media directories.

## Hardware acceleration

See the [GPU Passthrough guide](gpu-passthrough.md) for
vendor-specific Docker configuration (NVIDIA, Intel, AMD).

## Unraid

An Unraid Community App template is planned. For now, use
Docker Compose or the manual container setup.

## Updating

```bash
docker compose pull
docker compose up -d
```

Your config and database are preserved in the mounted volumes.
Alchemist runs migrations automatically on startup when the
schema needs updating.

## Nightly builds

```bash
image: ghcr.io/bybrooklyn/alchemist:nightly
```

Nightly builds are published on every push to `main` that
passes Rust checks. Use `:latest` for stable releases.
