"""0.6.11 (P2, P3): `hit_testable` on every node (a plain `text` takes pointer events by
default) and `vertical_align` for `text` and `text_input`, with `letter_spacing` and
`line_height` on a text input."""

import pytest

from tess_re import Window
from helpers import add


# --- hit_testable --------------------------------------------------------------------

def button_with_label(**label_props):
    window = Window(width=200, height=100)
    button = add(window, "box", width=120, height=40, fill=(51, 102, 153, 255), padding=8)
    label = window.create("text", text="Save", font_size=14, **label_props)
    button.add_child(label)
    return window, button, label


def test_a_plain_text_gets_pointer_enter_and_leave():
    window, _button, label = button_with_label()
    seen = []
    label.on("pointer_enter", lambda e: seen.append("enter"))
    label.on("pointer_leave", lambda e: seen.append("leave"))
    window.simulate("pointer_move", node=label)
    assert seen == ["enter"]
    window.simulate("pointer_leave")
    assert seen == ["enter", "leave"]


def test_a_click_on_a_label_inside_a_button_is_the_buttons_as_before():
    window, button, label = button_with_label()
    clicks, label_clicks = [], []
    button.on("click", lambda e: clicks.append(e.target == button))
    label.on("click", lambda e: label_clicks.append(1))
    window.simulate("click", node=label)
    assert clicks == [True], "the button's click fires once, its own target"
    assert label_clicks == [], "a press does not reach a plain label"


def test_a_label_that_opts_in_is_its_own_press_target_and_the_click_bubbles():
    window, button, label = button_with_label(hit_testable=True)
    clicks, label_clicks = [], []
    button.on("click", lambda e: clicks.append(e.target == label))
    label.on("click", lambda e: label_clicks.append(1))
    window.simulate("click", node=label)
    assert label_clicks == [1]
    assert clicks == [True], "bubbled from the label, once"


def test_the_button_is_hovered_while_the_pointer_is_over_its_label():
    window, button, label = button_with_label()
    seen = []
    button.on("pointer_enter", lambda e: seen.append("enter"))
    button.on("pointer_leave", lambda e: seen.append("leave"))
    window.simulate("pointer_move", node=label)
    window.simulate("pointer_move", node=button)
    assert seen == ["enter"], "moving from the label to the button stays inside the button"
    window.simulate("pointer_leave")
    assert seen == ["enter", "leave"]


def test_hit_testable_false_makes_a_label_transparent_to_the_pointer():
    window, button, label = button_with_label(hit_testable=False)
    assert label.get("hit_testable") is False
    seen = []
    label.on("pointer_enter", lambda e: seen.append("label"))
    button.on("pointer_enter", lambda e: seen.append("button"))
    clicks = []
    button.on("click", lambda e: clicks.append(e.target == button))
    window.simulate("pointer_move", node=label)
    assert seen == ["button"]
    window.simulate("click", node=label)
    assert clicks == [True], "the press went to what is under the label"


REQUIRED = {
    "text": {"text": "x"},
    "path": {"data": "M0 0 L10 10"},
    "canvas": {"draw": lambda painter: None},
    "image": {"rgba": bytes(4), "pixel_width": 1, "pixel_height": 1},
}


@pytest.mark.parametrize("kind", ["box", "text", "text_input", "path", "canvas", "image"])
def test_hit_testable_is_settable_and_readable_on_every_kind(kind):
    window = Window(width=100, height=100)
    node = add(window, kind, width=10, height=10, **REQUIRED.get(kind, {}))
    assert node.get("hit_testable") is None
    node.set(hit_testable=False)
    assert node.get("hit_testable") is False
    node.set(hit_testable=True)
    assert node.get("hit_testable") is True
    node.set(hit_testable=None)
    assert node.get("hit_testable") is None
    with pytest.raises(ValueError):
        node.set(hit_testable="no")


def test_hit_testable_false_on_a_box_passes_events_to_what_is_under_it():
    window = Window(width=100, height=100)
    under = add(window, "box", width=100, height=100, fill=(255, 255, 255, 255))
    over = window.create("box", width=60, height=60, fill=(255, 0, 0, 255), hit_testable=False)
    under.add_child(over)
    seen = []
    under.on("pointer_enter", lambda e: seen.append("under"))
    over.on("pointer_enter", lambda e: seen.append("over"))
    window.simulate("pointer_move", node=over)
    assert seen == ["under"]


def test_hit_testable_true_on_a_box_with_no_listeners_still_takes_the_hit():
    window = Window(width=100, height=100)
    under = add(window, "box", width=100, height=100, fill=(255, 255, 255, 255))
    over = window.create("box", width=60, height=60, hit_testable=True)
    under.add_child(over)
    seen = []
    under.on("click", lambda e: seen.append(e.target == over))
    window.simulate("click", node=over)
    assert seen == [True]


# --- vertical_align --------------------------------------------------------------------

def input_node(**props):
    window = Window(width=200, height=80)
    props.setdefault("text", "hello")
    node = add(window, "text_input", width=120, height=32, padding=4, font_size=16, **props)
    node.focus()
    return window, node


def test_a_single_line_input_is_centred_by_default_and_reads_back_none():
    window, node = input_node()
    assert node.get("vertical_align") is None
    _, y, _, h = node.caret_rect()
    # The caret sits in the middle of the 24 px content box (4..28), within a pixel or two.
    assert abs((y + h / 2) - 16) <= 2.5


def test_vertical_align_start_center_end_move_the_caret():
    ys = {}
    for align in ("start", "center", "end"):
        _window, node = input_node(vertical_align=align)
        assert node.get("vertical_align") == align
        ys[align] = node.caret_rect()[1]
    assert ys["start"] < ys["center"] < ys["end"]
    assert abs((ys["center"] - ys["start"]) - (ys["end"] - ys["center"])) <= 1.0


def test_a_multiline_input_defaults_to_start():
    _w, plain = input_node(multiline=True)
    _w, start = input_node(multiline=True, vertical_align="start")
    _w, centre = input_node(multiline=True, vertical_align="center")
    assert plain.caret_rect() == start.caret_rect()
    assert centre.caret_rect()[1] > start.caret_rect()[1]


def test_text_rects_follow_the_centred_line():
    _w, start = input_node(vertical_align="start")
    _w, centre = input_node()
    assert centre.text_rects(1, 3)[0][1] > start.text_rects(1, 3)[0][1]


def test_clicking_in_the_centred_line_places_the_caret():
    window, node = input_node(text="hello")
    x0, y0, _w, h = node.caret_rect()
    node.set(selection=(0, 0))
    window.simulate("pointer_down", node=node, x=100.0, y=y0 + h / 2, button="primary")
    window.simulate("pointer_up", node=node, x=100.0, y=y0 + h / 2, button="primary")
    assert node.get("selection") == (5, 5)


def test_the_text_nodes_vertical_align():
    window = Window(width=100, height=80)
    label = add(window, "text", text="Hi", font_size=16, width=100, height=60)
    assert label.get("vertical_align") == "start"
    for align in ("center", "end", "start"):
        label.set(vertical_align=align)
        assert label.get("vertical_align") == align
    with pytest.raises(ValueError):
        label.set(vertical_align=None)
    with pytest.raises(ValueError):
        label.set(vertical_align="middle")


def test_vertical_align_applies_only_to_text_and_text_input():
    window = Window(width=100, height=100)
    box = add(window, "box", width=10, height=10)
    with pytest.raises(ValueError):
        box.set(vertical_align="center")


def test_letter_spacing_and_line_height_on_a_text_input():
    _w, node = input_node(vertical_align="start", text="abc")
    base = node.caret_rect()
    node.set(letter_spacing=3)
    assert node.get("letter_spacing") == 3
    spaced = node.caret_rect()
    assert spaced[0] - base[0] == pytest.approx(9, abs=0.5)
    node.set(line_height=2.0)
    assert node.get("line_height") == 2.0
    assert node.caret_rect()[3] > base[3] * 1.5
    node.set(line_height=None)
    assert node.get("line_height") is None
