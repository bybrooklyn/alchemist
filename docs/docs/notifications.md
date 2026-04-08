---
title: Notifications
description: Configure Discord, Gotify, Telegram, email, and webhook alerts.
---

Configure notification targets in **Settings → Notifications**.

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

### Generic webhook

Alchemist sends a JSON POST to any URL you configure.
Works with Home Assistant, ntfy, Apprise, and custom scripts.

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

## Troubleshooting

If notifications aren't arriving:

1. Check the URL, token, SMTP host, or chat ID for extra whitespace
2. Check **Logs** — Alchemist logs notification failures
   with response code and body
3. Verify the server has network access to the target
