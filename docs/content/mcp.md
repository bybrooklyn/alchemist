---
title: MCP Server
description: Use Alchemist's read-only Model Context Protocol server for local assistant integrations.
---

Alchemist includes a read-only Model Context Protocol (MCP) server over stdio.
It is meant for local assistant integrations that need operational context
without receiving permission to mutate the queue, engine, or configuration.

## Start the server

```bash
alchemist --mcp
```

The MCP server uses the same config and SQLite database paths as the normal
Alchemist binary. Set `ALCHEMIST_CONFIG_PATH` or `ALCHEMIST_DB_PATH` first if
your instance does not use the default paths.

## Protocol behavior

- Transport: stdio
- JSON-RPC version: `2.0`
- Protocol version: `2025-06-18`
- Mode: read-only
- Mutating queue, engine, settings, scan, and file operations are intentionally
  not exposed in v1.

## Tools

| Tool | Purpose |
|------|---------|
| `alchemist_engine_status` | Read engine mode, pause/drain state, and concurrency limit |
| `alchemist_job_summary` | Read aggregate active, queued, completed, and failed job counts |
| `alchemist_recent_jobs` | Read recently updated jobs, with a `limit` from 1 to 50 |
| `alchemist_savings_summary` | Read storage savings and codec savings metrics |
| `alchemist_scan_status` | Read current library scan progress |
| `alchemist_system_health` | Read version, MCP mode, protocol version, database readiness, and tool names |

All tool definitions advertise `readOnlyHint: true` and
`destructiveHint: false`.

## Example client command

Use the built binary path your MCP client can execute:

```json
{
  "command": "/usr/local/bin/alchemist",
  "args": ["--mcp"]
}
```

For source checkouts, point at the compiled binary:

```json
{
  "command": "/path/to/alchemist/target/release/alchemist",
  "args": ["--mcp"]
}
```
