# Meeseeks — README Spec and Release Workflow

Written 2026-11-14. Source of truth for how Meeseeks is built and released, and for how the README is maintained. Modeled on Rick's `specs/readme.md`; differences are deliberate.

This spec lifts the "No CI, no packaging" rule in `CLAUDE.md` for exactly what is described here: one release workflow and one tarball. That line is updated as part of the work. Flatpak stays under "Later" in `SPEC.md`.

## 1. Release workflow

### Trigger

- A pushed tag `vX.Y.Z` on `main` cuts a release. Nothing else does; no manual dispatch.

### Runner

- GitHub Actions on a Depot runner: `runs-on: depot-ubuntu-24.04`. Ubuntu 24.04 is the floor because it ships GTK 4.14, which the DMA-BUF path of the GTK 4 sink needs at compile time. Older images are out.
- Cargo registry and `target` are cached between runs, so a release build is not a cold build.
- The Depot GitHub app must be enabled for this repository, or the job queues forever. Serge does that.

### Build

- One job, Linux x86_64 only.
- Build packages via `apt`: `libgtk-4-dev`, `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`, `libpipewire-0.3-dev`, and anything else the dev machine already needs for `cargo build`.
- `gtk4paintablesink` is linked statically: add the `gst-plugin-gtk4` crate as a dependency and register it at startup (`gstgtk4::plugin_register_static()`) before the startup checks run and before the pipeline is built. Enable the crate's GTK 4.14 and DMA-BUF features so the zero-copy path exists in the release binary; confirm the exact feature names against the crate version in use. Users then need no distro package for the sink. Ubuntu 24.04 has none, which is the reason.
- With the sink linked in, the startup check for `gtk4paintablesink` has nothing left to install: the check, its hint and the `no-sink` test case go.
- `cargo fmt --check`, `cargo clippy` clean, tests green, then `cargo build --release`.

### Artifact

- `meeseeks-vX.Y.Z-linux-x86_64.tar.gz` containing `meeseeks` (binary), `meeseeks.desktop`, `icons/meeseeks.svg` (from `assets/icon/meeseeks-icon.svg`), `LICENSE`, `README.md`.
- Layout follows Rick's release tarballs (`Open-Breach/rick`, public) so Rick's tarball installer installs and uninstalls it unchanged.
- The app has no application id yet. It gets one, and the desktop file matches it (file name or `StartupWMClass`), otherwise the dock shows a generic icon for the running window.
- Attached to a GitHub Release created for the tag. The release body is a short list of what changed; the README is the user-facing release note.

### Runtime requirements on the user's machine (from the system, not bundled)

- Wayland session on GNOME (Mutter) or KDE Plasma (KWin).
- xdg-desktop-portal with the GNOME or KDE backend: ScreenCast with the virtual source type; RemoteDesktop for teleporter mode.
- PipeWire with the GStreamer PipeWire plugin (`pipewiresrc`).
- GTK 4.14 or newer; GStreamer 1.x core and base; a glibc at least as new as the build image's.
- The startup checks cover all of these except the library floor in the last line: on an older GTK or glibc the binary does not load at all, so no dialog can say so. The README has to.

### Release sequence

1. The agent gets the release issue: bump the version, regenerate the README sections marked *regenerated* below, take the screenshots, commit to `main`.
2. Serge reviews, then pushes the tag.
3. CI builds the tarball from the tag and publishes the GitHub Release.

The README update is part of the release, not a follow-up PR. The README describes the latest release, not `main`.

## 2. README structure

Top to bottom:

0. Title; one status line (version + maturity); one line noting the README describes the latest release, not `main`; call to action: download the current version (links to Install), try it, tell us what breaks (links to issues), and share your opinion, good or bad, respectfully.
1. What it is — **fixed**
2. Why it exists — **fixed**
3. Features — **fixed**
4. Screenshot — **regenerated**
5. Requirements — **regenerated**
6. Install — **regenerated**
7. How to use it — **fixed**
8. AI-written code warning — **fixed**, copied verbatim from Rick's README
9. Join in — **fixed**, copied verbatim from Rick's README

Licence as a line at the bottom.

### Agent rules

- Fixed sections are written by Serge with Claude and change only when they no longer match reality. The agent does not reword, trim or "improve" them.
- Regenerated sections are rewritten every release from the actual state of that release.
- Tone: terse, no babbling, no fluff, but positive and exciting.

### What leaves the README

The current README's build-from-source instructions ("How to try") and "Testing without a desktop session" are for people working on the code, not for users. They move to `docs/building.md`, unchanged in substance, minus the sink package, which is no longer needed. Install links there in one line.

## 3. Fixed texts

### 1. What it is

Meeseeks is a virtual second monitor for Linux. The system sees a real display; you see a window on your screen. Move the apps you want to show into it, then share or record that monitor. Everything else stays off camera.

### 2. Why it exists

Remote work is everywhere now, and presenting in a meeting should be as little hassle as possible. I work on a big monitor; the people on the other end usually don't. That leaves two bad choices: share one window, or share the whole screen. Neither works for a product demo. You want to move between several windows, and sometimes the one you're demoing crashes. You restart it, and then you forget to share it again.

The same goes for screen recording: you want a fixed, sensible size and nothing else leaking into the video. Meeseeks gives you a second monitor you don't have to own. Put the demo on it, share it or record it, and carry on.

One caveat: Meeseeks reaches deep into the desktop, so it needs a few things in place and won't run everywhere. Check the requirements below before you try it.

### 3. Features

- A virtual monitor your desktop treats as real, drawn in a window on your screen.
- Resolution presets (1920×1080 by default, 1280×720, 2560×1440, 3840×2160, vertical 1080×1920, a few 4:3) and custom sizes.
- Lock the window size, or let the window scale while the monitor keeps its resolution.
- An options dialog for the renderer and other tuning switches.
- *(pending test)* Teleporter mode: click in the window and your pointer is on the virtual monitor.

### 7. How to use it

Start Meeseeks, pick a resolution and press Start. From that moment your system has one more monitor. It appears in your display settings like any other, usually placed to the right of your real screen, and it behaves exactly like a physical one: push the mouse past the right edge and you're on it. The only difference is that it's drawn inside the Meeseeks window on your real screen. Drag the windows you want to show onto it, or launch them there.

*(pending test)* Meeseeks also has a teleporter mode: click anywhere in the Meeseeks window and your pointer jumps to that spot on the virtual monitor. For it to work, allow remote control when the system asks. During a demo this keeps you from getting sidetracked when the pointer slips off the edge.

Sharing in a meeting: in Zoom, Meet, Teams or whatever you use, share a screen, not a window, and pick the virtual monitor. *(pending: one line naming the monitor as it appears in the picker, once the GNOME rename result is known.)* Everything on it goes out; everything else stays private. Switch between apps on it as you like, and if one of them crashes, the share stays up.

Recording: in OBS or your recorder, add the virtual monitor as a screen source. You get a video at exactly the resolution you picked, with nothing else in the frame.

Done? Press Stop. The monitor disappears and the desktop moves any windows left on it back to your real screen.

## 4. Regenerated sections — what the agent does each release

### 4. Screenshot

- The idle state: the window before Start, showing the resolution selector and the Start button. No stream running.
- Light and dark theme, two PNGs in `docs/screenshots/`, same window size every release. Shown with a `<picture>` element and `prefers-color-scheme` so it swaps with the reader's GitHub theme.
- Taken fresh on the release build. The headless setup in `testing/` (sway and grim) can take them; if the result does not look like the app on GNOME, say so and Serge takes the two shots.

### 5. Requirements

- One line per startup check, in the order the app runs them, with the minimum version where a check has one. Generated from the checks in the code so the README and the dialog cannot drift apart.
- One more line for the floor no check covers: GTK 4.14 and the build image's glibc, with the distribution releases that meet it (Ubuntu 24.04, Fedora 40, Debian 13 or newer; verify at release time).
- A "Tested on" line: the desktops and distro versions this release was actually run on. Serge supplies it; the agent asks if it is unknown.
- Closing line, verbatim: "Not sure? Just start Meeseeks. It checks each requirement and tells you what's missing."

### 6. Install

- Link to the release tarball; unpack; run `./meeseeks`. Written from the actual state of the release, not from this spec.
- One line: Rick's tarball installer installs and uninstalls it in one click.
- One line linking to `docs/building.md` for building from source.

## 5. Open items

- Teleporter mode: Serge tests it next. If it stays, the pending paragraph in *How to use it* and the pending feature line stay and the RemoteDesktop check joins the requirements; if not, all three go.
- Monitor name in pickers: once it is known whether the GNOME rename to "Meeseeks" worked, add the one line in *How to use it* ("it shows up as Meeseeks" or "it shows up under a generic name, MetaVendor on GNOME").
- Sections 8 and 9 are copied verbatim from Rick's README (`Open-Breach/rick`) when the README is first written.

## 6. Issue draft — "Release workflow and README"

Add the tag-triggered release workflow and write the README per `docs/readme-and-release.md`.

1. Link `gtk4paintablesink` statically (section 1, Build), register it before the startup checks, and log at startup that the static sink registered. Remove the sink's startup check, its hint and the `no-sink` test case.
2. Give the app an application id; add `meeseeks.desktop` matching it. The icon is already in the repo (`assets/icon/meeseeks-icon.svg`).
3. Add `.github/workflows/release.yml`: on tag `v*`, Depot Ubuntu 24.04 runner, build deps via apt, fmt/clippy/test, release build, tarball per section 1, GitHub Release with the tarball attached.
4. Move the build-from-source and headless-testing parts of the current README to `docs/building.md`.
5. Write `README.md` per sections 2–4: fixed texts verbatim, regenerated sections from the current code, pending items left marked as in the spec until Serge resolves them.
6. Take the two screenshots per section 4 and commit them to `docs/screenshots/`.
7. Update `CLAUDE.md`: the "No CI, no packaging" rule now allows what this spec describes and nothing more; add the new files to the layout.

Done when a tag push produces a release with a tarball that unpacks and runs on a clean GNOME system of the Ubuntu 24.04 generation, and the README matches this spec.

## 7. Agent prompts

### Set-up (now)

Read `CLAUDE.md` and `docs/readme-and-release.md`. This is preparation only: do not write code, workflow files or the README yet.

1. Open a GitHub issue titled "Release workflow and README" with the text of section 6 of the spec. If you cannot open issues, give me the text and I open it.
2. Check the spec against the repository and the crates as they are today, and report as a comment on the issue: anything in the spec that does not fit the code; the `gst-plugin-gtk4` version that matches the `gstreamer` and `gtk4` crates in `Cargo.toml`, with its exact feature names for GTK 4.14 and DMA-BUF; the application id you propose; the tarball layout you propose, checked against Rick's release tarball; whether the headless setup in `testing/` can take the two screenshots; and every question you have.
3. Stop there and wait for my go.

### Go (when Serge says so)

Implement the issue "Release workflow and README" per `docs/readme-and-release.md`, steps in order, in one PR. Do not reword the fixed texts; copy them verbatim. Where the spec says *pending*, keep the marker, do not resolve it yourself. Ask before changing the tarball layout. Small commits with plain messages; `cargo fmt` and `cargo clippy` clean before each one. You cannot run a Wayland session where you work: say exactly what you could not test.
