# Meeseeks

A Linux app that creates a virtual monitor through the XDG ScreenCast portal and shows it as a window on the real screen, so windows moved into it can be shared or recorded as a normal-sized full screen. Prototype stage: prove the loop, nothing more.

## Read first

- SPEC.md is the source of truth. Do not build what it rules out.

## Stack

- Rust, stable toolchain, edition 2024, single binary crate.
- ashpd for the portals; gtk4-rs for the window; gstreamer-rs with `pipewiresrc` and `gtk4paintablesink` for the picture.
- Wayland on GNOME and KDE only. No X11, no wlroots in this stage.

## Layout

- `src/session.rs` — the portal session: ScreenCast, source type Virtual, cursor mode Embedded, returns the PipeWire fd and node id. No GTK or GStreamer imports.
- `src/display.rs` — the GStreamer pipeline and the paintable it exposes. No portal imports.
- `src/main.rs` — one GTK 4 window: Start/Stop button and the picture.

## Rules

- Keep the boundary: session knows nothing about rendering, display knows nothing about portals.
- On every start, log at info level which buffer path the pipeline negotiated: DMA-BUF or shared memory.
- No CI, no packaging, no settings, no input forwarding, no tray icon. Do not add them unprompted.
- `cargo fmt` and `cargo clippy` clean before every commit; small commits with plain messages.
- You cannot run a Wayland session where you work. Say exactly what you could not test; never claim it works.
- When the spec is silent, ask in the PR rather than inventing a feature.
