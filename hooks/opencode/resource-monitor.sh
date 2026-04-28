#!/usr/bin/env bash
set -euo pipefail

# Check system CPU and memory load before dispatching workers.
# Returns JSON with load_status (ok|high|critical), cpu_pct, mem_pct, and recommended_max_concurrent.

get_cpu_pct() {
  local cpu_pct=""
  if command -v top >/dev/null 2>&1; then
    # macOS top: first sample of CPU usage
    cpu_pct=$(top -l 1 -n 0 2>/dev/null | awk '/CPU usage/ {gsub(/%/,""); print 100 - $7}' | head -1)
  fi
  if [[ -z "$cpu_pct" && -f /proc/stat ]]; then
    # Linux: calculate from /proc/stat
    local stat1 stat2 idle1 idle2 total1 total2
    stat1=$(head -1 /proc/stat)
    idle1=$(echo "$stat1" | awk '{print $5}')
    total1=$(echo "$stat1" | awk '{s=0; for(i=2;i<=NF;i++) s+=$i; print s}')
    sleep 0.5
    stat2=$(head -1 /proc/stat)
    idle2=$(echo "$stat2" | awk '{print $5}')
    total2=$(echo "$stat2" | awk '{s=0; for(i=2;i<=NF;i++) s+=$i; print s}')
    cpu_pct=$(awk "BEGIN {printf \"%.0f\", 100 * (1 - ($idle2 - $idle1) / ($total2 - $total1))}")
  fi
  # Fallback: try ps or vmstat
  if [[ -z "$cpu_pct" && command -v vmstat >/dev/null 2>&1 ]]; then
    cpu_pct=$(vmstat 1 2 2>/dev/null | tail -1 | awk '{print 100 - $15}')
  fi
  printf '%s\n' "${cpu_pct:-0}"
}

get_mem_pct() {
  local mem_pct=""
  if command -v vm_stat >/dev/null 2>&1; then
    # macOS
    local pages_free pages_active pages_inactive pages_wired
    pages_free=$(vm_stat | awk '/Pages free/ {gsub(/\./,""); print $3}')
    pages_active=$(vm_stat | awk '/Pages active/ {gsub(/\./,""); print $3}')
    pages_inactive=$(vm_stat | awk '/Pages inactive/ {gsub(/\./,""); print $3}')
    pages_wired=$(vm_stat | awk '/Pages wired down/ {gsub(/\./,""); print $4}')
    local total_used total
    total_used=$((pages_active + pages_inactive + pages_wired))
    total=$((pages_free + total_used))
    if [[ "$total" -gt 0 ]]; then
      mem_pct=$(awk "BEGIN {printf \"%.0f\", 100 * $total_used / $total}")
    fi
  elif [[ -f /proc/meminfo ]]; then
    # Linux
    local mem_total mem_available
    mem_total=$(awk '/MemTotal/ {print $2}' /proc/meminfo)
    mem_available=$(awk '/MemAvailable/ {print $2}' /proc/meminfo 2>/dev/null || awk '/MemFree/ {print $2}' /proc/meminfo)
    if [[ "$mem_total" -gt 0 ]]; then
      mem_pct=$(awk "BEGIN {printf \"%.0f\", 100 * (1 - $mem_available / $mem_total)}")
    fi
  fi
  printf '%s\n' "${mem_pct:-0}"
}

cpu_pct=$(get_cpu_pct)
mem_pct=$(get_mem_pct)

# Validate numeric
[[ "$cpu_pct" =~ ^[0-9]+(\.[0-9]+)?$ ]] || cpu_pct=0
[[ "$mem_pct" =~ ^[0-9]+(\.[0-9]+)?$ ]] || mem_pct=0

# Determine load status and recommended concurrency
current_max="${1:-15}"
[[ "$current_max" =~ ^[0-9]+$ ]] || current_max=15

# Reduce concurrency cap if CPU or memory > 80%
load_status="ok"
recommended_max="$current_max"

if (( $(echo "$cpu_pct > 95" | bc -l 2>/dev/null || echo 0) )) || (( $(echo "$mem_pct > 95" | bc -l 2>/dev/null || echo 0) )); then
  load_status="critical"
  recommended_max=$((current_max / 4))
  [[ "$recommended_max" -lt 1 ]] && recommended_max=1
elif (( $(echo "$cpu_pct > 80" | bc -l 2>/dev/null || echo 0) )) || (( $(echo "$mem_pct > 80" | bc -l 2>/dev/null || echo 0) )); then
  load_status="high"
  recommended_max=$((current_max / 2))
  [[ "$recommended_max" -lt 1 ]] && recommended_max=1
fi

# Use awk for comparison if bc is not available
if ! command -v bc >/dev/null 2>&1; then
  cpu_int=$(printf '%.0f' "$cpu_pct")
  mem_int=$(printf '%.0f' "$mem_pct")
  if [[ "$cpu_int" -gt 95 || "$mem_int" -gt 95 ]]; then
    load_status="critical"
    recommended_max=$((current_max / 4))
    [[ "$recommended_max" -lt 1 ]] && recommended_max=1
  elif [[ "$cpu_int" -gt 80 || "$mem_int" -gt 80 ]]; then
    load_status="high"
    recommended_max=$((current_max / 2))
    [[ "$recommended_max" -lt 1 ]] && recommended_max=1
  fi
fi

jq -n \
  --arg status "$load_status" \
  --argjson cpu "${cpu_pct%.*}" \
  --argjson mem "${mem_pct%.*}" \
  --argjson recommended "$recommended_max" \
  --argjson current "$current_max" \
  '{load_status: $status, cpu_pct: $cpu, mem_pct: $mem, recommended_max_concurrent: $recommended, current_max_concurrent: $current}'
