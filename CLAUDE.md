# Meeseeks

A Linux app that creates a virtual monitor through the XDG ScreenCast portal and shows it as a window on the real screen, so windows moved into it can be shared or recorded as a normal-sized full screen. Prototype stage: prove the loop, nothing more.

## Read first

- SPEC.md is the source of truth. Do not build what it rules out.

## Stack

- Rust, stable toolchain, edition 2024, single binary crate.
- ashpd for the portals; gtk4-rs for the window; gstreamer-rs with `pipewiresrc` and `gtk4paintablesink` for the picture.
- Wayland on GNOME and KDE only. No X11, no wlroots in this stage.

## Layout

- `src/session.rs` — the portal session: ScreenCast, source type Virtual, cursor mode Embedded, returns the PipeWire fd and node id. On GNOME it is a RemoteDesktop session with that screen cast in it, which also grants the pointer if the user allows it; one dialog either way. No GTK or GStreamer imports.
- `src/display.rs` — the GStreamer pipeline and the paintable it exposes; hands out the current frame as a texture. No portal imports, and nothing of files or folders.
- `src/main.rs` — one GTK 4 window: the resolution selector while idle, the picture with a Stop button and the screenshot button in the title bar while running. Runs the startup checks first.
- `src/selector.rs` — the resolution selector: presets, custom size, fit check, Start button.
- `src/style.rs` — the app's GTK style sheet: the colours, the lists on white cards, the linked presets, the raised Start button. It styles only widgets marked with the app's own `mk-` classes; title bars stay the desktop theme's.
- `src/pictograms.rs` — the pictograms the app draws itself: the marks of the startup checks, the warning sign, the move-back sign, the screenshot button's camera.
- `src/screenshot.rs` — the screenshot button and the file it saves: a PNG in `Meeseeks` in the user's Pictures folder, named after the date and time. No pipeline or portal imports.
- `src/settings.rs` — the settings file, `~/.config/meeseeks/settings.toml`. No GTK imports.
- `src/mirror.rs` — tells when the window is on its own virtual monitor, which is any monitor that appeared after Start.
- `src/options.rs` — the options dialog. Saving restarts the app.
- `src/environment.rs` — puts the renderer and the `MEESEEKS_TRY` switches from the settings into the environment before GTK starts, unless the variables are set already, and relaunches the app.
- `src/checks.rs` — the startup checks, one small function each. `src/checks_dialog.rs` — the dialog that lists them when one fails.
- `src/renderer.rs` — which GTK renderer is asked for, through GTK's environment variables.
- `src/input.rs` — the teleporter: a left click on the picture sends the pointer to that spot of the virtual monitor. Nothing else is passed on. No portal imports.
- `src/pointer.rs` — the pointer a RemoteDesktop session grants. It is sent positions only, over libei (`ConnectToEIS`, the `reis` crate) or, where the portal gave a size for the stream, with the portal's own call. No GTK imports.
- `src/desktop.rs` — which desktop this is, from `XDG_CURRENT_DESKTOP`, and whether the teleporter is asked for on it. No GTK or portal imports.

## Rules

- Keep the boundary: session knows nothing about rendering, display knows nothing about portals.
- On every start, log at info level which buffer path the pipeline negotiated: DMA-BUF or shared memory.
- No CI, no packaging, no settings, no tray icon. Do not add them unprompted.
- Input is one thing only: a left click on the picture places the pointer on the virtual monitor. Pointer only, no keyboard; the click, the other buttons and the wheel are not passed on. Do not add to it unprompted.
- `cargo fmt` and `cargo clippy` clean before every commit; small commits with plain messages.
- You cannot run a Wayland session where you work. Say exactly what you could not test; never claim it works.
- When the spec is silent, ask in the PR rather than inventing a feature.
