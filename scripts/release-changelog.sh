#!/bin/sh
# Fold the unreleased commits into CHANGELOG.md as a dated release section.
#
# Usage: scripts/release-changelog.sh vX.Y.Z
#
# Generates the new version's section with git-cliff (Conventional Commits
# since the last tag) and inserts it directly under "## [Unreleased]",
# leaving the header and the curated baseline sections intact. Run this
# BEFORE committing the release. Version-compare links at the bottom of the
# changelog are optional and can be added by hand.
set -e

cd "$(git rev-parse --show-toplevel)"

ver="$1"
case "$ver" in
    v[0-9]*) ;;
    *) echo "usage: $0 vX.Y.Z" >&2; exit 2 ;;
esac

cliff="$(command -v git-cliff 2>/dev/null || echo "$HOME/.cargo/bin/git-cliff")"
[ -x "$cliff" ] || { echo "git-cliff not found — run: cargo install git-cliff" >&2; exit 1; }

prev="$(git describe --tags --abbrev=0 2>/dev/null || true)"
range="${prev:+$prev..}HEAD"

secfile="$(mktemp)"
trap 'rm -f "$secfile"' EXIT
"$cliff" "$range" --tag "$ver" --strip all > "$secfile"

if ! grep -q '^## \[' "$secfile"; then
    echo "no conventional commits since ${prev:-start} — nothing to add" >&2
    exit 0
fi

tmp="$(mktemp)"
awk -v secfile="$secfile" '
    { print }
    /^## \[Unreleased\]/ && !done {
        print "";
        while ((getline line < secfile) > 0) print line;
        done = 1
    }
' CHANGELOG.md > "$tmp"
mv "$tmp" CHANGELOG.md

echo "CHANGELOG.md: inserted section for $ver (commits ${range})."
echo "Next: review the diff, bump [workspace.package] version, commit, tag."
