"""T-018 (TRE-REQ-018): `tab_navigates` lets Tab and Shift+Tab leave a multiline `text_input`.

A multiline input takes Tab as a tab character (indentation), which holds a keyboard user in the
field. `tab_navigates=True` makes it leave like a single-line one does: focus moves, nothing is typed.
Off by default, so every existing multiline input behaves as before.
"""

import pytest
from helpers import add

from tess_re import Window


def scene(**props):
    window = Window(width=400, height=300)
    first = add(window, "text_input", width=200, height=60, text="ab", multiline=True, **props)
    middle = add(window, "text_input", width=200, height=30, text="")
    last = add(window, "text_input", width=200, height=30, text="")
    window.simulate("focus", node=middle)
    window.simulate("focus", node=first)
    return window, first, middle, last


def test_it_is_off_by_default_and_a_multiline_input_still_types_a_tab():
    window, first, middle, _ = scene()
    assert first.get("tab_navigates") is False
    window.simulate("key_down", key="tab")
    assert first.get("text") == "ab\t"
    assert first.get("focused") and not middle.get("focused")


def test_tab_moves_focus_to_the_next_node_and_types_nothing():
    window, first, middle, _ = scene(tab_navigates=True)
    assert first.get("tab_navigates") is True
    window.simulate("key_down", key="tab")
    assert first.get("text") == "ab"
    assert middle.get("focused") and not first.get("focused")


def test_shift_tab_goes_back_and_types_nothing():
    window, first, middle, _ = scene(tab_navigates=True)
    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="tab", shift=True)
    assert first.get("focused") and not middle.get("focused")
    assert first.get("text") == "ab"


def test_it_can_be_turned_on_and_off_while_the_input_lives():
    window, first, middle, _ = scene()
    first.set(tab_navigates=True)
    window.simulate("key_down", key="tab")
    assert middle.get("focused") and first.get("text") == "ab"
    window.simulate("focus", node=first)
    first.set(tab_navigates=False)
    window.simulate("key_down", key="tab")
    assert first.get("text") == "ab\t"


def test_a_single_line_input_is_unchanged_by_it():
    window = Window(width=400, height=300)
    one = add(window, "text_input", width=200, height=30, text="ab", tab_navigates=True)
    two = add(window, "text_input", width=200, height=30, text="")
    window.simulate("focus", node=one)
    window.simulate("key_down", key="tab")
    assert two.get("focused") and one.get("text") == "ab"


def test_a_node_that_is_not_a_text_input_refuses_it():
    window = Window()
    with pytest.raises(ValueError, match="applies only to a text_input"):
        window.create("box", tab_navigates=True)
