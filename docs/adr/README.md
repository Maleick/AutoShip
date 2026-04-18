# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs) documenting key design choices in TextQuest.

An ADR captures a significant architectural decision, its context, and the reasoning behind the choice. Each ADR is immutable once decided — new decisions spawn new ADRs rather than replacing prior records.

## ADRs

- **[ADR-001: Logout Sequencer Design](ADR-001-logout-sequencer-design.md)** — State machine approach to orchestrated account logout (avoid race conditions, coordinate shutdown events)
- **[ADR-002: Test Scenario Trait Architecture](ADR-002-test-scenario-trait.md)** — Trait-based test harness allowing pluggable async scenarios with structured result collection
- **[ADR-003: JSON Event Streaming](ADR-003-json-event-streaming.md)** — JSONL append-only event log for audit trail and replay
- **[ADR-004: Per-Account Test Runners](ADR-004-per-account-test-runners.md)** — Parallel test execution with one runner per account to isolate state
- **[ADR-005: Graceful Shutdown on Ctrl+C](ADR-005-graceful-shutdown-sigint.md)** — Watch-channel signal propagation for coordinated fleet shutdown
- **[ADR-006: Metrics with Percentiles](ADR-006-metrics-percentiles.md)** — P50/P95/P99 aggregation for fleet-wide latency and throughput monitoring

## Status Legend

- **Proposed** — Documented but not yet implemented or adopted
- **Accepted** — In active use; informs architecture
- **Superseded** — Replaced by a newer ADR
- **Deprecated** — No longer recommended; kept for historical reference
