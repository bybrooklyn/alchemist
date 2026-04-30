---
title: Notifications
description: Configure Discord, Gotify, ntfy, Telegram, email, and webhook alerts.
---

Configure notification targets in **Settings → Notifications**.
Targets subscribe to specific events, and quiet hours can
suppress non-critical sends during a local-time window.

## Supported targets

### Discord webhook

Create a webhook in your Discord channel settings
(channel → Integrations → Webhooks). Paste the URL into
Alchemist.

### Discord bot

Provide a bot token and target channel ID. This is useful
when you want a single bot identity instead of per-channel
webhooks.

### Gotify

Enter your Gotify server URL and app token. Gotify supports
the same event filtering model as the other providers.

### ntfy

Enter your ntfy server URL and topic. For private instances,
you can also provide an access token. Alchemist sends the
same human-readable event summaries it uses for Discord and
Gotify, with ntfy priority mapped from the event type.

### Generic webhook

Alchemist sends a JSON POST to any URL you configure.
Works with Home Assistant, Apprise, and custom scripts.

### Telegram

Provide a bot token and chat ID. Alchemist posts the same
human-readable event summaries it uses for Discord and
Gotify.

### Email

Configure an SMTP host, port, sender address, recipient
addresses, and security mode (`STARTTLS`, `TLS`, or `None`).

Webhook payloads now include structured explanation data
when relevant:

- `decision_explanation`
- `failure_explanation`

Discord and Gotify targets use the same structured
summary/detail/guidance internally, but render them as
human-readable message text instead of raw JSON.

## Event types

Targets can subscribe independently to:

- `encode.queued`
- `encode.started`
- `encode.completed`
- `encode.failed`
- `scan.completed`
- `engine.idle`
- `daily.summary`

Daily summaries are opt-in per target and use the global
local-time send window configured in **Settings →
Notifications**.

## Quiet hours

Quiet hours are global and use local wall-clock time.

- `quiet_hours_enabled`
- `quiet_hours_start_local`
- `quiet_hours_end_local`

When enabled, non-critical event notifications are suppressed
inside the window. Failure notifications remain immediate so
operators still see broken encodes.

## Config shape

Targets are stored as:

```json
{
  "name": "Example",
  "target_type": "webhook",
  "config_json": { "url": "https://example.com/hook" },
  "events": ["encode.completed", "encode.failed"],
  "enabled": true
}
```

Legacy `endpoint_url` / `auth_token` target fields are
migrated into `config_json` and kept only as compatibility
projections.

## Troubleshooting

If notifications aren't arriving:

1. Check the URL, token, SMTP host, or chat ID for extra whitespace
2. Check **Logs** — Alchemist logs notification failures
   with response code and body
3. Verify the server has network access to the target
