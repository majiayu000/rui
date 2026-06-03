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
LOG="${RUI_NATIVE_DOGFOOD_LOG:-target/rui-native-dogfood.log}"
mkdir -p "$(dirname "$PROFILE")" "$(dirname "$LOG")"
rm -f "$PROFILE" "$LOG"

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
cargo build --example native_dogfood >"$LOG" 2>&1

RUI_NATIVE_DOGFOOD_PROFILE="$PROFILE" \
RUI_NATIVE_DOGFOOD_TEXT="$TEXT" \
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_AUTOMATION=1 \
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

echo "native dogfood profile: $PROFILE"
