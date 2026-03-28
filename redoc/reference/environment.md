# Environment Variables

All environment variables Alchemist reads at startup.

Alchemist reads these environment variables at startup.
All are optional - sensible defaults apply when not set.

## Path configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `ALCHEMIST_CONFIG_PATH` | `~/.config/alchemist/config.toml` | Path to the TOML config file |
| `ALCHEMIST_CONFIG` | (alias) | Alias for `ALCHEMIST_CONFIG_PATH` |
| `ALCHEMIST_DB_PATH` | `~/.config/alchemist/alchemist.db` | Path to the SQLite database |
| `ALCHEMIST_DATA_DIR` | (none) | Sets the data directory; `alchemist.db` is placed here |
| `ALCHEMIST_CONFIG_MUTABLE` | `true` | Set to `false` to block all runtime config writes |

Default paths follow XDG on Linux and macOS
(`$XDG_CONFIG_HOME/alchemist/` or `~/.config/alchemist/`)
and `%APPDATA%\\Alchemist\\` on Windows.

## Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level filter. Examples: `info`, `debug`, `alchemist=trace,info` |

## Docker-specific notes

In Docker, always set both `ALCHEMIST_CONFIG_PATH` and
`ALCHEMIST_DB_PATH` explicitly to paths inside your mounted
volumes. If you don't, Alchemist will write config and
database files inside the container, which are lost when
the container is removed.

```yaml
environment:
  - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
  - ALCHEMIST_DB_PATH=/app/data/alchemist.db
```
