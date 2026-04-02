# External Automation Research Source Ledger

This ledger tracks external automation sources that can feed milestone slices and validation tasks. It is not the roadmap itself. Promotion into the roadmap requires repo-fit analysis, evidence state assignment, and at least one concrete slice or validation task.

## Promotion Rule

An external finding may enter the roadmap queue only when it includes:

1. at least one source citation
2. explicit DMFT repo-fit rationale
3. a concrete slice or validation task
4. an evidence state

## Source Tiers

### Primary

| Source | Focus | Current use |
| --- | --- | --- |
| [RedGuides Docs](https://www.redguides.com/docs/) | operator and plugin documentation | baseline plugin and workflow comparison |
| [RedGuides Plugins](https://www.redguides.com/docs/plugins/) | plugin catalog and capability discovery | plugin-family coverage and missing slices |
| [KissAssist](https://www.redguides.com/docs/projects/kissassist/) | combat automation, pull, heal, buff, assist workflows | first deep-dive source for TUI translation |
| [MacroQuest Docs](https://docs.macroquest.org/main/) | upstream behavior, TLOs, command/runtime model | upstream reference and scripting context |
| [MacroQuest Lua Docs](https://docs.macroquest.org/lua/) | scripting, binds, events, command routing | future runtime and operator extension slices |
| [JMB Basic Core](https://github.com/LavishSoftware/JMB-Basic-Core) | multibox control model | second-pass orchestration comparison |
| [JMB WinEQ 2022](https://github.com/LavishSoftware/JMB-WinEQ-2022) | windowing and control shell | orchestration and operator workflow comparison |
| [JMB Input Hook Example](https://github.com/LavishSoftware/JMB-Input-Hook-Example) | hook and input model example | bounded comparison input |
| [Daybreak Account Security Policy](https://help.daybreakgames.com/hc/en-us/articles/217581258-Account-Security-Policy) | official policy anchor | anti-cheat digest baseline |
| [What is considered cheating?](https://help.daybreakgames.com/hc/en-us/articles/115015664167-What-is-considered-cheating) | official detection/policy language | anti-cheat gates and risk labeling |
| [How do I fill out a ban appeal?](https://help.daybreakgames.com/hc/en-us/articles/217579038-How-do-I-fill-out-a-ban-appeal) | official ban and cross-game cheat signal | anti-cheat digest |

### Secondary

| Source | Focus | Current use |
| --- | --- | --- |
| [Joe Multiboxer](https://joemultiboxer.com/) | operator workflow and session control | orchestration comparison |
| [MMOBugs Forums](https://www.mmobugs.com/forums/index.php) | plugin behavior and community reporting | secondary capability/risk input |
| [EverQuestBot](https://everquestbot.com/home/) | broader automation workflow ideas | secondary comparison |
| [Bonzz How-To](https://www.bonzz.com/howto.htm) | gameplay and operator reference | secondary gameplay context |
| [EQ Might AA Codes](https://www.eqmight.com/setup/alternate-ability-codes) | AA code lookup | niche supporting reference |

### Low-Confidence / Risk-Oriented Inputs

| Source family | Handling rule |
| --- | --- |
| Tom's Hardware exploit thread | may suggest risk notes or edge-case comparisons, but never satisfy milestone gates alone |
| PiggyZone-style hack discussions | may generate anti-pattern notes or low-confidence slice ideas; require corroboration and explicit risk labeling |
| isolated forum claims without code or docs backing | keep provisional until corroborated |

## Current Deep-Dive Order

1. KissAssist capability audit and TUI translation
2. Joe Multiboxer and JMB orchestration pass
3. MacroQuest Lua and broader MacroQuest runtime pass
4. ongoing Daybreak detection digest updates

## Source Weighting

### Primary sources

May directly create `Research-backed` slice candidates when the repo fit is clear.

### Secondary sources

May suggest slice candidates or refine an existing slice, but they should not be the only support for milestone gates.

### Low-confidence sources

May only create provisional candidates, anti-pattern notes, or validation tasks until stronger evidence exists.
