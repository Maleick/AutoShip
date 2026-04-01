#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

require_remote() {
    local remote_name="$1"
    if ! git remote get-url "$remote_name" >/dev/null 2>&1; then
        echo "missing git remote: $remote_name" >&2
        echo "add the local vendor remotes before refreshing third_party snapshots" >&2
        exit 1
    fi
}

strip_git_metadata() {
    find third_party/eqlib third_party/macroquest -type d -name '.github' -prune -exec rm -rf {} +
    find third_party/eqlib third_party/macroquest \
        \( -name '.gitignore' -o -name '.gitattributes' -o -name '.gitmodules' -o -name '.git' \) \
        -exec rm -rf {} +
}

require_remote vendor-eqlib
require_remote vendor-macroquest

git subtree pull --prefix=third_party/eqlib vendor-eqlib live --squash \
    -m "chore(third_party): refresh eqlib snapshot"
git subtree pull --prefix=third_party/macroquest vendor-macroquest master --squash \
    -m "chore(third_party): refresh macroquest snapshot"

strip_git_metadata
