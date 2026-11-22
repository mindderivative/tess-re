"""UXREQ-007 (0.6.12): Escape during a held gesture -- a captured pointer, a scrollbar thumb
drag, a dock drag -- ends it with one `pointer_cancel` (no `pointer_up`, no `click`), restores
what the gesture moved, and is consumed: it reaches no `key_down`, terminal, text input or
overlay. Acceptance: Drag and Drop AC-DND-7, Drag and Drop Pattern AC-DNP-4/9, Dock AC-DCK-6.
Headless."""

import sys

import pytest

from tess_re import Window
from helpers import add
from test_docking import record, two_zones


def drag_handle(window):
    """A box that captures the pointer on press, and logs what reaches it."""
    handle = add(window, "box", width=60, height=40, focusable=True)
    seen = []
    handle.on("pointer_down", lambda e: (handle.capture_pointer(), seen.append("down")))
    for name in ("pointer_move", "pointer_up", "pointer_cancel", "click", "key_down"):
        handle.on(name, lambda e, name=name: seen.append(name))
    return handle, seen


def test_escape_cancels_a_capture_with_one_pointer_cancel_and_no_release_or_click():
    window = Window(width=300, height=200)
    handle, seen = drag_handle(window)
    window.simulate("pointer_down", node=handle)
    window.simulate("pointer_move", node=handle)
    window.simulate("key_down", key="escape")
    assert seen == ["down", "pointer_move", "pointer_cancel"]
    # The button's release, when it comes, reaches nothing; the capture is gone.
    window.simulate("pointer_up", node=handle)
    window.simulate("pointer_move", x=250, y=150)
    assert seen == ["down", "pointer_move", "pointer_cancel"]
    window.simulate("key_up", key="escape")
    assert seen == ["down", "pointer_move", "pointer_cancel"]


def test_a_new_press_after_a_cancel_works_as_usual():
    window = Window(width=300, height=200)
    handle, seen = drag_handle(window)
    window.simulate("pointer_down", node=handle)
    window.simulate("key_down", key="escape")
    window.simulate("pointer_up", node=handle)
    del seen[:]
    window.simulate("pointer_down", node=handle)
    window.simulate("pointer_up", node=handle)
    assert seen == ["down", "pointer_up", "click"]


def test_escape_without_a_gesture_is_unchanged():
    window = Window(width=300, height=200)
    handle, seen = drag_handle(window)
    window.simulate("focus", node=handle)
    window.simulate("key_down", key="escape")
    assert seen == ["key_down"]


def test_the_escape_of_a_gesture_does_not_also_dismiss_an_overlay_or_reach_the_focus():
    window = Window(width=300, height=200)
    handle, seen = drag_handle(window)
    menu = window.create("box", width=60, height=60)
    window.show_layer(menu)
    dismissed = []
    menu.on("dismiss", lambda: dismissed.append(1))
    window.simulate("focus", node=handle)
    handle.capture_pointer()  # a press outside a layer would dismiss it, so hold the gesture by hand
    window.simulate("key_down", key="escape")
    assert dismissed == [] and "key_down" not in seen and seen == ["pointer_cancel"]
    # The next Escape, with no gesture, closes it as before.
    window.simulate("key_down", key="escape")
    assert dismissed == [1]


def test_the_escape_of_a_gesture_leaves_a_focused_text_input_alone():
    window = Window(width=300, height=200)
    handle, seen = drag_handle(window)
    field = add(window, "text_input", width=100, height=30, text="draft")
    window.simulate("focus", node=field)
    window.simulate("pointer_down", node=handle)
    window.simulate("key_down", key="escape")
    assert field.get("text") == "draft"
    assert seen == ["down", "pointer_cancel"]


@pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")
def test_a_focused_terminal_gets_no_escape_byte_from_a_gesture(tmp_path):
    from test_terminal_escape import logged, make_window

    window, before, term, after, log = make_window(tmp_path)
    handle = add(window, "box", width=40, height=20)
    cancelled = []
    handle.on("pointer_cancel", lambda e: cancelled.append(1))
    window.simulate("click", node=term)
    assert term.get("focused") is True
    handle.capture_pointer()
    window.simulate("key_down", key="escape")
    assert cancelled == [1]
    window.simulate("input", text="x")
    window.simulate("key_down", key="enter")
    assert "^[" not in logged(log, "x")
    # With no gesture the same Escape reaches the shell.
    window.simulate("key_down", key="escape")
    window.simulate("input", text="y")
    window.simulate("key_down", key="enter")
    assert logged(log, "^[y").count("^[") == 1


def test_escape_cancels_a_dock_drag_and_the_panel_stays_with_zero_moves():
    """AC-DCK-6."""
    window, left, right, panel = two_zones()
    seen = record(window)
    window.on("dock_cancel", lambda e: seen.append(("cancel", e.panel)))
    moved = []
    panel.on("pointer_move", lambda e: moved.append(1))

    window.start_panel_drag(panel)
    window.simulate("pointer_move", node=right)
    assert seen == [("target", "right")]
    window.simulate("key_down", key="escape")
    # The highlight comes down at once (no side), then the cancel; no drop.
    assert seen == [("target", "right"), ("target", None), ("cancel", panel)]
    assert left.children() == [panel] and right.children() == []
    window.simulate("pointer_up", node=right)
    window.simulate("pointer_move", node=right)
    assert seen == [("target", "right"), ("target", None), ("cancel", panel)]
    assert left.children() == [panel] and moved == []


def test_a_dock_cancel_over_no_zone_sends_no_target_and_a_drag_can_start_again():
    window, left, right, panel = two_zones()
    seen = record(window)
    window.on("dock_cancel", lambda e: seen.append("cancel"))
    window.start_panel_drag(panel)
    window.simulate("key_down", key="escape")
    assert seen == ["cancel"]
    window.start_panel_drag(panel)
    window.simulate("pointer_move", node=right)
    window.simulate("pointer_up", node=right)
    assert seen[-1] == ("drop", "right", panel)


def test_escape_during_a_thumb_drag_puts_the_view_back():
    window = Window(width=300, height=200)
    window.root.set(padding=0)
    view = window.create("scroll_view", orientation="both", width=200, height=100)
    view.add_child(window.create("box", width=800, height=400, flex_shrink=0))
    window.root.add_child(view)
    window.advance(0)
    cancelled = []
    view.on("pointer_cancel", lambda e: cancelled.append(1))
    view.set(scroll_offset=40)
    # The thumb of the vertical bar is at the right edge; grab it and drag down.
    start = view.get("scroll_offset")
    window.simulate("pointer_down", node=view, x=194, y=2 + start * 52 / 300 + 4)
    window.simulate("pointer_move", node=view, x=194, y=60)
    assert view.get("scroll_offset") > start
    window.simulate("key_down", key="escape")
    assert view.get("scroll_offset") == start
    window.simulate("pointer_move", node=view, x=194, y=80)
    assert view.get("scroll_offset") == start, "the drag is over"
    window.simulate("pointer_up", node=view, x=194, y=80)
    assert view.get("scroll_offset") == start
