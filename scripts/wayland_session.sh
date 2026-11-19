# shellcheck shell=bash
# shellcheck disable=SC2034
# Starts a headless sway with config $1, logging to $2, and exports its
# WAYLAND_DISPLAY and SWAYSOCK. Sets sway_pid. Sourced by
# scripts/wayland_window_pixels.sh and scripts/wayland_atspi_tree.sh.
start_sway() {
  # A fresh runtime directory, so the compositor's sockets are the only ones.
  XDG_RUNTIME_DIR="$(mktemp -d)"
  export XDG_RUNTIME_DIR
  # Without DISPLAY winit cannot fall back to X11.
  unset DISPLAY WAYLAND_DISPLAY SWAYSOCK
  WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    sway --config "$1" >"$2" 2>&1 &
  sway_pid=$!
  for _ in $(seq 100); do
    compgen -G "$XDG_RUNTIME_DIR/sway-ipc.*.sock" >/dev/null && compgen -G "$XDG_RUNTIME_DIR/wayland-[0-9]" >/dev/null && break
    sleep 0.1
  done
  SWAYSOCK="$(compgen -G "$XDG_RUNTIME_DIR/sway-ipc.*.sock")"
  WAYLAND_DISPLAY="$(basename "$(compgen -G "$XDG_RUNTIME_DIR/wayland-[0-9]")")"
  export SWAYSOCK WAYLAND_DISPLAY
  swaymsg -t get_version
}
