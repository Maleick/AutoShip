#!/usr/bin/env bash
set -euo pipefail

# detect-tools.sh — Detect available AI CLI tools and output JSON report.

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || REPO_ROOT="$(pwd)"
# Outputs quota_pct for each tool: 100 if known-full, -1 if unknown, 0-100 if parseable.
# Always exits 0; missing tools are reported as unavailable, not errors.

# ---------------------------------------------------------------------------
# Quota helpers — return integer 0-100 or -1 (unknown)
# ---------------------------------------------------------------------------

QUOTA_FILE="${REPO_ROOT}/.autoship/quota.json"

# Claude uses a Max subscription — treat as always full.
quota_claude() {
  echo "100"
}

# Read quota from quota.json if it exists; fall back to 100 (optimistic default).
# Returns -1 only if the tool is unavailable (not installed).
_read_quota() {
  local tool="$1"
  if [[ -f "$QUOTA_FILE" ]]; then
    local q
    q=$(jq -r --arg t "$tool" '.[$t].quota_pct // -1' "$QUOTA_FILE" 2>/dev/null)
    echo "${q:--1}"
  else
    echo "100"  # No quota file yet → assume fresh/full
  fi
}

# Codex: try OpenAI usage API if OPENAI_API_KEY is set; fall back to decay estimate.
# Returns integer 0-100, representing the MOST RESTRICTIVE quota window (daily/weekly).
# Sets CODEX_QUOTA_SOURCE="api" or "est".
CODEX_QUOTA_SOURCE="est"
quota_codex_openai_api() {
  [[ -z "${OPENAI_API_KEY:-}" ]] && return 1

  local today
  today=$(date -u +"%Y-%m-%d")

  # Fetch and parse daily usage
  local response
  response=$(curl -sf --max-time 5 --config - 2>/dev/null <<EOF
url = "https://api.openai.com/v1/usage?date=${today}"
header = "Authorization: Bearer ${OPENAI_API_KEY}"
EOF
) || return 1

  local used_cents
  used_cents=$(echo "$response" | jq -r '[.data[].total_usage // 0] | add // 0' 2>/dev/null) || return 1
  local daily_budget_cents=2000  # ~$20/day budget assumption
  local daily_used_pct=$(( used_cents * 100 / daily_budget_cents ))
  [[ $daily_used_pct -gt 100 ]] && daily_used_pct=100
  local daily_remaining=$(( 100 - daily_used_pct ))

  # Fetch weekly usage in parallel (independent of daily)
  local week_ago
  week_ago=$(date -u -d "7 days ago" +"%Y-%m-%d" 2>/dev/null || date -u -v-7d +"%Y-%m-%d" 2>/dev/null)
  local weekly_remaining="$daily_remaining"
  
  curl -sf --max-time 5 --config - 2>/dev/null <<EOF > /tmp/weekly_usage_$$.json &
url = "https://api.openai.com/v1/usage?start_date=${week_ago}&end_date=${today}"
header = "Authorization: Bearer ${OPENAI_API_KEY}"
EOF
  local curl_pid=$!
  
  # Wait for weekly fetch (up to 6 seconds total for both calls)
  wait "$curl_pid" 2>/dev/null || :
  
  if [[ -f "/tmp/weekly_usage_$$.json" ]]; then
    local weekly_used_cents
    if weekly_used_cents=$(jq -r '[.data[].total_usage // 0] | add // 0' "/tmp/weekly_usage_$$.json" 2>/dev/null); then
      local weekly_budget_cents=10000  # ~$100/week budget assumption
      local weekly_used_pct=$(( weekly_used_cents * 100 / weekly_budget_cents ))
      [[ $weekly_used_pct -gt 100 ]] && weekly_used_pct=100
      weekly_remaining=$(( 100 - weekly_used_pct ))
    fi
    rm -f "/tmp/weekly_usage_$$.json"
  fi

  # Return the MOST RESTRICTIVE (minimum) quota window
  local most_restrictive
  most_restrictive=$(( daily_remaining < weekly_remaining ? daily_remaining : weekly_remaining ))

  echo "$most_restrictive"
  CODEX_QUOTA_SOURCE="api"
  return 0
}

quota_codex_spark() {
  local v
  if v=$(quota_codex_openai_api 2>/dev/null); then
    echo "$v"
  else
    CODEX_QUOTA_SOURCE="est"
    _read_quota "codex-spark"
  fi
}
quota_codex_gpt() {
  local v
  if v=$(quota_codex_openai_api 2>/dev/null); then
    echo "$v"
  else
    CODEX_QUOTA_SOURCE="est"
    _read_quota "codex-gpt"
  fi
}

# Gemini: try Google AI usage API if GEMINI_API_KEY or GOOGLE_API_KEY is set; fall back to decay.
GEMINI_QUOTA_SOURCE="est"
quota_gemini_api() {
  local api_key="${GEMINI_API_KEY:-${GOOGLE_API_KEY:-}}"
  [[ -z "$api_key" ]] && return 1
  # Check quota via generativelanguage API — returns 429 when rate-limited
  local status
  status=$(curl -sS --max-time 5 -o /dev/null -w "%{http_code}" --config - 2>/dev/null <<EOF
url = "https://generativelanguage.googleapis.com/v1beta/models"
header = "x-goog-api-key: ${api_key}"
EOF
) || return 1
  case "$status" in
    200) echo "100"; GEMINI_QUOTA_SOURCE="api" ;;  # reachable → assume full
    429) echo "0";   GEMINI_QUOTA_SOURCE="api" ;;  # rate limited → exhausted
    *)   return 1 ;;
  esac
}

quota_gemini() {
  local v
  if v=$(quota_gemini_api 2>/dev/null); then
    echo "$v"
  else
    GEMINI_QUOTA_SOURCE="est"
    _read_quota "gemini"
  fi
}

# ---------------------------------------------------------------------------
# Per-tool detection
# ---------------------------------------------------------------------------

detect_claude() {
  if command -v claude >/dev/null 2>&1; then
    local ver qpct
    ver=$(claude --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    qpct=$(quota_claude)
    printf '"claude": {"available": true, "version": "%s", "quota_pct": %s}' "$ver" "$qpct"
  else
    printf '"claude": {"available": false, "quota_pct": -1}'
  fi
}

detect_codex() {
  if command -v codex >/dev/null 2>&1; then
    local ver spark_q gpt_q
    ver=$(codex --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')

    # Probe app-server availability — both codex variants use the same binary
    if ! codex app-server help >/dev/null 2>&1; then
      bash "${REPO_ROOT}/hooks/quota-update.sh" stuck "codex-spark" || true
      bash "${REPO_ROOT}/hooks/quota-update.sh" stuck "codex-gpt"   || true
    fi

    spark_q=$(quota_codex_spark)
    gpt_q=$(quota_codex_gpt)
    printf '"codex-spark": {"available": true, "version": "%s", "quota_pct": %s, "quota_source": "%s"}, ' \
      "$ver" "$spark_q" "$CODEX_QUOTA_SOURCE"
    printf '"codex-gpt": {"available": true, "version": "%s", "quota_pct": %s, "quota_source": "%s"}' \
      "$ver" "$gpt_q" "$CODEX_QUOTA_SOURCE"
  else
    printf '"codex-spark": {"available": false, "quota_pct": -1, "quota_source": "n/a"}, '
    printf '"codex-gpt": {"available": false, "quota_pct": -1, "quota_source": "n/a"}'
  fi
}

detect_gemini() {
  if command -v gemini >/dev/null 2>&1 && gemini --version >/dev/null 2>&1; then
    local ver qpct
    ver=$(gemini --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    qpct=$(quota_gemini)
    printf '"gemini": {"available": true, "version": "%s", "quota_pct": %s, "quota_source": "%s"}' \
      "$ver" "$qpct" "$GEMINI_QUOTA_SOURCE"
  else
    printf '"gemini": {"available": false, "quota_pct": -1, "quota_source": "n/a"}'
  fi
}

detect_copilot() {
  if command -v gh >/dev/null 2>&1 && gh copilot --version >/dev/null 2>&1; then
    local ver
    ver=$(gh copilot --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    printf '"copilot": {"available": true, "version": "%s", "quota_pct": -1, "exhausted": false}' "$ver"
  elif command -v copilot >/dev/null 2>&1; then
    local ver
    ver=$(copilot --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    printf '"copilot": {"available": true, "version": "%s", "quota_pct": -1, "exhausted": false}' "$ver"
  else
    printf '"copilot": {"available": false, "quota_pct": -1}'
  fi
}

# ---------------------------------------------------------------------------
# Build JSON output and cache in quota.json
# ---------------------------------------------------------------------------

OUTPUT_JSON=$(cat <<EOF
{
  $(detect_claude), 
  $(detect_codex), 
  $(detect_gemini), 
  $(detect_copilot)
}
EOF
)

echo "$OUTPUT_JSON"

# Cache results to quota.json for fast lookup (avoid re-fetching on next run)
if [[ -n "$OUTPUT_JSON" ]]; then
  mkdir -p "$REPO_ROOT/.autoship"
  echo "$OUTPUT_JSON" | jq '.' > "${QUOTA_FILE}.tmp" 2>/dev/null && mv "${QUOTA_FILE}.tmp" "$QUOTA_FILE" || true
fi

exit 0
