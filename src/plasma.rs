//! What can be found out about the KDE Plasma this runs on, and with it
//! whether Plasma is asked for remote control. Knows nothing about GTK.
//!
//! A RemoteDesktop session with a virtual monitor in it is what the
//! teleporter needs. What Plasma makes of one depends on its portal backend,
//! xdg-desktop-portal-kde:
//!
//! - up to 6.4 the portal has no virtual monitors at all, and the app does
//!   not get past its startup checks;
//! - 6.5.0 to 6.5.3 make the monitor and move the pointer, but leave the
//!   cursor out of the frames, so the picture shows no pointer;
//! - from 6.5.4, and in 6.6.0, the cursor is in the frames as asked
//!   (8921a81a and 2f2a0145, "remotedesktop: honor the requested cursor
//!   mode").
//!
//! The backend does not say which release it is: it has no version on the
//! bus and none on its command line, and the versions KWin and plasmashell
//! give are their own, which a distribution may ship at another patch level.
//! Nothing tells 6.5.3 from 6.5.4. So the line is drawn at 6.6.0, the first
//! release that both has the fix and can be told from the ones before it.

use ashpd::zbus;

const BACKEND: &str = "org.freedesktop.impl.portal.desktop.kde";
const BACKEND_PATH: &str = "/org/freedesktop/portal/desktop";
const INHIBIT: &str = "org.freedesktop.impl.portal.Inhibit";

/// Why a Plasma is not asked for remote control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAsked {
    /// "Ask for remote control" is off in the options.
    SwitchedOff,
    /// The portal backend is older than 6.6, or does not say that it is not.
    Older,
}

/// Whether to ask this Plasma for remote control along with the monitor:
/// where the option is on and the portal backend is of 6.6 or later.
pub async fn asks_remote_control(bus: &zbus::Connection, option: bool) -> Result<(), NotAsked> {
    if !option {
        return Err(NotAsked::SwitchedOff);
    }
    if is_6_6_or_later(bus).await {
        Ok(())
    } else {
        Err(NotAsked::Older)
    }
}

/// Whether Plasma's portal backend is of 6.6 or later, by a marker: its
/// Inhibit interface has had a `version` property since 6.6.0 (ff3653fb),
/// and every release since has it, up to 6.8.3, the newest when this was
/// written. Present means 6.6 or later, whatever its value; it was seen
/// absent on 6.4.5 and 6.5.6 and as 3 on 6.6.4.
///
/// This is a marker, not a promise. `org.freedesktop.impl.portal.*` is the
/// private interface between xdg-desktop-portal and its backends, not one
/// for apps, and the property is there for a reason of its own that has
/// nothing to do with remote control. Nobody has said it will stay, and a
/// distribution that put that one commit into an older Plasma would open
/// the gate there. It is read because nothing better is on offer.
async fn is_6_6_or_later(bus: &zbus::Connection) -> bool {
    match marker(bus).await {
        Ok(version) => {
            log::debug!(
                "the KDE portal backend's Inhibit version is {version:?}: Plasma 6.6 or later"
            );
            true
        }
        Err(err) => {
            log::debug!(
                "the KDE portal backend has no Inhibit version, which makes it older than \
                 Plasma 6.6: {err}"
            );
            false
        }
    }
}

async fn marker(bus: &zbus::Connection) -> zbus::Result<zbus::zvariant::OwnedVariant> {
    let properties = zbus::fdo::PropertiesProxy::builder(bus)
        .destination(BACKEND)
        .path(BACKEND_PATH)
        .build()
        .await?;
    let interface = zbus::names::InterfaceName::from_static_str_unchecked(INHIBIT);
    Ok(properties.get(interface, "version").await?)
}
