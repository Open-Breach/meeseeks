# Test setups

Three ways to run the unmodified app where there is no GNOME or KDE session,
such as a container. The first two put stand-ins around it:

- a mock of `org.freedesktop.portal.ScreenCast` and of the pointer calls of
  `org.freedesktop.portal.RemoteDesktop` (`mock_portal.py`) that prints what
  the app asks of it,
- a test program in place of the compositor's virtual monitor (`producers/`),
- a private session bus, PipeWire, WirePlumber and a headless sway.

They show what the app does, not what a desktop does. There is no GPU, so
frames are shared memory and GTK draws with Cairo; the real portal, Mutter,
KWin and the DMA-BUF path are not exercised. The tests print what happened and
leave logs and screenshots in `out/`. Nothing passes or fails by itself: read
the output.

The third, further down, has no stand-ins: a real Mutter and the real portal,
headless.

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
| `tests/loop.sh` | Start, Stop, Start again, close the window while running. Look for the portal conversation (`SelectSources {'types': 4, 'multiple': 0, 'cursor_mode': 2}`), one `Session.Close` per session, the `buffer path` line and exit status 0. Screenshots of each stage are in `out/` |
| `tests/checks.sh <case>` | The startup checks. `ok`: no dialog, the main window. `no-sink`, `no-portal`, `no-virtual`, `no-pipewire`, `x11`: the dialog with that check failing, the same list in the log, and exit after Close. `x11` runs the app on sway's Xwayland with the environment of an X11 session and needs `xwayland`; it is not an X11 desktop |
| `tests/failures.sh <case>` | A declined dialog, the window closed while the dialog is open, the stream taken away, a dead PipeWire remote. With `bad-fd` and `not-a-remote` the window freezes, which is a known limitation |
| `tests/resolution.sh 3440x1440` | The selector on an output of that size: in `out/resolution-idle.png`, the screen's size and which presets are greyed out. Then a preset and a custom size are each started; look for the producer's `negotiated size` lines. A size too large for the output brings the warning and a window scaled down to the room there is. After a restart the window has the size last started, from the settings file it prints. A second argument sets the output's scale |
| `tests/resize.sh [locked\|unlocked]` | Resizing before Start, with sway resizing the window on request. The fields follow the window (`out/resize-*-idle.png`), a size too large brings the warning (`out/resize-*-too-large.png`), and Start takes the size the window has: look for the producer's `negotiated size: 1100x750` and the picture's white edge in the corner pixels. While it runs the lock decides, read off the picture as in `window-size.sh`. After Stop the window is back at the size it started with and can be resized again |
| `tests/away.sh` | The window on its own virtual monitor. The mock portal makes no monitor, so a sway output created after Start stands in for it. On an output that was there before Start the picture stays; on the new one, and across its edge, the message takes its place (`out/away-on-3.png`, `out/away-half-on-3.png`); back on the first output the picture returns. The log says when the stream stops being drawn and when it is drawn again |
| `tests/window-size.sh 2560x1440 [locked\|unlocked]` | The area under the title bar is exactly 1920×1080 and the picture is drawn pixel for pixel. Of the eight pixels it reads, the first two are the picture's white edge, the next two its gradient, the next two the white edge again, and the last two lie just outside the picture. Then sway is asked to shrink the window: locked, the picture still runs on past the frame sway drew; unlocked, it is scaled into it |
| `tests/input.sh <teleport\|click\|forward> [stay\|warp\|echo\|nosize\|denied] [locked\|unlocked]` | The input experiment, `MEESEEKS_INPUT`, with a virtual pointer for a mouse. In the portal log, look for the RemoteDesktop conversation: `RemoteDesktop.SelectDevices {'types': 2}`, the same `SelectSources` as ever, one `RemoteDesktop.Start`. After each step the test prints what the portal was sent. `stay`: the mock only prints it, which no desktop does but shows all the app would pass on: one position for each time the pointer comes onto the picture with `teleport`; one for each click on the picture and none for moving over it with `click`; every position, button and scroll with `forward`. `warp`: the mock puts sway's cursor on a second output, as a desktop does with its one cursor, and either variant gets one position out before the pointer is gone. `echo`: the cursor stays and the mock has the mouse repeat every button and scroll it is sent, so the window gets each twice; `forward` must pass each on once (`not passed on` in the app log), not for ever. `nosize`: Start gives no size for the stream, as GNOME's portal does for a virtual monitor; every position is refused, and no button or scroll is passed on. On GNOME itself the app moves the pointer over libei instead, which the mock does not have; see "On a headless GNOME". `denied`: no pointer is granted and nothing is sent. `unlocked` goes on to a scaled window, with bands beside the picture and above it. The app log has one line for each stay of the pointer over the picture |
| `tests/options.sh` | The options dialog, worked with the keyboard. Cairo and both switches are saved, and the app starts again by itself: look for `GTK draws with GskCairoRenderer` and `trying: copy,srgb`. Reset to defaults, with a monitor running, closes its session and brings GL back, or Cairo with a warning where there is no GL. Then with Vulkan in the file: `GSK_RENDERER=cairo` and `MEESEEKS_TRY=srgb` in the environment win, and `out/options-overridden.png` shows the dialog saying so; without them Vulkan is asked for, and where it cannot be initialised the log says what GTK fell back to |
| `tests/size-control.sh` | The control for that: a consumer that asks for no size gets 1280×720 from the same producer |
| `tests/frame-rate.sh <ms or none> <seconds>` | Frame rates against a producer set up like Mutter's virtual monitor, which renegotiates the stream that many milliseconds after the start. Look for about 60 frames a second in the app's log and no ticks without a free buffer in the producer's |

The app's settings file is kept in the run's own directory, so every test starts from the defaults.

Variables: `APP` is the binary to test (default `target/debug/meeseeks`),
`PYTHON` a Python that has the distribution's `dbus` and `gi` modules (default
`python3`), and `GSK_RENDERER` defaults to `cairo`, except in `tests/options.sh`,
where the settings decide.

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

## On a headless GNOME

Mutter can run without a screen and without a GPU. `gnome/env.sh` brings up a
private one with one virtual monitor for a screen, and next to it the real
xdg-desktop-portal with its GNOME backend. The app then talks to the portal a
GNOME session has: the portal's own dialog comes up, and Mutter makes the
virtual monitor.

Needs, by their Ubuntu names, on top of the list above: `mutter
xdg-desktop-portal xdg-desktop-portal-gnome gir1.2-gtk-4.0
gstreamer1.0-plugins-base gstreamer1.0-plugins-good`. sway and the test
producers are not used.

```
cargo build
testing/gnome/env.sh testing/gnome/tests/loop.sh
testing/gnome/env.sh testing/gnome/tests/input.sh teleport
testing/gnome/env.sh testing/gnome/tests/input.sh click
```

Nobody sits at that desktop. `gnome/hands.py` works its keyboard and its
pointer through Mutter's remote desktop interface, and takes the screenshots
through its screen cast one. The portal's dialog is answered with the
dialog's own mnemonics. `gnome/landing.py` is a window that fills the virtual
monitor and prints where the pointer is on it.

It is the distribution's GNOME, not a session somebody works in. There is no
GPU, so frames are shared memory and GTK draws with Cairo. There is no
gnome-shell. The mouse is told where to be rather than moved, except where a
test says it goes as a real mouse does: `hands.py` then moves it by a distance
from where it is, which is also the way onto another monitor.

| Test | What it shows |
| --- | --- |
| `gnome/tests/loop.sh` | Start, Stop, Start again, close the window while running, without the input experiment. The dialog is GNOME's "Share Screen" (`out/gnome-loop-dialog.png`). Look for `Added virtual monitor` and `Removed virtual monitor` in Mutter's log for each start, the `buffer path` line and exit status 0 |
| `gnome/tests/input.sh <teleport\|click\|forward> [eis\|portal\|denied] [locked\|unlocked]` | The input experiment. The dialog is GNOME's "Remote Desktop" (`out/gnome-input-*-dialog.png`), with Allow Remote Interaction off as it comes. After each step the test prints what the app says it sent and what the window on the virtual monitor got. `eis`: the app chooses libei; the monitor's window gets `enter` at the very spot of the picture the pointer came onto, then the click and the wheel that follow, since the cursor is now there. With `click` the pointer first crosses the picture without a click, and nothing is sent. The step called the way out, run when the desktop is not scaled, sends the pointer to the monitor and then moves it as a real mouse does: off the monitor over its left edge, and on across the picture. With `teleport` that crossing sends it to the monitor again (`enter` a second time); with `click` nothing happens. `portal`: the app is told to use the portal's own calls, and each position comes back as `Invalid position`. `denied`: the switch is left off and no pointer is granted. `unlocked` tiles the window, so that the picture is scaled and has bands. With `SCREEN=3200x2000 SCALE=2`, and with `LAYOUT=logical` on top, the desktop is scaled; the monitor's window then counts in units of two pixels, half of what the app says it sent |

These were last run on Ubuntu 24.04 with Mutter 46.2, xdg-desktop-portal
1.18.4 and xdg-desktop-portal-gnome 46.2.

### On a newer GNOME than the machine has

`gnome/in-root.sh` runs the same tests inside the root that
`newer-pipewire/make-root.sh` makes, see above, with the repository mounted
into it. The app is the one built on the machine. The root needs GNOME on top
of what `make-root.sh` installs; the head of `in-root.sh` has the packages.

```
testing/gnome/in-root.sh /var/tmp/pw-root gnome/tests/loop.sh
testing/gnome/in-root.sh /var/tmp/pw-root gnome/tests/input.sh click
```

Last run with Ubuntu 26.10 in the root: Mutter 51.0, xdg-desktop-portal
1.21.1, xdg-desktop-portal-gnome 51.0, PipeWire 1.6.8.

## The test programs

- `producers/picture-source.c`: a stream with a recognisable picture that
  takes the size the consumer asks for. The default producer.
- `producers/renegotiating-source.c`: a stream set up like Mutter's virtual
  monitor (it drives the graph, has a pool of 8 buffers, sends 60 frames a
  second), which can renegotiate after the start. It counts the ticks on which
  it had no free buffer. The same file is attached to the upstream report.
- `producers/plain-consumer.c`: a bare PipeWire consumer, to compare with
  `pipewiresrc`.
- `pointer.py`: a mouse for the headless seat, which has none. It holds a
  virtual pointer and moves, clicks and scrolls as it is told on its standard
  input.
- `gnome/hands.py` and `gnome/landing.py`: see "On a headless GNOME".

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
