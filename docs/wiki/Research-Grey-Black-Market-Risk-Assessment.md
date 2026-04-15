# Grey/Black Market Risk Assessment

This page evaluates whether off-platform EverQuest monetization should enter `M10` planning.

It is intentionally risk-first. It does not document evasion, listing, or payment workflows.

## Current Decision

- Default posture: do not commit capital, inventory, or paid accounts to off-platform RMT.
- Use only Daybreak-approved monetization surfaces. In current EverQuest policy, that means official purchases from Daybreak, including Krono bought from Daybreak and then traded only for in-game value.
- Treat any proposal that depends on off-platform item sales, account sales, hired play, unattended farming, or anti-cheat bypass assumptions as blocked.

## Source Priority

Use these sources in this order:

1. current official Daybreak and EverQuest policy
2. current repo-grounded anti-cheat research
3. current official TLP ruleset announcements
4. clearly labeled payment or marketplace policy docs

## Executive Summary

| Dimension | Current finding | Confidence | Why it matters |
| --- | --- | --- | --- |
| Legal and policy risk | Current Daybreak policy explicitly prohibits account sales, account sharing, hired play, and selling in-game items for real money. | High | This is enough to block normal `M10` execution even before detection details are considered. |
| Detection and enforcement risk | Official policy allows monitoring for unauthorized third-party programs, player reports remain an explicit enforcement input, and current repo research shows meaningful client and machine-level detection surface. | High | A farm should be modeled as correlated account risk, not as independent accounts. |
| Financial risk | Account loss, item loss, linked-account bans, and chargeback exposure can erase inventory value quickly. We do not have reliable EQ-specific reversal-rate data. | High | A positive spreadsheet model that assumes smooth liquidation is not evidence-backed. |
| Operational risk | Current 2026 TLP rules can improve liquidity, but they also widen exposure and increase the number of variables that can invalidate a pricing model. | Medium | Randomized loot and free trade improve tradability, but they do not make the activity compliant or stable. |

Current repo-level conclusion: keep the issue in research mode and preserve the no-capital default.

## Legal And Policy Boundaries

### What is clearly prohibited by current Daybreak policy

Current Daybreak policy is unambiguous on the following points:

- Daybreak's Conduct Policy says: do not sell, transfer, lease, or share any account; do not buy or sell any in-game item or other Daybreak intellectual property for real world money; and do not use another Daybreak account to evade enforcement.
- The EverQuest help article on Krono says Krono may be bought from Daybreak for real money and traded in-game for in-game currency or items, but selling Krono to each other for real world money is a Terms of Service violation.
- The EULA limits the game to personal, non-commercial use and prohibits bots, unattended play, and unauthorized exchanges for accounts, characters, Paid Content, or Daybreak Game Assets.
- EverQuest Rules of Conduct also prohibit unattended gameplay and official-forum advertising of EverQuest characters or items for real-world money.

### What is clearly real-world illegal

This repo does not provide legal advice. However, the following categories are plainly outside any "grey area" framing:

- payment fraud and chargeback abuse
- stolen account or stolen card use
- unauthorized access to another person's account
- false identity or account-recovery fraud

Current Daybreak policy explicitly frames fraud, deceptive conduct, and illegal activity as bannable.

### What remains a legal grey area for this repo

The repo does not currently contain counsel-backed research that would let us classify a manual item-for-cash swap as lawful across relevant jurisdictions.

The evidence we do have is enough to say this:

- Daybreak treats the activity as prohibited even when no further fraud is proven.
- The practical risk profile quickly spills into fraud and chargeback territory once accounts, stored payment methods, or shared credentials are involved.

For planning purposes, the correct operating assumption is not "maybe legal enough." The correct assumption is "policy-prohibited and financially fragile unless proven otherwise."

## Detection And Enforcement Risk

### Official Daybreak signals

Official Daybreak sources currently state that:

- using third-party software or cheat programs, modifying the OS or machine environment to circumvent anti-cheat, and exploiting bugs or mechanics are cheating categories
- Daybreak may monitor the system for unauthorized third-party programs running concurrently with the game and may send details about the program and its behavior back to Daybreak
- botting and cheating reports are supposed to be filed with a support ticket, server and zone details, date and time, and ideally video evidence
- consequences can include warning, suspension, permanent banishment, termination of all accounts accessed by the same user, and permanent blocks on repeat offenders creating or accessing new accounts
- account suspensions and bans now explicitly include "farming resources to sell illegitimately, creating an imbalance in the game economy"
- ban appeals will not reveal the evidence, and Daybreak states that a cheat program detected from a different game can still trigger a ban across all Daybreak games

### Repo-grounded detection posture

The repo's current anti-cheat research already points to meaningful client and machine surface:

- [Daybreak Detection](Research-Daybreak-Detection) records official policy anchors and the current interpretation rules for using community reporting.
- [EQ AntiCheat Notes](Research-EQ-AntiCheat-Notes) documents in-client hook validation and report packets from prior reversing work.
- [Anti-Detection](Research-Anti-Detection) and [Security and Anti-Detection Notes](Security-and-Anti-Detection-Notes) treat module presence, detour hooks, in-process calls, IPC naming, timing variation, and operator environment as separate exposure categories.

Current repo interpretation:

- any same-host farm should be treated as a correlated-risk cluster
- any model that assumes Daybreak sees only one account at a time is weaker than current repo evidence
- exact internal scan coverage is still not an official public fact and should not be documented as one

## Activity Risk Matrix

| Activity type | Policy basis | Detection and ban-cascade risk | Evidence state |
| --- | --- | --- | --- |
| Manual multibox on an unrestricted-client TLP, no RMT, no automation | Allowed only to the extent the server rules permit the boxing pattern and no unattended play occurs. | Medium: still visible to reports and linked-account enforcement if other violations occur. | Research-backed |
| Manual multibox on a True Box server or any one-key-to-many setup | Official True Box guidance treats multi-character single-computer play, third-party software, and one-key broadcasting as actionable. | High: server-rule violations can escalate across associated accounts. | Research-backed |
| Unattended farming, even without selling anything | EverQuest unattended gameplay is explicitly prohibited. | High: official policy says disciplinary action can include warning, suspension, and termination. | Research-backed |
| Third-party automation, injected hooks, or anti-cheat circumvention | Official cheating policy and EULA prohibit this directly. | Very high: enforcement includes ban, linked-account action, and no evidence disclosure. | Research-backed |
| Off-platform sale of items or Krono while otherwise playing manually | Current Conduct Policy and Krono guidance prohibit real-money item sales. | High: direct policy violation plus report and payment-dispute exposure. | Research-backed |
| Account sale, account transfer, account sharing, or hired pilots | Conduct Policy and Account Security Policy prohibit this directly. | Critical: linked-account bans, compromised-account investigations, and chargeback issues are all explicitly documented. | Research-backed |

## Financial Risk

### Why the downside is asymmetric

Current Daybreak billing and security policy makes the downside stack worse than a normal marketplace business:

- virtual items and Paid Content are non-refundable at the Daybreak layer
- sold, shared, transferred, or traded accounts are explicitly called out by Daybreak as a chargeback trigger
- Daybreak says these accounts can and will not be released
- linked accounts may also be banned or suspended while a dispute is unresolved

That means off-platform revenue is exposed to at least three independent failure modes:

1. Daybreak enforcement removes the account, items, or both.
2. The buyer or prior account owner disputes the payment.
3. Linked accounts are pulled into the same investigation.

### What we do not know

We do not have a trustworthy public EverQuest-specific number for:

- chargeback rate
- friendly-fraud rate
- payout hold rate
- account-recovery reversal rate after a disputed sale

Because that denominator is missing, any "exit strategy" model that assumes smooth liquidation should be treated as speculative, not as evidence.

### Bounded payment-rail inference

Current public payment guidance is enough to set a conservative bound even without EQ-specific numbers:

- Stripe's dispute guidance warns that excessive dispute activity can put an account into a chargeback monitoring program and that small payment volumes are especially vulnerable to one or two disputes distorting the rate.
- Daybreak's own chargeback article confirms that disputed payments create merchant fees, account bans or suspensions, and linked-account exposure.

Inference: if a grey-market model only works when the dispute rate stays unrealistically low, the model is not robust enough for repo planning.

## TLP Ruleset Effects On Supply And Pricing

### Current official ruleset signals

Official EverQuest announcements show that the 2026 Frostreaver TLP chose:

- unrestricted clients per computer
- free trade
- randomized loot

The 2025 Fangbreaker ruleset FAQ also documents that official TLP variants may randomize daily zone bonuses that increase loot, coin, rare NPC spawns, or respawn speed.

### Current inference for pricing

This is an inference from official rulesets, not a direct Daybreak pricing statement.

Current best read:

- free trade increases liquidity by allowing more items to move between players
- randomized loot increases substitute supply for many ordinary farm targets because the same level-and-era loot pool can come from more than one named or raid target
- random zone bonuses make short-run supply more volatile because loot and rare-spawn incentives can change daily

Likely pricing effect:

- commodity farm margins should compress faster than on a fixed-drop server
- chase-item spikes can still happen, but price discovery gets noisier
- bottlenecks outside the randomized or freely tradable pool can keep premium pricing longer

The official Fangbreaker FAQ is important here because it also says old quests are not adjusted and keying or flagging still remains required. That means some non-randomized bottlenecks can persist even when tradeable drop supply broadens.

## Blocking Questions And Current Answers

| Question | Current answer | Evidence state |
| --- | --- | --- |
| What percentage of TLP multibox farms use RMT? | No trustworthy public denominator found. Community anecdotes exist, but they are not strong enough for planning. | Low |
| What is the actual ban rate for grey-market activity? | No reliable public base rate found. Daybreak publishes causes and consequences, not a ban-rate dashboard. | Low |
| How does randomized loot affect RMT pricing? | Medium-confidence inference: it broadens substitute supply and compresses commodity pricing, while some bottlenecks remain premium because they sit outside the random pool or behind quest gating. | Medium |

## Non-Transactional Validation Protocol

This protocol is intentionally bounded. It is for observation only and does not authorize off-platform selling.

1. Capture the official target-server ruleset.
   Record whether the current TLP is True Box or unrestricted, whether free trade is enabled, whether randomized loot is enabled, and whether bonus systems like Resource Hunter are active.
2. Observe only public in-game liquidity.
   Track Krono prices, bazaar spreads, and auction-chat frequency inside the game. Do not open off-platform listings or payment accounts from this repo workflow.
3. Keep enforcement evidence separate from community rumor.
   Log official policy changes, public report vectors, and repo-grounded anti-cheat findings separately. Do not promote forum rumor to "fact."
4. Model opportunity cost before any capital thesis.
   If a proposed farm only looks attractive after assuming off-platform RMT, classify the model as blocked instead of quietly carrying that assumption forward.
5. Require a separate issue before any higher-risk proposal.
   Any future proposal that wants to move beyond observation must first supply stronger legal review, dispute-handling assumptions, and a linked-account loss model. This issue does not provide that approval.

## Current Recommendation For `M10`

Keep `M10` economy work inside operator-visible, Daybreak-compliant loops:

- loot intake
- distribution ownership and reserve rules
- vendor and banking workflows
- plat and item ledgers
- pause, skip, abort, and resume controls

Do not treat off-platform monetization as a hidden dependency for `M10` viability.

## References

Official Daybreak and EverQuest sources:

- [Daybreak Conduct Policy](https://www.daybreakgames.com/conduct-policy)
- [Daybreak Terms of Service](https://www.daybreakgames.com/terms-of-service?locale=en)
- [Daybreak EULA](https://www.daybreakgames.com/eula?locale=en_US)
- [Account Security Policy](https://help.daybreakgames.com/hc/en-us/articles/217581258-Account-Security-Policy)
- [What is a chargeback?](https://help.daybreakgames.com/hc/en-us/articles/217994497-What-is-a-chargeback)
- [What is considered cheating?](https://help.daybreakgames.com/hc/en-us/articles/115015664167-What-is-considered-cheating)
- [Why is my account suspended?](https://help.daybreakgames.com/hc/en-us/articles/217997417-Why-is-my-account-suspended)
- [Why is my account banned?](https://help.daybreakgames.com/hc/en-us/articles/217509308-Why-is-my-account-banned)
- [How do I fill out a ban appeal?](https://help.daybreakgames.com/hc/en-us/articles/217579038-How-do-I-fill-out-a-ban-appeal)
- [How do I report cheaters/botters in EverQuest?](https://help.daybreakgames.com/hc/en-us/articles/360052236954-How-do-I-report-cheaters-botters-in-EverQuest)
- [What is the policy on AFK or Unattended Gameplay in EverQuest?](https://help.daybreakgames.com/hc/en-us/articles/231115088-What-is-the-policy-on-AFK-or-Unattended-Gameplay-in-EverQuest)
- [What are the EverQuest Rules of Conduct?](https://help.daybreakgames.com/hc/en-us/articles/230629007-What-are-the-EverQuest-Rules-of-Conduct)
- [What things are not allowed on a True Box Progression server in EverQuest?](https://help.daybreakgames.com/hc/en-us/articles/230630767-What-things-are-not-allowed-on-a-True-Box-Progression-server-in-EverQuest)
- [How does Krono work in EverQuest?](https://help.daybreakgames.com/hc/en-us/articles/231116448-How-does-Krono-work-in-EverQuest)
- [Votes are in! Frostreaver is taking shape.](https://www.everquest.com/news/eq-2026-tlp-polls-outcome)
- [Fangbreaker Rulesets FAQ](https://www.everquest.com/guides/eq-2025-tlp-ruleset-faq)

Repo-grounded context:

- [Daybreak Detection](Research-Daybreak-Detection)
- [EQ AntiCheat Notes](Research-EQ-AntiCheat-Notes)
- [Anti-Detection](Research-Anti-Detection)
- [Security and Anti-Detection Notes](Security-and-Anti-Detection-Notes)

General payment-risk context:

- [Stripe dispute prevention guidance](https://docs.stripe.com/disputes/prevention/best-practices)
- [Stripe dispute monitoring guidance](https://docs.stripe.com/disputes/monitoring-programs)
