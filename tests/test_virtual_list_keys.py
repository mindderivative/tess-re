"""Keyboard scrolling, `scroll_to_index` and scroll-into-view for a
`virtual_list` (UX spec notes "List" and "Scrollbar and Scroll Container"):
the arrows, Page Up/Down and Home/End scroll the list around the focused node
by 40 dp, a page (the viewport less 10 percent) and to the ends, at once; the
same exclusions as a scroll view apply; a list that cannot move a key's way
passes it to the scroller around it; moving focus to a row, or
`scroll_into_view`, scrolls by the least distance; and `scroll` fires.
"""

import pytest

from tess_re import Window


#: The rows each list has built, by index (the newest build of each row).
BUILT = {}


def make_list(window, count=50, extent=20.0, size_hint=None, parent=None, height=100, **props):
    """A 200 wide, `height` tall virtual list of focusable 20 dp rows."""
    kind = {"size_hint": size_hint} if size_hint else {"item_extent": extent}
    built = {}

    def materialize(i):
        built[i] = window.create("box", width=100, height=20, focusable=True)
        return built[i]

    lst = window.create(
        "virtual_list",
        item_count=count,
        materialize=materialize,
        width=200,
        height=height,
        flex_shrink=0,
        **kind,
        **props,
    )
    BUILT[id(lst)] = built
    (parent or window.root).add_child(lst)
    window.advance(16)
    return lst


def offset(lst):
    return lst.get("scroll_offset")


def row(lst, index):
    """The row `index` last built for `lst` (it must be on screen)."""
    return BUILT[id(lst)][index]


def focus_first_row(lst):
    row(lst, 0).focus()


def test_keys_scroll_a_list_around_the_focused_row():
    window = Window(width=800, height=600)
    lst = make_list(window)  # 50 * 20 = 1000 dp in 100: furthest offset 900
    focus_first_row(lst)
    for key, expected in [
        ("arrow_down", 40.0),
        ("page_down", 130.0),  # viewport 100 less 10 percent
        ("arrow_up", 90.0),
        ("end", 900.0),
        ("page_down", 900.0),
        ("page_up", 810.0),
        ("home", 0.0),
        ("arrow_up", 0.0),
    ]:
        window.simulate("key_down", key=key)
        window.advance(16)
        assert offset(lst) == expected, key


def test_keys_scroll_a_list_that_is_itself_focused():
    window = Window(width=800, height=600)
    lst = make_list(window, focusable=True)
    lst.focus()
    window.simulate("key_down", key="arrow_down")
    assert offset(lst) == 40.0


def test_keys_scroll_a_size_hint_list_by_the_same_sizes():
    window = Window(width=800, height=600)
    lst = make_list(window, count=30, size_hint=lambda i: 20.0 + (i % 2) * 10.0)
    # Heights 20, 30, 20, 30 ...: 750 dp in all, furthest offset 650.
    focus_first_row(lst)
    window.simulate("key_down", key="page_down")
    assert offset(lst) == 90.0
    window.simulate("key_down", key="end")
    assert offset(lst) == 650.0


def test_a_text_input_keeps_its_arrows_but_not_page_keys():
    window = Window(width=800, height=600)
    field = window.create("text_input", text="hello", width=150, height=24)
    lst = window.create(
        "virtual_list", item_count=50, item_extent=24.0, width=200, height=100,
        materialize=lambda i: field if i == 0 else window.create("box", width=100, height=24),
    )
    window.root.add_child(lst)
    window.advance(16)
    field.focus()
    window.simulate("key_down", key="arrow_down")
    window.simulate("key_down", key="end")
    assert offset(lst) == 0.0, "the text input used them"
    window.simulate("key_down", key="page_down")
    assert offset(lst) == 90.0


def test_a_key_down_listener_on_the_way_keeps_the_keys():
    window = Window(width=800, height=600)
    lst = make_list(window)
    focus_first_row(lst)
    row(lst, 0).on("key_down", lambda: None)
    window.simulate("key_down", key="arrow_down")
    assert offset(lst) == 0.0, "the row handles its own keys"

    other = make_list(window)
    other.on("key_down", lambda: None)
    row(other, 0).focus()
    window.simulate("key_down", key="page_down")
    assert offset(other) == 0.0, "the list's own listener handles them"


@pytest.mark.parametrize("modifier", ["ctrl", "alt", "meta"])
@pytest.mark.parametrize("key", ["arrow_down", "page_down", "end"])
def test_a_shortcut_modifier_keeps_keys_from_scrolling(modifier, key):
    window = Window(width=800, height=600)
    lst = make_list(window)
    focus_first_row(lst)
    window.simulate("key_down", key=key, **{modifier: True})
    assert offset(lst) == 0.0
    window.simulate("key_down", key=key)
    assert offset(lst) > 0.0


def test_keys_scroll_nothing_without_focus():
    window = Window(width=800, height=600)
    lst = make_list(window)
    window.simulate("key_down", key="page_down")
    assert offset(lst) == 0.0


def test_a_list_that_cannot_move_passes_the_key_to_the_scroller_around_it():
    window = Window(width=800, height=600)
    outer = window.create("scroll_view", width=300, height=150)
    content = window.create("box", width=300, height=1000, flex_shrink=0)
    outer.add_child(content)
    window.root.add_child(outer)
    lst = make_list(window, count=20, parent=content)  # furthest offset 300
    window.advance(16)
    row(lst, 0).focus()

    window.simulate("key_down", key="arrow_down")
    assert (offset(lst), offset(outer)) == (40.0, 0.0)
    window.simulate("key_down", key="end")
    assert (offset(lst), offset(outer)) == (300.0, 0.0)
    window.simulate("key_down", key="arrow_down")
    assert offset(outer) == 40.0, "the list is at its end"
    window.simulate("key_down", key="arrow_up")
    assert (offset(lst), offset(outer)) == (260.0, 40.0), "but it can still go up"
    window.simulate("key_down", key="home")
    window.simulate("key_down", key="home")
    assert (offset(lst), offset(outer)) == (0.0, 0.0)


def test_focus_moving_to_a_row_scrolls_by_the_least_distance():
    window = Window(width=800, height=600)
    lst = make_list(window, height=90)  # row 4 (80..100) is cut off at the bottom
    row(lst, 3).focus()
    assert offset(lst) == 0.0, "row 3 is shown"
    row(lst, 4).focus()
    assert offset(lst) == 10.0, "its bottom at the viewport's bottom"


def test_the_focused_row_stays_built_so_its_keys_keep_working():
    window = Window(width=800, height=600)
    lst = make_list(window)
    row(lst, 0).focus()
    for _ in range(4):
        window.simulate("key_down", key="arrow_down")
        window.advance(16)
    assert offset(lst) == 160.0, "row 0 is far above the viewport, still focused"
    window.simulate("key_down", key="arrow_up")
    assert offset(lst) == 120.0


def test_scroll_into_view_scrolls_a_list_to_a_built_row():
    window = Window(width=800, height=600)
    lst = make_list(window)
    target = row(lst, 0)
    target.focus()
    for _ in range(3):
        window.simulate("key_down", key="arrow_down")
    window.advance(16)
    assert offset(lst) == 120.0
    target.scroll_into_view()  # the focused row was kept built, above the viewport
    assert offset(lst) == 0.0
    target.scroll_into_view()
    assert offset(lst) == 0.0, "already shown"


def test_scroll_to_index_aligns_a_row_that_is_not_built():
    window = Window(width=800, height=600)
    lst = make_list(window)
    assert lst.scroll_to_index(30) is True  # default nearest: its bottom at the bottom
    assert offset(lst) == 520.0
    assert lst.scroll_to_index(30) is False
    assert lst.scroll_to_index(30, align="start") is True
    assert offset(lst) == 600.0
    lst.scroll_to_index(30, align="center")
    assert offset(lst) == 560.0
    lst.scroll_to_index(30, align="end")
    assert offset(lst) == 520.0
    lst.scroll_to_index(49, align="start")
    assert offset(lst) == 900.0, "clamped"
    lst.scroll_to_index(0, align="nearest")
    assert offset(lst) == 0.0
    window.advance(16)
    assert len(lst.children()) <= 10, "only the rows around the offset are built"


def test_scroll_to_index_uses_size_hint_offsets():
    window = Window(width=800, height=600)
    lst = make_list(window, count=10, size_hint=lambda i: 10.0 * (i + 1))
    # Row 5 spans 150..210.
    lst.scroll_to_index(5, align="start")
    assert offset(lst) == 150.0
    lst.scroll_to_index(5, align="center")
    assert offset(lst) == 130.0
    lst.scroll_to_index(5, align="end")
    assert offset(lst) == 110.0


def test_scroll_to_index_errors():
    window = Window(width=800, height=600)
    lst = make_list(window, count=10)
    for bad in (10, -1):
        with pytest.raises(ValueError, match="outside"):
            lst.scroll_to_index(bad)
    with pytest.raises(ValueError, match="align"):
        lst.scroll_to_index(1, align="middle")
    with pytest.raises(ValueError, match="virtual_list"):
        window.root.scroll_to_index(0)


def test_scroll_fires_for_keys_scroll_to_index_and_the_wheel():
    window = Window(width=800, height=600)
    lst = make_list(window)
    seen = []
    lst.on("scroll", lambda e: seen.append((e.old_value, e.new_value)))
    row(lst, 0).focus()
    window.simulate("key_down", key="arrow_up")  # already at the top
    assert seen == []
    window.simulate("key_down", key="arrow_down")
    assert seen == [(0.0, 40.0)]
    lst.scroll_to_index(30, align="start")
    assert seen[-1] == (40.0, 600.0)
    window.simulate("wheel", node=lst, delta_y=-100.0)
    assert seen[-1][0] == 600.0 and seen[-1][1] != 600.0
