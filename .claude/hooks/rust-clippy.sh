#!/usr/bin/env bash
# PostToolUse hook: Run clippy check after editing .rs files
# Only reports warnings/errors — does not block

input=$(cat)
file_path=$(echo "$input" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)

# Skip if no file path or not a Rust file
[ -z "$file_path" ] && exit 0
case "$file_path" in
  *.rs) ;;
  *) exit 0 ;;
esac

# Run clippy on the workspace — timeout is handled by Claude Code (120s in settings)
if command -v cargo &>/dev/null; then
  cargo clippy --message-format=short 2>&1 | grep -E "^(warning|error)" | head -10 || true
fi

exit 0
