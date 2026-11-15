//! The app's style sheet: its colours, the lists on white cards, the linked
//! presets and the raised Start button. It styles only the widgets the app
//! marks with its own classes; the title bars stay the desktop theme's.

const CSS: &str = r#"
@define-color mk_window_bg #FAFAFB;
@define-color mk_card_bg #FFFFFF;
@define-color mk_card_border #E0E0E3;
@define-color mk_row_line #EBEBED;
@define-color mk_control_bg #F4F4F5;
@define-color mk_control_hover #EDEDEF;
@define-color mk_control_active #E2E2E5;
@define-color mk_control_border #CFCFD3;
@define-color mk_switch_off #C4C4C8;
@define-color mk_text #2E3436;
@define-color mk_dim #5F6569;
@define-color mk_disabled #A8ABAE;
@define-color mk_accent #1C71D8;
@define-color mk_accent_hover #1A65C2;
@define-color mk_accent_active #1657A8;
@define-color mk_destructive #C01C28;
@define-color mk_warning #8A5300;
@define-color mk_error_bg #FDEBEC;

/* What the app paints itself: the selector, the away message, the dialogs. */
.mk-page,
.mk-dialog {
  background-color: @mk_window_bg;
  color: @mk_text;
}

.mk-title-1 { font-size: 160%; font-weight: 800; }
.mk-title-2 { font-size: 133%; font-weight: 800; }
.mk-heading { font-weight: 700; }
.mk-dim { color: @mk_dim; }
.mk-caption { font-size: 87%; }
.mk-warning { color: @mk_warning; }

/* A list of rows on a white card. */
.mk-list {
  background-color: @mk_card_bg;
  border: 1px solid @mk_card_border;
  border-radius: 12px;
  box-shadow: 0 1px 3px alpha(black, 0.06);
}
.mk-row { padding: 11px 16px; }
.mk-row:not(:last-child) { border-bottom: 1px solid @mk_row_line; }
.mk-row.mk-failed { background-color: @mk_error_bg; }

/* The presets of one aspect ratio, joined into one control. */
.mk-segments {
  border: 1px solid @mk_control_border;
  border-radius: 8px;
}
.mk-segment {
  margin: 0;
  padding: 7px 12px;
  min-width: 0;
  min-height: 0;
  border: none;
  border-radius: 0;
  box-shadow: none;
  text-shadow: none;
  background-image: none;
  background-color: @mk_card_bg;
  color: @mk_text;
  font-weight: 500;
  outline: 0 solid transparent;
  outline-offset: -3px;
}
.mk-segment:not(:first-child) { border-left: 1px solid @mk_control_border; }
.mk-segment:hover { background-color: @mk_control_hover; }
.mk-segment:active { background-color: @mk_control_active; }
.mk-segment:checked { background-color: @mk_accent; color: white; }
.mk-segment:checked:hover { background-color: @mk_accent_hover; }
.mk-segment:disabled { background-color: @mk_control_bg; color: @mk_disabled; }
/* The chosen preset stays the chosen one while everything waits. */
.mk-segment:checked:disabled { background-color: @mk_accent; color: white; }
.mk-segment:focus-visible { outline: 2px solid alpha(@mk_accent, 0.7); }
.mk-segment:checked:focus-visible { outline-color: white; }

/* The two fields of the custom size. */
.mk-spin {
  background-color: @mk_card_bg;
  color: @mk_text;
  border: 1px solid @mk_control_border;
  border-radius: 8px;
  box-shadow: none;
  outline: 0 solid transparent;
  outline-offset: -1px;
}
.mk-spin:focus-within { outline: 2px solid alpha(@mk_accent, 0.7); }
.mk-spin > button {
  margin: 0;
  min-width: 34px;
  border: none;
  border-left: 1px solid @mk_control_border;
  border-radius: 0;
  box-shadow: none;
  background-image: none;
  background-color: @mk_control_bg;
  color: @mk_text;
}
.mk-spin > button:hover { background-color: @mk_control_hover; }
.mk-spin > button:active { background-color: @mk_control_active; }
.mk-spin > button:disabled { color: @mk_disabled; }
.mk-spin > button:last-child { border-radius: 0 7px 7px 0; }

.mk-sizes:disabled { opacity: 0.5; }

/* The Start button: a glossy cap standing proud of a dark collar. The cap's
   side wall is the hard-edged shadow under it, so that the wall's foot stays
   where it is while the cap moves. */
.mk-collar {
  border-radius: 9999px;
  background-image: linear-gradient(#434C5C, #252C37);
  box-shadow: inset 0 0 0 1px alpha(white, 0.12),
              0 2px 3px alpha(black, 0.22),
              0 8px 16px alpha(black, 0.28);
}
.mk-collar.mk-focus {
  box-shadow: 0 0 0 4px alpha(@mk_accent, 0.5),
              0 2px 3px alpha(black, 0.22),
              0 8px 16px alpha(black, 0.28);
}
.mk-start {
  min-width: 96px;
  min-height: 96px;
  padding: 0;
  border: none;
  border-radius: 9999px;
  outline: none;
  color: white;
  font-size: 120%;
  font-weight: 700;
  text-shadow: 0 1px 1.5px alpha(black, 0.32);
  background-color: @mk_accent;
  background-image:
    radial-gradient(ellipse 29px 13px at 50% 19px, alpha(white, 0.42), alpha(white, 0)),
    radial-gradient(circle 79px at 38% 26%, #6AB3F8, #1C71D8 60%, #1860BC);
  box-shadow: inset 0 2px 1.5px alpha(white, 0.55),
              inset 0 -4px 9px alpha(black, 0.24),
              0 8px 0 #1657A8;
  transition: transform 160ms cubic-bezier(0.3, 1.5, 0.5, 1),
              box-shadow 160ms cubic-bezier(0.3, 1.5, 0.5, 1),
              filter 160ms ease-out;
}
.mk-start:hover {
  transform: translateY(-3px);
  filter: brightness(1.1);
  box-shadow: inset 0 2px 1.5px alpha(white, 0.6),
              inset 0 -4px 9px alpha(black, 0.22),
              0 11px 0 #1657A8;
  transition-duration: 120ms;
  transition-timing-function: ease-out;
}
.mk-start:active,
.mk-start:disabled {
  transform: translateY(6px);
  box-shadow: inset 0 3px 7px alpha(black, 0.34),
              inset 0 -2px 5px alpha(black, 0.14),
              0 2px 0 #124F9E;
  transition-duration: 80ms;
  transition-timing-function: ease-out;
}
.mk-start:active { filter: brightness(0.9); }
.mk-start:disabled { filter: saturate(0.5) brightness(1.32); color: alpha(white, 0.92); }

/* The buttons at the foot of a dialog. */
.mk-button {
  padding: 7px 16px;
  min-height: 0;
  border: 1px solid @mk_control_border;
  border-radius: 8px;
  box-shadow: none;
  text-shadow: none;
  background-image: none;
  background-color: @mk_control_bg;
  color: @mk_text;
  font-weight: 500;
}
.mk-button:hover { background-color: @mk_control_hover; }
.mk-button:active { background-color: @mk_control_active; }
.mk-button:focus-visible { outline: 2px solid alpha(@mk_accent, 0.7); outline-offset: 2px; }
.mk-button.mk-suggested {
  border-color: @mk_accent;
  background-color: @mk_accent;
  color: white;
  font-weight: 600;
}
.mk-button.mk-suggested:hover { border-color: @mk_accent_hover; background-color: @mk_accent_hover; }
.mk-button.mk-suggested:active { border-color: @mk_accent_active; background-color: @mk_accent_active; }

/* The controls of the options. */
.mk-dialog switch {
  min-width: 46px;
  min-height: 26px;
  padding: 0;
  border: none;
  border-radius: 9999px;
  box-shadow: none;
  background-image: none;
  background-color: @mk_switch_off;
  outline-offset: 2px;
}
.mk-dialog switch:checked { background-color: @mk_accent; }
.mk-dialog switch:focus-visible { outline: 2px solid alpha(@mk_accent, 0.7); }
.mk-dialog switch > slider {
  min-width: 22px;
  min-height: 22px;
  margin: 2px;
  border: none;
  border-radius: 9999px;
  background-image: none;
  background-color: white;
  box-shadow: 0 1px 2px alpha(black, 0.25);
}
.mk-dialog switch > image { color: transparent; }
.mk-dialog dropdown > button {
  padding: 6px 12px;
  border: 1px solid @mk_control_border;
  border-radius: 8px;
  box-shadow: none;
  text-shadow: none;
  background-image: none;
  background-color: @mk_control_bg;
  color: @mk_text;
  font-weight: 500;
}
.mk-dialog dropdown > button:hover { background-color: @mk_control_hover; }
.mk-dialog dropdown > button:focus-visible { outline: 2px solid alpha(@mk_accent, 0.7); outline-offset: 2px; }

/* The question GTK asks before the restart: the answer that goes ahead. */
window.dialog.message button.default { color: @mk_accent; font-weight: 600; }

/* Stop, in the title bar. */
.mk-stop {
  padding: 5px 12px;
  border: none;
  box-shadow: none;
  text-shadow: none;
  background-image: none;
  background-color: @mk_destructive;
  color: white;
  font-weight: 600;
}
.mk-stop:hover { background-color: #A91822; }
.mk-stop:active { background-color: #96141D; }
.mk-stop:focus-visible { outline: 2px solid alpha(@mk_destructive, 0.5); outline-offset: 2px; }
.mk-stop-square {
  min-width: 10px;
  min-height: 10px;
  border-radius: 2px;
  background-color: white;
}
"#;

/// Puts the style sheet in force. To be called once GTK is initialised.
pub fn load() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    // GTK drops a rule it cannot read and carries on; say which one.
    provider.connect_parsing_error(|_, section, error| {
        log::warn!(
            "style sheet: {error} at {section}",
            section = section.to_str()
        );
    });
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
