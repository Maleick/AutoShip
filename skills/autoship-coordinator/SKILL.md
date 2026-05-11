---
name: autoship-coordinator
description: Long-running AutoShip coordinator that spawns implementer subagents via OpenCode's task tool, tracks them by task_id, and handles lifecycle.
version: 1.0.0
author: AutoShip
license: MIT
metadata:
  hermes:
    tags: [autoship, coordinator, orchestration, subagent, task-tool]
---

# AutoShip Coordinator

Orchestrates AutoShip workers as OpenCode subagents. Scans for queued workspaces, spawns implementations via the `task` tool, tracks each worker by `task_id`, and handles completion, retry, and fallback.

## When to Use

Use this when running the coordinator to process queued workspaces, resume an interrupted coordinator session, or debug worker lifecycle.

## Workflow

1. Scan `.autoship/workspaces/*/` for status = `QUEUED`
2. Check concurrency cap from config (`max_workers` or `max_concurrent`)
3. For each eligible workspace up to the cap:
   a. Read `workspace/model` and `workspace/AUTOSHIP_PROMPT.md`
   b. Set status `RUNNING`
   c. Call `task(subagent_type="general", prompt=<prompt>)`
   d. Write returned `task_id` to `workspace/task_id`
4. Wait for subagents to finish
5. For each: read `workspace/status` — if COMPLETE update state, if STUCK try fallback model
6. Repeat until no QUEUED workspaces remain

## Spawning Subagents

Use `task` with `subagent_type="general"` and the full prompt from `AUTOSHIP_PROMPT.md`. Record the returned `task_id` in `workspace/task_id` for tracking and resume.

## Monitoring

The coordinator relies on `workspace/status` files and `task` tool returns. On restart, it can resume subagents via stored `task_id`.

## Boundaries

Does NOT write implementation code, modify prompts, or change issue labels. Scope is limited to subagent lifecycle management.
