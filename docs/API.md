# AutoShip API Documentation

Generated automatically from `src/types.ts`.

## Table of Contents

- [Enums / String-Literal Unions](#enums--string-literal-unions)
- [Issue & State](#issue--state)
- [Model Routing](#model-routing)
- [Runtime Config](#runtime-config)
- [Event Queue](#event-queue)
- [Failure Artifacts](#failure-artifacts)
- [Token Ledger](#token-ledger)
- [Quota](#quota)
- [Diagnostics](#diagnostics)
- [CLI / Plugin](#cli--plugin)

### IssueState

Valid lifecycle states for an individual issue.

```typescript
export type IssueState =
  | "unclaimed"
  | "claimed"
  | "queued"
  | "running"
  | "verifying"
  | "completed"
  | "blocked"
  | "stuck"
  | "merged"
  | "paused";
```

### WorkspaceStatus

Status values written to workspace status files by workers.

```typescript
export type WorkspaceStatus = "QUEUED" | "RUNNING" | "COMPLETE" | "BLOCKED" | "STUCK";
```

### AgentRole

Frontier / specialised agent roles used for model routing.

```typescript
export type AgentRole =
  | "planner"
  | "coordinator"
  | "orchestrator"
  | "reviewer"
  | "lead"
  | "implementer"
  | "simplifier"
  | "tester"
  | "docs"
  | "release";
```

### ModelCost

Cost classification for a worker model.

```typescript
export type ModelCost = "free" | "selected";
```

### TaskType

Classification of a GitHub issue into a task type.

```typescript
export type TaskType =
  | "research"
  | "docs"
  | "simple_code"
  | "medium_code"
  | "complex"
  | "mechanical"
  | "ci_fix"
  | "rust_unsafe";
```

### FailureCategory

Categories used when capturing failure artifacts.

```typescript
export type FailureCategory =
  | "stuck"
  | "failed_verification"
  | "reviewer_rejection"
  | "model_failure"
  | "e2e_failure"
  | "timeout"
  | "dead_worker"
  | "salvaged_truncation"
  | "tests_only";
```

### EventType

Event types that can be emitted into the event queue.

```typescript
export type EventType = "blocked" | "stuck" | "verify" | "force_dispatch";
```

### CheckStatus

Result of a diagnostic check run by `opencode-autoship doctor`.

```typescript
export type CheckStatus = "PASS" | "WARN" | "FAIL";
```

### FailureEvidence

Failure evidence recorded when an issue enters the `blocked` / `stuck` state.

```typescript
export interface FailureEvidence {
  /** Path to the captured failure artifact JSON file. */
  failure_file: string;
  /** ID of the failure artifact. */
  failure_id: string;
  /** Normalised failure category. */
  failure_category: FailureCategory;
  /** Human-readable summary of the error. */
  error_summary: string;
  /** Attempt number at which the failure occurred. */
  attempt: number;
  /** ISO-8601 timestamp of the failure. */
  timestamp: string;
}
```

### Issue

Per-issue metadata stored in `.autoship/state.json`.

```typescript
export interface Issue {
  /** Current lifecycle state. */
  state: IssueState;
  /** GitHub issue title (cached for PR generation). */
  title?: string;
  /** GitHub issue labels (cached for PR generation). */
  labels?: string;
  /** Model ID selected for this issue. */
  model?: string;
  /** Agent role assigned to this issue. */
  role?: AgentRole;
  /** Task classification (defaults to `"medium_code"`). */
  task_type?: TaskType;
  /** Complexity estimate (`"low"`, `"medium"`, `"high"`). */
  complexity?: string;
  /** Worker name or model ID (alias for `model`). */
  agent?: string;
  /** Retry / attempt counter. */
  attempt?: number;
  /** Timestamp when the current attempt started. */
  started_at?: string;
  /** Timestamp of the first attempt (preserved across retries). */
  first_started_at?: string;
  /** Git worktree path for this issue. */
  worktree?: string;
  /** Associated PR number, once created. */
  pr_number?: number;
  /** PR creation mode (`"dry-run"` or `"live"`). */
  pr_mode?: "dry-run" | "live";
  /** PR title, once generated. */
  pr_title?: string;
  /** Number of tokens consumed by the worker. */
  tokens_used?: number;
  /** Number of retry attempts already executed. */
  retry_count?: number;
  /** Maximum number of retries allowed. */
  retry_limit?: number;
  /** Whether the issue is still eligible for automatic retry. */
  retry_eligible?: boolean;
  /** Whether the failure is terminal (no more retries). */
  terminal_failure?: boolean;
  /** Reason for escalation / blocking. */
  escalation_reason?: string;
  /** ISO-8601 timestamp of terminal failure. */
  terminal_failed_at?: string;
  /** Structured failure evidence. */
  failure_evidence?: FailureEvidence;
  /** History of all attempt transitions. */
  attempts_history?: Array<{ state: IssueState; at: string }>;
  /** Whether the workspace has a fresh AUTOSHIP_RESULT.md. */
  has_result?: boolean;
  /** Whether uncommitted changes exist in the workspace. */
  has_uncommitted_changes?: boolean;
  /** Raw workspace status file value at last reconcile. */
  workspace_status?: WorkspaceStatus;
  /** ISO-8601 timestamp when the issue was marked completed. */
  completed_at?: string;
}
```

### Stats

Session-level statistics counters.

```typescript
export interface Stats {
  /** Issues dispatched in the current session. */
  session_dispatched: number;
  /** Issues completed in the current session. */
  session_completed: number;
  /** Total issues dispatched across all sessions. */
  total_dispatched_all_time: number;
  /** Total issues completed across all sessions. */
  total_completed_all_time: number;
  /** Number of stuck / failed issues. */
  failed: number;
  /** Number of blocked issues. */
  blocked: number;
}
```

### Plan

Plan phase tracking (used by init.sh).

```typescript
export interface Plan {
  /** Ordered list of plan phases. */
  phases: Array<{ name: string; description?: string }>;
  /** Index of the currently active phase. */
  current_phase: number;
  /** Whether a checkpoint is pending review. */
  checkpoint_pending: boolean;
}
```

### ToolStatus

Tool availability entry (e.g. `opencode`).

```typescript
export interface ToolStatus {
  /** `"available"` | `"unavailable"` */
  status: "available" | "unavailable";
  /** Quota percentage consumed (`-1` when unlimited / unknown). */
  quota_pct: number;
}
```

### StateConfig

Runtime configuration embedded in `.autoship/state.json`.

```typescript
export interface StateConfig {
  /** Maximum concurrent worker agents (default: `15`). */
  maxConcurrentAgents: number;
  /** Legacy alias for `maxConcurrentAgents`. */
  max_concurrent_agents?: number;
  /** Maximum retry attempts per issue (default: `3`). */
  maxRetries?: number;
  /** Legacy alias for `maxRetries`. */
  max_retries?: number;
  /** Worker stall timeout in milliseconds (default: `900000`). */
  workerTimeoutMs?: number;
  /** Legacy alias for `workerTimeoutMs`. */
  stall_timeout_ms?: number;
  /** Cargo target isolation threshold. */
  cargoTargetIsolationThreshold?: number;
  /** Whether to salvage truncated workers. */
  truncationSalvage?: boolean;
}
```

### StateFile

Top-level structure of `.autoship/state.json`.

```typescript
export interface StateFile {
  /** Schema version (currently `1`). */
  version?: number;
  /** AutoShip package version string. */
  autoship_version: string;
  /** Runtime platform (`"opencode"`). */
  platform: string;
  /** GitHub repo slug (`owner/repo`). */
  repo: string;
  /** ISO-8601 timestamp when the session started. */
  started_at: string;
  /** ISO-8601 timestamp of the last state update. */
  updated_at: string;
  /** Whether orchestration is paused. */
  paused: boolean;
  /** Plan phase tracking. */
  plan: Plan;
  /** Map of issue-id → {@link Issue}. */
  issues: Record<string, Issue>;
  /** Map of tool name → {@link ToolStatus}. */
  tools: Record<string, ToolStatus>;
  /** Session statistics. */
  stats: Stats;
  /** Runtime configuration. */
  config: StateConfig;
}
```

### ModelEntry

Individual worker model entry in `.autoship/model-routing.json`.

```typescript
export interface ModelEntry {
  /** Model ID from `opencode models`. */
  id: string;
  /** Cost classification. */
  cost: ModelCost;
  /** Capability score (`0`–`150`). */
  strength: number;
  /** Compatible task types. */
  max_task_types: TaskType[];
  /** Whether the model is enabled (default: `true`). */
  enabled?: boolean;
}
```

### ModelPool

A named worker pool in `.autoship/model-routing.json`.

```typescript
export interface ModelPool {
  /** Human-readable description. */
  description: string;
  /** Ordered list of model IDs in this pool. */
  models: string[];
}
```

### RoleAssignments

Role-to-model assignments.

```typescript
export type RoleAssignments = Record<AgentRole, string>;
```

### ModelRouting

Top-level structure of `.autoship/model-routing.json`.

```typescript
export interface ModelRouting {
  /** Frontier role assignments. */
  roles: RoleAssignments;
  /** Named worker pools. */
  pools: Record<string, ModelPool>;
  /** Default fallback model ID. */
  defaultFallback: string;
  /** Full list of configured worker models. */
  models: ModelEntry[];
}
```

### ModelHistoryEntry

Per-model success / failure history in `.autoship/model-history.json`.

```typescript
export interface ModelHistoryEntry {
  /** Number of successful runs. */
  success: number;
  /** Number of failed runs. */
  fail: number;
  /** Last error log excerpt. */
  last_error?: string;
  /** ISO-8601 timestamp of the last failure. */
  last_failed_at?: string;
}
```

### AutoshipConfig

Top-level structure of `.autoship/config.json`.

```typescript
export interface AutoshipConfig {
  /** Runtime identifier (`"opencode"`). */
  runtime: string;
  /** Maximum concurrent agents (duplicated from state for consumer convenience). */
  maxConcurrentAgents: number;
  /** Legacy alias. */
  max_agents?: number;
  /** Labels to monitor (default: `["agent:ready"]`). */
  labels: string[];
  /** Frontier role model IDs (legacy, prefer `model-routing.json`). */
  plannerModel?: string;
  coordinatorModel?: string;
  orchestratorModel?: string;
  reviewerModel?: string;
  leadModel?: string;
  /** Full list of configured model IDs. */
  models: string[];
  /** Whether model refresh was requested during setup. */
  refreshModels: boolean;
}
```

### EventData

Payload carried by an event in `.autoship/event-queue.json`.

```typescript
export interface EventData {
  /** Workspace status that triggered the event. */
  status: WorkspaceStatus;
}
```

### Event

Single event entry in `.autoship/event-queue.json`.

```typescript
export interface Event {
  /** Event classification. */
  type: EventType;
  /** Normalised issue key (`issue-<N>`). */
  issue: string;
  /** Priority (`1`=high … `3`=low). */
  priority: number;
  /** Event payload. */
  data: EventData;
  /** ISO-8601 timestamp when the event was queued. */
  queued_at: string;
}
```

### FailureArtifact

Structure of a captured failure artifact in `.autoship/failures/*.json`.

```typescript
export interface FailureArtifact {
  /** Unique failure ID (`<timestamp>-<issue-key>`). */
  failure_id: string;
  /** Associated issue key. */
  issue: string;
  /** Normalised failure category. */
  failure_category: FailureCategory;
  /** Model ID in use when the failure occurred. */
  model: string;
  /** Agent role in use when the failure occurred. */
  role: string;
  /** Absolute path to the workspace directory. */
  workspace: string;
  /** Hook script responsible for the failure. */
  hook: string;
  /** Relevant log excerpt (last 100 lines). */
  logs: string;
  /** Brief human-readable error summary. */
  error_summary: string;
  /** Attempt number at failure time. */
  attempt: number;
  /** ISO-8601 timestamp of the failure. */
  timestamp: string;
}
```

### TokenRecord

Per-issue token-usage record inside a session.

```typescript
export interface TokenRecord {
  /** GitHub issue number. */
  number: number;
  /** Task classification. */
  type: TaskType;
  /** Complexity estimate. */
  complexity: string;
  /** Model / agent used. */
  agent: string;
  /** Tokens consumed. */
  tokens_used: number;
  /** Duration in milliseconds. */
  duration_ms: number;
  /** Outcome (`"pass"` | `"fail"` | `"blocked"`). */
  verdict: "pass" | "fail" | "blocked";
  /** Associated PR number, if any. */
  pr_number: number;
  /** Attempt number. */
  attempt: number;
}
```

### Session

A single orchestration session in `.autoship/token-ledger.json`.

```typescript
export interface Session {
  /** Unique session identifier. */
  session_id: string;
  /** ISO-8601 timestamp when the session started. */
  started_at: string;
  /** GitHub repo slug. */
  repo: string;
  /** Ordered list of issue records. */
  issues: TokenRecord[];
}
```

### TokenLedger

Top-level structure of `.autoship/token-ledger.json`.

```typescript
export interface TokenLedger {
  /** Schema version. */
  schema_version: number;
  /** Ordered list of sessions. */
  sessions: Session[];
}
```

### QuotaEntry

Per-provider quota status in `.autoship/quota.json`.

```typescript
export interface QuotaEntry {
  /** Whether the provider is available. */
  available: boolean;
  /** Quota percentage consumed (`-1` when unknown). */
  quota_pct: number;
  /** Source of the quota data. */
  quota_source: string;
  /** Number of dispatches recorded. */
  dispatches: number;
}
```

### QuotaFile

Top-level structure of `.autoship/quota.json`.

```typescript
export interface QuotaFile {
  opencode: QuotaEntry;
}
```

### DoctorCheck

Single check emitted by `opencode-autoship doctor`.

```typescript
export interface DoctorCheck {
  /** Check identifier (e.g. `"package-registration"`). */
  name: string;
  /** Outcome. */
  status: CheckStatus;
  /** Human-readable message. */
  message: string;
}
```

### PluginServer

Minimal OpenCode plugin server shape returned by {@link server}.

```typescript
export interface PluginServer {
  /** Event handler (currently a no-op stub). */
  event(): undefined;
}
```

