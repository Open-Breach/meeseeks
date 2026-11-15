# Meeseeks — Screenshot Button

Written 2026-11-14. A small feature: one click saves what is on the virtual monitor as a picture.

## Behaviour

- A button in the title bar, next to Stop, shown only while the virtual monitor runs.
- A click saves the current frame of the virtual monitor as a PNG file.
- The picture is the monitor, not the window: the monitor's exact resolution whatever size the window is drawn at, with the cursor where the frame has it. It is the stream's frame also while the window shows the "move back" message.
- The frame comes from the stream the app already holds. No portal call, no dialog.
- A file only. Nothing goes to the clipboard.

## Where the file lands

- In the user's Pictures folder, in a folder `Meeseeks` that is created on first use.
- The Pictures folder is the XDG user directory, not a hard-coded name: it is localised on many systems. `~/Pictures` where none is set.
- File name: `Screenshot 2026-11-14 16-32-05.png`, date and time in local time, so the files sort by time.
- An existing file is never overwritten: a second shot in the same second gets ` (2)` before the extension.

## Feedback

- After a save the button shows a tick for about a second and a half, then its usual face again, as Rick's copy buttons do.
- The saved path is logged at info level.
- If the save fails (the folder cannot be made, the disk is full), the button shows the warning mark for the same time and the reason is logged at warn level. Nothing pops up and the app keeps running.
- The button is insensitive until the first frame has arrived.

## Look

- The title bar is the desktop theme's (`DESIGN.md`). The button is a plain theme button like Options, at the start of the bar, after Stop.
- Its face is a camera pictogram drawn with cairo in `src/pictograms.rs`, like the others, with an accessible name. Tooltip: "Save a screenshot".
- The button and its states are recorded in `DESIGN.md`.

## For the implementation

- It has to work on both buffer paths, DMA-BUF and shared memory. How the frame is read (the paintable's current image, the sink's last sample, a branch of the pipeline) is the implementer's choice; the PR says which, and why.
- Writing the file must not stall the picture: a 3840×2160 frame is encoded off the main thread if it takes noticeable time.
- Keep the boundary: `src/display.rs` hands out a frame and knows nothing of files or folders.

## Out of scope

The clipboard, a keyboard shortcut, a setting for the folder or the format, video, and a screenshot of the idle window.

## Acceptance

1. Start, click the button: `Pictures/Meeseeks/Screenshot <date> <time>.png` exists, has the monitor's resolution and shows what was on the monitor, the cursor included when it was there.
2. With the lock off and the window scaled down, the file still has the monitor's resolution.
3. The folder is made on first use. Two clicks within one second leave two files.
4. The button shows the tick and turns back. It is not there while idle.
5. With a Pictures folder that cannot be written to: the warning mark, a warn line in the log, the app keeps running.
6. On GNOME with DMA-BUF frames, the file's colours look like the picture in the window. Serge tests this one; it cannot be run headless.

## README

One line in the features list, already in `docs/readme-and-release.md`: "A screenshot button: the virtual monitor as a PNG at its exact resolution, saved to Pictures/Meeseeks."

## Issue draft — "Screenshot button"

Add a screenshot button per `docs/screenshot.md`.

1. `src/display.rs` hands out the current frame, on both buffer paths.
2. The button in the title bar after Stop, with its camera pictogram, the tick after a save and the warning mark after a failure.
3. The file: Pictures folder from the XDG user directories, the `Meeseeks` folder, the name with date and time, never overwriting.
4. A test in `testing/` that clicks the button against the mock portal and checks the file's name and size in pixels.
5. `DESIGN.md` and the layout in `CLAUDE.md` updated.

Done when the acceptance steps pass; step 6 is Serge's.

## Agent prompt

Read `CLAUDE.md` and `docs/screenshot.md`. On a new branch off `main`, after PR 5 is merged: open the issue "Screenshot button" with the text of the issue draft, then build it in one PR. Do not merge the PR; I test it on my desktop first. Small commits with plain messages; `cargo fmt` and `cargo clippy` clean before each one. You cannot run a Wayland session where you work: say exactly what you could not test, and which buffer path your tests ran on.
