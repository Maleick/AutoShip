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

## DMFT Interpretation Rules

Use this digest to feed anti-cheat gates, not to justify unsupported features.

Rules:

- official Daybreak policy can create or tighten `M7` gates immediately
- community reports may create provisional or research-backed validation tasks
- exploit-style or hack-style travel/control claims stay low-confidence until corroborated
- no secondary source alone can satisfy an anti-cheat milestone exit gate

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

## Evidence State

- official policy anchors: `Research-backed`
- community inferences: `Provisional` unless corroborated
