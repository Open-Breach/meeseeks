# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Linux users who present: people on GNOME or KDE Plasma, under Wayland, with an ultrawide or otherwise large screen, who share that screen in video calls or record it. Meeseeks is an open-source tool meant to be picked up by strangers, not only by its author. (Confirmed by Serge, 2026-11-14.)

Their situation: about to join a call or start a recording, or already in one. Their job: give viewers a screen of a normal size that keeps working when they switch apps and when one of those apps crashes.

## Product Purpose

Meeseeks pops a virtual monitor into existence for one job, sharing or recording a sane-sized screen off a large one, and vanishes the moment it is done. The virtual monitor shows as a window on the real screen. Windows dragged into it can be shared or recorded as a full screen of a fixed size, independent of the physical display.

Success, from SPEC.md's acceptance test: the viewer of a shared call sees exactly the picture of the chosen size; the share follows when the presenter switches apps; it stays up when one app is killed; stopping moves the windows back to the real screen.

## Positioning

A real extra screen that exists only for sharing. Sharing the whole ultrawide gives viewers a squashed, unreadable picture. Sharing a single window breaks when the presenter switches apps and dies with the window. Meeseeks gives a monitor of a normal size that real windows live in and that survives any one of them crashing.

It is created through the desktop's own ScreenCast portal (virtual source type), and it disappears when the app stops.

## Operating Context

- **Start.** The user picks a size and presses Start. The desktop portal shows its own dialog on every start; the user approves the virtual screen. The monitor then appears in the desktop's display layout: GNOME places it, KDE shows a placement prompt.
- **While running.** The user drags windows past the screen edge into the virtual monitor, or sends them with Super+Shift+Arrow, and works there as on any monitor. The cursor is drawn into the picture. The Meeseeks window only shows the monitor; clicks in it do nothing there (no input forwarding).
- **The share itself happens elsewhere.** In a browser call (Meet, Zoom) or a recorder, the user picks the virtual monitor as the screen to share. Meeseeks runs beside that, while the user's attention is on the call.
- **Stop, or window closed.** The portal session closes, the monitor vanishes, and the desktop moves any windows left on it back to the real screen.
- **Run from a terminal today.** `cargo run` from a checkout; the terminal prints one `buffer path:` line per start. There is no installer or package.

## Capabilities and Constraints

**The interface is GTK 4, not a web page.** It is written in Rust with gtk4-rs: GTK widgets, styled where needed with GTK CSS. `## Platform` says `web` only because Impeccable's record has no value for a desktop toolkit and reads any other value as `web` with a warning. Browser-based tooling does not apply here: no HTML or CSS detector runs, no dev server, no live mode, no mobile breakpoints. Design work is done by editing the Rust sources and looking at the running window.

**Where the interface lives**

- On `main`: one window with a Start/Stop button and the picture, fixed at 1920×1080.
- In open pull request #4 (not merged; as of 2026-11-14 not yet run on a GNOME or KDE session): a startup-check dialog listing what the system lacks, with hints; a resolution selector (presets by aspect ratio, custom width and height, a fit check against the screen, a round Start button, Stop in the title bar); an options dialog (renderer, two experimental switches, lock window size); and a message shown in place of the picture when the window sits on its own virtual monitor.
- Specs: `SPEC.md` and `docs/startup-checks-and-options.md`.

**Technical constraints**

- Wayland only, on GNOME (Mutter) and KDE Plasma (KWin). X11 is out for good. wlroots compositors and COSMIC are later, if ever.
- The picture is shown pixel for pixel at the chosen size; the window is locked to that size while running unless the user lifts the lock.
- Out of scope for the prototype: input forwarding, a tray icon, overlays on the picture, packaging.
- The virtual monitor's name in GNOME's display settings cannot be set by a client; it shows as "MetaVendor".
- A real GNOME or KDE session is needed to see the app as users do. `testing/` runs it headless against a mock portal for everything else.

**Terms the app and docs use:** virtual monitor, the picture, Start, Stop, presets, Custom, buffer path (DMA-BUF or shared memory), renderer.

**Status:** a prototype. The README says so, and that the code is written by an AI agent from the spec.

**Undecided:** whether graphics offload becomes the default (it changes the colour curve GTK assumes); how the app will be distributed (Flatpak is on SPEC.md's "later" list); a restore token to skip the portal dialog on later starts.

## Brand Commitments

Confirmed as fixed by Serge, 2026-11-14:

- **The name, Meeseeks.** After Mr. Meeseeks from Rick and Morty: it appears for one job and vanishes when the job is done.
- **The icon.** A blue box with a red button on top and a dark screen on its front. `assets/icon/meeseeks-icon.svg` and `.png` (1024×1024), the Blender source `assets/icon/meeseeks-icon.blend`, and four rendered angles in `assets/icon/angles/`.
- **Plain GTK 4, no libadwaita,** so the window looks at home on KDE as well as GNOME.

Observed in the repo, not confirmed as binding: the wording in the app and docs is plain and direct, in full sentences ("Move this window back to a physical screen.", "This system lacks something Meeseeks needs.").

## Evidence on Hand

- `SPEC.md`: problem, scope, behaviour, acceptance test.
- `docs/startup-checks-and-options.md`: the startup checks, the resolution selector and the options dialog.
- `README.md`: what it is, how to build and try it.
- `assets/icon/`: the icon in SVG, PNG and Blender form, and four angle renders.
- `testing/`: a headless setup (sway, a mock portal, a test stream) that can start the app and take screenshots of its window without a GNOME or KDE session.
- Pull requests #2 and #4 on GitHub: what was run, on what, and what was not.
- One machine has run it on real GNOME (Fedora 45, Mutter 51.1): the monitor appears, the picture shows, and it holds about 60 frames a second. KDE has not been run.

Absent, and not to be made up: screenshots of the app in the repo, a website, a wordmark or logo beyond the icon, user numbers, testimonials, benchmarks, artwork from the show the name refers to.

## Product Principles

1. **One job, then gone.** Quick to start, clean to stop, nothing left behind on the desktop.
2. **The picture is the product.** It is shown as viewers will see it; everything else in the window stays out of its way.
3. **A stranger must succeed without a manual.** When something is missing or a choice will not work, the app says what and what to do about it, before the user is in a call.
4. **At home on both desktops.** GNOME and KDE users should each find the window ordinary.
5. **Say what is true.** It is a prototype; what has and has not been tested is stated plainly.
