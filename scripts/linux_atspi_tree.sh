#!/usr/bin/env bash
# Reads the accessibility trees of examples/interaction_acceptance and
# examples/controls back over AT-SPI, as a screen reader would, and drives
# them: focus, set text, select, press, set a slider (scripts/atspi_tree.py).
#
# Run in a D-Bus session under an X server, after building the examples with
# the winit feature:
#   cargo build --no-default-features --features winit --example interaction_acceptance --example controls
#   xvfb-run -a -s "-screen 0 1024x768x24" dbus-run-session -- scripts/linux_atspi_tree.sh
# Needs at-spi2-core, python3-gi, gir1.2-atspi-2.0, gdbus and a Vulkan driver
# (Mesa lavapipe in CI).
set -euo pipefail

EXAMPLES="${CARGO_TARGET_DIR:-target}/debug/examples"
OUT="${RUI_ATSPI_OUT:-target/linux-atspi}"
mkdir -p "$OUT"

set_enabled() {
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled "<$1>"
}

app=""
trap 'kill "$app" 2>/dev/null || true' EXIT

# Start example $1, whose window is titled $2, and check it.
read_example() {
  # Start the accessibility bus with no assistive technology running.
  set_enabled false
  RUST_LOG=rui=info "$EXAMPLES/$1" 2>"$OUT/$1.log" &
  app=$!
  # Xvfb has no window manager to focus the window, and AT-SPI reports an
  # element focused only inside the focused window.
  window="$(xdotool search --sync --onlyvisible --pid "$app" | head -n 1)"
  xdotool windowfocus --sync "$window"
  # Now say one started, as a screen reader does: AccessKit joins the bus when
  # this changes.
  set_enabled true

  python3 -u "$(dirname "$0")/atspi_tree.py" "$2" "$1" | tee "$OUT/$1-tree.txt"
  kill "$app"
  wait "$app" || true
}

read_example interaction_acceptance "RUI Interaction Acceptance"
read_example controls "RUI Application"
