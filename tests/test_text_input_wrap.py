"""#285 (TRE-REQ-016): `wrap` and `overflow` on a `text_input`.

A multiline input wraps by word (`wrap="word"`) or keeps each line whole and scrolls sideways
(`"none"`); a single-line input is one line however long, and `overflow` says what happens to the
part that does not fit: `"scroll"` (the caret stays in view), `"ellipsis"` (a cut line ended with
"…" until the input is edited, then it scrolls) or `"clip"`. Unset, both are as they were in 0.6.5.
Pixels come from `Window.snapshot()`, the caret from `Node.caret_rect()`.
"""

import pytest

from tess_re import Window

TEXT = "the quick brown fox jumps over the lazy dog and keeps going on"  # 62 characters
WHITE = (255, 255, 255, 255)


def scene(height=120, width=100, **props):
    window = Window(width=800, height=300)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    field = window.create("text_input", width=width, height=height, text=TEXT, fill=WHITE, caret_visible=False, **props)
    window.root.add_child(field)
    return window, field


def ink(window, field):
    """`(lines, left, right)` of the bright pixels inside the field's box."""
    rgba, width, _ = window.snapshot()
    x0, y0 = int(field.get("layout_x")), int(field.get("layout_y"))
    w, h = int(field.get("layout_width")), int(field.get("layout_height"))
    rows = [y for y in range(y0, y0 + h) if any(rgba[(y * width + x) * 4] > 100 for x in range(x0, x0 + w))]
    cols = [x - x0 for x in range(x0, x0 + w) if any(rgba[(y * width + x) * 4] > 100 for y in range(y0, y0 + h))]
    lines, last = 0, -10
    for y in rows:
        lines += y - last > 2
        last = y
    return lines, (min(cols) if cols else None), (max(cols) if cols else None)


def move_caret(field, offset):
    field.focus()
    field.set(selection=(offset, offset))


def test_unset_is_as_it_was_in_0_6_5():
    window, single = scene()
    assert (single.get("wrap"), single.get("overflow")) == (None, None)
    assert ink(window, single)[0] > 1, "a single-line input still wraps by word"
    window, multi = scene(multiline=True)
    assert (multi.get("wrap"), multi.get("overflow")) == (None, None)
    assert ink(window, multi)[0] == 1, "a multiline input still keeps its line whole"


def test_a_multiline_input_wraps_by_word_inside_its_box():
    window, field = scene(multiline=True, wrap="word")
    lines, left, right = ink(window, field)
    assert lines > 1
    assert right is not None and right < 100
    assert field.get("wrap") == "word"
    # No line is cut: the same words are on screen as in a wrapping text node.
    reference = window.create("text", width=100, text=TEXT, fill=WHITE)
    window.root.add_child(reference)
    assert lines == ink(window, reference)[0]


def test_a_wrapped_multiline_input_scrolls_down_to_its_caret_and_back():
    window, field = scene(height=40, multiline=True, wrap="word")
    move_caret(field, len(TEXT))
    x, y, w, h = field.caret_rect()
    assert y >= -1 and y + h <= 41, "the caret at the end is in view, not below the box"
    assert 0 <= x <= 100
    move_caret(field, 0)
    x, y, w, h = field.caret_rect()
    assert x == 0 and y >= -1, "back at the start the first line shows"


def test_wrap_none_scrolls_sideways_as_before():
    window, field = scene(multiline=True, wrap="none")
    assert ink(window, field)[0] == 1
    move_caret(field, len(TEXT))
    x, _, w, _ = field.caret_rect()
    assert x >= 0 and x + w <= 100.5, "the caret at the end of a long line is in view"
    window.simulate("key_down", key="home")
    assert field.caret_rect()[0] == 0
    window.simulate("key_down", key="end")
    x, _, w, _ = field.caret_rect()
    assert x >= 0 and x + w <= 100.5, "and after the End key"


def test_a_single_line_input_with_scroll_is_one_line_and_keeps_the_caret_in_view():
    window, field = scene(overflow="scroll")
    start = ink(window, field)
    assert start[0] == 1
    move_caret(field, len(TEXT))
    x, _, w, _ = field.caret_rect()
    assert 98 <= x + w <= 100.5, "at the end the caret is at the right edge, not 450 px away"
    _, left, right = ink(window, field)
    assert (right or 0) >= 95, "the text's end is what shows"
    move_caret(field, 0)
    x, _, _, _ = field.caret_rect()
    assert x == 0
    assert ink(window, field)[1] == start[1], "and back at the start the text starts at the left edge"


def test_scrolling_moves_only_as_far_as_the_caret_needs():
    window, field = scene(overflow="scroll")
    move_caret(field, len(TEXT))
    end = field.caret_rect()[0]
    before = ink(window, field)
    move_caret(field, len(TEXT) - 3)  # a few characters left of the end: no need to scroll
    assert ink(window, field) == before, "the text stays where it is while the caret is in view"
    assert field.caret_rect()[0] < end


def test_a_wider_box_brings_the_scrolled_text_back():
    window, field = scene(overflow="scroll")
    start = ink(window, field)
    move_caret(field, len(TEXT))
    assert ink(window, field) != start
    field.set(width=700)
    x, _, _, _ = field.caret_rect()
    assert x < 700
    assert ink(window, field)[1] == start[1], "the whole line fits, so nothing is scrolled off"


def test_a_scrolling_input_scrolls_while_typing():
    window, field = scene(overflow="scroll")
    move_caret(field, len(TEXT))
    for char in "XYZ":
        window.simulate("input", text=char)
    x, _, w, _ = field.caret_rect()
    assert x + w <= 100.5, "typing keeps the caret in view"
    assert field.get("text").endswith("XYZ")


def test_ellipsis_cuts_the_line_until_the_input_is_edited():
    window, field = scene(overflow="ellipsis")
    # What the engine already draws for a text node with an ellipsis, in a box of the same size.
    other = Window(width=800, height=300)
    other.root.set(fill=(0, 0, 0, 255), padding=0)
    label = other.create("text", width=100, height=120, text=TEXT, wrap="none", overflow="ellipsis", fill=WHITE)
    other.root.add_child(label)
    assert ink(window, field) == ink(other, label)
    assert window.snapshot()[0] == other.snapshot()[0]
    # Focus it and move to the end: now it scrolls, and shows the end.
    move_caret(field, len(TEXT))
    assert (ink(window, field)[2] or 0) >= 95
    assert field.caret_rect()[0] <= 100
    # Move the focus away: the ellipsis is back, and the start with it.
    other_box = window.create("box", width=10, height=10, focusable=True)
    window.root.add_child(other_box)
    other_box.focus()
    assert window.snapshot()[0] == other.snapshot()[0]


def test_clip_cuts_at_the_box_edge_and_never_scrolls():
    window, field = scene(overflow="clip")
    lines, left, right = ink(window, field)
    assert lines == 1 and (right or 0) <= 99
    move_caret(field, len(TEXT))
    assert ink(window, field) == (lines, left, right), "clip does not follow the caret"
    assert field.caret_rect()[0] > 100, "so the caret is past the edge"


def test_clearing_a_property_gives_back_the_old_behaviour():
    window, field = scene(overflow="scroll")
    field.set(overflow=None)
    assert field.get("overflow") is None
    assert ink(window, field)[0] > 1, "a single-line input wraps again"
    field.set(multiline=True, wrap="word")
    field.set(wrap=None)
    assert ink(window, field)[0] == 1


def test_wrap_does_not_change_a_single_line_input_and_overflow_does_not_change_a_multiline_one():
    window, field = scene(overflow="scroll", wrap="word")
    assert ink(window, field)[0] == 1, "a single-line input never wraps once overflow is set"
    window, field = scene(multiline=True, wrap="word", overflow="clip")
    assert ink(window, field)[0] > 1, "overflow means nothing to a multiline input"


@pytest.mark.parametrize(
    "props",
    [{"wrap": "char"}, {"wrap": True}, {"wrap": 1}, {"overflow": "fade"}, {"overflow": "word"}, {"overflow": 3}],
)
def test_a_bad_value_raises_and_changes_nothing(props):
    window, field = scene()
    key = next(iter(props))
    with pytest.raises(ValueError, match=key):
        field.set(**props)
    assert field.get(key) is None
    with pytest.raises(ValueError, match=key):
        window.create("text_input", **props)


def test_other_nodes_still_refuse_them_and_text_nodes_keep_their_own_values():
    window = Window(width=100, height=100)
    box = window.create("box", width=10, height=10)
    for key, value in (("wrap", "word"), ("overflow", "clip")):
        with pytest.raises(ValueError, match="text or text_input"):
            box.set(**{key: value})
    label = window.create("text", text="x")
    assert (label.get("wrap"), label.get("overflow")) == ("word", "clip")
    with pytest.raises(ValueError, match="overflow"):
        label.set(overflow="scroll")
