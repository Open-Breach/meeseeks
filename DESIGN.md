---
name: Meeseeks
description: A virtual monitor shown as a window; an ordinary GTK 4 utility with one raised round button.
colors:
  accent: "#1C71D8"
  accent-hover: "#1A65C2"
  accent-active: "#1657A8"
  start-waiting: "#8DB6EB"
  collar-top: "#434C5C"
  collar-bottom: "#252C37"
  destructive: "#C01C28"
  destructive-hover: "#A91822"
  destructive-active: "#96141D"
  error-bg: "#FDEBEC"
  warning: "#8A5300"
  sign-amber: "#E5A50A"
  pass-green: "#1B8553"
  window-bg: "#FAFAFB"
  card-bg: "#FFFFFF"
  card-border: "#E0E0E3"
  row-line: "#EBEBED"
  control-bg: "#F4F4F5"
  control-hover: "#EDEDEF"
  control-active: "#E2E2E5"
  control-border: "#CFCFD3"
  switch-off: "#C4C4C8"
  text: "#2E3436"
  dim: "#5F6569"
  disabled: "#A8ABAE"
typography:
  title-1:
    fontFamily: "system-ui"
    fontSize: "160%"
    fontWeight: 800
  title-2:
    fontFamily: "system-ui"
    fontSize: "133%"
    fontWeight: 800
  start-word:
    fontFamily: "system-ui"
    fontSize: "130%"
    fontWeight: 700
  heading:
    fontFamily: "system-ui"
    fontSize: "100%"
    fontWeight: 700
  body:
    fontFamily: "system-ui"
    fontSize: "100%"
    fontWeight: 400
  control:
    fontFamily: "system-ui"
    fontSize: "100%"
    fontWeight: 500
  caption:
    fontFamily: "system-ui"
    fontSize: "87%"
    fontWeight: 400
rounded:
  mark: "2px"
  control: "8px"
  card: "12px"
  round: "9999px"
spacing:
  "2": "2px"
  "4": "4px"
  "8": "8px"
  "10": "10px"
  "12": "12px"
  "16": "16px"
  "20": "20px"
  "24": "24px"
components:
  page:
    backgroundColor: "{colors.window-bg}"
    textColor: "{colors.text}"
    padding: "24px"
  list-card:
    backgroundColor: "{colors.card-bg}"
    rounded: "{rounded.card}"
  list-row:
    padding: "11px 16px"
  list-row-failed:
    backgroundColor: "{colors.error-bg}"
    padding: "11px 16px"
  segment:
    backgroundColor: "{colors.card-bg}"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    padding: "7px 12px"
  segment-hover:
    backgroundColor: "{colors.control-hover}"
  segment-active:
    backgroundColor: "{colors.control-active}"
  segment-checked:
    backgroundColor: "{colors.accent}"
    textColor: "#FFFFFF"
  segment-checked-hover:
    backgroundColor: "{colors.accent-hover}"
  segment-disabled:
    backgroundColor: "{colors.control-bg}"
    textColor: "{colors.disabled}"
  spin-field:
    backgroundColor: "{colors.card-bg}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
  spin-field-button:
    backgroundColor: "{colors.control-bg}"
    textColor: "{colors.text}"
    width: "34px"
  start-collar:
    backgroundColor: "{colors.collar-bottom}"
    rounded: "{rounded.round}"
    size: "116px"
  start-cap:
    backgroundColor: "{colors.accent}"
    textColor: "#FFFFFF"
    typography: "{typography.start-word}"
    rounded: "{rounded.round}"
    size: "96px"
  start-cap-waiting:
    backgroundColor: "{colors.start-waiting}"
    textColor: "#FFFFFF"
  button:
    backgroundColor: "{colors.control-bg}"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "7px 16px"
  button-hover:
    backgroundColor: "{colors.control-hover}"
  button-active:
    backgroundColor: "{colors.control-active}"
  button-suggested:
    backgroundColor: "{colors.accent}"
    textColor: "#FFFFFF"
    rounded: "{rounded.control}"
    padding: "7px 16px"
  button-suggested-hover:
    backgroundColor: "{colors.accent-hover}"
  button-suggested-active:
    backgroundColor: "{colors.accent-active}"
  switch:
    backgroundColor: "{colors.switch-off}"
    rounded: "{rounded.round}"
    width: "46px"
    height: "26px"
  switch-checked:
    backgroundColor: "{colors.accent}"
  dropdown-button:
    backgroundColor: "{colors.control-bg}"
    textColor: "{colors.text}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "6px 12px"
  stop:
    backgroundColor: "{colors.destructive}"
    textColor: "#FFFFFF"
    padding: "5px 12px"
  stop-hover:
    backgroundColor: "{colors.destructive-hover}"
  stop-active:
    backgroundColor: "{colors.destructive-active}"
---

# Design System: Meeseeks

Recorded on 2026-11-15 from the build on branch `claude/charming-cori-1w7kav` (HEAD `aabad54`). Everything below is read from the shipped sources, not from the mock-ups.

**This is GTK 4 CSS, not web CSS.** PRODUCT.md's `## Platform` says `web` only because Impeccable has no value for a desktop toolkit. The tokens live in `src/style.rs`, a GTK style sheet in a Rust string: colours are GTK named colours (`@define-color mk_accent #1C71D8;`, used as `@mk_accent`), and every class the app styles is prefixed `mk-`. The frontmatter keys above are the named colours without the `mk_` prefix (`accent` is `@mk_accent`, `window-bg` is `@mk_window_bg`). Colours with no named colour behind them are marked as literals in the Colors section. Spacing is not in the style sheet at all: it is set on the widgets in Rust (`spacing`, `margin_*`). Selectors and values in this document are written as they stand in `src/style.rs`.

## Overview

**Creative North Star: "The ordinary utility with one button"**

Meeseeks looks like a well-made GNOME utility written in plain GTK: a near-white window, one white rounded card of hairline-separated rows, linked segmented presets, and a single blue for whatever is chosen or goes ahead. It greets with its icon, asks one thing ("Choose a size"), and offers one round button. Nothing in it is meant to be noticed twice; the user is seconds from a call and should forget the window once the picture is up.

The one object with depth is Start: a glossy blue cap standing 8 px proud of a dark collar, like the button on the box in the app's icon. It rises under the pointer, sinks when pressed, and stays down, paler, while the desktop is asked. Everything else is flat: fills, 1 px borders, and one faint shadow under the cards.

The app styles only what it marks with its own classes. The title bars, the Options button in the header bar, the drop-down's arrow and its pop-up list are the desktop theme's, on purpose, so that the window is at home on GNOME and on KDE. No libadwaita, and no font is shipped. (The name of the North Star is taken from the direction contract's thesis; the user has not been asked to confirm it as a name.)

**Key Characteristics:**
- Light surfaces only: window `#FAFAFB`, cards white, one blue (`#1C71D8`).
- One raised object, the Start button; all other controls are flat.
- Lists of rows on white cards with 12 px corners; controls with 8 px corners; round things fully round.
- The system UI font at relative sizes; weight, not a second face, makes the hierarchy.
- Colour for state is never alone: a failed row has a tint and a pictogram, a warning has a sign and words.
- The desktop theme keeps its title bars and its own widgets' chrome.

## Colors

A neutral, slightly cool light palette with GNOME blue as the only accent, and red, amber and green kept for state.

### Primary
- **GNOME blue** (`accent`, `@mk_accent`): the chosen preset (`.mk-segment:checked`), a switch that is on, the suggested button (`.mk-button.mk-suggested`), the Start cap's base colour, the default answer's text in the restart question, and every focus ring (2 px, or 4 px around the Start collar).
- **Blue, pointer over** (`accent-hover`, `@mk_accent_hover`): hover of the chosen preset and of the suggested button.
- **Blue, pressed** (`accent-active`, `@mk_accent_active`): the suggested button while pressed. The Start cap's side wall is the same value, written as a literal `#1657A8`.
- **Pale blue, waiting** (`start-waiting`, literal in `.mk-start:disabled > .mk-cap`): the Start cap held down while the desktop's dialog is open.

### Secondary
- **Collar slate** (`collar-top` to `collar-bottom`, literals in `.mk-collar`): the dark ring the Start cap sits in, a top-to-bottom gradient. Used nowhere else.

### Tertiary
- **Stop red** (`destructive`, `@mk_destructive`): the Stop button in the title bar and the disc of the "failed" pictogram (`RED` in `src/pictograms.rs`). Hover and pressed are the literals `destructive-hover` and `destructive-active` in `.mk-stop`.
- **Failure tint** (`error-bg`, `@mk_error_bg`): the background of a failed row (`.mk-row.mk-failed`).
- **Warning ink** (`warning`, `@mk_warning`): the words of a warning (`.mk-warning`): the custom size that does not fit, the note that an environment variable overrides a setting.
- **Sign amber** (`sign-amber`, `AMBER` in `src/pictograms.rs`): the warning triangle only. It carries a dark exclamation mark in `text`; it is never used for words.
- **Pass green** (`pass-green`, `GREEN` in `src/pictograms.rs`): the disc of the "passed" pictogram only.

### Neutral
- **Window ground** (`window-bg`): `.mk-page` and `.mk-dialog`, everything the app paints behind its content.
- **Card white** (`card-bg`): `.mk-list`, unchosen presets, the spin fields, the restart question.
- **Card border** (`card-border`) and **row line** (`row-line`): the 1 px outline of a card and the 1 px line between its rows.
- **Control ground** (`control-bg`), **hover** (`control-hover`), **pressed** (`control-active`), **border** (`control-border`): plain buttons, the drop-down, the spin field's − and + buttons, the outline and dividers of the segmented presets.
- **Switch off** (`switch-off`): the track of a switch that is off.
- **Text** (`text`): all primary text, and the mark on the warning sign.
- **Dim** (`dim`): row labels, descriptions, captions, the screen's size, the × between the fields, the "move back" pictogram.
- **Disabled** (`disabled`): the label of a preset that does not fit and of a spin button at its limit.

### Named Rules
**The One Blue Rule.** Blue means "chosen" or "goes ahead": the chosen preset, a switch that is on, Save, Start, the focus ring. It is not used for decoration, headings or links.

**The State Colours Rule.** Red is for Stop and for what failed, amber for warnings, green for what passed. Each comes with a pictogram or a word; none of them carries its meaning by colour alone.

## Typography

**Display Font:** the system UI font (Adwaita Sans on current GNOME, Noto Sans on KDE, Cantarell in the test desktop)
**Body Font:** the same
**Label/Mono Font:** none

**Character:** One face, the desktop's own, so the app reads like the desktop it runs on. No font is shipped and no family is named in the style sheet; sizes are percentages of the desktop's text size, so the app follows the user's font setting.

### Hierarchy
- **Title 1** (`.mk-title-1`: 800, 160 %): "Choose a size" on the selector and the away message "Move this window back to a physical screen." One per page.
- **Title 2** (`.mk-title-2`: 800, 133 %): the sentence at the top of the startup-checks dialog.
- **Start word** (`.mk-cap`: 700, 130 %, white, `text-shadow: 0 1px 1.5px alpha(black, 0.32)`): the word on the Start cap only.
- **Heading** (`.mk-heading`: 700, 100 %): the name of a group of options above its card ("Experimental").
- **Body** (400, 100 %): row titles, check names. Row labels on the selector are body in `dim`.
- **Control** (500, 100 %; 600 on the suggested button, Stop, and the default answer): labels on presets, buttons, the drop-down.
- **Caption** (`.mk-caption`: 400, 87 %): descriptions, hints, warnings, the lines under the list and under Start. Usually with `.mk-dim`, or `.mk-warning`. Wrapped at 48 characters in the options, 68 in the startup checks.

### Named Rules
**The System Font Rule.** The font is the desktop's. Sizes are relative (160 %, 133 %, 130 %, 87 %), never in pixels or points.

**The Still Row Rule.** The chosen preset keeps the weight of the others (500). Choosing changes colour only, so the row does not shift.

## Layout

One centred column. On the selector, a vertical box, centred both ways in the window, with 24 px margins and 24 px between its three parts: the head (the icon at 96 px, 12 px, then the title and the screen's size 4 px apart), the sizes (the list card, 10 px, the line about greyed-out sizes), and the go (the Start collar, 12 px, the hint). The column is as wide as the list card comes out, about 510 px at the usual font size; no width is fixed. The selector sits in a scrolled window only so that it asks for no room of its own: the window is as large as the chosen size, never smaller than 480×480.

Rows are horizontal boxes with 16 px between label and control (12 px between pictogram and text in the startup checks). On the selector the row label has a fixed width of 72 px so the controls line up. In the options a row is title, description and note stacked 2 px apart on the left, the control at the right, centred vertically.

Dialogs: 24 px margins, 20 px between groups in the options, 16 px in the startup checks; a heading sits 8 px above its card, indented 4 px. The buttons at the foot are 8 px apart; in the options Reset stands at the left and Cancel and Save at the right, in the startup checks both stand at the right.

The spacing steps in use are 2, 4, 8, 10, 12, 16, 20 and 24 px. They are set on the widgets in Rust, not in the style sheet. There are no breakpoints.

## Elevation & Depth

Flat, with one exception. Surfaces are told apart by fill and 1 px borders. Cards carry one faint shadow; the thumb of a switch has a small one. Start is the only raised object.

### Shadow Vocabulary
- **Card** (`box-shadow: 0 1px 3px alpha(black, 0.06)`): under every `.mk-list`. Barely there; the border does the work.
- **Switch thumb** (`box-shadow: 0 1px 2px alpha(black, 0.25)`): `.mk-dialog switch > slider`.
- **Collar** (`box-shadow: inset 0 0 0 1px alpha(white, 0.12), 0 2px 3px alpha(black, 0.22), 0 8px 16px alpha(black, 0.28)`): the Start collar's rim and the soft shadow it casts on the page.
- **Cap wall** (`0 8px 0 #1657A8` at rest, `0 11px 0 #1657A8` under the pointer, `0 2px 0 #124F9E` pressed): the side wall of the Start cap, drawn as a hard-edged shadow so that its foot stays put while the cap rises and sinks. It belongs to Start and to nothing else.
- **Cap lighting** (`inset 0 2px 1.5px alpha(white, 0.55), inset 0 -4px 9px alpha(black, 0.24)`): the lit top edge and shaded underside of the cap; when pressed, `inset 0 3px 7px alpha(black, 0.34), inset 0 -2px 5px alpha(black, 0.14)`.

### Named Rules
**The One Raised Thing Rule.** Start is the only object with depth. Gloss, a collar, a side wall and a cast shadow appear on it and nowhere else.

## Shapes

Three radii and the circle. Cards have 12 px corners (`.mk-list`); controls have 8 px (`.mk-segments`, `.mk-spin`, `.mk-button`, the drop-down, the bottom corners of the restart question's buttons); the white square in Stop has 2 px; Start, its collar and the switches are fully round (`9999px`).

Borders are 1 px and solid: `card-border` around cards, `row-line` between rows, `control-border` around and inside controls. Cards and segmented groups clip their children (`overflow: hidden`, set in Rust), so rows and presets follow the rounded corners. A group of presets is one outlined shape divided by 1 px lines; the single presets inside have no radius of their own.

Stop has no radius of its own: it keeps the desktop theme's button corners.

The pictograms are drawn with cairo in `src/pictograms.rs`, not taken from the icon theme, so they look the same on every desktop: 20 px marks (a white tick on a green disc, a white cross on a red disc, an amber triangle with a dark exclamation mark) with round-ended strokes of 1.6 to 1.8 px, one 64 px line drawing in `dim` with 3 px strokes (a screen with an arrow leaving it), and a 20 px camera in 1.6 px strokes. The camera is the one pictogram without a colour of its own: it sits in the title bar, which may be light or dark, and is drawn in the colour its button has for text. Each has an accessible name.

## Components

### Start button (the signature)
A fixed button holding a moving cap, inside a collar. Built in `src/selector.rs`.
- **Structure:** `.mk-collar` (a box, 116 px across) holds `.mk-start` (the `GtkButton`: 96×96, transparent, no border or outline; margins 2 px top, 10 px sides, 18 px bottom), which holds `.mk-cap` (a box with the word "Start"). The button is only the place to press and never moves; the cap is what is seen and what moves. Were the button to move, it would slip from under the pointer at its edges.
- **Collar:** `linear-gradient(#434C5C, #252C37)`, fully round, with the Collar shadow.
- **Cap at rest:** `@mk_accent` under two gradients: a soft white highlight (`radial-gradient(ellipse 29px 12px at 50% 19px, alpha(white, 0.5), alpha(white, 0.3) 78%, alpha(white, 0))`) and the body (`radial-gradient(circle 79px at 36% 18%, #6AB3F8, #2477DB 20%, #1C71D8 40%, #1860BC)`). The bright spot stays up and to the left of the word. The wall is 8 px.
- **Pointer over** (`.mk-start:hover > .mk-cap`): `translateY(-3px)`, `brightness(1.08)`, the wall grows to 11 px; 120 ms ease-out.
- **Pressed** (`:active`): `translateY(6px)`, the wall shrinks to 2 px, inner shadow, `brightness(0.9)`; 80 ms ease-out.
- **Waiting** (`:disabled`, while the desktop is asked): stays down at `translateY(6px)`, flat and pale (`#8DB6EB` with `radial-gradient(circle 79px at 38% 26%, #B4D2F5, #8DB6EB 60%, #7BA3D9)`, wall `0 2px 0 #6E8FBC`), the word at `alpha(white, 0.92)`. The hint under it changes to "Waiting for your desktop…".
- **Return to rest:** `transform` and `box-shadow` over 160 ms with `cubic-bezier(0.3, 1.5, 0.5, 1)` (a small overshoot), `filter` over 160 ms ease-out.
- **Keyboard focus:** a 4 px ring in `@mk_accent` around the collar (`.mk-collar.mk-focus`, replacing the collar's inset rim). The class is set from Rust when the button is `FOCUS_VISIBLE`, because the style sheet cannot tell the collar about the button's state.
- **Carried limit:** the white word has 4.5:1 against the cap at rest and about 4.0:1 under the pointer. At 130 % and weight 700 it counts as large text when the system font is 11 pt, but not when it is 10 pt.

### Segmented presets
The sizes of one aspect ratio joined into one control (`.mk-segments` holding `.mk-segment` toggle buttons).
- **Shape:** one 1 px `control-border` outline with 8 px corners; 1 px dividers; padding 7 px 12 px; weight 500.
- **States:** white at rest; `control-hover`; `control-active` while pressed; chosen is `accent` with white text (`accent-hover` under the pointer). A size that does not fit is `control-bg` with `disabled` text. The chosen preset stays blue while everything is disabled for the wait.
- **Focus:** a 2 px `accent` outline set 3 px inside the segment; white when the segment is the chosen one.
- All presets are one radio group, so a click on the chosen one leaves it chosen.

### List card and rows
- **Card** (`.mk-list`): white, 1 px `card-border`, 12 px corners, the Card shadow, children clipped.
- **Row** (`.mk-row`): padding 11 px 16 px, a 1 px `row-line` under every row but the last.
- **Failed row** (`.mk-row.mk-failed`): `error-bg`, with the red cross pictogram in front.
- While the desktop is asked the whole sizes block is at half strength (`.mk-sizes:disabled { opacity: 0.5; }`).

### Custom size fields
Two `GtkSpinButton`s (`.mk-spin`) with a dim × between them, 10 px apart.
- **Style:** white, 1 px `control-border`, 8 px corners; the − and + buttons are 34 px wide, `control-bg`, divided by 1 px lines, with hover and pressed as plain buttons.
- **Focus:** a 2 px `accent` outline on the whole field (`:focus-within`), set 1 px inside.
- **Does not fit:** a line under the fields, 8 px below: the warning sign and the reason in `.mk-warning .mk-caption`.
- **Notice:** the same sign and type under the Start button's hint, 12 px below, wrapping at 64 characters, for what the user has to know before the next Start; it goes when Start is pressed.

### Dialog buttons
- **Plain** (`.mk-button`): `control-bg`, 1 px `control-border`, 8 px corners, padding 7 px 16 px, weight 500; `control-hover`, `control-active`.
- **Suggested** (`.mk-button.mk-suggested`): `accent` fill and border, white text, weight 600; `accent-hover`, `accent-active`. One per dialog, the button that goes ahead (Save, Close).
- **Focus:** a 2 px `accent` outline, 2 px off the button.

### Switch and drop-down (options)
Styled only inside `.mk-dialog`.
- **Switch:** 46×26 track, fully round, `switch-off`, `accent` when on; a 22 px white thumb with a 2 px margin and the Switch thumb shadow; the theme's on/off glyph is hidden. Focus: 2 px `accent` outline, 2 px off.
- **Drop-down button:** as a plain button with padding 6 px 12 px. Its arrow and its pop-up list are the desktop theme's.
- **Option row:** title (body), description (dim caption), and, when an environment variable overrides the setting, a note 4 px below: the warning sign and `.mk-warning .mk-caption` text.

### Stop
In the desktop's header bar, at the start, shown only while running (`.mk-stop`).
- **Style:** `destructive`, white text at weight 600, padding 5 px 12 px, no border; a 10 px white square with 2 px corners (`.mk-stop-square`), 8 px, then the word "Stop".
- **States:** `destructive-hover`, `destructive-active`; focus is a 2 px `destructive` outline, 2 px off.

### Screenshot button
In the desktop's header bar, after Stop, shown only while running. Built in `src/screenshot.rs`.
- **Style:** a plain button of the desktop theme's, like Options, with nothing of the app's style sheet on it. Its face is the 20 px camera pictogram. Tooltip and accessible name: "Save a screenshot".
- **States:** insensitive until the first frame has arrived. After a save the face is the "passed" tick for 1.5 s, then the camera again; after a save that failed it is the warning sign for the same time. Nothing else moves and nothing pops up. Hover, pressed and focus are the theme's.

### Restart question
GTK's own alert (`GtkAlertDialog`), kept because it brings the desktop's behaviour with it (Escape, Return, the screen reader's alert role). The style sheet reaches it through `window.dialog.message`: white ground, white buttons with `control-hover` and `control-active`, the default answer in `accent` at weight 600, a 2 px `accent` focus outline set 2 px inside. Its layout is GTK's.

### Away message
Shown in place of the picture when the window sits on its own virtual monitor: on `.mk-page`, centred, the 64 px "move back" pictogram, 16 px, then the message in Title 1 and the reason in `dim`, 6 px apart, both centred and wrapped, with 24 px side margins.

## Do's and Don'ts

### Do:
- **Do** take colours from the named colours in `src/style.rs` (`@mk_*`) and prefix every new class with `mk-`.
- **Do** put groups of settings or results in a `.mk-list` card of `.mk-row` rows, and give a dialog exactly one `.mk-suggested` button.
- **Do** give every control the app restyles its own focus ring: 2 px `@mk_accent`, as the existing ones have.
- **Do** pair every state colour with a pictogram or words, as the failed row and the warnings do.
- **Do** draw new pictograms with cairo in `src/pictograms.rs`, with round-ended strokes and an accessible name, so they look the same under any icon theme.
- **Do** keep sizes relative to the system font (160 %, 133 %, 130 %, 87 %) and spacing on the steps in use (2, 4, 8, 10, 12, 16, 20, 24 px).
- **Do** keep the Start button's press target still and move only the cap inside it.

### Don't:
- **Don't** style the title bars or the theme's own widgets beyond what is listed here; the header bar, the Options button, the screenshot button and the drop-down's pop-up are the desktop's.
- **Don't** give a second object depth: no gloss, collar, side wall or large cast shadow on anything but Start.
- **Don't** use red for anything but Stop and failures, or blue for anything but what is chosen, what goes ahead, and focus.
- **Don't** ship or name a font, and don't set sizes in pixels or points.
- **Don't** use libadwaita; the app is plain GTK 4 so that it is at home on KDE as well as GNOME.
- **Don't** make the chosen preset bolder than the others; the row must not shift when the choice changes.

## Open Points

These are not rules. They are what the build has not settled or has not been seen doing.

- **Scope of the finish review.** Disposition: ship, for the headless test desktop only (sway, GTK 4.14 default theme, Cairo renderer, still frames; screenshots in `.impeccable/review/gtk/`). Not seen: GNOME, KDE Plasma (Breeze), the dialogs' title bars, and the Start button's motion (rise, sink, spring back). GTK drops style transitions when the desktop turns animations off; that has not been tried.
- **Dark desktops.** There is no dark palette. Under a dark desktop theme the title bar goes dark and the app's own surfaces stay light. Whether the app gets a dark palette or says it is light only is the user's decision, and is open.
- **The word on Start.** White on the cap has 4.5:1 at rest and about 4.0:1 under the pointer. It passes as large text at an 11 pt system font and not at 10 pt.
- **Where the colours live.** The cap's gradient stops, the collar's gradient, Stop's hover and pressed reds, and the pictograms' green and amber are literals in `src/style.rs` and `src/pictograms.rs`, not named colours. The pictograms repeat `destructive`, `text` and `dim` as RGB triples.
- **No measured comparison with the mock-ups.** The build was compared with the Figma comps by eye and by pixel samples of the Start button.
