#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
INSTALLED_VERSION_FILE=""
RELEASE_TAG_FILE=""

failures=()

usage() {
  cat <<'EOF'
Usage: validate-version-alignment.sh [--repo DIR] [--installed-version FILE] [--release-tag FILE]

Validates that release version surfaces agree:
  - VERSION
  - package.json version
  - CHANGELOG release heading
  - optional installed asset marker file
  - optional GitHub release tag marker file
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO_ROOT="$2"
      shift 2
      ;;
    --installed-version)
      INSTALLED_VERSION_FILE="$2"
      shift 2
      ;;
    --release-tag)
      RELEASE_TAG_FILE="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

trim_file() {
  tr -d '[:space:]' < "$1"
}

record_mismatch() {
  failures+=("$1")
}

VERSION_FILE="$REPO_ROOT/VERSION"
PACKAGE_FILE="$REPO_ROOT/package.json"
CHANGELOG_FILE="$REPO_ROOT/CHANGELOG.md"

if [[ ! -f "$VERSION_FILE" ]]; then
  record_mismatch "VERSION file is missing"
else
  expected_version="$(trim_file "$VERSION_FILE")"
fi

if [[ -z "${expected_version:-}" ]]; then
  record_mismatch "VERSION file is empty"
else
  if [[ ! -f "$PACKAGE_FILE" ]]; then
    record_mismatch "package.json is missing"
  else
    package_version="v$(jq -r '.version // empty' "$PACKAGE_FILE")"
    if [[ "$package_version" != "$expected_version" ]]; then
      record_mismatch "package.json version $package_version does not match VERSION $expected_version"
    fi
  fi

  if [[ ! -f "$CHANGELOG_FILE" ]]; then
    record_mismatch "CHANGELOG.md is missing"
  elif ! grep -Eq "^## ${expected_version//./\\.}$" "$CHANGELOG_FILE"; then
    record_mismatch "CHANGELOG release heading for $expected_version is missing"
  fi

  if [[ -n "$INSTALLED_VERSION_FILE" ]]; then
    if [[ ! -f "$INSTALLED_VERSION_FILE" ]]; then
      record_mismatch "installed asset marker $INSTALLED_VERSION_FILE is missing"
    else
      installed_version="$(trim_file "$INSTALLED_VERSION_FILE")"
      if [[ "$installed_version" != "$expected_version" ]]; then
        record_mismatch "installed asset marker $installed_version does not match VERSION $expected_version"
      fi
    fi
  fi

  if [[ -n "$RELEASE_TAG_FILE" ]]; then
    if [[ ! -f "$RELEASE_TAG_FILE" ]]; then
      record_mismatch "GitHub release tag marker $RELEASE_TAG_FILE is missing"
    else
      release_tag="$(trim_file "$RELEASE_TAG_FILE")"
      if [[ "$release_tag" != "$expected_version" ]]; then
        record_mismatch "GitHub release tag marker $release_tag does not match VERSION $expected_version"
      fi
    fi
  fi
fi

if [[ ${#failures[@]} -gt 0 ]]; then
  printf 'Version alignment validation failed:\n' >&2
  for failure in "${failures[@]}"; do
    printf -- '- %s\n' "$failure" >&2
  done
  exit 1
fi

printf 'Version alignment validation passed for %s\n' "$expected_version"
