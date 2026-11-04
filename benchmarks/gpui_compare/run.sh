#!/usr/bin/env bash
# Runs the long-list fixture on RUI and on GPUI, on this machine.
#
#   benchmarks/gpui_compare/run.sh [zed-checkout-dir]
#
# The Zed checkout defaults to target/gpui_compare/zed and is cloned at ZED_REV when
# missing. GPUI uses runtime_shaders so the Xcode Metal toolchain is not required.
set -euo pipefail

ZED_REV="${ZED_REV:-784374c6794ace885137bc4806427a5f47cc3f06}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ZED_DIR="${1:-$ROOT/target/gpui_compare/zed}"

if [ ! -d "$ZED_DIR/.git" ]; then
  git clone --filter=blob:none https://github.com/zed-industries/zed.git "$ZED_DIR"
fi
git -C "$ZED_DIR" fetch --depth 1 origin "$ZED_REV"
git -C "$ZED_DIR" checkout --quiet "$ZED_REV"

cp "$ROOT/benchmarks/gpui_compare/long_list.rs" "$ZED_DIR/crates/gpui_platform/benches/long_list.rs"
manifest="$ZED_DIR/crates/gpui_platform/Cargo.toml"
if ! grep -q 'name = "long_list"' "$manifest"; then
  printf '\n[[bench]]\nname = "long_list"\nharness = false\nrequired-features = ["bench-support"]\n' >> "$manifest"
fi

echo "== RUI"
(cd "$ROOT" && cargo bench --bench compare_frame)

echo "== GPUI ($ZED_REV)"
(cd "$ZED_DIR" && BENCH_MEASUREMENT=instructions \
  cargo bench -p gpui_platform --features bench-support,runtime_shaders --bench long_list)
