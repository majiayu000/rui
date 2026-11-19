#!/usr/bin/env python3
"""Read an example's accessibility tree back over AT-SPI and drive it.

Finds the window titled argv[1] the way a screen reader does and prints its
accessibility tree. For examples/interaction_acceptance (argv[2], the
default) it checks the controls' roles, names, states and text, focuses the
Filter field, sets its text, selects in it, moves its caret, reads its
characters' extents and presses Save and Cancel through AT-SPI, checking each
reached the app. For examples/controls it sets the Volume slider to a number.
Exits non-zero on the first failure. Run by scripts/linux_atspi_tree.sh.
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


def print_desktop():
    """What AT-SPI holds, for a failed wait."""
    desktop = Atspi.get_desktop(0)
    print(f"desktop: {desktop.get_child_count()} applications")
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is None:
            continue
        app.clear_cache()
        print(f"  application {app.get_name()!r}, {app.get_child_count()} windows")
        for j in range(app.get_child_count()):
            window = app.get_child_at_index(j)
            if window is not None:
                for depth, node in walk(window):
                    print("    " + "  " * depth + describe(node))


# What the app announced (object:announcement), in order.
ANNOUNCEMENTS = []


def on_announcement(event):
    ANNOUNCEMENTS.append(event.any_data)


def wait_for(what, find):
    deadline = time.monotonic() + TIMEOUT
    while True:
        # Deliver the events that arrived, such as announcements.
        while GLib.MainContext.default().iteration(False):
            pass
        try:
            found = find()
        except GLib.Error:
            found = None
        if found is not None:
            return found
        if time.monotonic() > deadline:
            print_desktop()
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
    example = sys.argv[2] if len(sys.argv) > 2 else "interaction_acceptance"
    # Listen from the start: AccessKit only sends events someone listens for.
    listener = Atspi.EventListener.new(on_announcement)
    listener.register("object:announcement")
    window = wait_for(f"window {title!r}", lambda: find_window(title))
    check_example = {
        "interaction_acceptance": check_interaction_acceptance,
        "controls": check_controls,
    }[example]
    check_example(window)


def print_tree(window, role, name):
    # The first tree arrives with the first frame after the bus asks for it.
    wait_for(f"the {name} {role}", lambda: find_node(window, role, name))
    for depth, node in walk(window):
        print("  " * depth + describe(node))


def check_controls(window):
    """examples/controls: set the Volume slider to a number."""
    print_tree(window, "slider", "Volume")
    volume = find_node(window, "slider", "Volume")
    value = volume.get_value_iface()
    check(Atspi.Value.get_current_value(value) == 40.0, "Volume starts at 40")
    Atspi.Value.set_current_value(value, 75.0)
    wait_for("the slider to move", lambda: find_node(window, "label", "Volume: 75"))
    volume = find_node(window, "slider", "Volume")
    check(
        Atspi.Value.get_current_value(volume.get_value_iface()) == 75.0,
        "setting the value moved the Volume slider to 75",
    )


def check_announced(window, message):
    """The app announced `message` as action feedback: an object:announcement
    event, from a live label under the window that says it."""
    wait_for(f"the announcement {message!r}", lambda: message in ANNOUNCEMENTS or None)
    print(f"announcements so far: {ANNOUNCEMENTS}")
    check(True, f"the app announced {message!r}")
    label = find_node(window, "label", message)
    check(label is not None, f"a label under the window says {message!r}")


def selection(window, name):
    node = find_node(window, "entry", name)
    text = node.get_text_iface()
    if Atspi.Text.get_n_selections(text) != 1:
        return None
    selected = Atspi.Text.get_selection(text, 0)
    return (selected.start_offset, selected.end_offset)


def check_text_editing(window):
    """Select, move the caret and read where each character is, in the
    focused Filter field holding "zzz"."""
    field = find_node(window, "entry", "Filter")
    Atspi.Text.set_selection(field.get_text_iface(), 0, 1, 3)
    wait_for("the selection", lambda: selection(window, "Filter") == (1, 3) or None)
    field = find_node(window, "entry", "Filter")
    text = field.get_text_iface()
    check(
        Atspi.Text.get_text(text, 1, 3) == "zz",
        "set selection selected characters 1-3 of Filter",
    )

    Atspi.Text.set_caret_offset(text, 2)
    wait_for(
        "the caret to move",
        lambda: Atspi.Text.get_caret_offset(
            find_node(window, "entry", "Filter").get_text_iface()
        )
        == 2
        or None,
    )
    check(selection(window, "Filter") is None, "moving the caret cleared the selection")

    text = find_node(window, "entry", "Filter").get_text_iface()
    window_coords = Atspi.CoordType.WINDOW
    boxes = [Atspi.Text.get_character_extents(text, i, window_coords) for i in range(3)]
    for i, box in enumerate(boxes):
        print(f"character {i}: {box.x},{box.y} {box.width}x{box.height}")
    check(all(box.width > 0 and box.height > 0 for box in boxes), "each character has a size")
    check(
        all(left.x + left.width <= right.x + 1 for left, right in zip(boxes, boxes[1:])),
        "the characters go left to right",
    )
    field_box = Atspi.Component.get_extents(
        find_node(window, "entry", "Filter").get_component_iface(), window_coords
    )
    check(
        all(
            field_box.x <= box.x and box.x + box.width <= field_box.x + field_box.width
            for box in boxes
        ),
        "the characters lie inside the field",
    )


def check_interaction_acceptance(window):
    print_tree(window, "push button", "Save")

    check(window.get_role_name() == "frame", "the window is a frame")
    heading = find_node(window, "label", "RUI interaction acceptance")
    check(heading is not None, "the heading is a label")
    save = find_node(window, "push button", "Save")
    check(save.get_state_set().contains(Atspi.StateType.ENABLED), "Save is enabled")
    # Window coordinates: under Wayland a client cannot learn where its window
    # is on screen, so screen coordinates there are the window's.
    extents = Atspi.Component.get_extents(save.get_component_iface(), Atspi.CoordType.WINDOW)
    print(f"Save extents in the window: {extents.x},{extents.y} {extents.width}x{extents.height}")
    check(extents.width > 0 and extents.height > 0, "Save has a size in the window")
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
    check_text_editing(window)

    Atspi.Action.do_action(save.get_action_iface(), 0)
    wait_for("the confirm dialog", lambda: find_node(window, "dialog", "Confirm save"))
    check(True, "pressing Save opened the confirm dialog")
    check_announced(window, "Save activated")
    cancel = wait_for("the Cancel button", lambda: find_node(window, "push button", "Cancel"))
    Atspi.Action.do_action(cancel.get_action_iface(), 0)
    wait_for("the canceled status", lambda: find_node(window, "label", "Status: Canceled"))
    check_announced(window, "Cancel activated")
    check(True, "pressing Cancel reached the app")


if __name__ == "__main__":
    main()
