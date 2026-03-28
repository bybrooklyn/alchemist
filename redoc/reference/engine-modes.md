# Engine Modes

Background, Balanced, and Throughput - what they mean and when to use each.

Engine modes control how aggressively Alchemist uses your
system's resources. The mode determines the **concurrent job
limit** - how many files can be encoded at the same time.

## Modes

### Background

**1 concurrent job, always.**

Use this when the server is in active use - gaming, Plex
streaming, browsing - and you want Alchemist to make progress
without noticably impacting anything else.

### Balanced (default)

**floor(cpu_count / 2), minimum 1, maximum 4.**

The default. On a 4-core machine this is 2 jobs. On an 8-core
machine this is 4 jobs. Good for most setups where the server
is shared with other workloads.

### Throughput

**floor(cpu_count / 2), minimum 1, no cap.**

Same formula as Balanced but without the maximum of 4. On a
32-core machine this could mean 16 concurrent jobs. Use this
when you want to clear a large backlog as fast as possible and
the server is dedicated to transcoding.

## Manual override

You can override the computed limit from Settings -> Runtime.
The override takes effect immediately without restarting.
The "manual" label appears in the engine status display when
an override is active. Switching modes clears the override
and returns to auto-computed limits.

## Engine states vs. modes

Modes are separate from engine states:

- **Mode** determines how many jobs run concurrently
- **State** determines whether jobs run at all

You can be in Throughput mode while the engine is Paused -
no jobs will run. When you Resume, jobs will run at the
Throughput concurrent limit.

| State | Behavior |
|-------|----------|
| Running | Jobs start normally up to the mode's limit |
| Paused | No jobs start; active jobs freeze |
| Draining | Active jobs finish; no new jobs start |
| Scheduler paused | Paused by a schedule window |

## Changing modes

Modes are set in **Settings -> Runtime**. The change takes
effect immediately - in-progress jobs are not cancelled.
