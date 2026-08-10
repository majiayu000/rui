#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "native dogfood requires macOS" >&2
  exit 1
fi

TEXT="${RUI_NATIVE_DOGFOOD_TEXT:-rui-native-dogfood}"
if [[ "$TEXT" == *[^A-Za-z0-9_-]* ]]; then
  echo "RUI_NATIVE_DOGFOOD_TEXT may only contain ASCII letters, numbers, hyphen, and underscore" >&2
  exit 2
fi

PROFILE="${RUI_NATIVE_DOGFOOD_PROFILE:-target/rui-native-dogfood-profile.json}"
RENDERER_PROFILE="${RUI_NATIVE_DOGFOOD_RENDERER_PROFILE:-target/rui-native-dogfood-renderer-profile.jsonl}"
LOG="${RUI_NATIVE_DOGFOOD_LOG:-target/rui-native-dogfood.log}"
mkdir -p "$(dirname "$PROFILE")" "$(dirname "$RENDERER_PROFILE")" "$(dirname "$LOG")"
rm -f "$PROFILE" "$RENDERER_PROFILE" "$LOG"

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
cargo build --example native_dogfood >"$LOG" 2>&1

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
RUI_PROFILE=1 \
cargo run --example native_dogfood >>"$LOG" 2>&1 &
app_pid=$!

cleanup() {
  if kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

exit_code=
for _ in $(seq 1 200); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    if wait "$app_pid"; then
      exit_code=0
    else
      exit_code=$?
    fi
    break
  fi
  sleep 0.1
done

if [[ -z "${exit_code:-}" ]]; then
  echo "native dogfood timed out waiting for profile-producing exit" >&2
  echo "cargo log: $LOG" >&2
  exit 1
fi

if [[ "$exit_code" -ne 0 ]]; then
  echo "native dogfood exited with status $exit_code" >&2
  echo "cargo log: $LOG" >&2
  exit "$exit_code"
fi

trap - EXIT

if [[ ! -s "$PROFILE" ]]; then
  echo "native dogfood did not write RUI_NATIVE_DOGFOOD_PROFILE at $PROFILE" >&2
  echo "cargo log: $LOG" >&2
  exit 1
fi

grep -q '"status":"passed"' "$PROFILE"
grep -q "\"typed_text\":\"$TEXT\"" "$PROFILE"
grep -q '"script_requires_minimize_reopen":true' "$PROFILE"

awk 'index($0, "{\"schema\":\"rui.renderer.profile.v1\"") == 1 { print }' \
  "$LOG" >"$RENDERER_PROFILE"

if [[ ! -s "$RENDERER_PROFILE" ]]; then
  echo "native dogfood did not capture RUI_PROFILE renderer telemetry at $RENDERER_PROFILE" >&2
  echo "cargo log: $LOG" >&2
  exit 1
fi

for metric in \
  frame_interval_ns \
  event_to_render_latency_ns \
  layout_ns \
  dispatch_ns \
  paint_ns \
  render_ns \
  render_p95_ns \
  render_p99_ns \
  jank_count
do
  if ! grep -Eq "\"${metric}\":[0-9]+" "$RENDERER_PROFILE"; then
    echo "renderer telemetry did not contain a numeric $metric value" >&2
    echo "renderer profile: $RENDERER_PROFILE" >&2
    echo "cargo log: $LOG" >&2
    exit 1
  fi
done

echo "native dogfood profile: $PROFILE"
echo "renderer telemetry profile: $RENDERER_PROFILE"
