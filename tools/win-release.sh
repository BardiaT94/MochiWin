#!/bin/sh
# A new Windows version: tools/win-release.sh X.Y.Z ["notes"]
# Writes MOCHI_VERSION, builds dist-win/ and MochiSetup.exe (tools/win-build.sh), commits
# and tags as BardiaT94, pushes, and makes a GitHub release on BardiaT94/MochiWin with
# MochiSetup.exe, its .sha256 and mochi-windows.zip. Every installed Mochi on Windows finds
# it within a day and offers it in its ⋮ menu (it runs the installer quietly).
# MESA_7Z and NSIS_DIR default to FileManager/wintools/.
set -e
C=$(dirname "$(dirname "$(readlink -f "$0")")")
cd "$C"
v=${1:-}; notes=${2:-}
echo "$v" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo 'usage: tools/win-release.sh X.Y.Z ["notes"]' >&2; exit 1; }
git rev-parse -q --verify "refs/tags/v$v" > /dev/null && { echo "v$v is already released" >&2; exit 1; }
: "${MESA_7Z:=$C/../wintools/mesa3d-26.2.6-release-mingw.7z}"
: "${NSIS_DIR:=$C/../wintools/nsis-3.10}"
export MESA_7Z NSIS_DIR
printf '%s\n' "$v" > MOCHI_VERSION
tools/win-build.sh
[ -f MochiSetup.exe ] || { echo "no MochiSetup.exe: is NSIS_DIR right?" >&2; exit 1; }
sha256sum MochiSetup.exe > MochiSetup.exe.sha256
rm -f mochi-windows.zip
T=$(mktemp -d)
cp -r dist-win "$T/Mochi"
(cd "$T" && 7z a -tzip -bd -bso0 "$C/mochi-windows.zip" Mochi)
rm -rf "$T"
git add -A
git diff --cached --quiet || git -c user.name=BardiaT94 -c user.email=BardiaT94@users.noreply.github.com commit -q -m "Mochi $v"
git -c user.name=BardiaT94 -c user.email=BardiaT94@users.noreply.github.com tag -a "v$v" -m "Mochi $v"
git push -q origin HEAD "v$v"
body=$(printf '%s\n\n---\n\n**Install:** download `MochiSetup.exe` and run it (no admin rights needed). Windows may say "Windows protected your PC" because the installer is not signed yet: click **More info**, then **Run anyway**.\n\nNo installer: `mochi-windows.zip` holds the same program; unzip it and run `mochi.exe`.\n\nAfter installing, Mochi looks for new versions once a day and offers them in its ⋮ menu.\n' "${notes:-Mochi $v}")
GH_TOKEN=$(gh auth token -u BardiaT94) gh release create "v$v" -R BardiaT94/MochiWin --title "Mochi $v for Windows" \
  --notes "$body" MochiSetup.exe MochiSetup.exe.sha256 mochi-windows.zip
