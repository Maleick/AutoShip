#!/usr/bin/env bash
set -euo pipefail

checksum_worktree() {
  local dir="${1:-.}"
  (cd "$dir" && git ls-files -z \
    | xargs -0 shasum -a 256 2>/dev/null \
    | LC_ALL=C sort \
    | shasum -a 256 \
    | awk '{print $1}')
}

changed_files_since_base() {
  local dir="${1:-.}"
  (cd "$dir" && git status --porcelain | sed -E 's/^...//; s/^.* -> //')
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  case "${1:-checksum}" in
    checksum) checksum_worktree "${2:-.}" ;;
    changed) changed_files_since_base "${2:-.}" ;;
    *)
      echo "Usage: $0 checksum|changed [dir]" >&2
      exit 2
      ;;
  esac
fi
