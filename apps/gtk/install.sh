#!/bin/sh
# Builds the GTK app and installs it for this user: the program in ~/.local/bin, and its
# desktop entry, which is how GNOME lists it and how the portals know it — the shortcuts
# from anywhere are refused to an app with no entry.
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
sed "s|^Exec=lumenna-gtk|Exec=$bin/lumenna-gtk|" "$here/data/io.github.emassey0135.Lumenna.desktop" \
    > "$data/applications/io.github.emassey0135.Lumenna.desktop"
update-desktop-database "$data/applications" 2>/dev/null || true
echo "Installed $bin/lumenna-gtk and its desktop entry."
