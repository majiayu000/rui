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
  directory="$(dirname -- "$path")" || return 1
  filename="$(basename -- "$path")" || return 1
  mkdir -p -- "$directory" || return 1
  directory="$(cd -- "$directory" && pwd -P)" || return 1
  printf '%s/%s\n' "$directory" "$filename" || return 1
}

TEXT="${RUI_NATIVE_DOGFOOD_TEXT:-rui-native-dogfood}"
if [[ "$TEXT" == *[^A-Za-z0-9_-]* ]]; then
  echo "RUI_NATIVE_DOGFOOD_TEXT may only contain ASCII letters, numbers, hyphen, and underscore" >&2
  exit 2
fi

PROFILE="${RUI_NATIVE_DOGFOOD_PROFILE:-target/rui-native-dogfood-profile.json}"
RENDERER_PROFILE="${RUI_NATIVE_DOGFOOD_RENDERER_PROFILE:-target/rui-native-dogfood-renderer-profile.jsonl}"
LOG="${RUI_NATIVE_DOGFOOD_LOG:-target/rui-native-dogfood.log}"
if ! PROFILE_PATH="$(canonical_artifact_path "$PROFILE")" \
  || ! RENDERER_PROFILE_PATH="$(canonical_artifact_path "$RENDERER_PROFILE")" \
  || ! LOG_PATH="$(canonical_artifact_path "$LOG")"; then
  echo "native dogfood artifact paths could not be canonicalized" >&2
  exit 2
fi
if [[ "$PROFILE_PATH" == "$RENDERER_PROFILE_PATH" \
  || "$PROFILE_PATH" == "$LOG_PATH" \
  || "$RENDERER_PROFILE_PATH" == "$LOG_PATH" ]]; then
  echo "native dogfood artifact paths must be distinct" >&2
  exit 2
fi
rm -f -- "$PROFILE_PATH" "$RENDERER_PROFILE_PATH" "$LOG_PATH"

if ! { : >"$PROFILE_PATH" && : >"$RENDERER_PROFILE_PATH" && : >"$LOG_PATH"; }; then
  rm -f -- "$PROFILE_PATH" "$RENDERER_PROFILE_PATH" "$LOG_PATH"
  echo "native dogfood artifact paths must be writable regular files" >&2
  exit 2
fi
if [[ "$PROFILE_PATH" -ef "$RENDERER_PROFILE_PATH" \
  || "$PROFILE_PATH" -ef "$LOG_PATH" \
  || "$RENDERER_PROFILE_PATH" -ef "$LOG_PATH" ]]; then
  rm -f -- "$PROFILE_PATH" "$RENDERER_PROFILE_PATH" "$LOG_PATH"
  echo "native dogfood artifact paths must be distinct filesystem entries" >&2
  exit 2
fi
rm -f -- "$PROFILE_PATH" "$RENDERER_PROFILE_PATH" "$LOG_PATH"

POLL_ATTEMPTS="${RUI_NATIVE_DOGFOOD_POLL_ATTEMPTS:-200}"
POLL_INTERVAL="${RUI_NATIVE_DOGFOOD_POLL_INTERVAL:-0.1}"
TERMINATION_GRACE="${RUI_NATIVE_DOGFOOD_TERMINATION_GRACE:-1}"
if [[ ! "$POLL_ATTEMPTS" =~ ^[1-9][0-9]*$ \
  || ! "$POLL_INTERVAL" =~ ^(0|[1-9][0-9]*)(\.[0-9]+)?$ \
  || ! "$TERMINATION_GRACE" =~ ^(0|[1-9][0-9]*)(\.[0-9]+)?$ ]]; then
  echo "native dogfood polling controls must be positive numeric values" >&2
  exit 2
fi

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE_PATH" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
cargo build --example native_dogfood --message-format=json-render-diagnostics \
  >"$LOG_PATH" 2>&1

APP_PATH="$(sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' "$LOG_PATH" | tail -n 1)"
if [[ -z "$APP_PATH" || ! -x "$APP_PATH" ]]; then
  echo "native dogfood build did not report an executable example artifact" >&2
  echo "cargo log: $LOG_PATH" >&2
  exit 1
fi

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE_PATH" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
RUI_PROFILE=1 \
"$APP_PATH" >>"$LOG_PATH" 2>&1 &
app_pid=$!

cleanup() {
  if kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid" 2>/dev/null || true
    sleep "$TERMINATION_GRACE"
    if kill -0 "$app_pid" 2>/dev/null; then
      kill -KILL "$app_pid" 2>/dev/null || true
    fi
  fi
  wait "$app_pid" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

exit_code=
for _ in $(seq 1 "$POLL_ATTEMPTS"); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    if wait "$app_pid"; then
      exit_code=0
    else
      exit_code=$?
    fi
    break
  fi
  sleep "$POLL_INTERVAL"
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
trap - INT TERM

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
