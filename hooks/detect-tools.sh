#!/usr/bin/env bash
set -euo pipefail

# detect-tools.sh — Detect available AI CLI tools and output JSON report.
# Outputs quota_pct for each tool: 100 if known-full, -1 if unknown, 0-100 if parseable.
# Always exits 0; missing tools are reported as unavailable, not errors.

# ---------------------------------------------------------------------------
# Quota helpers — return integer 0-100 or -1 (unknown)
# ---------------------------------------------------------------------------

# Claude uses a Max subscription — treat as always full.
quota_claude() {
  echo "100"
}

# Codex CLI: no public quota/status command as of 2025. Default to -1.
# If a future `codex quota --model spark` or similar command exists, parse it here.
quota_codex_spark() {
  # Attempt: codex features may expose quota info in the future.
  # For now there is no machine-readable quota output — return unknown.
  echo "-1"
}

quota_codex_gpt() {
  echo "-1"
}

# Gemini CLI: no --quota or status subcommand as of 2025. Default to -1.
quota_gemini() {
  echo "-1"
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
    spark_q=$(quota_codex_spark)
    gpt_q=$(quota_codex_gpt)
    printf '"codex-spark": {"available": true, "version": "%s", "quota_pct": %s}, ' \
      "$ver" "$spark_q"
    printf '"codex-gpt": {"available": true, "version": "%s", "quota_pct": %s}' \
      "$ver" "$gpt_q"
  else
    printf '"codex-spark": {"available": false, "quota_pct": -1}, '
    printf '"codex-gpt": {"available": false, "quota_pct": -1}'
  fi
}

detect_gemini() {
  if command -v gemini >/dev/null 2>&1; then
    local ver qpct
    ver=$(gemini --version 2>/dev/null | head -1) || ver="unknown"
    ver=$(printf '%s' "$ver" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n')
    qpct=$(quota_gemini)
    printf '"gemini": {"available": true, "version": "%s", "quota_pct": %s}' "$ver" "$qpct"
  else
    printf '"gemini": {"available": false, "quota_pct": -1}'
  fi
}

# ---------------------------------------------------------------------------
# Build JSON output
# ---------------------------------------------------------------------------

echo -n "{"
detect_claude
echo -n ", "
detect_codex
echo -n ", "
detect_gemini
echo "}"

exit 0
