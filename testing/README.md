# Test setups

Two ways to run the unmodified app where there is no GNOME or KDE session, such
as a container. Both put stand-ins around it:

- a mock of `org.freedesktop.portal.ScreenCast` (`mock_portal.py`) that prints
  what the app asks of it,
- a test program in place of the compositor's virtual monitor (`producers/`),
- a private session bus, PipeWire, WirePlumber and a headless sway.

They show what the app does, not what a desktop does. There is no GPU, so
frames are shared memory and GTK draws with Cairo; the real portal, Mutter,
KWin and the DMA-BUF path are not exercised. The tests print what happened and
leave logs and screenshots in `out/`. Nothing passes or fails by itself: read
the output.

Nothing of the machine's own session is used. Each run makes its own runtime
directory under `/tmp` and removes it again.

## On the machine's own PipeWire

Needs, by their Ubuntu names: `sway wtype grim pipewire wireplumber
gstreamer1.0-pipewire gstreamer1.0-tools python3-dbus python3-gi
libpipewire-0.3-dev gcc pkg-config dbus-daemon`, and `gtk4paintablesink` (see
the end of this file if the distribution packages none).

```
cargo build
testing/build.sh
testing/env.sh testing/tests/loop.sh
```

`env.sh` brings the test desktop up, runs the script it is given inside it and
takes everything down again.

| Test | What it shows |
| --- | --- |
| `tests/loop.sh` | Start, resize, Stop, Start again, close the window while running. Look for the portal conversation (`SelectSources {'types': 4, 'multiple': 0, 'cursor_mode': 2}`), one `Session.Close` per session, the `buffer path` line and exit status 0. Screenshots of each stage are in `out/` |
| `tests/no-portal.sh` | Start with no portal on the bus: an error in the log, and the app keeps running |
| `tests/failures.sh <case>` | A declined dialog, the window closed while the dialog is open, the stream taken away, a missing sink, a dead PipeWire remote. With `bad-fd` and `not-a-remote` the window freezes, which is a known limitation |
| `tests/window-size.sh 2560x1440` | The window opens with the picture area at exactly 1920×1080 and the picture is drawn pixel for pixel. Of the eight pixels it reads, the first two are the picture's white edge, the next two its gradient, the next two the white edge again, and the last two lie just outside the picture |
| `tests/size-control.sh` | The control for that: a consumer that asks for no size gets 1280×720 from the same producer |
| `tests/frame-rate.sh <ms or none> <seconds>` | Frame rates against a producer set up like Mutter's virtual monitor, which renegotiates the stream that many milliseconds after the start. Look for about 60 frames a second in the app's log and no ticks without a free buffer in the producer's |

Variables: `APP` is the binary to test (default `target/debug/meeseeks`),
`PYTHON` a Python that has the distribution's `dbus` and `gi` modules (default
`python3`), and `GSK_RENDERER` defaults to `cairo`.

These were last run on Ubuntu 24.04 with PipeWire 1.0.5.

## In a root with a newer PipeWire

The stutter fixed in `src/display.rs` only shows with PipeWire 1.6; it is
reported upstream as
<https://gitlab.freedesktop.org/pipewire/pipewire/-/work_items/5586>. To have
that version next to an older system, `newer-pipewire/` builds an Ubuntu root
with `debootstrap` and runs the PipeWire side of the test inside it with
`chroot`. sway still runs on the machine itself.

This needs root and mounts `/proc`, `/dev` and a runtime directory into the
root while it runs, so it is for a container or a virtual machine. It also
needs `debootstrap`.

```
testing/newer-pipewire/make-root.sh /var/tmp/pw-root
testing/newer-pipewire/run.sh /var/tmp/pw-root 300 12
```

`run.sh <root> <ms or none> <seconds>` is `tests/frame-rate.sh` with the app
inside the root. With the fix it shows 60 frames a second; without the
`use_clock` line in `src/display.rs` it showed 14 to 18, with the producer out
of buffers on most ticks.

`consumer.sh <root> <ms or none> <seconds> <command>` runs the producer with
any consumer and no app. This is the reproduction in the upstream report, and
the way to see whether a PipeWire release has fixed it:

```
testing/newer-pipewire/consumer.sh /var/tmp/pw-root 400 11 \
  'gst-launch-1.0 pipewiresrc path=ID ! video/x-raw,width=1920,height=1080 ! fakesink'
```

On PipeWire 1.6.8 the producer gets 70 to 89 of 300 frames out per 5 seconds.
It gets about 300 with `provide-clock=false` on `pipewiresrc`, with
`python3 system-clock-consumer.py ID system-clock`, or with `./plain-consumer
ID`, a bare PipeWire client. `python3 clockwatch.py ID` prints how far
`pipewiresrc`'s clock falls behind.

These were last run with Ubuntu 26.10 in the root: PipeWire 1.6.8, GStreamer
1.28.7, GTK 4.24.1.

## The test programs

- `producers/picture-source.c`: a stream with a recognisable picture that
  takes the size the consumer asks for. The default producer.
- `producers/renegotiating-source.c`: a stream set up like Mutter's virtual
  monitor (it drives the graph, has a pool of 8 buffers, sends 60 frames a
  second), which can renegotiate after the start. It counts the ticks on which
  it had no free buffer. The same file is attached to the upstream report.
- `producers/plain-consumer.c`: a bare PipeWire consumer, to compare with
  `pipewiresrc`.

## Where the distribution has no `gtk4paintablesink`

Ubuntu 24.04 does not package it. Built from its crate, with the same Rust
toolchain as the app, and put where `lib.sh` looks for it:

```
cargo new --lib /tmp/sink && cd /tmp/sink
cargo add gst-plugin-gtk4@0.15 --features dmabuf,wayland
cargo build --release -p gst-plugin-gtk4
mkdir -p <repository>/testing/build/plugins
cp target/release/libgstgtk4.so <repository>/testing/build/plugins/
```
