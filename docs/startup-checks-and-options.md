# Meeseeks — Startup Checks, Resolution Selector and Options Dialog

Three features for the Meeseeks prototype: a compatibility check on startup, a resolution selector, and an options dialog.

> **Note, 2026-11-15.** This spec is left as it was written; two things in it no longer hold. The check for `gtk4paintablesink` was dropped: since the first release the sink is linked into the binary (`docs/readme-and-release.md`), because Ubuntu 24.04 has no package for it, so there is nothing left for a user to install and nothing to check. And of the experimental switches, `srgb` is gone, removed at the same time; `copy` is the one that remains.

## 1. Compatibility check on startup

### Goal

If the system lacks something Meeseeks needs, the user gets a clear dialog instead of a crash or a silent failure.

### Behaviour

- Run all checks at startup, before opening the portal session.
- If every check passes, show nothing and start normally.
- If any required check fails, show a GTK dialog with **one line per check**: a pass or fail mark, the check name, and for failures a short hint (for installable items, the likely package to install, e.g. for Fedora/Debian/Arch where known).
- The dialog has a Close button; closing it quits the app. Optionally a "Copy report" button that copies the results as text.
- Also print the same results to stderr, so runs from a terminal show them too.

### Checks (initial list)

1. **Wayland session**: `XDG_SESSION_TYPE=wayland` or `WAYLAND_DISPLAY` set. X11 is out of scope.
2. **Desktop**: GNOME (Mutter) or KDE (KWin), from `XDG_CURRENT_DESKTOP`. Other desktops: warn, not fail, if the remaining checks pass.
3. **PipeWire running**: can connect to the PipeWire daemon.
4. **xdg-desktop-portal present**: `org.freedesktop.portal.Desktop` reachable on the session bus.
5. **ScreenCast portal**: the ScreenCast interface exists and its `AvailableSourceTypes` includes VIRTUAL (bit value 4).
6. **GStreamer plugins**: elements `pipewiresrc` and `gtk4paintablesink` can be created (package hints: GStreamer PipeWire plugin; gst-plugins-rs GTK 4 sink).
7. **GTK 4 renderer available**: the renderer selected in settings (see section 3) can be initialised; if not, fall back to GL and report it.

### Notes

- Each check is a small function returning a name, a status (pass, warn or fail) and an optional hint, so new checks are easy to add.
- Keep the check code separate from the portal session and rendering code.

### Acceptance

1. On a GNOME Wayland system with everything installed: no dialog, app starts as before.
2. With the GTK 4 GStreamer sink removed (or simulated): the dialog shows that check failing, with a hint, and the app quits on Close.
3. In an X11 session: the Wayland check fails in the dialog.

## 2. Resolution selector

### Goal

When the virtual monitor isn't running, the big central area of the Meeseeks window shows a resolution picker instead of staying empty.

### Behaviour

- Show the physical screen's resolution at the top, e.g. "Your screen: 3440×1440".
- Presets, grouped by aspect ratio:
  - 16:9: 1280×720, 1920×1080 (default), 2560×1440, 3840×2160
  - 4:3: 1024×768, 1280×960, 1600×1200
  - Vertical: 1080×1920
- A custom width and height field below the presets.
- Remember the last used resolution (in the same settings file as the options) and preselect it next time.
- A round Start button, centred underneath the selector, that starts the virtual monitor at the chosen size.
- While running, a Stop button in the title bar stops the virtual monitor and brings back the resolution selector.

### Fit check

- Available space = the monitor's full width, and its height minus a fixed margin of about 80 to 100 px (top panel plus title bar). Make the margin a constant that is easy to tweak.
- Reason: GTK 4 on Wayland only reports the full monitor geometry, not the work area.
- Presets that don't fit the available space are grayed out and can't be picked.
- A custom size that doesn't fit shows a warning next to the fields.
- Use the monitor the Meeseeks window is on.

### Window size

- Choosing a resolution sizes the window's inner area (the stream area, excluding the title bar) to exactly that resolution, so the picture is shown 1:1.
- While the virtual monitor is not running, the window is freely resizable, whatever the "Lock window size" setting. Dragging it updates the custom width and height fields live to the size of the window's inner area, and switches the selection from a preset to Custom. The fit check still applies.
- Option "Lock window size", on by default: once the virtual monitor has started, the window can't be resized.
- With the lock off, resizing the window while the virtual monitor runs only scales the picture. The virtual monitor keeps its resolution.

### Monitor name

- On GNOME the virtual monitor shows as "MetaVendor" in the display settings.
- Check whether a client can set the vendor or product name to "Meeseeks": in the Mutter source, in the ScreenCast portal, and in the options of Mutter's own D-Bus interface.
- Implement it only if there is a supported way. Otherwise report in the PR what was found. No workarounds.

### Feedback loop

- The Meeseeks window can be dragged onto its own virtual monitor. The stream then shows the window, which shows the stream: a hall-of-mirrors loop.
- Detect when the window's surface enters the virtual monitor (GdkSurface enter-monitor, the monitor list).
- While it is there, stop drawing the stream and show a centred message: "Move this window back to a physical screen."
- Resume drawing as soon as the window is back on a physical monitor.
- Do not try to move or snap the window back.

### Acceptance

1. On start, the central area shows the screen's resolution, the presets and the custom fields.
2. On a 1440 px tall screen, 2560×1440, 3840×2160, 1600×1200 and 1080×1920 are grayed out.
3. Picking a preset or a valid custom size and pressing Start opens the virtual monitor at that size.
4. After a restart, the last used resolution is preselected.
5. After choosing 1920×1080, the stream area measures exactly 1920×1080. Once started, the window can't be resized; with the lock off, resizing scales the picture and the virtual monitor stays at 1920×1080.
6. The title-bar Stop button stops the virtual monitor and brings the selector back.
7. Before Start, the window can be resized with the lock on and with it off. While it is dragged, the custom fields follow the size of the stream area and no preset stays selected.
8. Dragging the window larger than the available space brings up the fit warning.
9. Pressing Start after a drag starts the virtual monitor at the dragged size, and the picture is shown 1:1.
10. Monitor name: if there is a supported way, GNOME's display settings show the virtual monitor as "Meeseeks". If there is none, nothing changes and the PR says what was found.
11. Dragging the running Meeseeks window onto the virtual monitor replaces the picture with the message, and no hall-of-mirrors loop builds up.
12. Dragging it back onto a physical monitor brings the picture back. The app never moves the window itself.

## 3. Options dialog

### Goal

Let users pick the renderer and the experimental switches themselves, for further testing and for systems where the defaults don't behave.

### Settings

- **Renderer**: GL (default), Vulkan, Cairo (software). Maps to `GSK_RENDERER` / `GDK_DISABLE` as the current code does.
- **MEESEEKS_TRY switches**: copy, srgb, as on/off toggles. (The reconnect switch was removed and is not offered.)
- **Lock window size** (see section 2).
- Room for more tuning parameters later.

### Behaviour

- Opened from the app (e.g. a menu or settings button on the Meeseeks window).
- Settings are saved to a config file, e.g. `~/.config/meeseeks/settings.toml`.
- Changing settings needs a restart: on Save, tell the user the app will close and restart, then close and relaunch it with the new settings applied before GTK initialises.
- Environment variables still win over the file, so existing test setups keep working.
- A "Reset to defaults" button.

### Acceptance

1. Switching the renderer to Vulkan in Options: the app restarts and runs with Vulkan; switching back to GL works the same way.
2. Toggling copy or srgb takes effect after the restart.
3. With `GSK_RENDERER` set in the environment, the environment value wins over the saved setting.
