---
title: Engine Modes
description: Background, Balanced, and Throughput — what they mean and when to use each.
---

Engine modes set the concurrent job limit.

## Modes

| Mode | Concurrent jobs | Use when |
|------|----------------|----------|
| Background | 1 | Server in active use |
| Balanced (default) | floor(cpu_count / 2), min 1, max 4 | General shared server |
| Throughput | floor(cpu_count / 2), min 1, no cap | Dedicated server, clear a backlog |

## Manual override

Override the computed limit in **Settings → Runtime**. Takes
effect immediately. A "manual" badge appears in engine
status. Switching modes clears the override.

## States vs. modes

Modes determine *how many* jobs run. States determine
*whether* they run.

| State | Behavior |
|-------|----------|
| Running | Jobs start up to the mode's limit |
| Paused | No jobs start; active jobs freeze |
| Draining | Active jobs finish; no new jobs start |
| Scheduler paused | Paused by a schedule window |

## Changing modes

**Settings → Runtime**. Takes effect immediately; in-progress
jobs are not cancelled.
