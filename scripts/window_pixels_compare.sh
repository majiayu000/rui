# shellcheck shell=bash
# The sourcing script exits with $failed.
# shellcheck disable=SC2034
# Compares a window capture with its committed reference. Sourced by
# scripts/linux_window_pixels.sh and scripts/wayland_window_pixels.sh, which set
# REFERENCES and OUT. Needs ImageMagick.
# RUI_PIXELS_UPDATE=1 rewrites the references from this run.

# Channels off by more than FUZZ make a pixel differ; a window may have at most
# MAX_DIFFERENT such pixels, room for antialiasing that varies by CPU but not
# for a changed glyph, color or layout.
FUZZ="${RUI_PIXELS_FUZZ:-10%}"
MAX_DIFFERENT="${RUI_PIXELS_MAX_DIFFERENT:-150}"
failed=0

# Compare $OUT/$1.png with reference $REFERENCES/$1.png.
compare_capture() {
  local actual="$OUT/$1.png" reference="$REFERENCES/$1.png"
  if [[ "${RUI_PIXELS_UPDATE:-}" == 1 ]]; then
    mkdir -p "$REFERENCES"
    cp "$actual" "$reference"
    echo "updated $reference"
    return
  fi
  local result
  result="$(compare -metric AE -fuzz "$FUZZ" "$reference" "$actual" "$OUT/$1-diff.png" 2>&1)" || true
  local different="${result%% *}"
  if [[ "$different" =~ ^[0-9.e+]+$ ]] && awk -v n="$different" -v max="$MAX_DIFFERENT" 'BEGIN { exit !(n <= max) }'; then
    echo "ok $1: $different pixels differ"
  else
    echo "FAIL $1: $result (at most $MAX_DIFFERENT pixels may differ; see $OUT)" >&2
    failed=1
  fi
}
