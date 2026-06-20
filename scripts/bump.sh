#!/usr/bin/env bash
set -euo pipefail

BUMP="${1:-}"
case "$BUMP" in patch|minor|major) ;; *)
  echo "usage: scripts/bump.sh <patch|minor|major>" >&2; exit 2 ;;
esac
[ -f Cargo.toml ] || { echo "run from repo root (Cargo.toml not found)" >&2; exit 1; }

CURRENT=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"
case "$BUMP" in
  major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
  patch) PATCH=$((PATCH + 1)) ;;
esac
NEW="${MAJOR}.${MINOR}.${PATCH}"

# Portable in-place edit (GNU + BSD/macOS): rewrite first `version = ` line.
tmp=$(mktemp)
sed "0,/^version = /s/^version = \"[^\"]*\"/version = \"$NEW\"/" Cargo.toml > "$tmp"
mv "$tmp" Cargo.toml

cargo check --quiet            # refresh the dcc entry in Cargo.lock (matches CI)
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to v$NEW"
echo "Bumped $CURRENT -> $NEW and committed. Push with: git push origin main"
