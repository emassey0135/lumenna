#!/bin/bash
# Runs a command inside a session of its own: a private D-Bus session bus, a headless GNOME
# compositor (mutter) on a Wayland display of its own, and the accessibility bus and registry.
# Nothing reaches the desktop the tests are started from, its screen reader included.
#
# Use it under dbus-run-session:  dbus-run-session -- ./headless.sh python3 -m unittest
set -u
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
unset DISPLAY
export WAYLAND_DISPLAY=lumenna-test-$$
export GSETTINGS_BACKEND=memory
export GDK_BACKEND=wayland
# No portal: one starting on a private bus wants a desktop it cannot find.
export GTK_USE_PORTAL=0
mutter --headless --wayland --no-x11 --virtual-monitor 1280x800 --wayland-display="$WAYLAND_DISPLAY" \
    >"${MUTTER_LOG:-/dev/null}" 2>&1 &
mutter=$!
for _ in $(seq 100); do
    [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break
    sleep 0.1
done
# The registry is normally started by systemd, which a private bus does not have.
/usr/lib/at-spi-bus-launcher --launch-immediately >/dev/null 2>&1 &
bus=$!
sleep 0.5
/usr/lib/at-spi2-registryd >/dev/null 2>&1 &
registry=$!
sleep 0.5
"$@"
status=$?
kill $registry $bus $mutter 2>/dev/null
wait $mutter 2>/dev/null
exit $status
