#!/bin/bash

# Bump version script for TextQuest
# Usage: ./scripts/bump-version.sh <major|minor|patch>
#
# This script updates version numbers in all Cargo.toml files and commits the change.
# Example: ./scripts/bump-version.sh minor
#
# The script will:
# 1. Parse current version from root Cargo.toml or primary crate
# 2. Increment version based on argument (major, minor, or patch)
# 3. Update all workspace crate Cargo.toml files
# 4. Commit the change with message: chore: bump version to vX.Y.Z

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="${ROOT_DIR}/textquest/Cargo.toml"
BUMP_TYPE="${1:-}"

# Validate argument
if [[ -z "$BUMP_TYPE" ]]; then
    echo -e "${RED}Error: Missing argument${NC}"
    echo "Usage: $0 <major|minor|patch>"
    echo ""
    echo "Examples:"
    echo "  $0 major   # Bump 0.6.0 -> 1.0.0"
    echo "  $0 minor   # Bump 0.6.0 -> 0.7.0"
    echo "  $0 patch   # Bump 0.6.0 -> 0.6.1"
    exit 1
fi

# Validate argument value
if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
    echo -e "${RED}Error: Invalid argument '$BUMP_TYPE'${NC}"
    echo "Valid options: major, minor, patch"
    exit 1
fi

# Extract current version from primary crate
echo -e "${YELLOW}Extracting current version...${NC}"
if [[ ! -f "$VERSION_FILE" ]]; then
    echo -e "${RED}Error: Version file not found at $VERSION_FILE${NC}"
    exit 1
fi

# Extract version using grep
CURRENT_VERSION=$(grep -m1 'version = "' "$VERSION_FILE" | sed 's/.*version = "\([^"]*\)".*/\1/')

if [[ -z "$CURRENT_VERSION" ]]; then
    echo -e "${RED}Error: Could not extract version from $VERSION_FILE${NC}"
    exit 1
fi

echo -e "${GREEN}Current version: $CURRENT_VERSION${NC}"

# Parse version components
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Validate version format
if ! [[ "$MAJOR" =~ ^[0-9]+$ ]] || ! [[ "$MINOR" =~ ^[0-9]+$ ]] || ! [[ "$PATCH" =~ ^[0-9]+$ ]]; then
    echo -e "${RED}Error: Invalid version format: $CURRENT_VERSION${NC}"
    echo "Expected format: MAJOR.MINOR.PATCH (e.g., 0.6.0)"
    exit 1
fi

# Calculate new version
case "$BUMP_TYPE" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"

echo -e "${GREEN}New version: $NEW_VERSION${NC}"

# Find all Cargo.toml files in workspace
echo -e "${YELLOW}Updating Cargo.toml files...${NC}"
CARGO_TOMLS=(
    "${ROOT_DIR}/Cargo.toml"
    "${ROOT_DIR}/textquest/Cargo.toml"
    "${ROOT_DIR}/textquest-common/Cargo.toml"
    "${ROOT_DIR}/textquest-dll/Cargo.toml"
    "${ROOT_DIR}/textquest-soul/Cargo.toml"
    "${ROOT_DIR}/textquest-web/Cargo.toml"
    "${ROOT_DIR}/textquest-web-sdk/Cargo.toml"
    "${ROOT_DIR}/tools/llm-client/Cargo.toml"
    "${ROOT_DIR}/tools/eqdiff/Cargo.toml"
)

# Update each Cargo.toml
for toml in "${CARGO_TOMLS[@]}"; do
    if [[ -f "$toml" ]]; then
        # Only update if it has a [package] section with version
        if grep -q '^\[package\]' "$toml" && grep -q 'version = "' "$toml"; then
            echo "  Updating $toml"
            # Use sed to replace version in [package] section
            # This matches version = "X.Y.Z" and replaces with new version
            sed -i '' "s/version = \"[^\"]*\"/version = \"${NEW_VERSION}\"/" "$toml"
        fi
    fi
done

# Also check if root Cargo.toml has workspace.package.version (optional)
if grep -q '\[workspace.package\]' "${ROOT_DIR}/Cargo.toml"; then
    if grep -A 2 '\[workspace.package\]' "${ROOT_DIR}/Cargo.toml" | grep -q 'version'; then
        echo "  Updating workspace.package.version in Cargo.toml"
        sed -i '' "/\[workspace.package\]/,/^$/s/version = \"[^\"]*\"/version = \"${NEW_VERSION}\"/" "${ROOT_DIR}/Cargo.toml"
    fi
fi

echo -e "${GREEN}Version updated in all Cargo.toml files${NC}"

# Commit changes
echo -e "${YELLOW}Committing changes...${NC}"
cd "$ROOT_DIR"
git add Cargo.toml Cargo.lock textquest*/Cargo.toml tools/*/Cargo.toml 2>/dev/null || true
git commit -m "chore: bump version to v${NEW_VERSION}" || {
    echo -e "${YELLOW}Warning: No changes to commit (version may already be v${NEW_VERSION})${NC}"
}

echo -e "${GREEN}Done! Version bumped to v${NEW_VERSION}${NC}"
echo ""
echo "Next steps:"
echo "1. Update CHANGELOG.md with release notes"
echo "2. Create git tag: git tag -a v${NEW_VERSION} -m \"Release v${NEW_VERSION}\""
echo "3. Push tag: git push origin v${NEW_VERSION}"
