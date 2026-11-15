#!/bin/bash
# What a program can find out about which Plasma it runs on, before it asks
# the portal for anything: the versions the Plasma programs give for
# themselves, and what the KDE portal backend says of itself.
# Run with SHELL_TOO=1 to have plasmashell there as well.
. "$T/kde/lib.sh"
get() {  # <bus name> <path> <interface> <property>
  gdbus call --session --dest "$1" --object-path "$2" --method org.freedesktop.DBus.Properties.Get "$3" "$4" 2>&1 \
    | sed -e 's/^(<\(.*\)>,)$/\1/' -e 's/^Error: GDBus.Error:\([^:]*\):.*/no (\1)/' | head -1
}
echo "--- the packages, by dpkg"
dpkg -l kwin-wayland plasma-workspace xdg-desktop-portal-kde xdg-desktop-portal 2>/dev/null | awk '/^ii/{print "   " $2, $3}'
echo "--- the programs, asked for their version"
echo "   kwin_wayland --version: $(kwin_wayland --version 2>/dev/null | tail -1)"
echo "   plasmashell --version: $(plasmashell --version 2>/dev/null | tail -1)"
backend=$(ls /usr/lib/*/libexec/xdg-desktop-portal-kde /usr/libexec/xdg-desktop-portal-kde 2>/dev/null | head -1)
echo "   $backend --version: $(timeout 5 "$backend" --version 2>&1 | grep -v -e UTF-8 -e locale -e '^$' | tail -1 | cut -c1-120)"
echo "--- names on the session bus"
bus_names | tr '\n' ' '; echo
echo "--- the versions on the bus"
echo "   org.kde.KWin /MainApplication applicationVersion: $(get org.kde.KWin /MainApplication org.qtproject.Qt.QCoreApplication applicationVersion)"
echo "   org.kde.plasmashell /MainApplication applicationVersion: $(get org.kde.plasmashell /MainApplication org.qtproject.Qt.QCoreApplication applicationVersion)"
echo "   org.kde.KWin /KWin supportInformation: $(gdbus call --session --dest org.kde.KWin --object-path /KWin --method org.kde.KWin.supportInformation 2>&1 | tr -d '\\' | grep -o 'KWin version: [0-9.]*' | head -1)"
echo "--- the portal backend on the bus (org.freedesktop.impl.portal.desktop.kde)"
pid=$(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.GetConnectionUnixProcessID org.freedesktop.impl.portal.desktop.kde 2>/dev/null | tr -dc 0-9)
echo "   process $pid: $(readlink /proc/$pid/exe 2>/dev/null)"
echo "   /MainApplication applicationVersion: $(get org.freedesktop.impl.portal.desktop.kde /MainApplication org.qtproject.Qt.QCoreApplication applicationVersion)"
for interface in RemoteDesktop ScreenCast Screenshot InputCapture Settings Inhibit Session Access FileChooser Notification Print GlobalShortcuts; do
  echo "   org.freedesktop.impl.portal.$interface version: $(get org.freedesktop.impl.portal.desktop.kde /org/freedesktop/portal/desktop org.freedesktop.impl.portal.$interface version)"
done
echo "--- the portal itself (org.freedesktop.portal.Desktop)"
for p in "ScreenCast version" "ScreenCast AvailableSourceTypes" "ScreenCast AvailableCursorModes" "RemoteDesktop version" "RemoteDesktop AvailableDeviceTypes"; do
  set -- $p
  echo "   org.freedesktop.portal.$1 $2: $(get org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.$1 $2)"
done
