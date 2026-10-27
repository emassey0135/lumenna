#!/bin/sh
# Builds Lumenna's core for the BTSpeak app: the Crux app and the device's store, through
# BoltFFI's Python target (crates/ffi), as a CPython extension for the Python that runs it,
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
# The core as a cdylib alone: crates/ffi is an rlib too, for the Rust apps, and rustc builds an
# artifact made beside an rlib without whole-program optimization (20.0 MB, where alone 17.7).
# With the command surface (`rpc`), which the background service serves while it keeps the
# device in sync, as the daemon does.
(cd "$ROOT/crates/ffi" && boltffi pack python $RELEASE --cargo-arg=--crate-type=cdylib --cargo-arg=--features=rpc --overlay "$BOLT/overlay.toml")

# The wheel's package, unpacked where the app finds it (core.py), in place of the last.
rm -rf "$APP/native"
mkdir -p "$APP/native"
"$PYTHON" -m zipfile -e "$BOLT"/wheels/*.whl "$APP/native"
rm -rf "$APP"/native/*.dist-info

# The app's version is Lumenna's (`[workspace.metadata.lumenna]` in the workspace's Cargo.toml),
# written into pyproject.toml here, so a version bump is that one edit. pyproject keeps it as a
# plain value, not `dynamic`: BT Code reads `project.version` itself when it sends the app to the
# Code Store, and writes the next free one back there.
"$PYTHON" - "$ROOT/Cargo.toml" "$APP/pyproject.toml" <<'PY'
import re
import sys

cargo, pyproject = sys.argv[1:3]
with open(cargo, encoding="utf-8") as f:
    section = f.read().split("[workspace.metadata.lumenna]", 1)[1].split("\n[", 1)[0]
version = re.search(r'^version = "([^"]+)"$', section, re.M).group(1)
with open(pyproject, encoding="utf-8") as f:
    text = f.read()
written = re.sub(r'^version = "[^"]*"$', f'version = "{version}"', text, count=1, flags=re.M)
if written != text:
    with open(pyproject, "w", encoding="utf-8") as f:
        f.write(written)
PY
