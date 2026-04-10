#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/reconcile-agent-queue.sh [--dry-run] [--project-number N] <owner/repo>

Legacy GitHub Project / Agent Status repair helper.
Active TextQuest tracking now lives in issues, linked PRs, and milestones; keep
this script as historical transition tooling only.
EOF
}

DRY_RUN=0
PROJECT_NUMBER=1
REPO=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      ;;
    --project-number)
      shift
      [ "$#" -gt 0 ] || {
        echo "Error: --project-number requires a value." >&2
        exit 1
      }
      PROJECT_NUMBER="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [ -n "$REPO" ]; then
        echo "Error: unexpected argument: $1" >&2
        usage >&2
        exit 1
      fi
      REPO="$1"
      ;;
  esac
  shift
done

if [ -z "$REPO" ]; then
  if [ -n "${GITHUB_REPOSITORY:-}" ]; then
    REPO="$GITHUB_REPOSITORY"
  else
    echo "Error: repository must be provided as <owner/repo> or via GITHUB_REPOSITORY." >&2
    exit 1
  fi
fi

OWNER="${REPO%%/*}"
REPO_NAME="${REPO#*/}"

READY_LABEL="agent:ready"
SKIP_LABEL="agent:skip-ready"
WORKING_LABEL="agent:working"
BLOCKED_LABEL="agent:blocked"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

OPEN_ISSUES_JSON="$TMP_DIR/open-issues.json"
OPEN_PR_ISSUES="$TMP_DIR/open-pr-issues.txt"
OPEN_PR_URLS="$TMP_DIR/open-pr-urls.txt"
PROJECT_MAP="$TMP_DIR/project-map.tsv"

PROJECT_ID=""
AGENT_STATUS_FIELD_ID=""
BACKLOG_OPTION_ID=""
READY_OPTION_ID=""
WORKING_OPTION_ID=""
PR_OPEN_OPTION_ID=""
BLOCKED_OPTION_ID=""

PROJECT_ADDS=0
LABEL_EDITS=0
STATUS_EDITS=0
READY_ADDS=0
READY_REMOVALS=0
SKIP_ADDS=0
SKIP_REMOVALS=0
WORKING_REMOVALS=0

log() {
  printf '%s\n' "$*"
}

uri_encode() {
  jq -nr --arg value "$1" '$value | @uri'
}

run_cmd() {
  if [ "$DRY_RUN" -eq 1 ]; then
    printf '[dry-run] %s\n' "$*"
    return 0
  fi
  "$@"
}

ensure_label() {
  local name="$1"
  local color="$2"
  local description="$3"
  local encoded_name

  encoded_name="$(uri_encode "$name")"
  if gh api "repos/$REPO/labels/$encoded_name" >/dev/null 2>&1; then
    return 0
  fi

  log "Creating label $name"
  run_cmd gh label create --repo "$REPO" "$name" --color "$color" --description "$description" >/dev/null
}

load_project_metadata() {
  local project_json fields_json

  project_json="$(gh project list --owner "$OWNER" --format json)"
  PROJECT_ID="$(
    jq -r --argjson number "$PROJECT_NUMBER" '
      .projects[]
      | select(.number == $number)
      | .id
    ' <<<"$project_json"
  )"

  if [ -z "$PROJECT_ID" ] || [ "$PROJECT_ID" = "null" ]; then
    echo "Error: could not resolve project $PROJECT_NUMBER for owner $OWNER." >&2
    exit 1
  fi

  fields_json="$(gh project field-list "$PROJECT_NUMBER" --owner "$OWNER" --format json)"
  AGENT_STATUS_FIELD_ID="$(
    jq -r '
      .fields[]
      | select(.name == "Agent Status")
      | .id
    ' <<<"$fields_json"
  )"

  if [ -z "$AGENT_STATUS_FIELD_ID" ] || [ "$AGENT_STATUS_FIELD_ID" = "null" ]; then
    echo "Error: Agent Status field is missing from project $PROJECT_NUMBER." >&2
    exit 1
  fi

  option_id() {
    local option_name="$1"
    jq -r --arg option_name "$option_name" '
      .fields[]
      | select(.name == "Agent Status")
      | .options[]
      | select(.name == $option_name)
      | .id
    ' <<<"$fields_json"
  }

  BACKLOG_OPTION_ID="$(option_id "Backlog")"
  READY_OPTION_ID="$(option_id "Ready for Agent")"
  WORKING_OPTION_ID="$(option_id "Agent Working")"
  PR_OPEN_OPTION_ID="$(option_id "PR Open")"
  BLOCKED_OPTION_ID="$(option_id "Blocked")"
}

refresh_open_issues() {
  gh api --paginate --slurp "repos/$REPO/issues?state=open&per_page=100" \
    | jq '
        [
          .[][]
          | select(has("pull_request") | not)
          | {
              number,
              title,
              url,
              labels: (.labels // [])
            }
        ]
      ' \
    >"$OPEN_ISSUES_JSON"
}

refresh_open_pr_issue_numbers() {
  local open_prs_json

  open_prs_json="$(gh api --paginate --slurp "repos/$REPO/pulls?state=open&per_page=100")"

  jq -r '.[].[] | .html_url' <<<"$open_prs_json" | sort -u >"$OPEN_PR_URLS"

  jq -cr '.[].[] | {body: (.body // "")}' <<<"$open_prs_json" \
    | while IFS= read -r pr_row; do
        [ -z "$pr_row" ] && continue
        jq -r '.body' <<<"$pr_row" \
          | grep -Eio '(close[sd]?|fix(e[sd])?|resolve[sd]?) #[0-9]+' \
          | grep -Eo '[0-9]+' \
          || true
      done \
    | sort -u \
    >"$OPEN_PR_ISSUES"
}

refresh_project_map() {
  gh project item-list "$PROJECT_NUMBER" --owner "$OWNER" --format json -L 1000 \
    | jq -r '
        .items[]
        | select(.content.type == "Issue")
        | [
            .content.number,
            .id,
            (."agent Status" // ""),
            ((."linked pull requests" // []) | join(","))
          ]
        | @tsv
      ' \
    >"$PROJECT_MAP"
}

is_roadmap_container_title() {
  local title="$1"
  printf '%s\n' "$title" | grep -Eiq '^M([0-9]+|x)\b'
}

item_row_for_issue() {
  local issue_number="$1"
  awk -F '\t' -v issue_number="$issue_number" '$1 == issue_number { print; exit }' "$PROJECT_MAP"
}

issue_has_open_pr() {
  local issue_number="$1"
  local project_row linked_pr_urls pr_url

  if grep -qx "$issue_number" "$OPEN_PR_ISSUES"; then
    return 0
  fi

  project_row="$(item_row_for_issue "$issue_number")"
  [ -n "$project_row" ] || return 1

  linked_pr_urls="$(printf '%s\n' "$project_row" | cut -f4)"
  [ -n "$linked_pr_urls" ] || return 1

  IFS=',' read -r -a linked_pr_array <<<"$linked_pr_urls"
  for pr_url in "${linked_pr_array[@]}"; do
    [ -z "$pr_url" ] && continue
    if grep -Fxq "$pr_url" "$OPEN_PR_URLS"; then
      return 0
    fi
  done

  return 1
}

desired_status_for_issue() {
  local issue_number="$1"
  local title="$2"
  local labels_json="$3"

  if is_roadmap_container_title "$title"; then
    printf 'Backlog\n'
    return 0
  fi

  if jq -e --arg blocked "$BLOCKED_LABEL" 'index($blocked) != null' <<<"$labels_json" >/dev/null; then
    printf 'Blocked\n'
    return 0
  fi

  if issue_has_open_pr "$issue_number"; then
    printf 'PR Open\n'
    return 0
  fi

  if jq -e --arg working "$WORKING_LABEL" 'index($working) != null' <<<"$labels_json" >/dev/null; then
    printf 'Agent Working\n'
    return 0
  fi

  printf 'Ready for Agent\n'
}

option_id_for_status() {
  case "$1" in
    Backlog) printf '%s\n' "$BACKLOG_OPTION_ID" ;;
    "Ready for Agent") printf '%s\n' "$READY_OPTION_ID" ;;
    "Agent Working") printf '%s\n' "$WORKING_OPTION_ID" ;;
    "PR Open") printf '%s\n' "$PR_OPEN_OPTION_ID" ;;
    Blocked) printf '%s\n' "$BLOCKED_OPTION_ID" ;;
    *)
      echo "Error: unsupported Agent Status '$1'." >&2
      exit 1
      ;;
  esac
}

ensure_project_membership() {
  local issue_number="$1"
  local issue_url="$2"

  if [ -n "$(item_row_for_issue "$issue_number")" ]; then
    return 0
  fi

  log "Adding issue #$issue_number to project $PROJECT_NUMBER"
  run_cmd gh project item-add "$PROJECT_NUMBER" --owner "$OWNER" --url "$issue_url" >/dev/null
  PROJECT_ADDS=$((PROJECT_ADDS + 1))
}

apply_issue_labels() {
  local issue_number="$1"
  local title="$2"
  local labels_json="$3"
  local desired_status="$4"
  local add_labels=""
  local remove_labels=""

  if is_roadmap_container_title "$title"; then
    if jq -e --arg skip "$SKIP_LABEL" 'index($skip) == null' <<<"$labels_json" >/dev/null; then
      add_labels="$SKIP_LABEL"
      SKIP_ADDS=$((SKIP_ADDS + 1))
    fi
    if jq -e --arg ready "$READY_LABEL" 'index($ready) != null' <<<"$labels_json" >/dev/null; then
      remove_labels="$READY_LABEL"
      READY_REMOVALS=$((READY_REMOVALS + 1))
    fi
  else
    if jq -e --arg skip "$SKIP_LABEL" 'index($skip) != null' <<<"$labels_json" >/dev/null; then
      remove_labels="$SKIP_LABEL"
      SKIP_REMOVALS=$((SKIP_REMOVALS + 1))
    fi

    if [ "$desired_status" = "Ready for Agent" ]; then
      if jq -e --arg ready "$READY_LABEL" 'index($ready) == null' <<<"$labels_json" >/dev/null; then
        if [ -n "$add_labels" ]; then
          add_labels="$add_labels,$READY_LABEL"
        else
          add_labels="$READY_LABEL"
        fi
        READY_ADDS=$((READY_ADDS + 1))
      fi
    elif jq -e --arg ready "$READY_LABEL" 'index($ready) != null' <<<"$labels_json" >/dev/null; then
      if [ -n "$remove_labels" ]; then
        remove_labels="$remove_labels,$READY_LABEL"
      else
        remove_labels="$READY_LABEL"
      fi
      READY_REMOVALS=$((READY_REMOVALS + 1))
    fi

    if { [ "$desired_status" = "PR Open" ] || [ "$desired_status" = "Blocked" ]; } \
      && jq -e --arg working "$WORKING_LABEL" 'index($working) != null' <<<"$labels_json" >/dev/null
    then
      if [ -n "$remove_labels" ]; then
        remove_labels="$remove_labels,$WORKING_LABEL"
      else
        remove_labels="$WORKING_LABEL"
      fi
      WORKING_REMOVALS=$((WORKING_REMOVALS + 1))
    fi
  fi

  if [ -z "$add_labels" ] && [ -z "$remove_labels" ]; then
    return 0
  fi

  log "Updating labels for issue #$issue_number"
  LABEL_EDITS=$((LABEL_EDITS + 1))

  if [ "$DRY_RUN" -eq 1 ]; then
    [ -n "$add_labels" ] && log "  would add labels: $add_labels"
    [ -n "$remove_labels" ] && log "  would remove labels: $remove_labels"
    return 0
  fi

  local label encoded_label

  if [ -n "$add_labels" ]; then
    local add_cmd=(gh api -X POST "repos/$REPO/issues/$issue_number/labels")
    IFS=',' read -r -a add_array <<<"$add_labels"
    for label in "${add_array[@]}"; do
      add_cmd+=(-f "labels[]=$label")
    done
    "${add_cmd[@]}" >/dev/null
  fi

  if [ -n "$remove_labels" ]; then
    IFS=',' read -r -a remove_array <<<"$remove_labels"
    for label in "${remove_array[@]}"; do
      encoded_label="$(uri_encode "$label")"
      gh api -X DELETE "repos/$REPO/issues/$issue_number/labels/$encoded_label" >/dev/null
    done
  fi
}

apply_project_status() {
  local issue_number="$1"
  local desired_status="$2"
  local project_row item_id current_status option_id

  project_row="$(item_row_for_issue "$issue_number")"
  if [ -z "$project_row" ]; then
    if [ "$DRY_RUN" -eq 1 ]; then
      current_status=""
      item_id=""
    else
      echo "Error: issue #$issue_number is missing from project $PROJECT_NUMBER after reconciliation." >&2
      exit 1
    fi
  else
    item_id="$(printf '%s\n' "$project_row" | cut -f2)"
    current_status="$(printf '%s\n' "$project_row" | cut -f3)"
  fi

  if [ "$current_status" = "$desired_status" ]; then
    return 0
  fi

  option_id="$(option_id_for_status "$desired_status")"
  log "Setting Agent Status for issue #$issue_number: ${current_status:-<unset>} -> $desired_status"
  STATUS_EDITS=$((STATUS_EDITS + 1))

  if [ "$DRY_RUN" -eq 1 ]; then
    return 0
  fi

  gh project item-edit \
    --id "$item_id" \
    --project-id "$PROJECT_ID" \
    --field-id "$AGENT_STATUS_FIELD_ID" \
    --single-select-option-id "$option_id" \
    >/dev/null
}

main() {
  ensure_label "$READY_LABEL" "0E8A16" "Issue is eligible for autonomous pickup."
  ensure_label "$SKIP_LABEL" "D93F0B" "Roadmap container issues are intentionally excluded from agent-ready."

  load_project_metadata
  refresh_open_issues
  refresh_open_pr_issue_numbers
  refresh_project_map

  while IFS= read -r issue_row; do
    [ -z "$issue_row" ] && continue
    ensure_project_membership \
      "$(jq -r '.number' <<<"$issue_row")" \
      "$(jq -r '.url' <<<"$issue_row")"
  done < <(jq -c '.[]' "$OPEN_ISSUES_JSON")

  refresh_project_map

  while IFS= read -r issue_row; do
    [ -z "$issue_row" ] && continue

    issue_number="$(jq -r '.number' <<<"$issue_row")"
    issue_title="$(jq -r '.title' <<<"$issue_row")"
    issue_labels="$(jq -c '[.labels[]?.name]' <<<"$issue_row")"
    desired_status="$(desired_status_for_issue "$issue_number" "$issue_title" "$issue_labels")"

    apply_issue_labels "$issue_number" "$issue_title" "$issue_labels" "$desired_status"
    apply_project_status "$issue_number" "$desired_status"
  done < <(jq -c '.[]' "$OPEN_ISSUES_JSON")

  log ""
  log "Queue reconciliation summary for $REPO"
  log "  project items added: $PROJECT_ADDS"
  log "  issue label edits: $LABEL_EDITS"
  log "  Agent Status edits: $STATUS_EDITS"
  log "  agent:ready added: $READY_ADDS"
  log "  agent:ready removed: $READY_REMOVALS"
  log "  agent:skip-ready added: $SKIP_ADDS"
  log "  agent:skip-ready removed: $SKIP_REMOVALS"
  log "  agent:working removed: $WORKING_REMOVALS"
}

main "$@"
