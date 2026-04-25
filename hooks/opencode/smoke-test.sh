#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}

capture_e2e_failure() {
  local status="$1"
  if [[ $status -ne 0 && -n "${AUTOSHIP_FAILURE_ISSUE:-${AUTOSHIP_ISSUE_ID:-}}" && -x "$REPO_ROOT/hooks/capture-failure.sh" ]]; then
    AUTOSHIP_FAILURE_HOOK="hooks/opencode/smoke-test.sh" \
      bash "$REPO_ROOT/hooks/capture-failure.sh" e2e_failure "${AUTOSHIP_FAILURE_ISSUE:-${AUTOSHIP_ISSUE_ID:-}}" "error_summary=smoke test failed with exit $status" 2>/dev/null || true
  fi
}

CONFIG_HOME="$(mktemp -d)"
REAL_CONFIG_DIR="$(mktemp -d)"
PACKAGE_FIXTURE_DIR="$(mktemp -d)"
AUTOSHIP_BACKUP=""
if [[ -d "$REPO_ROOT/.autoship" ]]; then
  AUTOSHIP_BACKUP="$(mktemp -d)"
  cp -R "$REPO_ROOT/.autoship" "$AUTOSHIP_BACKUP/"
  rm -rf "$REPO_ROOT/.autoship"
fi

trap 'status=$?; capture_e2e_failure "$status"; rm -rf "$CONFIG_HOME" "$REAL_CONFIG_DIR" "$PACKAGE_FIXTURE_DIR"; if [[ -n "$AUTOSHIP_BACKUP" && -d "$AUTOSHIP_BACKUP/.autoship" ]]; then rm -rf "$REPO_ROOT/.autoship"; cp -R "$AUTOSHIP_BACKUP/.autoship" "$REPO_ROOT/"; fi; rm -rf "$AUTOSHIP_BACKUP"; exit $status' EXIT

export XDG_CONFIG_HOME="$CONFIG_HOME"
export OPENCODE_CONFIG_DIR="$REAL_CONFIG_DIR/opencode"
mkdir -p "$OPENCODE_CONFIG_DIR"
cat > "$OPENCODE_CONFIG_DIR/opencode.json" <<'JSON'
{"plugin":["real-user-plugin"]}
JSON
BIN_DIR="$CONFIG_HOME/bin"
mkdir -p "$BIN_DIR"
cat > "$BIN_DIR/opencode" <<'SH'
#!/usr/bin/env bash
if [[ "$1" == "models" ]]; then
  printf '%s\n' 'opencode/minimax-m2.5-free' 'opencode/nemotron-3-super-free' 'openai/gpt-5.5'
  exit 0
fi
printf '%s\n' '1.14.22'
SH
cat > "$BIN_DIR/gh" <<'SH'
#!/usr/bin/env bash
if [[ "$1 $2" == "auth status" ]]; then
  exit 0
fi
if [[ "$1 $2" == "label create" ]]; then
  exit 0
fi
exit 0
SH
chmod +x "$BIN_DIR/opencode" "$BIN_DIR/gh"
export PATH="$BIN_DIR:$PATH"

source "$REPO_ROOT/hooks/opencode/e2e-package-install-fixture.sh"
autoship_install_package_fixture "$REPO_ROOT" "$CONFIG_HOME" "$PACKAGE_FIXTURE_DIR"

jq -e '.plugin == ["real-user-plugin"]' "$REAL_CONFIG_DIR/opencode/opencode.json" >/dev/null
[[ ! -d "$REAL_CONFIG_DIR/opencode/.autoship" ]]

CONFIG_FILE="$CONFIG_HOME/opencode/opencode.json"
STATE_FILE="$REPO_ROOT/.autoship/state.json"
HOOKS_FILE="$REPO_ROOT/.autoship/hooks_dir"
AUTOSHIP_INSTALL_DIR="$CONFIG_HOME/opencode/.autoship"

jq -e '.plugin | index("opencode-autoship")' "$CONFIG_FILE" >/dev/null
if jq -e '.plugin[] | select(type == "string" and contains("autoship.ts"))' "$CONFIG_FILE" >/dev/null; then
  echo "FAIL: package install registered legacy autoship.ts plugin" >&2
  exit 1
fi
[[ -d "$AUTOSHIP_INSTALL_DIR/hooks" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/commands" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/skills" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/AGENTS.md" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/VERSION" ]]

bash "$REPO_ROOT/hooks/opencode/init.sh" >/dev/null

[[ -f "$STATE_FILE" ]]
[[ "$(cat "$HOOKS_FILE")" == "$REPO_ROOT/hooks" ]]
jq -e '.config.maxConcurrentAgents == 15' "$STATE_FILE" >/dev/null
[[ -f "$REPO_ROOT/.autoship/model-routing.json" ]]
jq -e '[.models[] | select(.cost == "free")] | length > 0' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null
jq -e 'all(.models[]; .id | test("^[a-z0-9._-]+/.+"))' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null

bash "$REPO_ROOT/hooks/opencode/test-policy.sh" >/dev/null

echo "OpenCode install smoke test passed"
