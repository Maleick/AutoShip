# Design Decisions

All locked architectural decisions with rationale. These were finalized during the v3 architecture discussion and are reflected in `BEACON_V3_ARCHITECTURE.md`.

---

## Decision 1: Haiku Task Scope

**Choice:** Haiku handles simple tasks only (2-3 files, clear acceptance criteria)

**Rejected:** Single-file only (too narrow), repetitive medium tasks (subtle cross-file mistakes)

**Rationale:** Haiku is capable on well-scoped tasks when given high-quality prompts with clear acceptance criteria — which the dispatch skill already provides. The key insight is that prompt quality matters more than model size for tasks with unambiguous scope. Repetitive medium-complexity work across multiple files risks subtle cross-file mistakes that cost more in debugging time than any savings from a smaller model.

---

## Decision 2: Haiku Failure Escalation

**Choice:** One retry with failure context appended, then promote to Sonnet automatically

**Rejected:** Immediate Sonnet escalation (wastes Haiku's second shot), 2+ Haiku retries (diminishing returns)

**Rationale:** First failures are often prompt clarity issues rather than capability limits — appending the failure reason to a retry prompt usually resolves the issue. If Haiku fails twice on the same task, it indicates the task is beyond the model's capability. At that point, immediate escalation to Sonnet is more efficient than burning a third attempt.

---

## Decision 3: Monitor Architecture

**Choice:** Three separate monitors with tuned poll intervals (5s agents, 30s PRs, 60s issues)

**Rejected:** Single unified watcher, two-monitor split (local + external)

**Rationale:** Different event types require different poll frequencies. Agent completions need 5-second polling for fast verification startup. PR status can use 30-second intervals since CI inherently takes minutes. GitHub issues can use 60-second polling since they're external events. A unified monitor would either waste GitHub API calls by polling everything at 5 seconds or delay agent detection by polling at 60 seconds. Separate monitors allow independent tuning and debugging.

---

## Decision 4: Dispatch Priority

**Choice:** Third-party tools (Codex/Gemini/Grok) first for simple/medium; Claude for complex

**Rejected:** Claude-first (original v2), round-robin weighted by quota

**Rationale:** Third-party tools have separate quota pools — dispatching them first maximizes total work done per session. They lack autoresearch, plugins, and the native verify pipeline, which limits them to prompt-in/result-out operations. This works well for simple and medium tasks. Complex tasks benefit significantly from the full Claude ecosystem, and failed complex tasks on third-party tools would cost more in retries than any savings from cheaper tooling.

---

## Decision 5: Agent Completion Detection

**Choice:** Real-time status words (COMPLETE/BLOCKED/STUCK) via `tmux pipe-pane` log

**Rejected:** `pane_dead` polling, 5-state vocabulary (PARTIAL, ERROR added)

**Rationale:** Polling `pane_dead` introduces up to 5 seconds of latency and can't distinguish crash from completion. Status words emitted to the pane log are detected instantly. Three words cover the full decision space: "I did it" (COMPLETE), "something external stopped me" (BLOCKED), "I can't figure it out" (STUCK). Additional states like PARTIAL or ERROR duplicate the reviewer's responsibilities — the reviewer determines final pass/fail, not the agent.

---

## Decision 6: Third-Party Agent Completion

**Choice:** `pane_dead=1` + `BEACON_RESULT.md` existence check

**Rejected:** Exit codes only, exit codes + pane_dead + file check

**Rationale:** Exit codes from third-party CLIs are unreliable — they may return 0 even on failure. `BEACON_RESULT.md` serves as the agent completion contract: if the file exists when the pane dies, the agent believes it finished successfully. If missing, something went wrong mid-execution. This provides a tool-agnostic, reliable completion signal.

---

## Decision 7: Haiku + Monitor Integration

**Choice:** Bash scripts poll (Monitor tool), Haiku interprets events, Sonnet orchestrates

**Rejected:** Haiku wrapping each monitor, Haiku as unified event router

**Rationale:** Bash excels at fast, reliable polling without consuming tokens. Haiku's strength is interpreting what events mean and deciding next steps. This establishes a layered architecture where each component operates in its optimal domain. Separating raw event watching (bash) from event interpretation (Haiku) from orchestration (Sonnet) prevents token burn on mechanical polling and leverages each model's strengths.

---

## Decision 8: PR Comment Triage

**Choice:** Haiku categorizes all comments (nit/bug/design) → tiered resolution

**Rejected:** Sonnet handles all comments, original author handles own PR comments

**Rationale:** Most automated reviewer comments from Copilot and CodeRabbit are style nits (variable naming, formatting, unused imports) that Haiku can fix in seconds. Routing all comments to Sonnet wastes expensive model time on trivial fixes. Haiku's categorization layer ensures Sonnet focuses on real bugs while design-level concerns escalate to Opus for strategic judgment. Matches the four-tier model.

---

## Decision 9: Event Queue Pattern

**Choice:** Producer-consumer — Haiku queues events, Sonnet pulls after each pipeline step

**Rationale:** Haiku controls event generation rate. Sonnet controls its own processing rate. Pull-based model prevents pipeline overload during event storms (e.g., 5 agents complete simultaneously). Sonnet finishes its current pipeline step before pulling the next event, preventing context flooding.

---

## Decision 10: Opus Trigger Strategy

**Choice:** Hybrid — hardcoded triggers at known critical moments + Sonnet-initiated escalation

**Rejected:** Pure Sonnet-decided (too unpredictable), hardcoded-only (too rigid)

**Rationale:** Hardcoded triggers guarantee Opus consultation at known critical points (UltraPlan, phase checkpoints, repeated failures, LOW_CONFIDENCE verdicts). This provides cost predictability and guaranteed oversight. Sonnet retains escalation ability for unpredictable situations inherent in agentic work. The hybrid approach balances cost control with adaptive decision-making.

---

## Decision 11: CI Autofix Loop

**Choice:** Tiered by error type — Haiku (lint/format/type), Sonnet (test/build), Opus (2+ fails)

**Rationale:** Mechanical CI failures (lint, format, type errors) require no logical reasoning and Haiku fixes them in seconds. Test failures and build errors require understanding code behavior — Sonnet's domain. If CI continues to fail after 2+ attempts, the problem is likely architectural — Opus decides whether to re-approach or block the PR. Closed-loop remediation without manual intervention.

---

## Original v2 Decisions (still valid)

| #     | Decision         | Choice                                                      |
| ----- | ---------------- | ----------------------------------------------------------- |
| v2-3  | Result capture   | BEACON_RESULT.md per worktree + git diff                    |
| v2-4  | State management | .beacon/state.json (local) + GitHub labels (durable)        |
| v2-5  | Verification     | Dedicated Sonnet reviewer agent                             |
| v2-7  | Post-completion  | verify → simplify → verify → PR → monitor CI → cleanup      |
| v2-8  | Autoresearch     | Automatic for Claude Sonnet/complex agents only             |
| v2-9  | Merge gates      | CI green (simple), CI + review (medium/complex)             |
| v2-10 | Repo scope       | Single repo per session                                     |
| v2-13 | Plugin format    | Multi-skill Claude Code plugin with marketplace auto-update |
| v2-14 | Tmux layout      | Grid (tiled) not stacked                                    |
