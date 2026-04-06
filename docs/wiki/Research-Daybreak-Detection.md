# Daybreak Detection Digest

This digest feeds `M7` Anti-Cheat. It separates official Daybreak signals from secondary community reporting.

## Official Anchors

### [Account Security Policy](https://help.daybreakgames.com/hc/en-us/articles/217581258-Account-Security-Policy)

Use as the policy anchor for account risk, enforcement framing, and appeal expectations.

### [What is considered cheating?](https://help.daybreakgames.com/hc/en-us/articles/115015664167-What-is-considered-cheating)

Official Daybreak guidance currently identifies these cheat-risk categories:

- use of third-party software or cheat programs to gain an unfair advantage
- modification of the computer or OS environment to falsify or circumvent anti-cheat detection
- exploitation of bugs or game mechanics

### [How do I fill out a ban appeal?](https://help.daybreakgames.com/hc/en-us/articles/217579038-How-do-I-fill-out-a-ban-appeal)

Current support guidance also states that a cheat program detected in a different game can still produce a ban across Daybreak games. That matters for operator hygiene and for keeping the runner and live machines clean.

## Secondary Community Signals

Secondary sources may refine risk posture, but they do not override official guidance.

Current recurring secondary signals:

- player reports remain a real detection vector for visible or disruptive automation
- progression or lower-population environments increase exposure because the same actors see each other repeatedly
- active hack categories such as warping or exploit-grade zoning are repeatedly described as high-risk and report-prone
- community plugin ecosystems often optimize for capability first, not for bounded operator safety or milestone-proof quality

Secondary source families in use:

- RedGuides community reporting
- MMOBugs discussions
- broader EQ automation communities

## TextQuest Interpretation Rules

Use this digest to feed anti-cheat gates, not to justify unsupported features.

Rules:

- official Daybreak policy can create or tighten `M7` gates immediately
- community reports may create provisional or research-backed validation tasks
- exploit-style or hack-style travel/control claims stay low-confidence until corroborated
- no secondary source alone can satisfy an anti-cheat milestone exit gate

## Exposure Categories For `M7`

Use these categories when turning the digest into roadmap tasks or operator checklists:

- `Module presence` — the injected DLL and any additional loaded module surface
- `Detour hooks` — game-loop, render, or future hook installation inside the client
- `In-process control paths` — `InterpretCmd`, UI widget clicks, login helpers, or similar internal calls
- `IPC and token handling` — named pipes, shared memory, session tokens, and local artifact retention
- `Timing and behavior visibility` — movement pacing, command jitter, and other observable automation patterns
- `Operator environment hygiene` — clean machines, runner boundaries, and avoiding unrelated cheat tooling

Treat the first four categories as directly repo-grounded when the code clearly implements them. Treat timing value and environment safety as bounded operational guidance, not proof of stealth.

## Current Milestone Inputs

### `M5` Packet Engine

- do not treat packet capability discovery as permission to use the riskiest path by default
- label any exploit-grade packet route or movement shortcut as high risk until proven otherwise

### `M6` Zoning/Movement

- maintain explicit validation tasks around zoning and movement visibility
- keep risky travel hacks separated from normal zoning workflows

### `M7` Anti-Cheat

- build gates around official policy categories
- separate module, hook, timing, and environment risks
- document operator hygiene tasks, including clean machine expectations
- require new `M7` tasks to name which exposure categories they touch and whether the confidence is high, medium, or low

## `M5` Through `M8` Validation Gate Matrix

Use this matrix when a task touches a risky control path. The point is to make the gate explicit before implementation or live claims.

| Milestone | Risky path or change type | Required gate | Default label or evidence handling |
| --- | --- | --- | --- |
| `M5` Packet Engine | new packet send path, packet fallback, or targetability claim | document the packet path, state why the existing in-process route is not sufficient, and open a validation task before claiming support | mark exploit-adjacent or unclear routes as `Provisional` or `Needs Live Proof`; keep high-risk travel shortcuts blocked by default |
| `M6` Zoning/Movement | zone transition automation, movement queue flushing, safe-coord recovery, or teleport-style routing | name the transition or recovery checkpoint, record the failure state, and define a live-proof step before promoting the path as normal operator workflow | keep risky movement claims labeled `Needs Live Proof`; separate exploit-style travel from normal zoning support |
| `M7` Anti-Cheat | new hook, wider module footprint, string or artifact exposure change, or new timing hardening claim | map the change to one or more exposure categories, cite whether the support comes from repo evidence or official policy, and record the confidence level | official-policy-backed items can be `Research-backed`; community-only claims stay `Provisional` |
| `M8` Orchestrator | broader broadcast scope, relay expansion, launch/session orchestration change, or more visible automation behavior | state the lowest-exposure control path, preserve operator-visible scope boundaries, and confirm the change does not silently widen packet, hook, or movement risk | if the change depends on an unresolved `M5`-`M7` risk, keep the item blocked or explicitly cross-link the validation task |

## Required Labels For New `M7` Tasks

Every new anti-cheat task or validation item should name:

- the exposure categories it touches
- the confidence level for each claim (`High`, `Medium`, or `Low`)
- whether the support comes from official policy, repo-grounded evidence, or community reporting
- whether the outcome is a hard gate, operator checklist item, or follow-on validation task

This keeps anti-cheat work reviewable and prevents broad safety claims from entering roadmap execution without evidence.

## Evidence State

- official policy anchors: `Research-backed`
- community inferences: `Provisional` unless corroborated
