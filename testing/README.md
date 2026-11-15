# Test setups

Three ways to run the unmodified app where there is no GNOME or KDE session,
such as a container. The first two put stand-ins around it:

- a mock of `org.freedesktop.portal.ScreenCast` and of the part of
  `org.freedesktop.portal.RemoteDesktop` that places the pointer
  (`mock_portal.py`), which prints what the app asks of it,
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
libpipewire-0.3-dev gcc pkg-config dbus-daemon`.

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
| `tests/checks.sh <case>` | The startup checks. `ok`: no dialog, the main window. `no-portal`, `no-virtual`, `no-pipewire`, `x11`: the dialog with that check failing, the same list in the log, and exit after Close. `x11` runs the app on sway's Xwayland with the environment of an X11 session and needs `xwayland`; it is not an X11 desktop |
| `tests/failures.sh <case>` | A declined dialog, the window closed while the dialog is open, the stream taken away, a dead PipeWire remote. With `bad-fd` and `not-a-remote` the window freezes, which is a known limitation |
| `tests/resolution.sh 3440x1440` | The selector on an output of that size: in `out/resolution-idle.png`, the screen's size and which presets are greyed out. Then a preset and a custom size are each started; look for the producer's `negotiated size` lines. A size too large for the output brings the warning and a window scaled down to the room there is. After a restart the window has the size last started, from the settings file it prints. A second argument sets the output's scale |
| `tests/resize.sh [locked\|unlocked]` | Resizing before Start, with sway resizing the window on request. The fields follow the window (`out/resize-*-idle.png`), a size too large brings the warning (`out/resize-*-too-large.png`), and Start takes the size the window has: look for the producer's `negotiated size: 1100x750` and the picture's white edge in the corner pixels. While it runs the lock decides, read off the picture as in `window-size.sh`. After Stop the window is back at the size it started with and can be resized again |
| `tests/away.sh` | The window on its own virtual monitor. The mock portal makes no monitor, so a sway output created after Start stands in for it. On an output that was there before Start the picture stays; on the new one, and across its edge, the message takes its place (`out/away-on-3.png`, `out/away-half-on-3.png`); back on the first output the picture returns. The log says when the stream stops being drawn and when it is drawn again |
| `tests/window-size.sh 2560x1440 [locked\|unlocked]` | The area under the title bar is exactly 1920×1080 and the picture is drawn pixel for pixel. Of the eight pixels it reads, the first two are the picture's white edge, the next two its gradient, the next two the white edge again, and the last two lie just outside the picture. Then sway is asked to shrink the window: locked, the picture still runs on past the frame sway drew; unlocked, it is scaled into it |
| `tests/input.sh [stay\|warp\|nosize\|denied\|absent\|kde] [locked\|unlocked]` | The teleporter, with a virtual pointer for a mouse and the desktop said to be GNOME. In the portal log, look for the RemoteDesktop conversation: `RemoteDesktop.SelectDevices {'types': 2}`, the same `SelectSources` as ever, one `RemoteDesktop.Start`. After each step the test prints what the portal was sent: one position for each left click on the picture, and nothing for moving over it, for the right button, the wheel or a swipe. `stay`: the mock only prints the position, which no desktop does. `warp`: the mock puts sway's cursor on a second output, as a desktop does with its one cursor. `nosize`: Start gives no size for the stream, as GNOME's portal does for a virtual monitor, and every position is refused; on GNOME itself the app moves the pointer over libei instead, which the mock does not have, see "On a headless GNOME". The other three are where remote control cannot be had, and each still gives a monitor with one Start. `denied`: no pointer is granted, and a click sends nothing. `absent`: the portal has no RemoteDesktop interface; the conversation is the plain ScreenCast one, and the startup checks warn in the log. `kde`: the desktop says it is KDE, where the app does not ask for remote control; the plain conversation again. `unlocked` goes on to a scaled window, with bands beside the picture and above it |
| `tests/screenshot.sh [ok\|scaled\|away\|no-folder]` | The screenshot button, pressed with the keyboard. The Pictures folder is the XDG user directory, set for the run to a folder called `Bilder`. `ok`: a press, then two more within a second; the test lists the files in `Bilder/Meeseeks` with their size in pixels and four of their pixels, which are the test picture's: three files named after the date and time, the third with ` (2)`, each 1280×720, white at the corners and `(2, 4, 160)` where the gradient begins. `scaled`: the lock off and the window at half its size; the file is 1280×720 all the same. `away`: the window on an output made after Start, showing its message in place of the picture (`out/screenshot-away-message.png`); the file is the test picture all the same. `no-folder`: a file stands where the folder would go; look for the warning sign in `out/screenshot-no-folder-pressed.png`, the `could not be saved` warning in the log and "the app is still running". The `out/screenshot-*-{idle,running,pressed,later,stopped}.png` are the title bar at each stage: no button while idle, the camera, the tick, the camera again |
| `tests/options.sh` | The options dialog, worked with the keyboard. Cairo and the copy switch are saved, and the app starts again by itself: look for `GTK draws with GskCairoRenderer` and `trying: copy`. Reset to defaults, with a monitor running, closes its session and brings GL back, or Cairo with a warning where there is no GL. Then with Vulkan in the file: `GSK_RENDERER=cairo` and `MEESEEKS_TRY=copy` in the environment win, and `out/options-overridden.png` shows the dialog saying so; without them Vulkan is asked for, and where it cannot be initialised the log says what GTK fell back to |
| `tests/readme-shot.sh` | Not a test: it takes the README's screenshot, the window before Start with 1280×720 chosen on an output of 3440×1440, and writes it to `docs/screenshots/readme.png`, 1280×767. Run it with `APP=target/release/meeseeks`, once per release, and look at the picture before committing it |
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
testing/gnome/env.sh testing/gnome/tests/input.sh
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
| `gnome/tests/screenshot.sh` | The screenshot button on Mutter's own frames. A click on the picture sends the pointer to 320,180 of the monitor and the button is pressed: the file, kept as `out/gnome-screenshot-with-cursor.png`, is 1280×720 and has the cursor at that spot. With the mouse back on the screen the next file has none. The test prints the pixels around the spot for both. `GSK_RENDERER=gl` runs it with GTK drawing in GL, which reads the frame back from the graphics card, or from Mesa's stand-in for one |
| `gnome/tests/loop.sh` | Start, Stop, Start again, close the window while running. The dialog is GNOME's "Remote Desktop" (`out/gnome-loop-dialog.png`) and is answered as it comes, with Allow Remote Interaction off: a monitor and no pointer. Look for `Added virtual monitor` and `Removed virtual monitor` in Mutter's log for each start, the `buffer path` line and exit status 0 |
| `gnome/tests/input.sh [granted\|denied] [locked\|unlocked]` | The teleporter. The dialog is GNOME's "Remote Desktop" (`out/gnome-input-*-dialog.png`), with Allow Remote Interaction off as it comes. After each step the test prints what the app says it sent and what the window on the virtual monitor got. `granted`: the switch is turned on and the app moves the pointer over libei. Crossing the picture, the right button and the wheel send nothing. A left click on the picture puts the cursor on the monitor: its window gets `enter` at the very spot that was clicked, then the click and the wheel that follow, which come from the mouse itself now that the cursor is there. The step called the way out, run when the desktop is not scaled, sends the pointer to the monitor and then moves it as a real mouse does: off the monitor over its left edge, and on across the picture, where nothing happens. `denied`: the switch is left off, no pointer is granted, and the monitor is there all the same. `unlocked` tiles the window, so that the picture is scaled and has bands. With `SCREEN=3200x2000 SCALE=2`, and with `LAYOUT=logical` on top, the desktop is scaled; the monitor's window then counts in units of two pixels, half of what the app says it sent |
