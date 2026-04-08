---
title: Environment Variables
description: All environment variables Alchemist reads at startup.
---

| Variable | Default | Description |
|----------|---------|-------------|
| `ALCHEMIST_CONFIG_PATH` | `~/.config/alchemist/config.toml` | Path to config file |
| `ALCHEMIST_CONFIG` | (alias) | Alias for `ALCHEMIST_CONFIG_PATH` |
| `ALCHEMIST_DB_PATH` | `~/.config/alchemist/alchemist.db` | Path to SQLite database |
| `ALCHEMIST_DATA_DIR` | (none) | Sets data dir; `alchemist.db` placed here |
| `ALCHEMIST_BASE_URL` | root (`/`) | Path prefix for serving Alchemist under a subpath such as `/alchemist` |
| `ALCHEMIST_CONFIG_MUTABLE` | `true` | Set `false` to block runtime config writes |
| `RUST_LOG` | `info` | Log level: `info`, `debug`, `alchemist=trace` |

Default paths: XDG on Linux/macOS, `%APPDATA%\Alchemist\`
on Windows.

## Docker

Always set path variables explicitly to paths inside your
mounted volumes. Without this, files are lost when the
container is removed.

```yaml
environment:
  - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
  - ALCHEMIST_DB_PATH=/app/data/alchemist.db
```

Recommended host bind mount:

```yaml
volumes:
  - ~/.config/alchemist:/app/config
  - ~/.config/alchemist:/app/data
```
