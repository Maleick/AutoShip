#!/usr/bin/env bash
# Kira-Kara Discord Bridge — Cross-agent communication coordinator
set -euo pipefail

# Shared coordination channel
COORDINATION_CHANNEL="discord:1500227697325117541"  # #vanguard home
KARA_THREAD="discord:1500370550412017836"  # Current thread (Kara will join)

# Message types
MSG_TASK="🎯 TASK"
MSG_STATUS="📊 STATUS"
MSG_COMPLETE="✅ COMPLETE"
MSG_BLOCKER="🚫 BLOCKER"
MSG_HANDOFF="🤝 HANDOFF"

log() {
  echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

# Send task to Kara
send_task_to_kara() {
  local issue="$1"
  local title="$2"
  local branch="$3"
  
  log "Dispatching to Kara: #$issue — $title"
  
  # Send via Discord
  send_message \
    --target "$KARA_THREAD" \
    --message "${MSG_TASK} Kara — Issue #$issue

**Title:** $title
**Branch:** $branch
**Worktree:** /Users/maleick/Projects/TextQuest.worktrees/issue-$issue
**Model:** opencode-zen/big-pickle (free) or gpt-5.5 (complex)
**Timeout:** 10 minutes

**Instructions:**
1. cd into worktree
2. cargo check —all
3. Implement acceptance criteria
4. git push origin $branch
5. gh pr create —title \"feat: $title\" —body \"Closes #$issue\"
6. Report back with PR link

**Evidence required:**
- Screenshot of tests passing
- PR link
- Any blockers"
}

# Receive Kara's completion report
receive_kara_report() {
  local issue="$1"
  
  log "Waiting for Kara report on #$issue..."
  
  # Check GitHub for PR
  local pr=$(gh pr list --head "autoship/issue-$issue" --json number --jq '.[0].number' 2>/dev/null || echo "")
  
  if [[ -n "$pr" ]]; then
    log "✅ Kara completed #$issue — PR #$pr"
    
    # Update issue
    gh issue edit "$issue" --remove-label atomic:live --add-label atomic:complete
    gh issue comment "$issue" --body "Completed by Kara on Frostreaver — PR #$pr"
    
    # Cleanup
    bash /Users/maleick/Projects/AutoShip/hooks/hermes/post-merge-cleanup.sh "$issue"
    
    return 0
  fi
  
  log "⏳ Kara still working on #$issue"
  return 1
}

# Handoff protocol
handoff_to_kara() {
  log "=== KIRA → KARA HANDOFF ==="
  
  # Get all atomic:live issues
  local issues=$(gh issue list --label atomic:live --state open --json number,title --jq '.[] | "\(.number):\(.title)"')
  
  local count=0
  for line in $issues; do
    local num=$(echo "$line" | cut -d: -f1)
    local title=$(echo "$line" | cut -d: -f2-)
    local branch="autoship/issue-$num"
    
    # Ensure worktree exists
    local wt="/Users/maleick/Projects/TextQuest.worktrees/issue-$num"
    if [[ ! -d "$wt" ]]; then
      cd /Users/maleick/Projects/TextQuest
      git branch "$branch" origin/master 2>/dev/null || true
      git worktree add "$wt" "$branch" 2>/dev/null || true
    fi
    
    send_task_to_kara "$num" "$title" "$branch"
    ((count++)) || true
  done
  
  log "Handed off $count issues to Kara"
}

# Status sync
sync_status() {
  local kira_ready=$(gh issue list --label atomic:ready --state open --json number --jq 'length')
  local kira_live=$(gh issue list --label atomic:live --state open --json number --jq 'length')
  local kara_pending=$(gh issue list --label atomic:live --state open --json number --jq 'length')
  
  log "=== STATUS SYNC ==="
  log "Kira (macOS): $kira_ready ready, $kira_live live"
  log "Kara (Windows): $kara_pending pending"
  
  # Send to Discord
  send_message \
    --target "$COORDINATION_CHANNEL" \
    --message "${MSG_STATUS} Vanguard Status

🐱 **Kira** (macOS): $kira_ready ready | $kira_live live
🪟 **Kara** (Windows): $kara_pending pending

**Next Actions:**
- Kira: Burn down atomic:ready queue
- Kara: Validate atomic:live on Frostreaver"
}

# Main
main() {
  case "${1:-status}" in
    handoff)
      handoff_to_kara
      ;;
    sync)
      sync_status
      ;;
    receive)
      receive_kara_report "$2"
      ;;
    *)
      sync_status
      ;;
  esac
}

main "$@"
