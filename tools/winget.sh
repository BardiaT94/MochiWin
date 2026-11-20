#!/bin/sh
# Offer a released version to winget: tools/winget.sh X.Y.Z
# Writes the three manifests for BardiaT94.Mochi (the installer from the GitHub release
# vX.Y.Z, its SHA-256) to a branch of BardiaT94's fork of microsoft/winget-pkgs, through
# GitHub's API (the repository is too big to clone), and opens the pull request.
# Microsoft's checks then install it in a VM; once merged, `winget install BardiaT94.Mochi`
# and `winget upgrade` find it.
set -e
v=${1:-}
echo "$v" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "usage: tools/winget.sh X.Y.Z" >&2; exit 1; }
GH_TOKEN=$(gh auth token -u BardiaT94); export GH_TOKEN
ID=BardiaT94.Mochi
REL=https://github.com/BardiaT94/MochiWin/releases/download/v$v
URL=$REL/MochiSetup.exe
SHA=$(curl -sfL "$REL/MochiSetup.exe.sha256" | cut -d' ' -f1 | tr a-f A-F)
[ ${#SHA} -eq 64 ] || { echo "no checksum for v$v" >&2; exit 1; }
DATE=$(gh api "repos/BardiaT94/MochiWin/releases/tags/v$v" --jq .published_at | cut -c1-10)
DIR=manifests/b/BardiaT94/Mochi/$v
BRANCH=BardiaT94.Mochi-$v
S='# yaml-language-server: $schema=https://aka.ms/winget-manifest'

version="$S.version.1.9.0.schema.json
PackageIdentifier: $ID
PackageVersion: $v
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.9.0
"
installer="$S.installer.1.9.0.schema.json
PackageIdentifier: $ID
PackageVersion: $v
InstallerType: nullsoft
Scope: user
InstallModes:
- interactive
- silent
UpgradeBehavior: install
ReleaseDate: $DATE
AppsAndFeaturesEntries:
- DisplayName: Mochi
  Publisher: Mochi
  ProductCode: Mochi
Installers:
- Architecture: x64
  InstallerUrl: $URL
  InstallerSha256: $SHA
ManifestType: installer
ManifestVersion: 1.9.0
"
locale="$S.defaultLocale.1.9.0.schema.json
PackageIdentifier: $ID
PackageVersion: $v
PackageLocale: en-US
Publisher: BardiaT94
PublisherUrl: https://github.com/BardiaT94
PublisherSupportUrl: https://github.com/BardiaT94/MochiWin/issues
PackageName: Mochi
PackageUrl: https://github.com/BardiaT94/MochiWin
License: GPL-3.0-or-later
LicenseUrl: https://github.com/BardiaT94/MochiWin/blob/HEAD/LICENSE
ShortDescription: A fast, keyboard-first file manager with a Catppuccin look.
Description: Mochi is a redesign of the Maia file manager (Rust, egui) with split panels, tabs, live search in subfolders, previews, a built-in terminal pane and coloured folders with symbols.
Tags:
- file-manager
- files
- explorer
- catppuccin
ReleaseNotesUrl: https://github.com/BardiaT94/MochiWin/releases/tag/v$v
ManifestType: defaultLocale
ManifestVersion: 1.9.0
"

gh api repos/BardiaT94/winget-pkgs > /dev/null 2>&1 || { gh repo fork microsoft/winget-pkgs --clone=false > /dev/null; sleep 5; }
gh api -X POST repos/BardiaT94/winget-pkgs/merge-upstream -f branch=master > /dev/null
BASE=$(gh api repos/BardiaT94/winget-pkgs/git/ref/heads/master --jq .object.sha)
gh api -X POST repos/BardiaT94/winget-pkgs/git/refs -f ref="refs/heads/$BRANCH" -f sha="$BASE" > /dev/null
put() {
  gh api -X PUT "repos/BardiaT94/winget-pkgs/contents/$DIR/$1" -f message="$ID $v: $1" \
    -f branch="$BRANCH" -f content="$(printf '%s' "$2" | base64 -w0)" > /dev/null
}
put "$ID.yaml" "$version"
put "$ID.installer.yaml" "$installer"
put "$ID.locale.en-US.yaml" "$locale"
NEW=$(gh api "repos/microsoft/winget-pkgs/contents/manifests/b/BardiaT94/Mochi" > /dev/null 2>&1 && echo "New version" || echo "New package")
gh pr create -R microsoft/winget-pkgs --head "BardiaT94:$BRANCH" --base master \
  --title "$NEW: $ID version $v" \
  --body "$NEW: Mochi $v ($URL), a file manager. Manifests written by hand to schema 1.9.0; the installer is NSIS, per-user."
