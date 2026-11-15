#!/bin/bash
# Stops a release that is not what its tag says (docs/readme-and-release.md).
# Usage: check-release.sh vX.Y.Z, in a checkout of the tagged commit.
#
# The tagged commit has to be on main, and the tag, Cargo.toml and the README
# have to name the same version: the README in the tarball describes this
# release, in its status line and in the link to the tarball.
set -euo pipefail
tag=${1:?usage: check-release.sh vX.Y.Z}
version=${tag#v}
stop() { echo "no release: $*" >&2; exit 1; }

[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || stop "the tag $tag is not of the form vX.Y.Z"

git fetch --quiet origin main
git merge-base --is-ancestor HEAD FETCH_HEAD \
  || stop "the tagged commit $(git rev-parse --short HEAD) is not on main"

in_cargo=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
[ "$in_cargo" = "$version" ] || stop "the tag says $version, Cargo.toml says $in_cargo"

# The status line is the one after the marker.
status=$(sed -n '/^<!-- generated:status -->$/{n;p;q}' README.md)
[[ $status == "$version, "* ]] \
  || stop "the tag says $version, the README's status line says: ${status:-nothing}"

tarball=releases/download/$tag/meeseeks-$version-linux-x64.tar.gz
grep -q -F "$tarball" README.md || stop "the README does not link to $tarball"

echo "release $version: the commit is on main; Cargo.toml and the README agree with the tag"
