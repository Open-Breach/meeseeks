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

If the system lacks something the app needs, a dialog lists what instead of the window; the same list goes to the terminal.

The window opens with a choice of sizes for the virtual monitor: presets, or a width and height of your own. Resizing the window chooses a size too: the width and height follow it. Presets that do not fit on your screen are greyed out. Press **Start** and approve the portal's dialog. The desktop should gain a second monitor of the chosen size, and the window shows what is on it, pixel for pixel. Move windows there with the mouse or Super+Shift+Arrow. **Stop** in the title bar, or closing the window, removes the monitor again.

A left click on the picture sends the mouse pointer to that spot of the virtual monitor, so there is no edge to find. The desktop has to allow that: on GNOME, switch **Allow Remote Interaction** on in the dialog at Start. It is off as it comes, and without it the monitor works all the same. The click only places the pointer and is not passed on; from there the mouse works on the virtual monitor as on any other and leaves it over its edge. Nothing is typed or clicked for you. On KDE Plasma the click does nothing yet.

Once the monitor has started the window cannot be resized, so that the picture stays unscaled. **Options** in the title bar can lift that lock; resizing then scales the picture and leaves the monitor as it is. The options also have the renderer GTK draws with (GL by default, Vulkan, or Cairo in software) and two experimental switches for how frames are taken. Saving them closes the app and starts it again, because they only take effect at startup.

If the window is moved onto its own virtual monitor it would show itself, over and over. It then shows a request to move it back to a physical screen instead, and the picture returns once it is back.

The options and the size last used are kept in `~/.config/meeseeks/settings.toml`. `GSK_RENDERER` and `MEESEEKS_TRY` in the environment win over the file.

On every start the terminal prints one `buffer path:` line: `DMA-BUF` means frames reach the window without a CPU copy, `shared memory` means each one is copied.

## Testing without a desktop session

[testing/](testing/README.md) has two setups that run the app against a mock portal and a test stream in a headless compositor, for a container or any machine with no GNOME or KDE session.

## License

Apache-2.0, see [LICENSE](LICENSE).
