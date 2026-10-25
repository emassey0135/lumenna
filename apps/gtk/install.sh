#!/bin/sh
# Builds the GTK app and installs it for this user: the program in ~/.local/bin, its icon, and
# its desktop entry, which is how GNOME lists it and how the portals know it — the shortcuts
# from anywhere are refused to an app with no entry — and its KRunner runner.
#
#   apps/gtk/install.sh            # a release build
#   PROFILE=dev apps/gtk/install.sh  # the debug build, faster to make
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$here/../..
bin=${XDG_BIN_HOME:-$HOME/.local/bin}
data=${XDG_DATA_HOME:-$HOME/.local/share}
if [ "${PROFILE:-release}" = release ]; then
    cargo build --release --manifest-path "$root/Cargo.toml" -p lumenna-gtk
    built=$root/target/release/lumenna-gtk
else
    cargo build --manifest-path "$root/Cargo.toml" -p lumenna-gtk
    built=$root/target/debug/lumenna-gtk
fi
mkdir -p "$bin" "$data/applications"
install -m 755 "$built" "$bin/lumenna-gtk"
# The entry names the program by its full path, since a desktop session's PATH may not hold
# ~/.local/bin.
sed "s|^Exec=lumenna-gtk|Exec=$bin/lumenna-gtk|" "$here/data/dev.emassey0135.Lumenna.desktop" \
    > "$data/applications/dev.emassey0135.Lumenna.desktop"
update-desktop-database "$data/applications" 2>/dev/null || true
# KRunner's runner, and the service that starts the app in the background to answer a search.
# GNOME Shell's search provider is not installed: the shell reads providers only from the system's
# data folders (XDG_DATA_DIRS), never ~/.local/share.
install -D -m 644 "$here/data/dev.emassey0135.Lumenna.krunner.desktop" "$data/krunner/dbusplugins/dev.emassey0135.Lumenna.desktop"
mkdir -p "$data/dbus-1/services"
sed "s|^Exec=lumenna-gtk|Exec=$bin/lumenna-gtk|" "$here/data/dev.emassey0135.Lumenna.service" \
    > "$data/dbus-1/services/dev.emassey0135.Lumenna.service"
# The app's icon, at every size it is drawn in, in the user's hicolor theme.
(cd "$here/data/icons" && find hicolor -type f) | while read -r icon; do
    install -D -m 644 "$here/data/icons/$icon" "$data/icons/$icon"
done
gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
echo "Installed $bin/lumenna-gtk, its desktop entry, its icon and its KRunner runner."
