#!/usr/bin/env bash
# Opens examples in real X11 windows, drives them with xdotool and compares
# screenshots of the windows with the references in tests/linux_window/.
#
# Run under an X server, after building the examples with the winit feature:
#   cargo build --no-default-features --features winit --example hello_world --example counter
#   xvfb-run -a -s "-screen 0 1024x768x24" scripts/linux_window_pixels.sh
# Needs xdotool, ImageMagick and a Vulkan driver (Mesa lavapipe in CI).
# RUI_PIXELS_UPDATE=1 rewrites the references from this run.
set -euo pipefail

EXAMPLES="${CARGO_TARGET_DIR:-target}/debug/examples"
REFERENCES=tests/linux_window
OUT="${RUI_PIXELS_OUT:-target/linux-window-pixels}"
mkdir -p "$OUT"
# shellcheck source=scripts/window_pixels_compare.sh
source "$(dirname "$0")/window_pixels_compare.sh"

app=""
window=""

# Start example $1 and wait until its window presented the first frame.
open_example() {
  local log="$OUT/$1.log"
  RUST_LOG=rui=info "$EXAMPLES/$1" 2>"$log" &
  app=$!
  window="$(xdotool search --sync --onlyvisible --pid "$app" | head -n 1)"
  for _ in $(seq 100); do
    grep -q "winit window presenting through" "$log" && break
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

click() {
  xdotool mousemove --window "$window" "$1" "$2" click 1
  sleep 0.2
}

# Compare the window with reference $1.png.
check() {
  # Park the pointer outside the window so no hover state shows.
  xdotool mousemove 1023 767
  settle
  import -window "$window" "PNG24:$OUT/$1.png"
  compare_capture "$1"
}

open_example hello_world
check hello_world
close_example

open_example counter
for _ in 1 2 3; do
  click 231 236
done
# Keys need focus, which a window manager gives on click; Xvfb has none.
xdotool windowfocus --sync "$window"
# Return presses the focused "+" button.
xdotool key Return
check counter_input
xdotool windowsize --sync "$window" 500 340
check counter_resized
close_example

exit "$failed"
