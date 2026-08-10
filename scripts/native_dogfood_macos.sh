#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "native dogfood requires macOS" >&2
  exit 1
fi

canonical_artifact_path() {
  local path="$1"
  local directory
  local filename
  directory="$(dirname "$path")"
  filename="$(basename "$path")"
  mkdir -p "$directory"
  directory="$(cd "$directory" && pwd -P)"
  printf '%s/%s\n' "$directory" "$filename"
}

TEXT="${RUI_NATIVE_DOGFOOD_TEXT:-rui-native-dogfood}"
if [[ "$TEXT" == *[^A-Za-z0-9_-]* ]]; then
  echo "RUI_NATIVE_DOGFOOD_TEXT may only contain ASCII letters, numbers, hyphen, and underscore" >&2
  exit 2
fi

PROFILE="${RUI_NATIVE_DOGFOOD_PROFILE:-target/rui-native-dogfood-profile.json}"
RENDERER_PROFILE="${RUI_NATIVE_DOGFOOD_RENDERER_PROFILE:-target/rui-native-dogfood-renderer-profile.jsonl}"
LOG="${RUI_NATIVE_DOGFOOD_LOG:-target/rui-native-dogfood.log}"
PROFILE_PATH="$(canonical_artifact_path "$PROFILE")"
RENDERER_PROFILE_PATH="$(canonical_artifact_path "$RENDERER_PROFILE")"
LOG_PATH="$(canonical_artifact_path "$LOG")"
if [[ "$PROFILE_PATH" == "$RENDERER_PROFILE_PATH" \
  || "$PROFILE_PATH" == "$LOG_PATH" \
  || "$RENDERER_PROFILE_PATH" == "$LOG_PATH" ]]; then
  echo "native dogfood artifact paths must be distinct" >&2
  exit 2
fi
rm -f -- "$PROFILE_PATH" "$RENDERER_PROFILE_PATH" "$LOG_PATH"

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE_PATH" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
cargo build --example native_dogfood >"$LOG_PATH" 2>&1

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE_PATH" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
RUI_PROFILE=1 \
cargo run --example native_dogfood >>"$LOG_PATH" 2>&1 &
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
  echo "cargo log: $LOG_PATH" >&2
  exit 1
fi

if [[ "$exit_code" -ne 0 ]]; then
  echo "native dogfood exited with status $exit_code" >&2
  echo "cargo log: $LOG_PATH" >&2
  exit "$exit_code"
fi

trap - EXIT

if [[ ! -s "$PROFILE_PATH" ]]; then
  echo "native dogfood did not write RUI_NATIVE_DOGFOOD_PROFILE at $PROFILE_PATH" >&2
  echo "cargo log: $LOG_PATH" >&2
  exit 1
fi

grep -q '"status":"passed"' "$PROFILE_PATH"
grep -q "\"typed_text\":\"$TEXT\"" "$PROFILE_PATH"
grep -q '"script_requires_minimize_reopen":true' "$PROFILE_PATH"

awk 'index($0, "{\"schema\":\"rui.renderer.profile.v1\"") == 1 { print }' \
  "$LOG_PATH" >"$RENDERER_PROFILE_PATH"

if [[ ! -s "$RENDERER_PROFILE_PATH" ]]; then
  echo "native dogfood did not capture RUI_PROFILE renderer telemetry at $RENDERER_PROFILE_PATH" >&2
  echo "cargo log: $LOG_PATH" >&2
  exit 1
fi

if ! cargo run --quiet --example validate_renderer_profile -- "$RENDERER_PROFILE_PATH" \
  >>"$LOG_PATH" 2>&1; then
  echo "native dogfood renderer telemetry validation failed" >&2
  echo "renderer profile: $RENDERER_PROFILE_PATH" >&2
  echo "cargo log: $LOG_PATH" >&2
  exit 1
fi

echo "native dogfood profile: $PROFILE_PATH"
echo "renderer telemetry profile: $RENDERER_PROFILE_PATH"
