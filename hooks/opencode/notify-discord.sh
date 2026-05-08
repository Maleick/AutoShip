#!/usr/bin/env bash
set -euo pipefail

FORCE=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)
      FORCE=true
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$REPO_ROOT"

AUTOSHIP_DIR=".autoship"
STATE_FILE="$AUTOSHIP_DIR/state.json"
CONFIG_FILE="$AUTOSHIP_DIR/config.json"
CHECKPOINT_FILE="$AUTOSHIP_DIR/discord-notify-state.json"

[[ -f "$STATE_FILE" ]] || exit 0

webhook_url="${AUTOSHIP_DISCORD_WEBHOOK_URL:-}"
[[ -n "$webhook_url" ]] || exit 0
case "$webhook_url" in
  *$'\n'* | *$'\r'* | *'"'*)
    echo "Invalid Discord webhook URL" >&2
    exit 0
    ;;
esac
case "$webhook_url" in
  https://discord.com/api/webhooks/* | https://discordapp.com/api/webhooks/*) ;;
  *)
    echo "Invalid Discord webhook URL" >&2
    exit 0
    ;;
esac

interval="900"
if [[ -f "$CONFIG_FILE" ]]; then
  interval=$(jq -r '.discordNotifyIntervalSeconds // .discord_notify_interval_seconds // 900' "$CONFIG_FILE" 2>/dev/null || echo 900)
fi
[[ "$interval" =~ ^[0-9]+$ ]] || interval=900

now=$(date +%s)
last_sent=0
if [[ -f "$CHECKPOINT_FILE" ]]; then
  last_sent=$(jq -r '.last_sent_epoch // 0' "$CHECKPOINT_FILE" 2>/dev/null || echo 0)
fi
if [[ "$FORCE" != true && $((now - last_sent)) -lt "$interval" ]]; then
  exit 0
fi

repo=$(jq -r '.repo // "unknown"' "$STATE_FILE")
count_state() {
  jq -r --arg state "$1" '[.issues // {} | to_entries[] | select((.value.state // .value.status // "") == $state)] | length' "$STATE_FILE"
}
running=$(count_state running)
queued=$(count_state queued)
verifying=$(count_state verifying)
completed=$(count_state completed)
blocked=$(count_state blocked)
stuck=$(count_state stuck)

content=$(printf 'AutoShip status for %s\nRunning: %s / Queued: %s / Verifying: %s / Completed: %s / Blocked: %s / Stuck: %s' \
  "$repo" "$running" "$queued" "$verifying" "$completed" "$blocked" "$stuck")
payload=$(jq -n --arg content "$content" '{content: $content, allowed_mentions: {parse: []}}')

curl_config=$(mktemp "$AUTOSHIP_DIR/discord-curl.tmp.XXXXXX")
chmod 600 "$curl_config"
trap 'rm -f "$curl_config"' EXIT
printf 'url = "%s"\n' "$webhook_url" >"$curl_config"

if ! curl -fsS --connect-timeout 5 --max-time 10 -H 'Content-Type: application/json' --data "$payload" --config "$curl_config" >/dev/null; then
  echo "Discord notification failed" >&2
  exit 1
fi
tmp=$(mktemp "$AUTOSHIP_DIR/discord-notify-state.tmp.XXXXXX")
jq -n --argjson now "$now" '{last_sent_epoch: $now}' >"$tmp"
mv "$tmp" "$CHECKPOINT_FILE"
