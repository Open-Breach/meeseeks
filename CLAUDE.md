# Meeseeks

A Linux app that creates a virtual monitor through the XDG ScreenCast portal and shows it as a window on the real screen, so windows moved into it can be shared or recorded as a normal-sized full screen. Prototype stage: prove the loop, nothing more.

## Read first

- SPEC.md is the source of truth. Do not build what it rules out.

## Stack

- Rust, stable toolchain, edition 2024, single binary crate.
- ashpd for the portals; gtk4-rs for the window; gstreamer-rs with `pipewiresrc` and `gtk4paintablesink` for the picture.
- Wayland on GNOME and KDE only. No X11, no wlroots in this stage.

## Layout

- `src/session.rs` — the portal session: ScreenCast, source type Virtual, cursor mode Embedded, returns the PipeWire fd and node id. For the input experiment, the same from a RemoteDesktop session that also grants the pointer. No GTK or GStreamer imports.
- `src/display.rs` — the GStreamer pipeline and the paintable it exposes. No portal imports.
- `src/main.rs` — one GTK 4 window: the resolution selector while idle, the picture and a Stop button in the title bar while running. Runs the startup checks first.
- `src/selector.rs` — the resolution selector: presets, custom size, fit check, Start button.
- `src/settings.rs` — the settings file, `~/.config/meeseeks/settings.toml`. No GTK imports.
- `src/mirror.rs` — tells when the window is on its own virtual monitor, which is any monitor that appeared after Start.
- `src/options.rs` — the options dialog. Saving restarts the app.
- `src/environment.rs` — puts the renderer and the `MEESEEKS_TRY` switches from the settings into the environment before GTK starts, unless the variables are set already, and relaunches the app.
- `src/checks.rs` — the startup checks, one small function each. `src/checks_dialog.rs` — the dialog that lists them when one fails.
- `src/renderer.rs` — which GTK renderer is asked for, through GTK's environment variables.
- `src/input.rs` — experiment, off unless `MEESEEKS_INPUT` is `teleport`, `click` or `forward`: passes the pointer over the picture, or a click on it, on to the virtual monitor through the RemoteDesktop session. No portal imports.
- `src/pointer.rs` — the experiment's pointer: moves the desktop's cursor with the portal's Notify calls or, where those cannot work, over libei (`ConnectToEIS`, the `reis` crate). No GTK imports.

## Rules

- Keep the boundary: session knows nothing about rendering, display knows nothing about portals.
- On every start, log at info level which buffer path the pipeline negotiated: DMA-BUF or shared memory.
- No CI, no packaging, no settings, no input forwarding, no tray icon. Do not add them unprompted.
- `cargo fmt` and `cargo clippy` clean before every commit; small commits with plain messages.
- You cannot run a Wayland session where you work. Say exactly what you could not test; never claim it works.
- When the spec is silent, ask in the PR rather than inventing a feature.
