---
title: Notifications
description: Configure Discord, Gotify, and webhook alerts.
---

Configure notification targets in **Settings → Notifications**.

## Supported targets

### Discord webhook

Create a webhook in your Discord channel settings
(channel → Integrations → Webhooks). Paste the URL into
Alchemist.

### Gotify

Enter your Gotify server URL and app token.

### Generic webhook

Alchemist sends a JSON POST to any URL you configure.
Works with Home Assistant, ntfy, Apprise, and custom scripts.

Webhook payloads now include structured explanation data
when relevant:

- `decision_explanation`
- `failure_explanation`

Discord and Gotify targets use the same structured
summary/detail/guidance internally, but render them as
human-readable message text instead of raw JSON.

## Troubleshooting

If notifications aren't arriving:

1. Check the URL or token for extra whitespace
2. Check **Logs** — Alchemist logs notification failures
   with response code and body
3. Verify the server has network access to the target
