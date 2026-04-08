# Ability Packet Coverage and Targetability Validation

Curated `M5` validation note for issue `#60`.

This document turns the April 2026 packet import set into an explicit ability-packet validation matrix. It does not treat imported reverse-engineering notes as proof that TextQuest already supports packet-driven ability execution. The current evidence state for this checkpoint set remains `Needs Live Proof`.

## Scope

- map imported ability packet families to concrete validation cases
- separate TextQuest's current in-process execution paths from packet-candidate paths
- document targetability and range assumptions before any packet-first implementation claim

## Source Stack

Primary repo inputs:

- `docs/implementation-roadmap.md`
- `textquest-dll/src/eq/mod.rs`
- `textquest-common/src/ipc.rs`

Curated import summaries:

- `docs/research-imports/2026-04-02-packet-zoning/EQ_Ability_Packet_Structures.md`
- `docs/research-imports/2026-04-02-packet-zoning/EQ_Network_Architecture.md`

Evidence handling notes:

- `EQ_Ability_Packet_Structures.md` is the strongest imported source for opcode families, payload shapes, and candidate target rules in this slice.
- `EQ_Network_Architecture.md` is the strongest imported source for the broader send-path split between high-level ability helpers, `NetworkSend`, and lower-level packet transmission.
- TextQuest's current executable baseline is still in-process: `do_combat_ability` calls `PcZoneClient::DoCombatAbility`, and the shared `CombatForceAbility` IPC enum exists as schema groundwork rather than a wired DLL command path.

## Current Repo Baseline

### Current executable control path

Repo-grounded finding:

- TextQuest already exposes an in-process combat-ability baseline through `textquest_dll::eq::do_combat_ability`, while `textquest_common::ipc::Command::CombatForceAbility` remains declared but not yet handled by the injected DLL dispatcher.
- This path is distinct from a packet-first implementation. It relies on the client function path, not on packet construction or a standalone outbound ability packet injector.

Repo-fit implication:

- `M5` packet work must be described as validation and capability mapping until a packet path is implemented and live-validated.
- Follow-on packet work should compare candidate packet paths against the current in-process baseline instead of implying parity by opcode research alone.

Current evidence state:

- `Live-validated` for the existence of the in-process baseline in repo code
- `Research-backed` for packet-family candidates imported from the April 2026 research set

## Imported Ability Families

### Self-only ability packets

Imported finding:

- The import set groups Feign Death, Hide, Mend, Sense Traps, and similar self-only skills into a shared 12-byte family keyed by player spawn ID plus reserved zeros.
- The network architecture summary ties these skills to high-level helpers such as `Cmd_UseSkill` and the `NetworkSend` path.

Repo-fit implication:

- These are the strongest packet candidates for early validation because they do not require a hostile target and appear to have the simplest payload shape.
- TextQuest still cannot claim packet execution support for these abilities until a current client proves the imported opcodes, payload layout, and any send-counter side effects.

Current evidence state:

- `Research-backed` for family shape and likely send path
- `Needs Live Proof` for opcode correctness and current-build behavior

### Target ability packets

Imported finding:

- Kick, Bash, Backstab, Round Kick, Flying Kick, and Frenzy are grouped into a compact 4-byte target-ability family with target spawn ID plus a skill identifier.
- The import set maps this family to `CharacterZoneClient__UseTargetAbility`.

Repo-fit implication:

- These abilities are packet candidates, but they are not "free target" actions. They depend on a current hostile target ID and likely inherit melee-range, line-of-sight, and facing constraints even when the payload itself is small.
- TextQuest should treat targetability and combat-state prerequisites as part of the validation case, not as assumed packet-only details.

Current evidence state:

- `Research-backed` for packet family and target requirement
- `Needs Live Proof` for exact current-build skill coverage and server acceptance rules

### Attack-style combat skill packets

Imported finding:

- The import set places Flying Kick, Tail Rake, Eagle Strike, Tiger Claw, and related damage skills in a 12-byte attack packet family with target ID, attack type, and skill type.
- The network architecture summary ties this family to `SendAttackPacketToServer`.

Repo-fit implication:

- These are higher-risk packet candidates than self-only skills because the import set explicitly calls out tighter server validation around melee range and position.
- TextQuest should treat these as validation targets with stricter acceptance criteria than the simpler self-only family.

Current evidence state:

- `Research-backed` for packet family and likely send path
- `Needs Live Proof` for current-build acceptance, melee constraints, and packet-vs-position coupling

### Utility and aggro skills

Imported finding:

- The import set identifies Taunt as a simple target-ID packet and lists Intimidation and Begging as likely simpler utility packets, but without the same confidence as the documented self-only or target-ability families.
- The network architecture summary also lists skill-specific opcodes for Intimidation and Begging, but not with the same direct packet-structure detail as the stronger families above.

Repo-fit implication:

- Taunt is a concrete validation candidate.
- Intimidation and Begging remain provisional packet candidates until a current client confirms packet shape, target rules, and send-path behavior.

Current evidence state:

- `Research-backed` for Taunt as a target-ID packet candidate
- `Provisional` for Intimidation and Begging packet structure
- `Needs Live Proof` for all current-build acceptance claims

## Validation Matrix

| Ability family | Example skills | Current TextQuest baseline | Imported packet candidate | Targetability rule to validate | What TextQuest can claim now | Evidence state |
| --- | --- | --- | --- | --- | --- | --- |
| Self-only utility | Feign Death, Hide, Mend, Sense Traps | in-process client helper path | dedicated self-only opcode per skill, 12-byte payload | self only; confirm player spawn ID and no hostile target dependency | packet family is research-backed but not implemented | Research-backed / Needs Live Proof |
| Target ability | Kick, Bash, Backstab, Round Kick, Flying Kick, Frenzy | in-process helper path | compact target-ability packet | hostile target ID required; validate facing, range, and combat-state rules | packet family is a validation target, not a supported feature | Research-backed / Needs Live Proof |
| Attack-style combat skill | Flying Kick, Tiger Claw, Tail Rake, Eagle Strike | in-process helper path | attack packet with target ID plus attack and skill types | hostile target ID plus melee-range and position validation | high-risk candidate; do not implement until live proof exists | Research-backed / Needs Live Proof |
| Aggro utility | Taunt | slash or in-process combat path | target-ID packet | hostile target ID required; validate looser server acceptance vs damage skills | concrete validation candidate with simpler payload | Research-backed / Needs Live Proof |
| Other utility | Intimidation, Begging | slash or in-process path | possible simple packet path | unclear target rules and payload shape | imported hints exist, but structure remains provisional | Provisional |

## Packet vs In-Process Decision Rules

### Keep in-process as the production default when

- TextQuest already has a working client-function path for the ability
- imported packet notes do not yet prove current-build opcode stability
- the ability depends on target, position, or melee-state rules that have not been live-validated

### Allow packet-first exploration only as validation work when

- the ability family has a cited packet structure in the curated import set
- the validation case names the exact targetability and range assumptions being tested
- the result can be labeled `Live-validated`, `Invalidated`, or remain explicitly `Provisional`

### Keep blocked or provisional when

- the import set only hints at an opcode family without a payload shape
- current-build client behavior, send-counter side effects, or server acceptance rules remain unknown
- the candidate path would require exploit-style movement, bypasses, or unsupported anti-detection claims

## Live Validation Tasks

### 1. Self-only family confirmation

Target:

- one current-build self-only skill from the imported family, ideally Feign Death or Mend

Record:

- whether the imported opcode and payload shape still match the live client
- whether the send path uses the expected high-level helper and `NetworkSend`
- whether any counter or sequencing side effect appears in the current client

Pass condition:

- at least one self-only packet family member is confirmed on a current build, or the imported opcode mapping is invalidated

### 2. Target ability rule confirmation

Target:

- one current-build target ability from the 4-byte family, ideally Kick or Bash

Record:

- target requirement
- whether facing, melee range, and combat state affect acceptance
- whether the imported packet family still maps cleanly to the current build

Pass condition:

- the current client confirms or invalidates the imported target-ability family and its practical targetability constraints

### 3. Attack packet coupling check

Target:

- one current-build attack-style skill from the 12-byte combat family, ideally Flying Kick

Record:

- whether the packet family remains valid on the current build
- whether the server rejects the packet without matching melee position or state
- whether the imported "stricter validation than utility skills" claim still holds

Pass condition:

- the current client yields a bounded rule set for attack-style packet candidates or invalidates the imported assumptions

### 4. Utility packet confidence pass

Target:

- Taunt first, then Intimidation or Begging only if their packet shape can be confirmed without speculative inference

Record:

- exact payload shape
- targetability rule
- whether the skill belongs in `Research-backed` or stays `Provisional`

Pass condition:

- Taunt is either confirmed or invalidated as a simple target-ID packet path, and weaker utility candidates are explicitly re-labeled if needed

## Follow-On Slice Candidates

- packet-family inventory for current-build self-only skills with exact live-proof notes
- current-build targetability and range rule matrix for target and attack abilities
- explicit comparison doc for when packet-first control is preferable to the existing in-process baseline, if ever

## Operator and Documentation Guidance

- Do not describe packet ability execution as implemented TextQuest behavior until a branch contains the packet path and a current client live-validates it.
- Keep self-only, target, and attack families separate in docs and future tasks; they do not share the same risk profile.
- When follow-on tasks are promoted into roadmap execution work, carry forward the exact evidence label instead of collapsing imported findings into generic "packet support" language.
