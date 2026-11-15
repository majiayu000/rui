#!/usr/bin/env python3
"""Holds a wlr virtual pointer open on the Wayland compositor until killed.

A headless sway seat has no pointer device, so clients bind no wl_pointer and
the pointer events `swaymsg "seat - cursor ..."` sends reach nobody. While this
runs the seat has a pointer. Used by scripts/wayland_window_pixels.sh; it speaks
the Wayland wire protocol directly so CI needs no client library.
"""

import os
import socket
import struct

DISPLAY, REGISTRY, SYNC, MANAGER, POINTER = 1, 2, 3, 4, 5
MANAGER_INTERFACE = "zwlr_virtual_pointer_manager_v1"


def message(obj, opcode, payload=b""):
    return struct.pack("=II", obj, (8 + len(payload)) << 16 | opcode) + payload


def string(text):
    data = text.encode() + b"\0"
    return struct.pack("=I", len(data)) + data + b"\0" * (-len(data) % 4)


def events(sock):
    """Yield (object, opcode, body) for each event until the server hangs up."""
    buffer = b""
    while True:
        data = sock.recv(4096)
        if not data:
            return
        buffer += data
        while len(buffer) >= 8:
            obj, word = struct.unpack_from("=II", buffer)
            size = word >> 16
            if len(buffer) < size:
                break
            yield obj, word & 0xFFFF, buffer[8:size]
            buffer = buffer[size:]


def main():
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], os.environ["WAYLAND_DISPLAY"]))
    # wl_display.get_registry, then wl_display.sync to know when all globals came.
    sock.sendall(
        message(DISPLAY, 1, struct.pack("=I", REGISTRY))
        + message(DISPLAY, 0, struct.pack("=I", SYNC))
    )
    manager = None
    stream = events(sock)
    for obj, opcode, body in stream:
        if obj == DISPLAY and opcode == 0:
            raise SystemExit(f"wayland error: {body!r}")
        if obj == REGISTRY and opcode == 0:  # wl_registry.global
            name, length = struct.unpack_from("=II", body)
            interface = body[8 : 8 + length - 1].decode()
            if interface == MANAGER_INTERFACE:
                manager = name
        if obj == SYNC:  # wl_callback.done
            break
    if manager is None:
        raise SystemExit(f"the compositor has no {MANAGER_INTERFACE}")
    # wl_registry.bind the manager, then create_virtual_pointer on the default seat.
    sock.sendall(
        message(REGISTRY, 0, struct.pack("=I", manager) + string(MANAGER_INTERFACE) + struct.pack("=II", 1, MANAGER))
        + message(MANAGER, 0, struct.pack("=II", 0, POINTER))
    )
    for obj, opcode, body in stream:
        if obj == DISPLAY and opcode == 0:
            raise SystemExit(f"wayland error: {body!r}")


if __name__ == "__main__":
    main()
