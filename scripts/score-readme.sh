#!/usr/bin/env bash
# score-readme.sh — README quality rubric for AutoShip
# Outputs a single integer 0-100 to stdout.
# Each check awards points if the pattern is found in README.md.

README="${1:-README.md}"
score=0

check() {
  local pts="$1"
  local pattern="$2"
  if grep -qE "$pattern" "$README" 2>/dev/null; then
    score=$((score + pts))
  fi
}

check_multiline() {
  local pts="$1"
  local pattern="$2"
  if grep -cE "$pattern" "$README" 2>/dev/null | grep -qv "^0$"; then
    score=$((score + pts))
  fi
}

# ── Hero section (20 pts) ────────────────────────────────────────────────────
check 5  'align="center"'                             # centered layout
check 4  '<h1 align="center">'                        # centered h1 title
check 5  '<strong>'                                   # tagline in strong
check 3  'img src=.*emoji|img src=.*png.*width'       # hero image/emoji
check 3  '</a> •'                                      # nav anchor bar bullets

# ── Badges (12 pts) ─────────────────────────────────────────────────────────
check 4  'shields\.io/github/stars'                   # stars badge
check 4  'shields\.io/github/last-commit'             # last-commit badge
check 4  'shields\.io/github/(license|v/release)'    # license or version badge

# ── Before / After (14 pts) ─────────────────────────────────────────────────
check 5  'Before.*After|Without.*With'                # before/after heading
check 5  '<table>'                                    # HTML comparison table
check 4  '<td width='                                 # two-column layout

# ── ASCII stats block (6 pts) ───────────────────────────────────────────────
check 3  '┌─'                                         # box drawing top
check 3  '└─'                                         # box drawing bottom

# ── Install section (14 pts) ────────────────────────────────────────────────
check 5  'claude plugin marketplace add'              # one-liner install
check 4  'claude plugin install'                      # install step 2
check 3  '## Install'                                 # install heading
check 2  '## Requirements|### Requirements'           # requirements listed

# ── Commands table (8 pts) ──────────────────────────────────────────────────
check 4  '/autoship:start'                              # start command documented
check 2  '/autoship:plan'                               # plan command documented
check 2  '/autoship:stop'                               # stop command documented

# ── How It Works / Architecture (16 pts) ────────────────────────────────────
check 4  '## How It Works|## How it Works'            # how it works section
check 4  '## Architecture'                            # architecture section
check 4  '──►|→|▼|├─|└─'                             # diagram characters
check 4  '\| Tier \|| Executor | Monitors'            # architecture table

# ── Plugin structure (6 pts) ────────────────────────────────────────────────
check 3  'plugin\.json|marketplace\.json'             # plugin files mentioned
check 3  'SKILL\.md|skills/'                          # skills directory mentioned

# ── Footer (4 pts) ──────────────────────────────────────────────────────────
check 2  'Star This Repo|leave a star|⭐'             # star CTA
check 2  '## License|^MIT'                            # license section

echo "$score"
