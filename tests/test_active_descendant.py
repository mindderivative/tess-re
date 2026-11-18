"""TRE-REQ-023: `active_descendant` names the node a screen reader treats as a focused node's current item.

A text field with a list under it keeps the keyboard; Up and Down move a selection. `field.set(active_descendant=row)`
tells assistive technology which row that is, and keyboard focus does not move: `get("focused")` stays True on the
field and no `focus` or `unfocus` event fires. What a screen reader receives is tested in
`test_layer_window_live.py` (a real AT-SPI bus); here the property itself, headless.
"""

import pytest
from helpers import add

from tess_re import Window


def scene():
    window = Window(width=400, height=300)
    field = add(window, "text_input", width=200, height=30, text="fi")
    first = add(window, "box", width=200, height=24, role="listitem", label="Firefox")
    second = add(window, "box", width=200, height=24, role="listitem", label="Files")
    window.simulate("focus", node=field)
    return window, field, first, second


def test_it_reads_back_what_was_set_and_defaults_to_none():
    _, field, first, second = scene()
    assert field.get("active_descendant") is None
    field.set(active_descendant=first)
    assert field.get("active_descendant") == first
    field.set(active_descendant=second)
    assert field.get("active_descendant") == second
    field.set(active_descendant=None)
    assert field.get("active_descendant") is None


def test_setting_it_does_not_move_focus_or_fire_focus_events():
    window, field, first, _ = scene()
    heard = []
    field.on("focus", lambda e: heard.append("focus"))
    field.on("unfocus", lambda e: heard.append("unfocus"))
    first.on("focus", lambda e: heard.append("row focus"))
    field.set(active_descendant=first)
    assert field.get("focused") and not first.get("focused")
    assert heard == []


def test_a_destroyed_target_reads_as_none():
    window, field, first, _ = scene()
    field.set(active_descendant=first)
    first.destroy()
    assert field.get("active_descendant") is None


def test_it_takes_one_node_or_none_and_a_node_of_this_window():
    window, field, first, second = scene()
    with pytest.raises(ValueError, match="a Node or None"):
        field.set(active_descendant=[first, second])
    with pytest.raises(ValueError, match="a Node or None"):
        field.set(active_descendant="row")
    with pytest.raises(ValueError, match="different Window"):
        field.set(active_descendant=add(Window(), "box", width=10, height=10))
    assert field.get("active_descendant") is None, "a refused set changes nothing"


def test_it_works_on_any_node_not_only_a_text_input():
    window = Window()
    combo = add(window, "box", width=100, height=20, role="button", focusable=True)
    item = add(window, "box", width=100, height=20, role="listitem")
    combo.set(active_descendant=item)
    assert combo.get("active_descendant") == item
