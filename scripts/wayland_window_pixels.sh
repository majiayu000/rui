#!/usr/bin/env bash
# Opens examples in real Wayland windows under a headless sway, drives them
# through sway's seat and wtype, and compares screenshots of the windows with
# the references in tests/wayland_window/.
#
# Run after building the examples with the winit feature:
#   cargo build --no-default-features --features winit --example hello_world --example counter
#   scripts/wayland_window_pixels.sh
# Needs sway, grim, wtype, jq, ImageMagick and a Vulkan driver (Mesa lavapipe in
# CI). RUI_PIXELS_UPDATE=1 rewrites the references from this run.
set -euo pipefail

EXAMPLES="${CARGO_TARGET_DIR:-target}/debug/examples"
REFERENCES=tests/wayland_window
OUT="${RUI_PIXELS_OUT:-target/wayland-window-pixels}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
# shellcheck source=scripts/window_pixels_compare.sh
source "$(dirname "$0")/window_pixels_compare.sh"

# A fresh runtime directory, so the compositor's sockets are the only ones.
XDG_RUNTIME_DIR="$(mktemp -d)"
export XDG_RUNTIME_DIR
# Without DISPLAY winit cannot fall back to X11.
unset DISPLAY WAYLAND_DISPLAY SWAYSOCK
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  sway --config "$REFERENCES/sway.conf" >"$OUT/sway.log" 2>&1 &
sway_pid=$!
trap 'kill "$sway_pid" 2>/dev/null || true' EXIT
for _ in $(seq 100); do
  compgen -G "$XDG_RUNTIME_DIR/sway-ipc.*.sock" >/dev/null && compgen -G "$XDG_RUNTIME_DIR/wayland-[0-9]" >/dev/null && break
  sleep 0.1
done
SWAYSOCK="$(compgen -G "$XDG_RUNTIME_DIR/sway-ipc.*.sock")"
WAYLAND_DISPLAY="$(basename "$(compgen -G "$XDG_RUNTIME_DIR/wayland-[0-9]")")"
export SWAYSOCK WAYLAND_DISPLAY
swaymsg -t get_version
# The headless seat has no pointer until one is plugged in.
python3 "$(dirname "$0")/wayland_virtual_pointer.py" &
pointer_pid=$!
trap 'kill "$pointer_pid" "$sway_pid" 2>/dev/null || true' EXIT
for _ in $(seq 50); do
  swaymsg -t get_inputs | jq -e 'any(.type == "pointer")' >/dev/null && break
  sleep 0.1
done
swaymsg -t get_inputs | jq -e 'any(.type == "pointer")' >/dev/null

app=""

# The window's position and size in the layout: "x y width height".
window_rect() {
  swaymsg -t get_tree | jq -r --argjson pid "$app" \
    '.. | objects | select(.pid? == $pid) | .rect | "\(.x) \(.y) \(.width) \(.height)"'
}

# Start example $1 and wait until its window presented the first frame.
open_example() {
  local log="$OUT/$1.log"
  RUST_LOG=rui=info "$EXAMPLES/$1" 2>"$log" &
  app=$!
  for _ in $(seq 100); do
    grep -q "winit window presenting through" "$log" && [[ -n "$(window_rect)" ]] && break
    sleep 0.1
  done
  grep "winit window presenting through" "$log"
  settle
}

close_example() {
  kill "$app"
  wait "$app" || true
}

# Let input reach the app and the next frame present.
settle() {
  sleep 1
}

# Move the pointer to $1,$2 in window coordinates.
pointer_to() {
  local x y _w _h
  read -r x y _w _h <<<"$(window_rect)"
  swaymsg -q "seat - cursor set $((x + $1)) $((y + $2))"
}

click() {
  pointer_to "$1" "$2"
  swaymsg -q "seat - cursor press button1"
  swaymsg -q "seat - cursor release button1"
  sleep 0.2
}

# Compare the window with reference $1.png.
check() {
  # Park the pointer outside the window so no hover state shows.
  swaymsg -q "seat - cursor set 1279 799"
  settle
  local x y w h
  read -r x y w h <<<"$(window_rect)"
  grim -g "$x,$y ${w}x$h" "$OUT/$1-grim.png"
  convert "$OUT/$1-grim.png" -alpha off "PNG24:$OUT/$1.png"
  compare_capture "$1"
}

open_example hello_world
check hello_world
close_example

open_example counter
for _ in 1 2 3; do
  click 231 236
done
# The click focused the window; Return presses the focused "+" button. wtype
# plugs in a keyboard for the call, so wait for the app to bind it first.
wtype -s 1000 -k Return
check counter_input
swaymsg -q "[pid=$app] resize set width 500 px height 340 px"
check counter_resized
close_example

exit "$failed"
