#!/bin/sh
# Publishes a release: bumps the version, builds Figghost.app, tags and pushes, uploads the app to the GitHub
# release and points the Homebrew cask (eliasbui/homebrew-figghost) at it.
#
#   scripts/release.sh 0.2.1
#   DRY_RUN=1 scripts/release.sh 0.2.1    # only the checks and the plan
#
# Needs a clean working tree on main and gh logged in (gh auth login). The app is built before anything is
# tagged or pushed, so a failed build leaves nothing half published. SIGN_IDENTITY is passed to bundle.sh.
set -eu

VERSION=${1:?usage: scripts/release.sh <version, such as 0.2.1>}
REPO=eliasbui/figghost
TAP=eliasbui/homebrew-figghost
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

fail() {
    echo "release: $*" >&2
    exit 1
}

echo "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || fail "a version is three numbers, such as 0.2.1"
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "releases are made from main"
[ -z "$(git status --porcelain)" ] || fail "the working tree has changes; commit them first"
git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null && fail "v$VERSION is already tagged"
gh auth status >/dev/null 2>&1 || fail "log in first: gh auth login"
docker info >/dev/null 2>&1 || fail "start Docker first; it builds the Windows and Linux beta"
CURRENT=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
PREVIOUS=$(git describe --tags --abbrev=0 2>/dev/null || true)

echo "release $VERSION (Cargo.toml has $CURRENT, last tag ${PREVIOUS:-none})"
echo "  build Figghost.app in a temporary folder (dist is left alone) and zip it, tag v$VERSION, push main and the tag to $REPO,"
echo "  build the Windows and Linux beta (Docker), create the GitHub release with the zips, and update Casks/figghost.rb in $TAP"
if [ "${DRY_RUN:-}" = 1 ]; then
    echo "dry run: stopping before any change"
    exit 0
fi

if [ "$CURRENT" != "$VERSION" ]; then
    python3 - "$VERSION" <<'PY'
import pathlib, re, sys
p = pathlib.Path("Cargo.toml")
head, mark, rest = p.read_text().partition("[workspace.package]")
rest = re.sub(r'(?m)^version = "[^"]*"', f'version = "{sys.argv[1]}"', rest, count=1)
p.write_text(head + mark + rest)
PY
    # Only the workspace's own entries change, so no network is needed.
    cargo update --workspace --offline -q
    git commit -q -m "Release $VERSION" Cargo.toml Cargo.lock
fi

# The app is built in a temporary folder and only the zip is kept for the upload, so dist is never touched
# except dist/desktop, which package-desktop.sh rewrites.
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
ZIP="$WORK/Figghost-$VERSION.zip"
scripts/bundle.sh "$WORK/build"
ditto -c -k --sequesterRsrc --keepParent "$WORK/build/Figghost.app" "$ZIP"
SHA=$(shasum -a 256 "$ZIP" | cut -d ' ' -f 1)
echo "zip: $ZIP $SHA"
# The Windows and Linux beta goes to dist/desktop; install.ps1 downloads the Windows zip from the release.
scripts/package-desktop.sh

git tag -a "v$VERSION" -m "Figghost $VERSION"
git push -q origin main "v$VERSION"

{
    echo "Figghost $VERSION, for Apple silicon Macs with macOS 13 or later."
    echo
    echo "    brew tap eliasbui/figghost"
    echo "    brew install --cask eliasbui/figghost/figghost"
    echo
    echo "Writing the cask out in full trusts only that cask, so no brew trust is needed. Without Homebrew:"
    echo
    echo "    curl -fsSL https://raw.githubusercontent.com/eliasbui/figghost/main/scripts/install.sh | sh"
    echo
    echo "Changes:"
    echo
    # The update popup shows this list. docs/releases/<version>.md, a list written for the people who use the
    # app, takes the place of the commit subjects when it exists.
    if [ -f "docs/releases/$VERSION.md" ]; then
        cat "docs/releases/$VERSION.md"
    else
        git log --format='- %s' ${PREVIOUS:+"$PREVIOUS..v$VERSION"}
    fi
    echo
    echo "The app is signed ad hoc and not notarized; the cask removes the quarantine flag after installing it."
    echo "Accessibility access stays granted through updates, because the signature names the app instead of one build; the update from a version that was signed per build asks for it once more."
} >"$WORK/notes.md"
gh release create "v$VERSION" "$ZIP" dist/desktop/* --repo "$REPO" --title "Figghost $VERSION" --notes-file "$WORK/notes.md"

gh repo clone "$TAP" "$WORK/tap" -- -q
sed -i '' -e "s/^  version \".*\"/  version \"$VERSION\"/" -e "s/^  sha256 \".*\"/  sha256 \"$SHA\"/" "$WORK/tap/Casks/figghost.rb"
git -C "$WORK/tap" -c user.name="$(git config user.name)" -c user.email="$(git config user.email)" \
    commit -q -am "Update figghost to $VERSION"
git -C "$WORK/tap" push -q

# Where the tap is installed here, check the new cask the way Homebrew will read it.
TAPPED=$(brew --repository eliasbui/figghost 2>/dev/null || true)
if [ -d "$TAPPED/.git" ]; then
    git -C "$TAPPED" pull -q --ff-only
    HOMEBREW_NO_AUTO_UPDATE=1 brew audit --cask --online eliasbui/figghost/figghost
fi
echo "released $VERSION: https://github.com/$REPO/releases/tag/v$VERSION"
