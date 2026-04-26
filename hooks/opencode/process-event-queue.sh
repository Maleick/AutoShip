#!/usr/bin/env bash
set -euo pipefail

AUTOSHIP_DIR=".autoship"
WORKSPACES_DIR="$AUTOSHIP_DIR/workspaces"
EVENT_QUEUE="$AUTOSHIP_DIR/event-queue.json"
PROCESSED_EVENTS="$AUTOSHIP_DIR/processed-events.json"
LOCK_FILE="$AUTOSHIP_DIR/event-queue.lock"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
cd "$REPO_ROOT"

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq is required but not found" >&2
  exit 1
fi

if [[ -z "${AUTOSHIP_QUEUE_LOCKED:-}" ]]; then
  export AUTOSHIP_QUEUE_LOCKED=1
  mkdir -p "$AUTOSHIP_DIR"
  touch "$LOCK_FILE"
  if [[ "$(uname -s 2>/dev/null || true)" == "Darwin" ]] && command -v lockf >/dev/null 2>&1; then
    exec lockf -k "$LOCK_FILE" "$0" "$@"
  elif command -v flock >/dev/null 2>&1; then
    exec 9>"$LOCK_FILE"
    flock -x 9
  elif command -v lockf >/dev/null 2>&1; then
    exec lockf -k "$LOCK_FILE" "$0" "$@"
  fi
fi

make_tmp() { mktemp "$AUTOSHIP_DIR/event-queue.tmp.XXXXXX"; }

ensure_array_file() {
  local file="$1"
  local tmp
  if [[ ! -f "$file" ]] || ! jq -e 'type == "array"' "$file" >/dev/null 2>&1; then
    tmp=$(make_tmp)
    printf '[]\n' > "$tmp"
    mv "$tmp" "$file"
  fi
}

event_key() {
  jq -c '[
    (.type // ""),
    (.issue // ""),
    (.data.status // .status // ""),
    (.pr_number // .data.pr_number // ""),
    (.action // "")
  ]' <<< "$1"
}

is_processed() {
  local key="$1"
  jq -e --arg key "$key" 'index($key) != null' "$PROCESSED_EVENTS" >/dev/null
}

mark_processed() {
  local key="$1"
  local tmp
  tmp=$(make_tmp)
  jq --arg key "$key" 'if index($key) then . else . + [$key] end' \
    "$PROCESSED_EVENTS" > "$tmp" && mv "$tmp" "$PROCESSED_EVENTS"
}

current_state() {
  local issue="$1"
  jq -r --arg issue "$issue" '.issues[$issue].state // empty' "$AUTOSHIP_DIR/state.json" 2>/dev/null || true
}

issue_number() {
  printf '%s' "${1#issue-}"
}

issue_title() {
  local issue="$1"
  jq -r --arg issue "$issue" '.issues[$issue].title // empty' "$AUTOSHIP_DIR/state.json" 2>/dev/null || true
}

issue_labels() {
  local issue="$1"
  jq -r --arg issue "$issue" '.issues[$issue].labels // empty' "$AUTOSHIP_DIR/state.json" 2>/dev/null || true
}

discover_test_command() {
  if [[ -f "$AUTOSHIP_DIR/config.json" ]]; then
    local configured
    configured=$(jq -r '.test_command // empty' "$AUTOSHIP_DIR/config.json" 2>/dev/null || true)
    [[ -n "$configured" ]] && { printf '%s\n' "$configured"; return 0; }
  fi

  if [[ -f package.json ]]; then
    printf 'npm test\n'
  elif [[ -f Makefile ]]; then
    printf 'make test\n'
  elif [[ -f Cargo.toml ]]; then
    printf 'cargo test\n'
  elif [[ -f pyproject.toml ]]; then
    printf 'pytest\n'
  elif [[ -f go.mod ]]; then
    printf 'go test ./...\n'
  else
    printf 'none\n'
  fi
}

run_verification() {
  local issue="$1"
  local workspace="$WORKSPACES_DIR/$issue"
  local test_command

  [[ -d "$workspace" ]] || return 2

  test_command=$(discover_test_command)
  if bash "$SCRIPT_DIR/verify-result.sh" "$issue" "$workspace" "$test_command" >/dev/null 2>&1; then
    return 0
  fi

  return 1
}

generate_pr_body() {
  local issue="$1"
  local workspace="$WORKSPACES_DIR/$issue"
  local result_path="$workspace/AUTOSHIP_RESULT.md"
  local body_path="$workspace/AUTOSHIP_PR_BODY.md"
  local number
  number=$(issue_number "$issue")

  {
    printf '## Summary\n'
    sed 's/^/- /' "$result_path"
    printf '\n## Verification\n'
    printf -- '- Reviewer: PASS\n'
    printf -- '- Tests: passing or not configured\n'
    printf '\nCloses #%s\n\nDispatched by AutoShip.\n' "$number"
  } > "$body_path"
}

create_verified_pr() {
  local issue="$1"
  local workspace="$WORKSPACES_DIR/$issue"
  local body_path="$workspace/AUTOSHIP_PR_BODY.md"
  local number title labels pr_title pr_url branch

  number=$(issue_number "$issue")
  title=$(issue_title "$issue")
  labels=$(issue_labels "$issue")
  pr_title=$(bash "$SCRIPT_DIR/pr-title.sh" --issue "$number" --title "$title" --labels "$labels")
  branch=$(git branch --show-current 2>/dev/null || true)
  [[ -n "$branch" ]] || branch="autoship/issue-$number"

  generate_pr_body "$issue"

  git add -A -- . ':!.autoship'
  if git diff --cached --quiet; then
    return 1
  fi
  git commit -m "$pr_title

Closes #$number
Dispatched by AutoShip." >/dev/null

  pr_url=$(gh pr create \
    --title "$pr_title" \
    --body-file "$body_path" \
    --label autoship \
    --head "$branch")

  if [[ -n "$pr_url" ]]; then
    local pr_number
    pr_number=$(printf '%s\n' "$pr_url" | grep -Eo '[0-9]+$' | tail -1 || true)
    [[ -n "$pr_number" ]] && printf '%s\n' "$pr_number"
  fi
}

verify_and_create_pr() {
  local issue="$1"
  local verification_status pr_number

  run_verification "$issue"
  verification_status=$?
  if [[ $verification_status -eq 2 ]]; then
    return 0
  fi
  if [[ $verification_status -ne 0 ]]; then
    apply_state_once "$issue" set-failed blocked
    return 0
  fi

  pr_number=$(create_verified_pr "$issue" || true)
  if [[ -n "$pr_number" ]]; then
    apply_state_once "$issue" set-completed completed pr_number="$pr_number"
  else
    apply_state_once "$issue" set-blocked blocked
  fi
}

apply_state_once() {
  local issue="$1"
  local action="$2"
  local target_state="$3"
  local current

  current=$(current_state "$issue")
  case "$current" in
    "$target_state"|merged)
      return 0
      ;;
  esac

  bash "$SCRIPT_DIR/../update-state.sh" "$action" "$issue"
}

process_event() {
  local event="$1"
  local type issue
  type=$(jq -r '.type // empty' <<< "$event")
  issue=$(jq -r '.issue // empty' <<< "$event")

  case "$type" in
    blocked)
      [[ -n "$issue" ]] || return 1
      apply_state_once "$issue" set-blocked blocked
      ;;
    stuck)
      [[ -n "$issue" ]] || return 1
      apply_state_once "$issue" set-stuck stuck
      ;;
    verify)
      [[ -n "$issue" ]] || return 1
      apply_state_once "$issue" set-verifying verifying
      verify_and_create_pr "$issue"
      ;;
    force_dispatch)
      [[ -n "$issue" ]] || return 1
      local state number task_type
      state=$(current_state "$issue")
      case "$state" in
        queued|running|verifying|completed|merged|blocked)
          return 0
          ;;
      esac
      number="${issue#issue-}"
      task_type=$(bash "$SCRIPT_DIR/classify-issue.sh" "$number")
      bash "$SCRIPT_DIR/dispatch.sh" "$number" "$task_type"
      bash "$SCRIPT_DIR/runner.sh"
      ;;
    *)
      echo "Skipping unsupported event type: ${type:-<missing>}" >&2
      ;;
  esac
}

ensure_array_file "$EVENT_QUEUE"
ensure_array_file "$PROCESSED_EVENTS"

remaining_tmp=$(make_tmp)
printf '[]\n' > "$remaining_tmp"

while IFS= read -r event; do
  [[ -n "$event" ]] || continue
  key=$(event_key "$event")
  if is_processed "$key"; then
    continue
  fi

  if process_event "$event"; then
    mark_processed "$key"
  else
    next_remaining=$(make_tmp)
    jq --argjson evt "$event" '. + [$evt]' "$remaining_tmp" > "$next_remaining" && mv "$next_remaining" "$remaining_tmp"
  fi
done < <(jq -c '.[]' "$EVENT_QUEUE")

mv "$remaining_tmp" "$EVENT_QUEUE"
