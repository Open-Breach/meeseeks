# Meeseeks — Prototype Spec

Meeseeks pops a virtual monitor into existence for one job — sharing or recording a sane-sized screen off an ultrawide — and vanishes the moment it's done. It is a small Linux app: the virtual monitor shows as a window on the real screen, and windows dragged into it can be shared or recorded as a full screen at a fixed size, independent of the physical display. Named after Mr. Meeseeks from Rick and Morty.

## Problem

Sharing or recording an ultrawide monitor gives viewers a squashed, unreadable picture. Sharing a single window avoids that but breaks the moment you switch apps, and if the shared window crashes the share dies with it. What's needed is a *screen* of a normal size that exists only for sharing, that real windows can live in, and that survives any one of them crashing.

## Goal of the prototype

Prove the loop end to end on GNOME and KDE: create the virtual monitor through the desktop portal, show its PipeWire stream in a window with zero-copy rendering, share that monitor from a browser meeting, and switch between two apps inside it. Nothing more.

## Scope

In scope:

- Create one virtual monitor via the XDG ScreenCast portal's virtual source type.
- Display the stream in a resizable window that keeps the monitor's aspect ratio.
- Cursor embedded in the stream so it is visible in the window.
- Start and stop; stopping closes the portal session so the monitor disappears and the desktop moves its windows back.

Out of scope for the prototype:

- Input forwarding of any kind. The user moves windows across with the mouse or Super+Shift+Arrow and works inside the virtual monitor by moving the cursor there. The cursor is seen in the window because it is embedded in the frames.
- Size presets, tray icon, settings, overlays.
- wlroots compositors (Sway, Hyprland), COSMIC, Cinnamon.
- X11 entirely — not now, not later.

## Target environments

| Environment | Status | Path |
| --- | --- | --- |
| GNOME (Mutter) | Prototype target | Portal, virtual source type. Monitor size follows the PipeWire format negotiation. |
| KDE Plasma (KWin) | Prototype target | Portal, virtual source type. KWin creates a 1920×1080 output at 1× scale; the user places it in the layout OSD. |
| wlroots (Sway, Hyprland, …) | Later | No virtual source in the portal; create a headless output through the compositor's own command, then capture it as an ordinary monitor. One small backend per compositor. |
| COSMIC | Later, if ever | Portal has ScreenCast but no virtual source yet; ordinary capture still unreliable. Would become the same portal call once supported. |
| Cinnamon / X11 | Out | X11; no virtual monitor machinery. |

The distribution underneath does not matter; only the compositor does.

## Architecture

Three modules with a hard boundary between the first two, so the display layer can be swapped without touching the session code.

1. **Session** — owns the portal conversation. Opens a ScreenCast session, selects a single source of type *virtual* with cursor mode *embedded*, starts it, receives the PipeWire node id, opens the PipeWire remote and hands back the file descriptor plus node id. Closes the session on stop. Knows nothing about rendering.
2. **Display** — owns one GStreamer pipeline and one widget. Takes a file descriptor and node id, builds `pipewiresrc` into `gtk4paintablesink`, and exposes the sink's paintable to the window. Requests 1920×1080 in the caps so GNOME sizes the monitor accordingly. Knows nothing about portals.
3. **Shell** — one GTK 4 window: a start/stop button and the picture. Nothing else.

## Technology

- **Rust** throughout.
- **ashpd** for the portals (ScreenCast; RemoteDesktop later if input forwarding is ever added).
- **gtk4-rs** for the window. Plain GTK 4, no libadwaita, so it looks acceptable on KDE as well.
- **gstreamer-rs** with `pipewiresrc` (gst-plugin-pipewire) and `gtk4paintablesink` (gst-plugins-rs). The sink imports PipeWire's DMA-BUF frames directly, and GTK's graphics offload can hand them straight to the compositor on Wayland — zero copy end to end.
- Zero copy needs recent versions: GTK 4.14 or newer and a gst-plugins-rs with DMA-BUF support in the GTK 4 sink. Older stacks fall back to a CPU copy silently; the prototype should log which path it got.

Why not egui: the UI is a button and a picture, and the cost that matters is moving frames, not drawing widgets. GTK with GStreamer gives the zero-copy path for free; egui would mean uploading every frame by hand.

## Behaviour

**Start.** The portal shows its picker; the user approves the virtual screen. The monitor appears in the desktop's display layout (GNOME places it; KDE shows a placement OSD). The window begins showing the stream.

**While running.** Drag any window past the edge into the virtual monitor, or send it with the keyboard. Work there as on any monitor; the cursor is drawn in the stream. Resize the viewer window freely; the picture keeps 16:9 and letterboxes.

**Stop, or window closed.** The session is closed, the monitor vanishes, and the desktop moves any windows left on it back to the real screen.

## Acceptance test

1. Start the app on GNOME; approve the portal dialog. The display settings show a second monitor.
2. Move a terminal and a browser into it; both appear in the window with the cursor.
3. In another browser, join a Meet or Zoom call and share a screen: the virtual monitor is offered and the viewer sees exactly the 1920×1080 picture.
4. Switch between the two apps while sharing; the share follows.
5. Kill the terminal; the share stays up.
6. Stop the app; the browser lands back on the real monitor.
7. Repeat steps 1–3 on a KDE session.

## Risks and open questions

- **Portal picker.** The portal shows a dialog on every start; a restore token can make it one-time on later runs.
- **Monitor size on GNOME** is whatever the caps negotiation settles on; confirm that requesting 1920×1080 actually yields that size and scale 1×.
- **HiDPI.** The real monitor may be scaled; the virtual one should stay at 1× so viewers get true 1080p.
- **Cursor wander.** With no input forwarding the cursor can slip onto the virtual monitor unintentionally at the screen edge. Acceptable for the prototype; the input design decides this later.
- **Zero-copy availability** depends on versions, see Technology. Measure CPU use with `top` while streaming 60 fps to know which path is active.

## Later, once the loop works

- Size presets (1080p, 1440p, 4:3) and a remembered restore token.
- Input forwarding via the RemoteDesktop portal, so clicking in the window acts in the virtual monitor.
- wlroots backends behind the Session boundary.
- Overlay widgets (e.g. a "live" border) stacked on the picture.
- Recording to file or a virtual camera by teeing the GStreamer pipeline.
- Flatpak on the GNOME runtime for distribution.
