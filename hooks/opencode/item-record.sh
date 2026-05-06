#!/usr/bin/env bash
set -euo pipefail

AUTOSHIP_ITEMS_DIR="${AUTOSHIP_ITEMS_DIR:-.autoship/items}"

die() {
  echo "$*" >&2
  exit 1
}

sanitize_issue_num() {
  local issue_num="$1"
  if [[ ! "$issue_num" =~ ^[0-9]+$ ]]; then
    echo "Invalid issue number: $issue_num" >&2
    return 1
  fi
  printf '%s' "$issue_num"
}

resolve_items_dir() {
  local base_dir="${1:-$AUTOSHIP_ITEMS_DIR}"
  if [[ -L "$base_dir" ]]; then
    echo "Refusing to operate on symlinked items directory: $base_dir" >&2
    return 1
  fi
  mkdir -p "$base_dir"
  if [[ -L "$base_dir" ]]; then
    echo "Refusing to operate on symlinked items directory: $base_dir" >&2
    return 1
  fi
  (cd "$base_dir" && pwd -P)
}

get_item_path() {
  local issue_num="$1"
  local base_dir="${2:-$AUTOSHIP_ITEMS_DIR}"
  issue_num=$(sanitize_issue_num "$issue_num") || return 1
  base_dir=$(resolve_items_dir "$base_dir") || return 1
  printf '%s/%s.md' "$base_dir" "$issue_num"
}

init_item_record() {
  local issue_num="$1"
  local issue_title="${2:-}"
  local base_dir="${3:-$AUTOSHIP_ITEMS_DIR}"
  local item_path
  item_path=$(get_item_path "$issue_num" "$base_dir")

  if [[ -L "$item_path" ]]; then
    die "Refusing to write to symlinked item record: $item_path"
  fi

  cat >"$item_path" <<EOF
# AutoShip Issue Record: #$issue_num

## Metadata
- **Issue:** $issue_num
- **Title:** ${issue_title:-Untitled}
- **Created:** $(date -u +"%Y-%m-%dT%H:%M:%SZ")
- **Updated:** $(date -u +"%Y-%m-%dT%H:%M:%SZ")

## State History

| Timestamp | State | Attempt | Model | Notes |
|-----------|-------|---------|-------|-------|
EOF

  printf '%s\n' "$item_path"
}

append_item_event() {
  local issue_num="$1"
  local state="$2"
  local attempt="${3:-1}"
  local model="${4:-}"
  local notes="${5:-}"
  local base_dir="${6:-$AUTOSHIP_ITEMS_DIR}"
  local item_path
  item_path=$(get_item_path "$issue_num" "$base_dir")

  if [[ -L "$item_path" ]]; then
    die "Refusing to write to symlinked item record: $item_path"
  fi

  if [[ ! -f "$item_path" ]]; then
    init_item_record "$issue_num" "" "$base_dir"
  fi

  local timestamp
  timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

  local model_cell=""
  if [[ -n "$model" ]]; then
    model_cell="$model"
  fi

  local notes_cell=""
  if [[ -n "$notes" ]]; then
    notes_cell="$notes"
  fi

  local escaped_notes
  escaped_notes=$(printf '%s' "$notes_cell" | sed 's/|/\\|/g')

  local new_line="| $timestamp | $state | $attempt | $model_cell | $escaped_notes |"

  local tmp_file
  tmp_file=$(mktemp)

  if grep -q "| Timestamp |" "$item_path" 2>/dev/null; then
    awk -v line="$new_line" '/\| Timestamp \|.*\|$/ && !done { print; print line; done=1; next } { print }' "$item_path" >"$tmp_file"
  else
    cp "$item_path" "$tmp_file"
    echo "$new_line" >>"$tmp_file"
  fi

  sed -i '' "s/^## Updated:.*/## Updated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")/" "$tmp_file"

  mv "$tmp_file" "$item_path"
}

update_item_title() {
  local issue_num="$1"
  local new_title="$2"
  local base_dir="${3:-$AUTOSHIP_ITEMS_DIR}"
  local item_path
  item_path=$(get_item_path "$issue_num" "$base_dir")

  if [[ -L "$item_path" ]]; then
    die "Refusing to write to symlinked item record: $item_path"
  fi

  if [[ ! -f "$item_path" ]]; then
    init_item_record "$issue_num" "$new_title" "$base_dir"
    return
  fi

  sed -i '' "s/^- \*\*Title:\*\*.*/- **Title:** $new_title/" "$item_path"
  sed -i '' "s/^## Updated:.*/## Updated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")/" "$item_path"
}

get_item_state() {
  local issue_num="$1"
  local base_dir="${2:-$AUTOSHIP_ITEMS_DIR}"
  local item_path
  item_path=$(get_item_path "$issue_num" "$base_dir")

  if [[ -f "$item_path" ]]; then
    tail -10 "$item_path" | grep -E '^\|' | tail -1 | awk -F'|' '{print $3}' | tr -d ' '
  fi
}

list_items() {
  local base_dir="${1:-$AUTOSHIP_ITEMS_DIR}"

  if [[ ! -d "$base_dir" ]]; then
    return
  fi

  for item_file in "$base_dir"/*.md; do
    [[ -f "$item_file" ]] || continue
    local issue_num
    issue_num=$(basename "$item_file" .md)
    echo "$issue_num"
  done | sort -n
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  COMMAND="${1:-}"
  shift || true

  case "$COMMAND" in
    init)
      init_item_record "$@"
      ;;
    append)
      append_item_event "$@"
      ;;
    title)
      update_item_title "$@"
      ;;
    state)
      get_item_state "$@"
      ;;
    list)
      list_items "$@"
      ;;
    *)
      echo "Usage: $0 <command> [args...]" >&2
      echo "Commands:" >&2
      echo "  init <issue-num> [title]       - Create new item record" >&2
      echo "  append <issue-num> <state> [attempt] [model] [notes] - Add state event" >&2
      echo "  title <issue-num> <new-title>  - Update issue title" >&2
      echo "  state <issue-num>              - Get current state" >&2
      echo "  list [dir]                     - List all items" >&2
      exit 1
      ;;
  esac
fi
