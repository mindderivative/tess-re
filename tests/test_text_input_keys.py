"""#288 (TRE-REQ-017): caret movement by visual row, Ctrl word keys, Insert and overwrite, and
double and triple click in a `text_input`. Keys are `Window.simulate("key_down", ...)`, clicks
`pointer_down`/`pointer_up`, positions `selection` and `caret_rect()`; nothing needs a display."""

import pytest

from tess_re import Window

PARAGRAPH = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda"  # six rows at 100 px
WHITE = (255, 255, 255, 255)


def field(text=PARAGRAPH, width=100, height=60, **props):
    window = Window(width=400, height=300)
    window.root.set(fill=(0, 0, 0, 255), padding=0)
    node = window.create("text_input", width=width, height=height, text=text, fill=WHITE,
                         caret_visible=False, **props)
    window.root.add_child(node)
    node.focus()
    return window, node


def wrapped(text=PARAGRAPH, **props):
    return field(text, multiline=True, wrap="word", **props)


def press(window, key, **modifiers):
    window.simulate("key_down", key=key, **modifiers)


def place(node, offset, end=None):
    node.set(selection=(offset, offset if end is None else end))


def row_of(node, offset):
    """The index of the visual row the character at `offset` is on."""
    y = node.text_rects(offset, offset + 1)[0][1]
    tops = sorted({round(r[1]) for r in node.text_rects(0, len(node.get("text")))})
    return tops.index(round(y))


def at(node):
    return node.get("selection")[1]


def click(window, node, offset, times=1):
    x, y, w, h = node.text_rects(offset, offset + 1)[0]
    px, py = node.get("layout_x") + x + w / 2, node.get("layout_y") + y + h / 2
    for _ in range(times):
        window.simulate("pointer_down", x=px, y=py)
        window.simulate("pointer_up", x=px, y=py)


# -- Up and Down by row ----------------------------------------------------------------------


def test_down_and_up_move_one_visual_row_at_the_nearest_column():
    window, node = wrapped()
    place(node, 12)  # the 2nd character of row 2
    start_row = row_of(node, at(node))
    press(window, "arrow_down")
    assert row_of(node, at(node)) == start_row + 1
    press(window, "arrow_up")
    assert row_of(node, at(node)) == start_row


def test_the_goal_column_survives_a_short_row():
    text = "alpha beta gamma delta epsilon\nxy\nalpha beta gamma delta epsilon"
    window, node = wrapped(text)
    place(node, 14)  # well to the right on a long row
    seen = [at(node)]
    for _ in range(4):
        press(window, "arrow_down")
        seen.append(at(node))
    for _ in range(4):
        press(window, "arrow_up")
        seen.append(at(node))
    assert seen[-1] == seen[0], seen
    assert seen[3] != seen[2], "it did move on past the short row"


def test_up_on_the_first_row_goes_to_the_start_and_down_on_the_last_to_the_end():
    window, node = wrapped()
    place(node, 4)
    press(window, "arrow_up")
    assert at(node) == 0
    place(node, len(PARAGRAPH) - 3)
    press(window, "arrow_down")
    assert at(node) == len(PARAGRAPH)


def test_down_over_a_selection_collapses_it_without_moving_further():
    window, node = wrapped()
    place(node, 2, 8)
    press(window, "arrow_down")
    assert node.get("selection") == (8, 8)
    place(node, 2, 8)
    press(window, "arrow_up")
    assert node.get("selection") == (2, 2)


# -- Home, End and the page keys ---------------------------------------------------------------


def test_home_and_end_go_to_the_start_and_end_of_the_visual_row():
    window, node = wrapped()
    place(node, 15)
    row = row_of(node, 15)
    press(window, "home")
    home = at(node)
    press(window, "end")
    end = at(node)
    assert home < 15 <= end
    assert row_of(node, home) == row
    assert PARAGRAPH[home - 1] == " " or home == 0, "row 2 starts after the space the wrap fell on"
    assert PARAGRAPH[end - 1] != " ", "End stops before the spaces the wrap fell on"
    assert row_of(node, end - 1) == row


def test_home_and_end_with_ctrl_go_to_the_ends_of_the_whole_text():
    window, node = wrapped()
    place(node, 15)
    press(window, "end", ctrl=True)
    assert at(node) == len(PARAGRAPH)
    press(window, "home", ctrl=True)
    assert at(node) == 0
    place(node, 15)
    press(window, "home", ctrl=True, shift=True)
    assert node.get("selection") == (15, 0)


def test_page_down_moves_by_the_rows_shown_and_the_view_follows():
    window, node = wrapped(height=60)  # three rows fit
    place(node, 2)
    press(window, "page_down")
    assert row_of(node, at(node)) == 3
    x, y, w, h = node.caret_rect()
    assert y >= -1 and y + h <= 61, "the caret is in the box: the view followed"
    press(window, "page_up")
    assert row_of(node, at(node)) == 0
    press(window, "page_up")
    assert at(node) == 0, "at the first row Page Up goes to the start"
    place(node, 2)
    for _ in range(3):
        press(window, "page_down")
    assert at(node) == len(PARAGRAPH), "and Page Down at the last row to the end"


def test_shift_with_home_end_and_page_keys_extends_the_selection():
    window, node = wrapped()
    place(node, 15)
    press(window, "end", shift=True)
    start, end = node.get("selection")
    assert start == 15 and end > 15
    place(node, 15)
    press(window, "page_down", shift=True)
    assert node.get("selection")[0] == 15 and at(node) > 15


# -- Shift selection --------------------------------------------------------------------------


def test_shift_right_and_left_grow_and_shrink_by_one_character():
    window, node = wrapped()
    place(node, 5)
    for _ in range(3):
        press(window, "arrow_right", shift=True)
    assert node.get("selection") == (5, 8)
    press(window, "arrow_left", shift=True)
    assert node.get("selection") == (5, 7)


def test_shift_down_and_up_select_by_row_around_the_anchor():
    window, node = wrapped()
    place(node, 12)
    press(window, "arrow_down", shift=True)
    first = node.get("selection")
    assert first[0] == 12 and row_of(node, first[1]) == row_of(node, 12) + 1
    press(window, "arrow_down", shift=True)
    assert row_of(node, at(node)) == row_of(node, 12) + 2
    press(window, "arrow_up", shift=True)
    press(window, "arrow_up", shift=True)
    assert node.get("selection") == (12, 12), "back at the anchor: nothing selected"
    press(window, "arrow_up", shift=True)
    start, end = node.get("selection")
    assert start == 12 and end < 12, "one more selects on the other side of the anchor"


# -- Backspace and Delete ----------------------------------------------------------------------


def test_backspace_and_delete_remove_one_character_each_way():
    window, node = wrapped("abcdef")
    place(node, 3)
    press(window, "backspace")
    assert node.get("text") == "abdef" and at(node) == 2
    press(window, "delete")
    assert node.get("text") == "abef"


def test_backspace_and_delete_over_a_selection_remove_it():
    window, node = wrapped("abcdef")
    place(node, 1, 4)
    press(window, "delete")
    assert node.get("text") == "aef"
    place(node, 0, 2)
    press(window, "backspace")
    assert node.get("text") == "f"


def test_backspace_at_a_row_start_and_delete_at_a_row_end_remove_the_wrap_space_or_break():
    window, node = wrapped()
    place(node, 0)
    press(window, "arrow_down")
    press(window, "home")
    row_start = at(node)
    assert PARAGRAPH[row_start - 1] == " "
    press(window, "backspace")
    assert node.get("text") == PARAGRAPH[: row_start - 1] + PARAGRAPH[row_start:], "the space the wrap fell on"
    window, node = wrapped("one\ntwo")
    place(node, 4)
    press(window, "backspace")
    assert node.get("text") == "onetwo", "at a line break the two lines join"
    window, node = wrapped("one\ntwo")
    place(node, 3)
    press(window, "delete")
    assert node.get("text") == "onetwo"


def test_backspace_at_the_very_start_and_delete_at_the_very_end_do_nothing():
    window, node = wrapped("abc")
    place(node, 0)
    press(window, "backspace")
    place(node, 3)
    press(window, "delete")
    assert node.get("text") == "abc"


# -- Ctrl word keys -----------------------------------------------------------------------------


@pytest.mark.parametrize("props", [{}, {"multiline": True}, {"multiline": True, "wrap": "word"}, {"overflow": "scroll"}])
def test_ctrl_left_and_right_move_by_word_in_any_text_input(props):
    window, node = field("hello world foo", **props)
    place(node, 0)
    press(window, "arrow_right", ctrl=True)
    assert at(node) == 6
    press(window, "arrow_right", ctrl=True)
    assert at(node) == 12
    press(window, "arrow_left", ctrl=True)
    assert at(node) == 6
    press(window, "arrow_left", ctrl=True, shift=True)
    assert node.get("selection") == (6, 0)


def test_ctrl_backspace_and_ctrl_delete_remove_a_word():
    window, node = field("hello world foo")
    place(node, 11)
    press(window, "backspace", ctrl=True)
    assert node.get("text") == "hello  foo" and at(node) == 6
    window, node = field("hello world foo")
    place(node, 6)
    press(window, "delete", ctrl=True)
    assert node.get("text") == "hello foo"
    window, node = field("hello world")
    place(node, 2, 7)
    press(window, "backspace", ctrl=True)
    assert node.get("text") == "heorld", "a selection goes first"


# -- Insert and overwrite -----------------------------------------------------------------------


def test_insert_toggles_overwrite_which_is_a_property_and_an_event():
    window, node = field("abcdefgh")
    seen = []
    node.on("overwrite_change", lambda e: seen.append((e.old_value, e.new_value)))
    assert node.get("overwrite") is False
    press(window, "insert")
    assert node.get("overwrite") is True
    press(window, "insert")
    assert node.get("overwrite") is False
    assert seen == [(False, True), (True, False)]
    node.set(overwrite=True)
    assert node.get("overwrite") is True
    assert seen == [(False, True), (True, False)], "setting it from code fires no event"


def test_overwrite_replaces_the_characters_after_the_caret_and_the_caret_moves_on():
    window, node = field("abcdefgh")
    place(node, 2)
    press(window, "insert")
    for char in "XYZ":
        window.simulate("input", text=char)
    assert node.get("text") == "abXYZfgh" and at(node) == 5, "three replaced, caret before the fourth"
    press(window, "insert")
    window.simulate("input", text="Q")
    assert node.get("text") == "abXYZQfgh", "insert mode adds again"


def test_overwrite_adds_at_the_end_and_never_eats_a_line_break():
    window, node = field("ab\ncd", multiline=True)
    node.set(overwrite=True)
    place(node, 1)
    for char in "XYZ":
        window.simulate("input", text=char)
    assert node.get("text") == "aXYZ\ncd", "the break stays: the extra characters are added"
    place(node, 7)
    window.simulate("input", text="!")
    assert node.get("text") == "aXYZ\ncd!", "at the end of the text a character is added"


def test_typing_over_a_selection_replaces_just_the_selection_in_both_modes():
    for overwrite in (False, True):
        window, node = field("abcdef", overwrite=overwrite)
        place(node, 1, 3)
        window.simulate("input", text="X")
        assert node.get("text") == "aXdef"


def test_overwrite_respects_max_length_and_the_space_key():
    window, node = field("abcd", max_length=4, overwrite=True)
    place(node, 1)
    window.simulate("input", text="X")
    assert node.get("text") == "aXcd", "replacing never grows the text, so the limit allows it"
    press(window, "space")
    assert node.get("text") == "aX d"


def test_overwrite_draws_a_block_caret_over_a_bar_one():
    def caret_pixels(overwrite):
        window = Window(width=120, height=60)
        window.root.set(fill=(0, 0, 0, 255), padding=0)
        node = window.create("text_input", width=100, height=30, text="HHHH", fill=(0, 0, 0, 0),
                             caret_color=(255, 0, 0, 255), caret_blink=0, overwrite=overwrite)
        window.root.add_child(node)
        node.focus()
        place(node, 1)
        rgba, width, height = window.snapshot()
        return sum(1 for i in range(0, len(rgba), 4) if rgba[i] > 200 and rgba[i + 1] < 50)

    assert caret_pixels(True) > 3 * caret_pixels(False)


# -- Double and triple click -------------------------------------------------------------------


def test_double_click_selects_the_word_not_the_hyphenated_string():
    window, node = field("well-known stuff here")
    click(window, node, 1, times=2)
    assert node.get("selection") == (0, 4)


def test_triple_click_selects_the_string_up_to_the_spaces():
    window, node = field("well-known stuff here")
    click(window, node, 1, times=3)
    assert node.get("selection") == (0, 10)


def test_double_click_on_punctuation_or_a_space_selects_that_run():
    window, node = field("well-known stuff")
    click(window, node, 4, times=2)
    assert node.get("selection") == (4, 5), "the hyphen"
    window, node = field("a   b")
    click(window, node, 2, times=2)
    assert node.get("selection") == (1, 4), "the run of spaces"


def test_underscore_and_digits_belong_to_a_word_and_a_click_in_one_is_a_drag_start_again():
    window, node = field("foo_bar2 baz")
    click(window, node, 2, times=2)
    assert node.get("selection") == (0, 8)
    window.simulate("pointer_down", x=node.get("layout_x") + 3, y=node.get("layout_y") + 8)
    window.simulate("pointer_up", x=node.get("layout_x") + 3, y=node.get("layout_y") + 8)
    assert node.get("selection")[0] == node.get("selection")[1], "a plain click collapses it"


def test_a_click_selection_stops_at_the_edge_of_a_wrapped_row():
    window, node = wrapped("aaaa-bbbb-cccc-dddd-eeee-ffff")
    rects = node.text_rects(0, len(node.get("text")))
    assert len(rects) > 1
    click(window, node, 0, times=3)
    start, end = node.get("selection")
    assert start == 0 and end < len("aaaa-bbbb-cccc-dddd-eeee-ffff"), "not past the row, though the string goes on"


# -- What does not change -------------------------------------------------------------------------


def test_without_a_wrap_the_caret_keys_are_as_they_were():
    window, node = field(PARAGRAPH, multiline=True)  # no wrap set
    place(node, 15)
    press(window, "arrow_up")
    assert at(node) == 15, "one logical line: Up does nothing, as before"
    press(window, "home")
    assert at(node) == 0, "and Home goes to the start of the logical line"
    press(window, "page_down")
    assert at(node) == 0, "Page Down is still the scroll view's"


def test_a_single_line_input_keeps_up_and_down_and_gains_the_ctrl_keys():
    window, node = field("hello world", overflow="scroll")
    place(node, 4)
    press(window, "arrow_up")
    press(window, "arrow_down")
    assert at(node) == 4
    press(window, "end", ctrl=True)
    assert at(node) == 11


@pytest.mark.parametrize("props", [{"overflow": "scroll"}, {"multiline": True, "wrap": "word"}])
def test_shift_with_ctrl_selects_by_word_and_to_the_ends_of_the_text_and_shift_alone_to_the_row(props):
    window, node = field("hello big world and more words here", **props)
    place(node, 6)
    press(window, "arrow_right", ctrl=True, shift=True)
    assert node.get("selection") == (6, 10), "to the start of the next word"
    place(node, 6)
    press(window, "end", ctrl=True, shift=True)
    assert node.get("selection") == (6, 35)
    place(node, 6)
    press(window, "home", shift=True)
    assert node.get("selection") == (6, 0)
    place(node, 6)
    press(window, "backspace", ctrl=True, shift=True)
    assert node.get("text") == "big world and more words here", "Shift does not change Ctrl+Backspace"
