#!/usr/bin/env bash
# PostToolUse hook: Run rustfmt on .rs files after Write|Edit

input=$(cat)
file_path=$(echo "$input" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)

# Skip if no file path or not a Rust file
[ -z "$file_path" ] && exit 0
case "$file_path" in
  *.rs) ;;
  *) exit 0 ;;
esac

# Only run if rustfmt is available and the file exists
if command -v rustfmt &>/dev/null && [ -f "$file_path" ]; then
  rustfmt "$file_path" 2>/dev/null || true
fi

exit 0
