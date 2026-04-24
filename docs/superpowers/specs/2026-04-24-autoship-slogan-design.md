# AutoShip Slogan Messaging Design

## Goal

Create one clear public slogan for AutoShip and apply it consistently across the repository About text, README, banner graphic, GitHub Pages site, and Wiki.

## Primary Message

Primary slogan:

> Turn backlog into reviewed PRs.

Supporting line:

> AutoShip is the OpenCode plugin for solo maintainers who want their GitHub issue queue planned, routed, verified, and packaged into pull requests without babysitting every worker.

## Positioning

- Primary audience: solo maintainers with growing GitHub issue queues.
- Primary value: ship backlog faster by turning ready issues into reviewed pull requests.
- Tone: confident operator, not hype-heavy or playful-first.
- Runtime positioning: keep OpenCode explicit in nearby subtitle/body copy rather than forcing it into the slogan.
- Outcome promise: reviewed PRs ready, not automatic merge as the primary claim.

## Surfaces To Update

- GitHub repository About description: use a concise version of the slogan and OpenCode positioning.
- `README.md`: replace the current hero slogan and opening description with the primary message.
- `assets/autoship-banner.svg`: make the slogan readable in the main graphic and keep OpenCode/GPT-5.5/15-worker details as secondary copy.
- `docs/assets/autoship-banner.svg`: keep the Pages-published graphic in sync with the root asset.
- `docs/index.html`: update page title, meta description, Open Graph text, hero alt text, hero copy, and stale worker/merge claims.
- `wiki/Home.md`: add the slogan above the core behavior list and keep technical details below it.

## Accuracy Fixes Included

- Replace stale “10 default active workers” page copy with `15`.
- Avoid saying AutoShip merges PRs in the main marketing promise unless a specific section explains operator-controlled merge behavior.
- Preserve OpenCode-only messaging and GPT-5.5 reviewer/planner details.

## Acceptance Criteria

- The exact slogan `Turn backlog into reviewed PRs.` appears in the README, Pages hero/meta, banner graphic, and Wiki home.
- Public descriptions consistently say OpenCode plugin or OpenCode-only plugin.
- No public-facing hero copy promises automatic merges as the main outcome.
- Root and Pages banner SVGs match.
- Verification runs before claiming completion.
