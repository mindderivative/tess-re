"""0.6.13: a multiline `text_input`'s own scrollbar. It is opt-in: `scrollbar_show` is `"never"`
until set, and then the spec's thumb (the `scrollbar_*` properties of a scroll view) draws over the
text and its padding. The thumb follows the input's own scroll (the caret-follow moves it, a drag,
a lane click and the wheel are the bar's), the input keeps its keys, and a single-line input takes
the properties and shows no bar. All headless: pixels come from `Window.snapshot()`, the scroll
from `Node.text_rects()`, which read it as it is."""

import pytest

from tess_re import Window

LINES = "\n".join(f"line {i}" for i in range(60))
WHITE = (255, 255, 255, 255)
BLACK = (0, 0, 0, 255)


def scene(text=LINES, **props):
    window = Window(width=300, height=300)
    window.root.set(fill=BLACK, padding=0)
    props = {
        "width": 120,
        "height": 100,
        "multiline": True,
        "wrap": "word",
        "fill": WHITE,
        "caret_visible": False,
        "position": "absolute",
        "x": 0,
        "y": 0,
        **props,
    }
    field = window.create("text_input", text=text, **props)
    window.root.add_child(field)
    window.advance(16)
    return window, field


def grey_runs(values):
    """The runs of consecutive indices in `values`, as `(first, last)` pairs."""
    runs: list[tuple[int, int]] = []
    for v in values:
        if runs and v == runs[-1][1] + 1:
            runs[-1] = (runs[-1][0], v)
        else:
            runs.append((v, v))
    return runs


def is_thumb_grey(pixel):
    return all(40 <= c <= 70 for c in pixel[:3])


def thumb(window, field):
    """`(top, bottom)` rows of the thumb in its column (the scheme's grey at 70 % on black, 8 px wide, 2 px
    from the edge), or None. A run of at least 20 rows: a glyph's grey edge is a pixel or two."""
    rgba, width, _ = window.snapshot()
    x = int(field.get("layout_width")) - 2 - 4
    rows = [y for y in range(int(field.get("layout_height"))) if is_thumb_grey(rgba[(y * width + x) * 4 :][:4])]
    runs = [run for run in grey_runs(rows) if run[1] - run[0] >= 19]
    return runs[0] if runs else None


def thumb_across(window, field):
    """`(left, right)` columns of the horizontal thumb along the bottom edge, or None."""
    rgba, width, _ = window.snapshot()
    y = int(field.get("layout_height")) - 2 - 4
    cols = [x for x in range(int(field.get("layout_width"))) if is_thumb_grey(rgba[(y * width + x) * 4 :][:4])]
    runs = [run for run in grey_runs(cols) if run[1] - run[0] >= 19]
    return runs[0] if runs else None


def scrolled(field):
    """How far the text has scrolled: the first word's rectangle starts a pixel above the box unscrolled
    (the line's own leading)."""
    return -field.text_rects(0, 1)[0][1] - 1.0


def test_the_bar_is_off_by_default_and_reads_the_spec_numbers():
    window, field = scene()
    assert field.get("scrollbar_show") == "never"
    assert thumb(window, field) is None, "nothing is drawn until asked for"
    assert field.get("scrollbar_width") == 8
    assert field.get("scrollbar_hit_width") == 12
    assert field.get("scrollbar_margin") == 2
    assert field.get("scrollbar_min_length") == 32
    assert field.get("scrollbar_opacity") == pytest.approx(0.7)
    assert field.get("scrollbar_lane_click") == "page"
    assert field.get("page_fraction") == pytest.approx(0.9)
    assert field.get("scrollbar_fill") is None


def test_opting_in_and_out():
    window, field = scene(scrollbar_show="overflow")
    assert field.get("scrollbar_show") == "overflow"
    assert thumb(window, field) is not None
    field.set(scrollbar_show=None)
    window.advance(16)
    assert field.get("scrollbar_show") == "never"
    assert thumb(window, field) is None
    field.set(scrollbar_show="auto_hide", scrollbar_width=6, scrollbar_fill=(255, 0, 0, 255))
    assert (field.get("scrollbar_width"), field.get("scrollbar_fill")) == (6, (255, 0, 0, 255))


@pytest.mark.parametrize(
    "name, value",
    [
        ("scrollbar_gutter", 12),
        ("scrollbar_gutter_reserve", True),
        ("key_line", 40),
        ("scroll_smooth_ms", 100),
        ("scroll_easing", "linear"),
        ("scroll_snap_settle_ms", 1),
        ("fling_tau_ms", 100),
        ("edge_scroll", True),
    ],
)
def test_the_properties_an_input_has_no_use_for_are_refused(name, value):
    window, field = scene()
    with pytest.raises(ValueError, match="does not apply to a text_input"):
        field.set(**{name: value})
    with pytest.raises(ValueError, match="does not apply to a text_input"):
        field.get(name)


def test_the_properties_it_has_use_for_are_taken():
    window, field = scene()
    field.set(
        scrollbar_side_v="left",
        scrollbar_side_h="top",
        scrollbar_corner=4,
        scrollbar_arrows="both",
        scrollbar_arrow_step=10,
        scrollbar_lane_click="jump",
        wheel_notch=30,
        page_fraction=0.5,
        page_repeat_ms=100,
    )
    assert field.get("scrollbar_lane_click") == "jump"
    assert field.get("wheel_notch") == 30


def test_bad_values_raise_as_on_a_scroll_view():
    window, field = scene()
    for props in ({"scrollbar_show": "sometimes"}, {"scrollbar_width": -1}, {"scrollbar_opacity": 2}):
        with pytest.raises(ValueError, match="scrollbar_"):
            field.set(**props)
    assert field.get("scrollbar_show") == "never"


def test_a_single_line_input_takes_the_properties_and_shows_no_bar():
    window, field = scene(text="one line " * 40, multiline=False, wrap=None, scrollbar_show="always")
    assert field.get("scrollbar_show") == "always"
    assert thumb(window, field) is None
    # Made multiline afterwards, the bar is there: the order the two are set in does not matter.
    field.set(multiline=True, wrap="word", text=LINES)
    window.advance(16)
    assert thumb(window, field) is not None


def test_always_draws_a_thumb_as_long_as_the_lane_over_text_that_fits_and_overflow_none():
    window, field = scene(text="short", scrollbar_show="always")
    top, bottom = thumb(window, field)
    assert top <= 3 and bottom >= int(field.get("layout_height")) - 4, "the whole lane"
    field.set(scrollbar_show="overflow")
    window.advance(16)
    assert thumb(window, field) is None, "no overflow, no bar"


def test_the_thumb_is_the_text_as_a_share_of_the_box_and_follows_the_caret():
    window, field = scene(scrollbar_show="overflow")
    top, bottom = thumb(window, field)
    height = int(field.get("layout_height"))
    assert top <= 3, "unscrolled: at the top"
    assert 31 <= bottom - top + 1 <= height / 2, "60 lines in a box of a few: a short thumb"
    # The caret at the end scrolls the text down to it, and the thumb to the foot of its lane.
    field.focus()
    field.set(selection=(len(LINES), len(LINES)))
    window.advance(16)
    top2, bottom2 = thumb(window, field)
    assert bottom2 >= height - 4 and top2 > top, (top2, bottom2)
    assert scrolled(field) > 100
    # And back at the start, to the top.
    field.set(selection=(0, 0))
    window.advance(16)
    assert thumb(window, field)[0] <= 3
    assert scrolled(field) == 0


def test_a_drag_scrolls_the_input_without_moving_the_caret_and_the_caret_follow_leaves_it_be():
    window, field = scene(scrollbar_show="overflow")
    field.focus()
    field.set(selection=(0, 0))
    window.advance(16)
    top, bottom = thumb(window, field)
    x = int(field.get("layout_width")) - 6
    middle = (top + bottom) // 2
    window.simulate("pointer_down", x=x, y=middle)
    window.simulate("pointer_move", x=x, y=middle + 20)
    window.simulate("pointer_up", x=x, y=middle + 20)
    window.advance(16)
    assert scrolled(field) > 50, "the drag scrolled the text"
    assert field.get("selection") == (0, 0), "the press placed no caret and selected nothing"
    moved = thumb(window, field)
    assert moved[0] > top + 15, "and the thumb is where the drag put it"
    # The caret did not move: nothing pulls the scroll back to it.
    window.advance(100)
    assert thumb(window, field) == moved
    assert scrolled(field) > 50
    # Typing does: the caret is shown again.
    window.simulate("input", text="x")
    window.advance(16)
    assert scrolled(field) == 0


def test_a_lane_click_pages_by_the_page_fraction_of_the_box():
    window, field = scene(scrollbar_show="overflow")
    height = float(field.get("layout_height"))
    x = int(field.get("layout_width")) - 6
    window.simulate("pointer_down", x=x, y=height - 6)
    window.simulate("pointer_up", x=x, y=height - 6)
    window.advance(16)
    assert scrolled(field) == pytest.approx(height * 0.9, abs=1.0)
    assert field.get("selection") == (len(LINES), len(LINES)), "the caret is where it was"


def test_the_wheel_is_the_bars_only_while_it_is_on_and_the_keys_stay_the_carets():
    window, field = scene()
    window.simulate("wheel", node=field, delta_y=50)
    window.advance(16)
    assert scrolled(field) == 0, "off: the wheel is not the input's, as before"

    field.set(scrollbar_show="overflow")
    window.simulate("wheel", node=field, delta_y=50)
    window.advance(16)
    after = scrolled(field)
    assert after > 0, "on: the wheel scrolls the text"
    assert thumb(window, field)[0] > 3, "and the thumb with it"

    # The caret keys move the caret (and the scroll follows it), never the bar's lines.
    field.focus()
    field.set(selection=(0, 0))
    window.advance(16)
    assert scrolled(field) == 0
    window.simulate("key_down", key="arrow_down")
    assert field.get("selection") == (7, 7), "Down moved the caret one line"
    window.simulate("key_down", key="page_down")
    assert field.get("selection")[0] > 7


def test_an_unwrapped_input_scrolls_sideways_with_a_horizontal_bar():
    text = "x" * 200
    window, field = scene(text=text, multiline=True, wrap="none", scrollbar_show="overflow", height=60)
    width = int(field.get("layout_width"))
    near = thumb_across(window, field)
    assert near is not None and near[0] <= 3, "a horizontal thumb at the start"
    field.focus()
    field.set(selection=(len(text), len(text)))
    window.advance(16)
    far = thumb_across(window, field)
    assert far is not None and far[1] >= width - 4 - 12, "at the end the thumb is at the far end"
    assert far[0] > near[0]
