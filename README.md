# Meeseeks

Meeseeks pops a virtual monitor into existence for one job — sharing or recording a sane-sized screen off an ultrawide — and vanishes the moment it's done. The virtual monitor shows as a window on your real screen; drag windows into it and share or record it as a normal 1920×1080 full screen, so you can switch apps while presenting and a crashing window doesn't take the share down with it. Linux only, Wayland on GNOME and KDE.

> **Prototype.** This is an experiment to prove the idea, not a product. The code is written by an AI agent from [SPEC.md](SPEC.md); use with caution.

## How to try

You need a Wayland session on GNOME or KDE Plasma and a current stable Rust toolchain (1.96 or newer; [rustup](https://rustup.rs) if your distribution's is older). From the distribution you need the GTK 4 and GStreamer development files to build, and two GStreamer plugins to run: the PipeWire one (`pipewiresrc`) and the GTK 4 sink from gst-plugins-rs (`gtk4paintablesink`).

Fedora:

```bash
sudo dnf install gcc gtk4-devel gstreamer1-devel pipewire-gstreamer gstreamer1-plugin-gtk4
```

Ubuntu 25.04 or newer:

```bash
sudo apt install build-essential pkg-config libgtk-4-dev libgstreamer1.0-dev gstreamer1.0-pipewire gstreamer1.0-gtk4
```

Ubuntu 24.04 has no package for the GTK 4 sink.

Then, from a checkout:

```bash
cargo run
```

Press **Start** and approve the portal's dialog. The desktop should gain a second, 1920×1080 monitor, and the window shows what is on it. Move windows there with the mouse or Super+Shift+Arrow. **Stop**, or closing the window, removes the monitor again.

On every start the terminal prints one `buffer path:` line: `DMA-BUF` means frames reach the window without a CPU copy, `shared memory` means each one is copied.

## Testing without a desktop session

[testing/](testing/README.md) has two setups that run the app against a mock portal and a test stream in a headless compositor, for a container or any machine with no GNOME or KDE session.

## License

Apache-2.0, see [LICENSE](LICENSE).
