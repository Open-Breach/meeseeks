#!/bin/bash
# Packs the release tarball (docs/readme-and-release.md), in the layout of
# Rick's release tarballs so that Rick's tarball installer takes it as it is:
#
#   meeseeks-X.Y.Z-linux-x64/
#     meeseeks
#     LICENSE  README.md  THIRD-PARTY-LICENSES.md
#     share/applications/meeseeks.desktop
#     share/icons/hicolor/scalable/apps/meeseeks.svg
#
# Usage: tarball.sh X.Y.Z, from the repository's root, after the release
# build and with THIRD-PARTY-LICENSES.md generated.
set -euo pipefail
version=${1:?usage: tarball.sh X.Y.Z}
name=meeseeks-$version-linux-x64
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

install -D -m 755 target/release/meeseeks "$stage/$name/meeseeks"
install -m 644 LICENSE README.md THIRD-PARTY-LICENSES.md "$stage/$name/"
install -D -m 644 packaging/meeseeks.desktop "$stage/$name/share/applications/meeseeks.desktop"
install -D -m 644 assets/icon/meeseeks-icon.svg \
  "$stage/$name/share/icons/hicolor/scalable/apps/meeseeks.svg"

# Nothing of the machine that packed it: no owner, and the files in a fixed order.
tar --create --gzip --file "$name.tar.gz" --directory "$stage" \
  --owner=0 --group=0 --numeric-owner --sort=name "$name"
echo "$name.tar.gz:"
tar --list --verbose --file "$name.tar.gz"
