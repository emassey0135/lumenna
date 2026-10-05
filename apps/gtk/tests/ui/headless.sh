#!/bin/bash
# Runs a command inside a session of its own: a runtime directory, a D-Bus session bus, a
# headless GNOME compositor (mutter) on a Wayland display, and the accessibility bus and
# registry, all private. Nothing reaches the desktop the tests are started from, its screen
# reader included.
#
#   ./headless.sh python3 -m unittest
set -u
if [ -z "${LUMENNA_HEADLESS:-}" ]; then
    # The runtime directory first, before the bus, so that everything the bus starts — the
    # portal, gvfs — uses it too. Sockets go in it at fixed names: the accessibility bus at
    # at-spi/bus. Run in the desktop's own directory, that replaced the desktop's socket, and
    # every app started on the desktop afterwards could not reach its screen reader.
    private=$(mktemp -d "${TMPDIR:-/tmp}/lumenna-session-XXXXXX")
    chmod 700 "$private"
    LUMENNA_HEADLESS=$private XDG_RUNTIME_DIR=$private dbus-run-session -- "$0" "$@"
    status=$?
    # The bus's services make doc/ and gvfs/ in it again as they shut down, so it goes once
    # they have.
    for _ in 1 2 3 4 5; do
        sleep 0.5
        rm -rf "$private" 2>/dev/null && [ ! -e "$private" ] && break
    done
    exit $status
fi
if [ "$XDG_RUNTIME_DIR" != "$LUMENNA_HEADLESS" ]; then
    echo "headless.sh: refusing to run in a shared runtime directory" >&2
    exit 1
fi
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
