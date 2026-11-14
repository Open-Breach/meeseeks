#!/bin/bash
# Compiles the test programs in producers/ into build/.
# Needs a C compiler, pkg-config and the libpipewire-0.3 headers.
set -eu
T=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$T/build"
for source in "$T"/producers/*.c; do
  name=$(basename "$source" .c)
  # shellcheck disable=SC2046
  cc -O2 -Wall -o "$T/build/$name" "$source" $(pkg-config --cflags --libs libpipewire-0.3)
  echo "built build/$name"
done
