#!/usr/bin/env bash
set -euo pipefail

mock_opencode_models_inventory() {
  printf '%s\n' \
    'opencode/nemotron-3-super-free' \
    'opencode/minimax-m2.5-free' \
    'openai/gpt-5.5' \
    'openai/gpt-5.3-codex-spark'
}

install_mock_opencode_models_fixture() {
  local bin_dir="$1"
  mkdir -p "$bin_dir"

  cat > "$bin_dir/opencode" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "models" ]]; then
  printf '%s\n' \
    'opencode/nemotron-3-super-free' \
    'opencode/minimax-m2.5-free' \
    'openai/gpt-5.5' \
    'openai/gpt-5.3-codex-spark'
  exit 0
fi

printf 'mock opencode fixture only permits `opencode models`, got:' >&2
printf ' %q' "$@" >&2
printf '\n' >&2
exit 99
SH

  cat > "$bin_dir/gh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-} ${2:-}" == "auth status" ]]; then
  exit 0
fi
if [[ "${1:-} ${2:-}" == "label create" ]]; then
  exit 0
fi
exit 0
SH

  chmod +x "$bin_dir/opencode" "$bin_dir/gh"
}
