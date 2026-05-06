/**
 * Core type definitions for AutoShip — OpenCode-only GitHub issue → PR orchestration.
 *
 * This module exports strongly typed interfaces for all JSON structures consumed
 * and produced by the AutoShip runtime.  All types are derived from the shell
 * scripts in hooks/ and hooks/opencode/ as well as the public documentation in
 * wiki/Configuration.md.
 *
 * @module autoship-types
 */

// ---------------------------------------------------------------------------
// Enums / String-Literal Unions
// ---------------------------------------------------------------------------

/** Valid lifecycle states for an individual issue. */
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

/** Status values written to workspace status files by workers. */
export type WorkspaceStatus = "QUEUED" | "RUNNING" | "COMPLETE" | "BLOCKED" | "STUCK";

/** Frontier / specialised agent roles used for model routing. */
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

/** Cost classification for a worker model. */
export type ModelCost = "free" | "selected";

/** Classification of a GitHub issue into a task type. */
export type TaskType =
  | "research"
  | "docs"
  | "simple_code"
  | "medium_code"
  | "complex"
  | "mechanical"
  | "ci_fix"
  | "rust_unsafe";

/** Categories used when capturing failure artifacts. */
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

/** Event types that can be emitted into the event queue. */
export type EventType = "blocked" | "stuck" | "verify" | "force_dispatch";

/** Result of a diagnostic check run by `opencode-autoship doctor`. */
export type CheckStatus = "PASS" | "WARN" | "FAIL";

// ---------------------------------------------------------------------------
// Issue & State
// ---------------------------------------------------------------------------

/** Failure evidence recorded when an issue enters the `blocked` / `stuck` state. */
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

/** Per-issue metadata stored in `.autoship/state.json`. */
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

/** Session-level statistics counters. */
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

/** Plan phase tracking (used by init.sh). */
export interface Plan {
  /** Ordered list of plan phases. */
  phases: Array<{ name: string; description?: string }>;
  /** Index of the currently active phase. */
  current_phase: number;
  /** Whether a checkpoint is pending review. */
  checkpoint_pending: boolean;
}

/** Tool availability entry (e.g. `opencode`). */
export interface ToolStatus {
  /** `"available"` | `"unavailable"` */
  status: "available" | "unavailable";
  /** Quota percentage consumed (`-1` when unlimited / unknown). */
  quota_pct: number;
}

/** Runtime configuration embedded in `.autoship/state.json`. */
export interface StateConfig {
  /** Maximum concurrent worker agents (default: `15`). */
  maxConcurrentAgents: number;
  /** Maximum retry attempts per issue (default: `3`). */
  maxRetries?: number;
  /** Worker stall timeout in milliseconds (default: `900000`). */
  workerTimeoutMs?: number;
  /** Cargo target isolation threshold. */
  cargoTargetIsolationThreshold?: number;
  /** Whether to salvage truncated workers. */
  truncationSalvage?: boolean;
}

/** Top-level structure of `.autoship/state.json`. */
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

// ---------------------------------------------------------------------------
// Model Routing
// ---------------------------------------------------------------------------

/** Individual worker model entry in `.autoship/model-routing.json`. */
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

/** A named worker pool in `.autoship/model-routing.json`. */
export interface ModelPool {
  /** Human-readable description. */
  description: string;
  /** Ordered list of model IDs in this pool. */
  models: string[];
}

/** Role-to-model assignments. */
export type RoleAssignments = Record<AgentRole, string>;

/** Top-level structure of `.autoship/model-routing.json`. */
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

/** Per-model success / failure history in `.autoship/model-history.json`. */
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

// ---------------------------------------------------------------------------
// Runtime Config
// ---------------------------------------------------------------------------

/** Top-level structure of `.autoship/config.json`. */
export interface AutoshipConfig {
  /** Runtime identifier (`"opencode"`). */
  runtime: string;
  /** Maximum concurrent agents (duplicated from state for consumer convenience). */
  maxConcurrentAgents: number;
  /** Labels to monitor (default: `["agent:ready"]`). */
  labels: string[];
  /** Full list of configured model IDs. */
  models: string[];
  /** Whether model refresh was requested during setup. */
  refreshModels: boolean;
}

// ---------------------------------------------------------------------------
// Event Queue
// ---------------------------------------------------------------------------

/** Payload carried by an event in `.autoship/event-queue.json`. */
export interface EventData {
  /** Workspace status that triggered the event. */
  status: WorkspaceStatus;
}

/** Single event entry in `.autoship/event-queue.json`. */
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

// ---------------------------------------------------------------------------
// Failure Artifacts
// ---------------------------------------------------------------------------

/** Structure of a captured failure artifact in `.autoship/failures/*.json`. */
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

// ---------------------------------------------------------------------------
// Token Ledger
// ---------------------------------------------------------------------------

/** Per-issue token-usage record inside a session. */
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

/** A single orchestration session in `.autoship/token-ledger.json`. */
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

/** Top-level structure of `.autoship/token-ledger.json`. */
export interface TokenLedger {
  /** Schema version. */
  schema_version: number;
  /** Ordered list of sessions. */
  sessions: Session[];
}

// ---------------------------------------------------------------------------
// Quota
// ---------------------------------------------------------------------------

/** Per-provider quota status in `.autoship/quota.json`. */
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

/** Top-level structure of `.autoship/quota.json`. */
export interface QuotaFile {
  opencode: QuotaEntry;
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/** Single check emitted by `opencode-autoship doctor`. */
export interface DoctorCheck {
  /** Check identifier (e.g. `"package-registration"`). */
  name: string;
  /** Outcome. */
  status: CheckStatus;
  /** Human-readable message. */
  message: string;
}

// ---------------------------------------------------------------------------
// CLI / Plugin
// ---------------------------------------------------------------------------

/** Minimal OpenCode plugin server shape returned by {@link server}. */
export interface PluginServer {
  /** Event handler (currently a no-op stub). */
  event(): undefined;
}
