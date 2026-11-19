#!/usr/bin/env bash
# Reads the accessibility tree of examples/interaction_acceptance back over
# AT-SPI under a headless sway, the Wayland twin of scripts/linux_atspi_tree.sh:
# the same checks, run by scripts/atspi_tree.py.
#
# Run in a D-Bus session, after building the example with the winit feature:
#   cargo build --no-default-features --features winit --example interaction_acceptance
#   dbus-run-session -- scripts/wayland_atspi_tree.sh
# Needs sway, wtype, jq, at-spi2-core, python3-gi, gir1.2-atspi-2.0, gdbus and a
# Vulkan driver (Mesa lavapipe in CI).
set -euo pipefail

EXAMPLES="${CARGO_TARGET_DIR:-target}/debug/examples"
OUT="${RUI_ATSPI_OUT:-target/wayland-atspi}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
# shellcheck source=scripts/wayland_session.sh
source "$(dirname "$0")/wayland_session.sh"
start_sway tests/wayland_window/sway.conf "$OUT/sway.log"

# Container focus alone does not send wl_keyboard.enter to winit. Keep a
# virtual keyboard attached for the AT-SPI checks without sending any keys.
wtype -s 300000 >"$OUT/keyboard.log" 2>&1 &
keyboard_pid=$!
trap 'kill "$keyboard_pid" "$sway_pid" 2>/dev/null || true' EXIT
for _ in $(seq 50); do
  swaymsg -t get_inputs | jq -e 'any(.type == "keyboard")' >/dev/null && break
  sleep 0.1
done
swaymsg -t get_inputs | jq -er 'first(.[] | select(.type == "keyboard") | .name)'

set_enabled() {
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled "<$1>"
}

# Start the accessibility bus with no assistive technology running.
set_enabled false
RUST_LOG=rui=info "$EXAMPLES/interaction_acceptance" 2>"$OUT/interaction_acceptance.log" &
app=$!
trap 'kill "$app" "$keyboard_pid" "$sway_pid" 2>/dev/null || true' EXIT
for _ in $(seq 100); do
  swaymsg -t get_tree | jq -e --argjson pid "$app" 'any(.. | objects; .pid? == $pid)' >/dev/null && break
  sleep 0.1
done
# sway focuses a new window, and AT-SPI reports an element focused only inside
# the focused window; say so explicitly rather than rely on it.
swaymsg "[pid=$app] focus"
swaymsg -t get_tree | jq -e --argjson pid "$app" 'any(.. | objects; .pid? == $pid and .focused)'
# Now say one started, as a screen reader does: AccessKit joins the bus when
# this changes.
set_enabled true

python3 -u "$(dirname "$0")/atspi_tree.py" "RUI Interaction Acceptance" | tee "$OUT/tree.txt"
