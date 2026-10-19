#!/bin/sh
# Builds Lumenna's core for the BTSpeak app: the Crux app and the device's store, through
# BoltFFI's Python target (crates/mobile), as a CPython extension for the Python that runs it,
# unpacked into native/ beside the app. Build output, not committed.
#
#   apps/btspeak/build-core.sh [--debug]
#
# Needs Rust, BoltFFI's CLI at the crates' version (`cargo install boltffi_cli --version 0.31.1
# --locked`), and a Python of 3.10 or later with pip, its headers and a C compiler: on the
# device, the system's python3. PYTHON names another interpreter. Release unless --debug: a
# BT Braille is a Raspberry Pi, and the debug core is several times the size and slower.
set -eu

APP="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$APP/../.." && pwd)"
PYTHON="${PYTHON:-python3}"
BOLT="$ROOT/target/boltffi-python"
RELEASE="--release"
[ "${1:-}" = "--debug" ] && RELEASE=""

rm -rf "$BOLT/wheels"
mkdir -p "$BOLT"
cat > "$BOLT/overlay.toml" <<TOML
[targets.python]
enabled = true
output = "$BOLT/package"
module_name = "lumenna_app"

[targets.python.wheel]
output = "$BOLT/wheels"
interpreters = ["$PYTHON"]
TOML
(cd "$ROOT/crates/mobile" && boltffi pack python $RELEASE --overlay "$BOLT/overlay.toml")

# The wheel's package, unpacked where the app finds it (core.py), in place of the last.
rm -rf "$APP/native"
mkdir -p "$APP/native"
"$PYTHON" -m zipfile -e "$BOLT"/wheels/*.whl "$APP/native"
rm -rf "$APP"/native/*.dist-info
