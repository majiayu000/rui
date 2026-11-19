#!/usr/bin/env python3
"""Read examples/interaction_acceptance back over AT-SPI and drive it.

Finds the window titled argv[1] the way a screen reader does, prints its
accessibility tree, checks the controls' roles, names, states and text, then
focuses the Filter field, sets its text and presses Save and Cancel through
AT-SPI, checking each reached the app. Exits non-zero on the first failure.
Run by scripts/linux_atspi_tree.sh.
"""

import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib  # noqa: E402

TIMEOUT = 20.0


def windows():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is None:
            continue
        # Read every property from the app, not from libatspi's cache, so
        # each check sees the tree the app last published.
        app.set_cache_mask(Atspi.Cache.NONE)
        app.clear_cache()
        for j in range(app.get_child_count()):
            window = app.get_child_at_index(j)
            if window is not None:
                yield window


def walk(node, depth=0):
    yield depth, node
    for i in range(node.get_child_count()):
        child = node.get_child_at_index(i)
        if child is not None:
            yield from walk(child, depth + 1)


def describe(node):
    states = node.get_state_set()
    flags = [
        name
        for name, state in (
            ("enabled", Atspi.StateType.ENABLED),
            ("focusable", Atspi.StateType.FOCUSABLE),
            ("focused", Atspi.StateType.FOCUSED),
            ("checked", Atspi.StateType.CHECKED),
        )
        if states.contains(state)
    ]
    text = node.get_text_iface()
    value = f" text={Atspi.Text.get_text(text, 0, -1)!r}" if text else ""
    return f"{node.get_role_name()} {node.get_name()!r}{value} [{' '.join(flags)}]"


def wait_for(what, find):
    deadline = time.monotonic() + TIMEOUT
    while True:
        try:
            found = find()
        except GLib.Error:
            found = None
        if found is not None:
            return found
        if time.monotonic() > deadline:
            sys.exit(f"FAIL: timed out waiting for {what}")
        time.sleep(0.2)


def find_window(title):
    return next((w for w in windows() if w.get_name() == title), None)


def find_node(window, role, name):
    for _, node in walk(window):
        if node.get_role_name() == role and node.get_name() == name:
            return node
    return None


def check(condition, message):
    if not condition:
        sys.exit(f"FAIL: {message}")
    print(f"ok: {message}")


def main():
    title = sys.argv[1]
    window = wait_for(f"window {title!r}", lambda: find_window(title))
    # The first tree arrives with the first frame after the bus asks for it.
    wait_for("the Save button", lambda: find_node(window, "push button", "Save"))
    for depth, node in walk(window):
        print("  " * depth + describe(node))

    check(window.get_role_name() == "frame", "the window is a frame")
    heading = find_node(window, "label", "RUI interaction acceptance")
    check(heading is not None, "the heading is a label")
    save = find_node(window, "push button", "Save")
    check(save.get_state_set().contains(Atspi.StateType.ENABLED), "Save is enabled")
    disabled = find_node(window, "push button", "Disabled")
    check(disabled is not None, "the disabled button is exposed")
    check(
        not disabled.get_state_set().contains(Atspi.StateType.ENABLED),
        "the disabled button is not enabled",
    )
    filter_field = find_node(window, "entry", "Filter")
    check(filter_field is not None, "the Filter field is an entry")
    check(find_node(window, "entry", "Draft") is not None, "the Draft field is an entry")

    Atspi.Component.grab_focus(filter_field.get_component_iface())
    def focused_filter():
        node = find_node(window, "entry", "Filter")
        focused = node is not None and node.get_state_set().contains(Atspi.StateType.FOCUSED)
        return node if focused else None

    wait_for("the Filter field to take focus", focused_filter)
    check(True, "focus reached the Filter field")

    Atspi.EditableText.set_text_contents(filter_field.get_editable_text_iface(), "zzz")
    wait_for(
        "the filter to hide the note",
        lambda: find_node(window, "label", "No matching notes"),
    )
    field = find_node(window, "entry", "Filter")
    check(
        Atspi.Text.get_text(field.get_text_iface(), 0, -1) == "zzz",
        "set text reached the Filter field",
    )

    Atspi.Action.do_action(save.get_action_iface(), 0)
    wait_for("the confirm dialog", lambda: find_node(window, "dialog", "Confirm save"))
    check(True, "pressing Save opened the confirm dialog")
    cancel = wait_for("the Cancel button", lambda: find_node(window, "push button", "Cancel"))
    Atspi.Action.do_action(cancel.get_action_iface(), 0)
    wait_for("the canceled status", lambda: find_node(window, "label", "Status: Canceled"))
    check(True, "pressing Cancel reached the app")


if __name__ == "__main__":
    main()
