# Shared by run.sh and consumer.sh: gets a root made by make-root.sh ready and
# starts things inside it. Sourced with $T set to the testing directory and
# $ROOT to the root.

IN_ROOT=/opt/meeseeks-test     # where the test files go inside the root
RUNTIME_IN_ROOT=/run/meeseeks-test

if [ ! -x "$ROOT/usr/bin/pipewire" ]; then
  echo "$ROOT does not look like a root made by make-root.sh" >&2
  exit 1
fi
ROOT=$(cd "$ROOT" && pwd)
mkdir -p "$T/out"

# A runtime directory of our own, shared with the root. Under a short path:
# the address of a unix socket can only be about a hundred bytes long.
XDG_RUNTIME_DIR=$(mktemp -d /tmp/meeseeks-test.XXXXXX) || exit 1
export XDG_RUNTIME_DIR
unset WAYLAND_DISPLAY DISPLAY SWAYSOCK PIPEWIRE_REMOTE PIPEWIRE_RUNTIME_DIR

mounted=()
mount_into_root() {  # <type or --bind> <source> <path in the root>
  mountpoint -q "$ROOT$3" && return
  mkdir -p "$ROOT$3"
  if [ "$1" = --bind ]; then mount --bind "$2" "$ROOT$3"; else mount -t "$1" "$2" "$ROOT$3"; fi
  mounted+=("$ROOT$3")
}
mount_into_root proc proc /proc
mount_into_root --bind /dev /dev
mount_into_root --bind "$XDG_RUNTIME_DIR" "$RUNTIME_IN_ROOT"

SESSION=
SWAY=
leave_root() {
  # Everything started inside the root shares one process group.
  [ -n "$SESSION" ] && kill -TERM -- "-$SESSION" 2>/dev/null
  [ -n "$SWAY" ] && kill "$SWAY" 2>/dev/null
  sleep 0.5
  for path in "${mounted[@]}"; do
    umount "$path" 2>/dev/null || echo "could not unmount $path" >&2
  done
  case "$XDG_RUNTIME_DIR" in
    /tmp/meeseeks-test.*) rm -rf -- "$XDG_RUNTIME_DIR" ;;
  esac
}
trap leave_root EXIT

# The test files, and the test programs compiled against the root's PipeWire.
mkdir -p "$ROOT$IN_ROOT"
cp "$T/mock_portal.py" "$T"/newer-pipewire/*.py "$T"/producers/*.c "$ROOT$IN_ROOT/"
for source in "$T"/producers/*.c; do
  name=$(basename "$source" .c)
  chroot "$ROOT" /bin/sh -c "cc -O2 -Wall -o $IN_ROOT/$name $IN_ROOT/$name.c \$(pkg-config --cflags --libs libpipewire-0.3)"
done

# Runs a bash script, given as text, inside the root on a session bus of its
# own with PipeWire and WirePlumber up, in the background. Arguments of the
# form VAR=value before the script set its environment.
start_in_root() {
  local script=${*: -1}
  setsid chroot "$ROOT" /usr/bin/env XDG_RUNTIME_DIR="$RUNTIME_IN_ROOT" "${@:1:$#-1}" \
    dbus-run-session -- bash -c '
      cd '"$IN_ROOT"'
      pipewire > pipewire.log 2>&1 &
      sleep 1
      wireplumber > wireplumber.log 2>&1 &
      sleep 1.5
      '"$script" &
  SESSION=$!
}
