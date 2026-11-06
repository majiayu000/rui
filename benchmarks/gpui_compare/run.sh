#!/usr/bin/env bash
# Runs every comparison fixture on RUI and on GPUI, on this machine, and prints one table.
#
#   benchmarks/gpui_compare/run.sh [zed-checkout-dir]
#
# The Zed checkout defaults to target/gpui_compare/zed and is cloned at ZED_REV when
# missing. GPUI uses runtime_shaders so the Xcode Metal toolchain is not required, and
# font-kit so it shapes and rasterizes real text.
# Each case runs in its own process on each side, RUI then GPUI, REPEATS times
# (default 3); the table shows the range over the repeats. CASES picks cases.
# Raw lines are kept in target/gpui_compare/results.txt.
set -euo pipefail

ZED_REV="${ZED_REV:-784374c6794ace885137bc4806427a5f47cc3f06}"
REPEATS="${REPEATS:-3}"
CASES="${CASES:-startup list_full list_full_one_row list_virtual_1k list_virtual_100k text_paragraphs quad_grid resize_full resize_virtual editor_scroll editor_edit}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ZED_DIR="${1:-$ROOT/target/gpui_compare/zed}"
OUT="$ROOT/target/gpui_compare"
mkdir -p "$OUT"

if [ ! -d "$ZED_DIR/.git" ]; then
  git clone --filter=blob:none https://github.com/zed-industries/zed.git "$ZED_DIR"
fi
if [ "$(git -C "$ZED_DIR" rev-parse HEAD)" != "$ZED_REV" ]; then
  git -C "$ZED_DIR" fetch --depth 1 origin "$ZED_REV"
  git -C "$ZED_DIR" checkout --quiet "$ZED_REV"
fi

cp "$ROOT/benchmarks/gpui_compare/fixtures.rs" "$ZED_DIR/crates/gpui_platform/benches/rui_compare.rs"
# Both sides read GPU time through the same hook; the bench declares `mod gpu_time;`.
cp "$ROOT/benchmarks/gpui_compare/gpu_time.rs" "$ZED_DIR/crates/gpui_platform/benches/gpu_time.rs"
manifest="$ZED_DIR/crates/gpui_platform/Cargo.toml"
if ! grep -q 'name = "rui_compare"' "$manifest"; then
  printf '\n[[bench]]\nname = "rui_compare"\nharness = false\nrequired-features = ["bench-support"]\n' >> "$manifest"
fi
# The draw-counting renderer wrapper names `image::RgbaImage`; `image` is already a
# Zed workspace dependency.
if ! grep -q '^image.workspace = true' "$manifest"; then
  sed -i '' 's/^criterion.workspace = true$/criterion.workspace = true\
image.workspace = true/' "$manifest"
fi

# Prints the bench executable cargo built, from its "Executable ... (path)" line.
executable() {
  sed -n 's/.*Executable .*(\(.*\))$/\1/p' | tail -1
}

echo "== building RUI ($(git -C "$ROOT" rev-parse --short HEAD))" >&2
rui_bin="$(cd "$ROOT" && cargo bench --bench compare_frame --no-run 2>&1 | executable)"
rui_bin="$ROOT/$rui_bin"
echo "== building GPUI (${ZED_REV:0:7})" >&2
# Without font-kit, gpui_macos uses NoopTextSystem: fixed advances and no glyphs.
gpui_bin="$(cd "$ZED_DIR" && cargo bench -p gpui_platform --features bench-support,runtime_shaders,font-kit \
  --bench rui_compare --no-run 2>&1 | executable)"
gpui_bin="$ZED_DIR/$gpui_bin"

# Turns one GPUI bench run's report into RUI's line format.
gpui_line() {
  awk -v case="$1" '
    function scale(unit) {
      if (unit ~ /^G/) return 1e9; if (unit ~ /^M/) return 1e6; if (unit ~ /^K/) return 1e3
      return 1
    }
    function ms(text) {
      value = text; sub(/[^0-9.]+$/, "", value)
      if (text ~ /µs$/) return value / 1e3; if (text ~ /ns$/) return value / 1e6
      if (text ~ /[0-9]s$/) return value * 1e3; return value
    }
    # The bench prints `key=value` lines: footprint after every call of the bench
    # function (criterion calls it many times), draw counts and startup once.
    $0 ~ "^gpui " case " " {
      for (i = 3; i <= NF; i++) {
        split($i, pair, "=")
        if (pair[1] == "footprint_mb") {
          footprint_end = pair[2]; if (footprint == "") footprint = footprint_end
        } else {
          if (!(pair[1] in extra)) keys[++key_count] = pair[1]
          extra[pair[1]] = pair[2]
        }
      }
    }
    $2 == "time:" || $1 == "time:" {
      for (i = 1; i <= NF; i++) if ($i ~ /^\[/) { first = i; break }
      instructions = $(first + 3) * scale($(first + 4)) / 1e6
    }
    /^  allocations per iteration:/ { allocations = $5 * scale($6) }
    # Median: whole iterations, which include the present, as the RUI frames include the
    # submit. GPUI keeps no per-iteration p95, so p95 is its draw histogram, which
    # excludes the present.
    /^  wall time per iteration:/ { p50 = ms($6 $7) }
    /^  window draw:/ { draw = 1 }
    draw && /p95:/ { p95 = ms($2); draw = 0 }
    END {
      line = "gpui " case
      if (case != "startup") {
        line = line sprintf(" instructions_m=%.2f ms_p50=%.3f ms_p95=%.3f allocations=%.0f footprint_end_mb=%s",
          instructions, p50, p95, allocations, footprint_end)
      }
      line = line " footprint_mb=" footprint
      for (k = 1; k <= key_count; k++) line = line " " keys[k] "=" extra[keys[k]]
      print line
    }'
}

# The first launch of a new binary waits for macOS to check its signature; keep that
# out of the startup case.
"$rui_bin" --list > /dev/null
(cd "$ZED_DIR" && "$gpui_bin" --list > /dev/null 2>&1)

results="$OUT/results.txt"
: > "$results"
echo "== load before: $(uptime | sed 's/.*load averages*: //')" | tee -a "$results" >&2
for round in $(seq "$REPEATS"); do
  for case in $CASES; do
    echo "-- round $round: $case" >&2
    "$rui_bin" "$case" | tee -a "$results" >&2
    (cd "$ZED_DIR" && BENCH_MEASUREMENT=instructions "$gpui_bin" --bench --exact "$case" 2>&1) \
      | gpui_line "$case" | tee -a "$results" >&2
  done
done
echo "== load after: $(uptime | sed 's/.*load averages*: //')" | tee -a "$results" >&2

# One row per case and side; each cell is the range over the repeats.
awk '
  /^(rui|gpui) / {
    key = $2 SUBSEP $1
    if (!(key in seen)) { seen[key] = 1; order[++rows] = key }
    for (i = 3; i <= NF; i++) {
      split($i, pair, "="); metric = pair[1]; value = pair[2] + 0
      if (!((key, metric) in low) || value < low[key, metric]) low[key, metric] = value
      if (!((key, metric) in high) || value > high[key, metric]) high[key, metric] = value
      has[key, metric] = 1
    }
  }
  # RUI against GPUI on one lower-is-better metric: a tie when the ranges overlap or
  # the midpoints are within `noise`, otherwise the ratio of the midpoints.
  function verdict(name, metric, noise,    rui, gpui, ratio) {
    rui = name SUBSEP "rui"; gpui = name SUBSEP "gpui"
    if (!has[rui, metric] || !has[gpui, metric]) return "-"
    if (low[rui, metric] <= high[gpui, metric] && low[gpui, metric] <= high[rui, metric])
      return "tie (ranges overlap)"
    ratio = (low[rui, metric] + high[rui, metric]) / (low[gpui, metric] + high[gpui, metric])
    if (ratio < 1 / (1 + noise)) return sprintf("RUI ahead %.1fx", 1 / ratio)
    if (ratio > 1 + noise) return sprintf("RUI behind %.1fx", ratio)
    return "tie (within noise)"
  }
  function cell(key, metric) {
    if (!has[key, metric]) return "-"
    if (low[key, metric] == high[key, metric]) return sprintf("%g", low[key, metric])
    return sprintf("%g-%g", low[key, metric], high[key, metric])
  }
  END {
    metric_count = split("instructions_m ms_p50 ms_p95 allocations draw_calls instances gpu_ms_p50 gpu_ms_p95 footprint_mb footprint_end_mb", frame_metrics, " ")
    print "| case | side | instructions (M) / frame | ms / frame p50 | ms / frame p95 | allocations / frame | draw calls / frame | instances / frame | GPU ms p50 | GPU ms p95 | footprint (MB) | footprint at end (MB) |"
    print "|---|---|---|---|---|---|---|---|---|---|---|---|"
    for (row = 1; row <= rows; row++) {
      split(order[row], parts, SUBSEP)
      if (parts[1] == "startup") continue
      line = "| " parts[1] " | " parts[2]
      for (m = 1; m <= metric_count; m++) line = line " | " cell(order[row], frame_metrics[m])
      print line " |"
    }
    print ""
    print "| startup | side | process start to fixture code (ms) | first frame (ms) | process start to first frame (ms) | footprint (MB) |"
    print "|---|---|---|---|---|---|"
    for (row = 1; row <= rows; row++) {
      split(order[row], parts, SUBSEP)
      if (parts[1] != "startup") continue
      print "| startup | " parts[2] " | " cell(order[row], "process_to_fixture_ms") " | " \
        cell(order[row], "first_frame_ms") " | " cell(order[row], "process_to_first_frame_ms") " | " \
        cell(order[row], "footprint_mb") " |"
    }
    print ""
    print "| case | verdict by instructions (5% noise) | verdict by wall p50 (10% noise) |"
    print "|---|---|---|"
    for (row = 1; row <= rows; row++) {
      split(order[row], parts, SUBSEP)
      if (parts[2] != "rui") continue
      if (parts[1] == "startup") {
        print "| startup | - | " verdict("startup", "process_to_first_frame_ms", 0.10) " (process start to first frame) |"
        continue
      }
      print "| " parts[1] " | " verdict(parts[1], "instructions_m", 0.05) " | " verdict(parts[1], "ms_p50", 0.10) " |"
    }
  }' "$results"
