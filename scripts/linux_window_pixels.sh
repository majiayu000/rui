#!/usr/bin/env bash
# Opens examples in real X11 windows, drives them with xdotool and compares
# screenshots of the windows with the references in tests/linux_window/.
#
# Run under an X server, after building the examples with the winit feature:
#   cargo build --no-default-features --features winit --example hello_world --example counter --example multi_window --example interaction_acceptance
#   xvfb-run -a -s "-screen 0 1024x768x24" scripts/linux_window_pixels.sh
# Needs xdotool, xclip, ImageMagick and a Vulkan driver (Mesa lavapipe in CI).
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

# Capture the window as $OUT/$1.png.
capture() {
  # Park the pointer outside the window so no hover state shows.
  xdotool mousemove 1023 767
  settle
  import -window "$window" "PNG24:$OUT/$1.png"
}

# How many pixels differ between captures $OUT/$1.png and $OUT/$2.png.
differing_pixels() {
  local result
  result="$(compare -metric AE -fuzz "$FUZZ" "$OUT/$1.png" "$OUT/$2.png" null: 2>&1)" || true
  echo "${result%% *}"
}

# Compare the window with reference $1.png.
check() {
  capture "$1"
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

# The system clipboard: paste text another app copied, then copy it back.
open_example interaction_acceptance
xdotool windowfocus --sync "$window"
printf 'from-system' | xclip -selection clipboard
# Tab focuses the first input; pasting twice shows the text came through the app.
xdotool key Tab ctrl+v ctrl+v ctrl+a ctrl+c
settle
check_clipboard "$(xclip -selection clipboard -o)"
close_example

# The example opens a second window from its first frame. A click in the
# second window redraws it and leaves the first window as it was.
open_example multi_window
first="$(xdotool search --all --onlyvisible --pid "$app" --name '^RUI first$' | head -n 1 || true)"
second="$(timeout 10 xdotool search --all --sync --onlyvisible --pid "$app" --name '^RUI second$' | head -n 1 || true)"
if [[ -z "$first" || -z "$second" ]]; then
  echo "FAIL multi_window: expected windows titled RUI first and RUI second" >&2
  failed=1
else
  # Xvfb has no window manager: both windows start at the origin.
  timeout 10 xdotool windowmove --sync "$second" 500 0 || true
  settle
  presented="$(grep -c "winit window presenting through" "$OUT/multi_window.log" || true)"
  window="$first"; capture multi_window_first_before
  window="$second"; capture multi_window_second_before
  # The second window's own click counter, its first button.
  click 32 28
  window="$first"; capture multi_window_first_after
  window="$second"; capture multi_window_second_after
  first_changed="$(differing_pixels multi_window_first_before multi_window_first_after)"
  second_changed="$(differing_pixels multi_window_second_before multi_window_second_after)"
  echo "multi_window: $presented windows presented; first $first_changed, second $second_changed pixels changed"
  if [[ "$presented" != 2 || "$first_changed" != 0 || ! "$second_changed" =~ ^[0-9]+$ || "$second_changed" -eq 0 ]]; then
    echo "FAIL multi_window: expected 2 windows presented, the first window unchanged and the second redrawn" >&2
    failed=1
  fi
fi
close_example

exit "$failed"
