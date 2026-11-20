#!/bin/sh
# A new Windows version: tools/win-release.sh X.Y.Z ["notes"]
# Writes MOCHI_VERSION, commits and tags vX.Y.Z as BardiaT94 (the notes are the tag's
# message) and pushes. GitHub Actions (.github/workflows/release.yml) then builds, signs once
# SignPath is set up, and makes the release with MochiSetup.exe, its .sha256 and
# mochi-windows.zip; this script waits for that run, then offers the version to winget
# (tools/winget.sh). Every installed Mochi on Windows finds the release within a day.
# A local build without releasing: tools/win-build.sh.
set -e
C=$(dirname "$(dirname "$(readlink -f "$0")")")
cd "$C"
v=${1:-}; notes=${2:-}
echo "$v" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo 'usage: tools/win-release.sh X.Y.Z ["notes"]' >&2; exit 1; }
git rev-parse -q --verify "refs/tags/v$v" > /dev/null && { echo "v$v is already released" >&2; exit 1; }
GH_TOKEN=$(gh auth token -u BardiaT94); export GH_TOKEN
id() { git -c user.name=BardiaT94 -c user.email=BardiaT94@users.noreply.github.com "$@"; }
printf '%s\n' "$v" > MOCHI_VERSION
git add -A
git diff --cached --quiet || id commit -q -m "Mochi $v"
id tag -a "v$v" -m "${notes:-Mochi $v}"
git push -q origin HEAD "v$v"
echo "pushed v$v; waiting for the release build on GitHub"
sleep 10
run=$(gh run list -R BardiaT94/MochiWin --workflow release.yml --branch "v$v" --limit 1 --json databaseId --jq '.[0].databaseId')
[ -n "$run" ] || { echo "no release run found for v$v" >&2; exit 1; }
gh run watch "$run" -R BardiaT94/MochiWin --exit-status --interval 30 > /dev/null || { echo "the release build failed: gh run view $run -R BardiaT94/MochiWin --log-failed" >&2; exit 1; }
echo "released: https://github.com/BardiaT94/MochiWin/releases/tag/v$v"
tools/winget.sh "$v" || echo "winget: the pull request could not be opened; run tools/winget.sh $v again" >&2
