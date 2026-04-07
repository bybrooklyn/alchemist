# Security Best Practices Report

## Executive Summary

I found one critical security bug and one additional high-severity issue in the setup/bootstrap flow.

The critical problem is that first-run setup is remotely accessible without authentication while the server listens on `0.0.0.0`. A network-reachable attacker can win the initial setup race, create the first admin account, and take over the instance.

I did not find evidence of major client-side XSS sinks or obvious SQL injection paths during this audit. Most of the remaining concerns I saw were hardening-level issues rather than immediately exploitable major bugs.

## Critical Findings

### ALCH-SEC-001

- Severity: Critical
- Location:
  - `src/server/middleware.rs:80-86`
  - `src/server/wizard.rs:95-210`
  - `src/server/mod.rs:176-197`
  - `README.md:61-79`
- Impact: Any attacker who can reach the service before the legitimate operator completes setup can create the first admin account and fully compromise the instance.

#### Evidence

`auth_middleware` exempts the full `/api/setup` namespace from authentication:

- `src/server/middleware.rs:80-86`

`setup_complete_handler` only checks `setup_required` and then creates the user, session cookie, and persisted config:

- `src/server/wizard.rs:95-210`

The server binds to all interfaces by default:

- `src/server/mod.rs:176-197`

The documented Docker quick-start publishes port `3000` directly:

- `README.md:61-79`

#### Why This Is Exploitable

On a fresh install, or any run where `setup_required == true`, the application accepts unauthenticated requests to `/api/setup/complete`. Because the listener binds `0.0.0.0`, that endpoint is reachable from any network that can reach the host unless an external firewall or reverse proxy blocks it.

That lets a remote attacker:

1. POST their own username and password to `/api/setup/complete`
2. Receive the initial authenticated session cookie
3. Persist attacker-controlled configuration and start operating as the admin user

This is a full-authentication-bypass takeover of the instance during bootstrap.

#### Recommended Fix

Require setup completion to come only from a trusted local origin during bootstrap, matching the stricter treatment already used for `/api/fs/*` during setup.

Minimal safe options:

1. Restrict `/api/setup/*` and `/api/settings/bundle` to loopback-only while `setup_required == true`.
2. Alternatively require an explicit one-time bootstrap secret/token generated on startup and printed locally.
3. Consider binding to `127.0.0.1` by default until setup is complete, then allowing an explicit public bind only after bootstrap.

#### Mitigation Until Fixed

- Do not expose the service to any network before setup is completed.
- Do not publish the container port directly on untrusted networks.
- Complete setup only through a local-only tunnel or host firewall rule.

## High Findings

### ALCH-SEC-002

- Severity: High
- Location:
  - `src/server/middleware.rs:116-117`
  - `src/server/settings.rs:244-285`
  - `src/config.rs:366-390`
  - `src/main.rs:369-383`
  - `src/db.rs:2566-2571`
- Impact: During setup mode, an unauthenticated remote attacker can read and overwrite the full runtime configuration; after `--reset-auth`, this can expose existing notification endpoints/tokens and let the attacker reconfigure the instance before the operator reclaims it.

#### Evidence

While `setup_required == true`, `auth_middleware` explicitly allows `/api/settings/bundle` without authentication:

- `src/server/middleware.rs:116-117`

`get_settings_bundle_handler` returns the full `Config`, and `update_settings_bundle_handler` writes an attacker-supplied `Config` back to disk and runtime state:

- `src/server/settings.rs:244-285`

The config structure includes notification targets and optional `auth_token` fields:

- `src/config.rs:366-390`

`--reset-auth` only clears users and sessions, then re-enters setup mode:

- `src/main.rs:369-383`
- `src/db.rs:2566-2571`

#### Why This Is Exploitable

This endpoint is effectively a public config API whenever the app is in setup mode. On a brand-new install that broadens the same bootstrap attack surface as ALCH-SEC-001. On an existing deployment where an operator runs `--reset-auth`, the previous configuration remains on disk while authentication is removed, so a remote caller can:

1. GET `/api/settings/bundle` and read the current config
2. Learn configured paths, schedules, webhook targets, and any stored notification bearer tokens
3. PUT a replacement config before the legitimate operator finishes recovery

That creates both confidential-data exposure and unauthenticated remote reconfiguration during recovery/bootstrap windows.

#### Recommended Fix

Do not expose `/api/settings/bundle` anonymously.

Safer options:

1. Apply the same loopback-only setup restriction used for `/api/fs/*`.
2. Split bootstrap-safe fields from privileged configuration and expose only the minimal bootstrap payload anonymously.
3. Redact secret-bearing config fields such as notification tokens from any unauthenticated response path.

## Notes

- I did not find a major DOM-XSS path in `web/src`; there were no `dangerouslySetInnerHTML`, `innerHTML`, `insertAdjacentHTML`, `eval`, or similar high-risk sinks in the audited code paths.
- I also did not see obvious raw SQL string interpolation issues; the database code I reviewed uses parameter binding.
