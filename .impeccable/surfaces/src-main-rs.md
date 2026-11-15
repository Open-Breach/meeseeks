---
version: 1
slug: "src-main-rs"
primary_target: "src/main.rs"
related_targets: ["src/selector.rs","src/options.rs","src/checks_dialog.rs","src/style.rs","src/pictograms.rs"]
---

# Meeseeks window and dialogs

Scope: the main window (resolution selector, the picture, the away message), the options dialog with its restart question, and the startup-checks dialog. Visitor mode: Operate.

Audience and job: a Linux presenter on GNOME or KDE, seconds before a call or recording, who has to pick a size that fits their screen and press Start, then forget the window. Constraints: plain GTK 4 with GTK CSS, no libadwaita; every behaviour in `docs/startup-checks-and-options.md` stays, including the round Start button and Stop in the title bar; the icon is fixed.

Asked for by Serge, 2026-11-14: mock-ups in Figma first, no code until they are approved; layout and wording may change; keep the round button; the current look is "too plain and not very friendly".

Direction round, 2026-11-14: four sketches were shown (Wayfinding, Size chart, Grid specimen, Category standard). Serge first clicked Wayfinding, then wrote "did you ask me about the 4 previews you sent? in that case the last one", which is the category standard. That is the direction. The craft bar is GNOME's own apps (Settings, Files); this was assumed from the sketch he picked, not asked.

Mock-up review, 2026-11-14: Serge approved everything except the Start button, in his words "can we make the round button mutch cooler like more 3d look with visual feedback everithing els is greate". Start was redrawn as a raised push button; its states are on the Figma board "Start button · states and feedback".

Approval, 2026-11-14: on the reworked button Serge wrote "yes its aprooved designe it in its own branch". The mock-ups in Figma are the approved comps.

Status, 2026-11-15: built in GTK on the branch `claude/charming-cori-1w7kav` (`src/style.rs`, `src/selector.rs`, `src/main.rs`, `src/options.rs`, `src/checks_dialog.rs`, `src/pictograms.rs`). Mock-ups: https://www.figma.com/design/uXzAlU04u3O3OMb6bVaZK8, page "Redesign". The build has been run only in the repo's headless test desktop (sway, GTK 4.14, its default theme), never on GNOME or KDE.

Where the build knowingly differs from the mock-ups: the list takes its natural width instead of a fixed 560 px; the chosen preset keeps the weight of the others, so that choosing does not shift the row; the drop-down's arrow is the desktop theme's; the system font at relative sizes stands where the mock-ups have Inter.

## Direction contract

THESIS: Meeseeks is an ordinary, well-made desktop utility: it greets with its icon, asks one thing, and offers one round button. It refuses novelty; what it leaves behind is the unstyled window with small buttons adrift in empty space.

OWN-WORLD: The GNOME utility idiom, in plain GTK. A near-white window (#FAFAFB), one white rounded list card with hairline rows, linked segmented presets, and GNOME blue (#1C71D8) for the chosen preset, switches and the round Start. Red (#C01C28) only for Stop and failures, amber for warnings, green for passes. The system UI sans: one extra-bold title, regular body, grey captions. The one object with depth is Start: a glossy blue cap standing 8 px proud of a dark collar, like the button on the icon's box. It rises under the pointer, sinks when pressed and stays down, paler, while the desktop is asked.

STORY: A stranger sees the icon and the question, picks a size that fits, presses Start, and from then on sees only the picture.

FIRST VIEWPORT: The desktop's own header bar with the title and Options. A centred column 560 px wide: the app icon at 96 px, "Choose a size", the screen's size, the list card with rows 16:9, 4:3, Vertical and Custom, a line about greyed-out sizes, the round Start (a 96 px cap in a 116 px collar), and one line about the desktop's dialog.

FORM: The category standard, the round's standing exit, taken in Serge's words ("the last one"); seed:6c82f9c9e41c536f.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- How the window looks on GNOME and on KDE Plasma (Breeze); neither has been rendered. Under GTK's dark variant the title bar goes dark and the app's own surfaces stay light: there is no dark palette yet.
- The title bars of the dialogs are the desktop's and are not drawn in the test desktop, so they have not been seen with the new dialogs.
- The motion of the Start button (rise, sink, spring back) has been seen only as still frames. GTK drops style transitions when the desktop turns animations off; that has not been tried either.
