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
| `ALCHEMIST_TEMP_DIR` | data dir + `/temp` | Directory for managed temporary uploads, conversion outputs, and update staging |
| `ALCHEMIST_CONFIG_MUTABLE` | `true` | Set `false` to block runtime config writes |
| `ALCHEMIST_SERVER_PORT` | auto from `3000` | Require one specific HTTP port instead of falling forward when `3000` is busy |
| `ALCHEMIST_SETUP_TOKEN` | (none) | Optional extra setup-mode guard; setup endpoints require the matching token query parameter when set |
| `ALCHEMIST_COOKIE_SECURE` | `false` | Set `true` only behind a TLS-terminating reverse proxy so session cookies include `Secure` |
| `ALCHEMIST_NO_PAUSE` | (unset) | Skip the Windows-style "Press Enter to exit" pause after a fatal startup error |
| `RUST_LOG` | `info` | Log level: `info`, `debug`, `alchemist=trace` |
| `ALCHEMIST_LOG_FORMAT` | `text` | Log output format: `text` (human-readable, default) or `json` (one structured object per line, for Loki / Elasticsearch / Datadog ingestion). Overrides `[system].log_format` in the config. |

Build and release automation also use `ALCHEMIST_UPDATE_PUBLIC_KEY_B64` to
embed the update-manifest verification key in release binaries. It is not a
runtime setting for normal installs.

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
