//! Which desktop this is, as far as `XDG_CURRENT_DESKTOP` tells. Knows
//! nothing about GTK or the portals.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    /// Another one, or the variable is not set.
    Other,
}

impl Desktop {
    /// Whether the app asks this desktop for remote control along with the
    /// monitor, which is what the teleporter needs. It does on GNOME, where
    /// that has been run.
    ///
    /// Not on KDE Plasma yet. Up to 6.4 its portal casts the real screens in
    /// a RemoteDesktop session, whatever source is asked for, and 6.5 makes
    /// the virtual monitor but leaves the cursor out of its frames; 6.6 does
    /// both as asked (xdg-desktop-portal-kde 8def3b79 and 2f2a0145). Nothing
    /// the portal says tells the three apart before its dialog has been
    /// shown, and finding out after it would take a second dialog.
    pub fn has_teleporter(self) -> bool {
        self == Self::Gnome
    }
}

/// The desktop, and its name as the variable has it; empty if it is not set.
pub fn current() -> (Desktop, String) {
    let name = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    (named(&name), name)
}

/// The variable is a list, separated by colons, of the names a desktop goes
/// by: Ubuntu's GNOME says `ubuntu:GNOME`.
fn named(name: &str) -> Desktop {
    let is = |wanted: &str| {
        name.split(':')
            .any(|part| part.eq_ignore_ascii_case(wanted))
    };
    if is("GNOME") {
        Desktop::Gnome
    } else if is("KDE") {
        Desktop::Kde
    } else {
        Desktop::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_desktop_by_any_of_its_names() {
        assert_eq!(named("GNOME"), Desktop::Gnome);
        assert_eq!(named("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(named("gnome"), Desktop::Gnome);
        assert_eq!(named("KDE"), Desktop::Kde);
        assert_eq!(named("sway"), Desktop::Other);
        assert_eq!(named(""), Desktop::Other);
    }
}
