#!/usr/bin/env bash
# Reads the accessibility tree of examples/interaction_acceptance back over
# AT-SPI, as a screen reader would, and drives it: focus, set text, press.
#
# Run in a D-Bus session under an X server, after building the example with
# the winit feature:
#   cargo build --no-default-features --features winit --example interaction_acceptance
#   xvfb-run -a -s "-screen 0 1024x768x24" dbus-run-session -- scripts/linux_atspi_tree.sh
# Needs at-spi2-core, python3-gi, gir1.2-atspi-2.0, gdbus and a Vulkan driver
# (Mesa lavapipe in CI).
set -euo pipefail

EXAMPLES="${CARGO_TARGET_DIR:-target}/debug/examples"
OUT="${RUI_ATSPI_OUT:-target/linux-atspi}"
mkdir -p "$OUT"

# Say an assistive technology is running, as a screen reader does. Asking
# starts the accessibility bus, and AccessKit only publishes once this is set.
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>'

RUST_LOG=rui=info "$EXAMPLES/interaction_acceptance" 2>"$OUT/interaction_acceptance.log" &
app=$!
trap 'kill "$app" 2>/dev/null || true' EXIT
# Xvfb has no window manager to focus the window, and AT-SPI reports an
# element focused only inside the focused window.
window="$(xdotool search --sync --onlyvisible --pid "$app" | head -n 1)"
xdotool windowfocus --sync "$window"

python3 -u "$(dirname "$0")/atspi_tree.py" "RUI Interaction Acceptance" | tee "$OUT/tree.txt"
