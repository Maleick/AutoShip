#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Error: not inside a git repository" >&2
  exit 1
}
source "$REPO_ROOT/hooks/opencode/test-fixtures/mock-opencode-models.sh"

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
cat >"$OPENCODE_CONFIG_DIR/opencode.json" <<'JSON'
{"plugin":["real-user-plugin"]}
JSON
BIN_DIR="$CONFIG_HOME/bin"
mkdir -p "$BIN_DIR"
install_mock_opencode_models_fixture "$BIN_DIR"
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
[[ -d "$AUTOSHIP_INSTALL_DIR/hooks" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/commands" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/skills" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/policies" ]]
[[ -f "$CONFIG_HOME/opencode/commands/autoship.md" ]]
[[ -f "$CONFIG_HOME/opencode/commands/autoship-setup.md" ]]
[[ -f "$CONFIG_HOME/opencode/skills/autoship-setup/SKILL.md" ]]
[[ -f "$CONFIG_HOME/opencode/skills/autoship-orchestrate/SKILL.md" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/plugins/autoship.ts" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/AGENTS.md" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/VERSION" ]]

bash "$REPO_ROOT/hooks/opencode/init.sh" >/dev/null

INSTALLED_PROJECT="$PACKAGE_FIXTURE_DIR/installed-project"
mkdir -p "$INSTALLED_PROJECT"
git init -q "$INSTALLED_PROJECT"
(
  cd "$INSTALLED_PROJECT"
  OPENCODE_CONFIG_DIR="$CONFIG_HOME/opencode" bash "$AUTOSHIP_INSTALL_DIR/hooks/opencode/init.sh" >/dev/null
  OPENCODE_CONFIG_DIR="$CONFIG_HOME/opencode" node "$AUTOSHIP_PACKAGE_FIXTURE_ROOT/dist/cli.js" doctor >/dev/null
)
[[ -d "$AUTOSHIP_INSTALL_DIR/hooks" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/commands" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/skills" ]]
[[ -d "$AUTOSHIP_INSTALL_DIR/policies" ]]
[[ -f "$CONFIG_HOME/opencode/commands/autoship.md" ]]
[[ -f "$CONFIG_HOME/opencode/commands/autoship-setup.md" ]]
[[ -f "$CONFIG_HOME/opencode/skills/autoship-setup/SKILL.md" ]]
[[ -f "$CONFIG_HOME/opencode/skills/autoship-orchestrate/SKILL.md" ]]
[[ -f "$AUTOSHIP_INSTALL_DIR/plugins/autoship.ts" ]]

[[ -f "$STATE_FILE" ]]
[[ "$(cat "$HOOKS_FILE")" == "$REPO_ROOT/hooks" ]]
jq -e '.config.maxConcurrentAgents == 20' "$STATE_FILE" >/dev/null
jq -e '.cargoConcurrencyCap == 8 and .mergeStrategy == "safe" and .policyProfile == "default"' "$REPO_ROOT/.autoship/config.json" >/dev/null
[[ -f "$REPO_ROOT/.autoship/model-history.json" ]]
[[ -f "$REPO_ROOT/.autoship/model-routing.json" ]]
jq -e '[.models[] | select(.cost == "free")] | length > 0' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null
jq -e 'all(.models[]; .id | test("^[a-z0-9._:-]+(/[a-z0-9._:-]+)?$"))' "$REPO_ROOT/.autoship/model-routing.json" >/dev/null

bash "$REPO_ROOT/hooks/opencode/test-policy.sh" >/dev/null

echo "OpenCode install smoke test passed"
