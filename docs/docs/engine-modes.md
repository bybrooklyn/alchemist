---
title: Engine Modes & States
description: Background, Balanced, and Throughput — understanding concurrency and execution flow.
---

Alchemist uses **Modes** to dictate performance limits and **States** to control execution flow.

## Engine Modes (Concurrency)

Modes define the maximum number of concurrent jobs the engine will attempt to run. 

| Mode | Concurrent Jobs | Ideal For |
|------|----------------|-----------|
| **Background** | 1 | Server in active use by other applications. |
| **Balanced** | `floor(cpu_count / 2)` (min 1, max 4) | Default. Shared server usage. |
| **Throughput** | `floor(cpu_count / 2)` (min 1, no cap) | Dedicated server; clearing a large backlog. |

:::tip Manual Override
You can override the computed limit in **Settings → Runtime**. A "Manual" badge will appear in the engine status. Switching modes clears manual overrides.
:::

---

## Engine States (Execution)

States determine whether the engine is actively processing the queue.

| State | Behavior |
|-------|----------|
| **Running** | Engine is active. Jobs start up to the current mode's limit. |
| **Paused** | Engine is suspended. No new jobs start; active jobs are frozen. |
| **Draining** | Engine is stopping. Active jobs finish, but no new jobs start. |
| **Scheduler Paused** | Engine is temporarily paused by a configured [Schedule Window](/scheduling). |

---

## Changing Engine Behavior

Engine behavior can be adjusted in real-time via the **Runtime** dashboard or the [API](/api#engine). Changes take effect immediately without cancelling in-progress jobs.
