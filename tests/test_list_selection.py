"""Choosing the rows of a `virtual_list` (UX spec "Selection", acceptance AC-SLT-1 to AC-SLT-6 and
AC-SLT-13): `selection_mode`, `selection`, `selection_anchor`, `select_all_key`, `select_on_focus`,
`unselectable_keys` and the `selection_change` event. A row's key is its index. Everything runs
headless through `Window.simulate`.
"""

import pytest

from tess_re import Window

#: The rows each list has built, by index (the newest build of each row), and what each list's
#: `selection_change` listener heard, as `(selection, anchor, source)`.
BUILT = {}
LOGS = {}


def make_list(window, count=5, extent=20.0, height=100, mode="multiple", **props):
    """A 200 wide, `height` tall virtual list of focusable 20 dp rows, with a `selection_change`
    recorder, `log(lst)`."""
    built = {}

    def materialize(i):
        built[i] = window.create("box", width=100, height=20, focusable=True)
        return built[i]

    lst = window.create(
        "virtual_list",
        item_count=count,
        item_extent=extent,
        materialize=materialize,
        width=200,
        height=height,
        flex_shrink=0,
        selection_mode=mode,
        **props,
    )
    LOGS[id(lst)] = log = []
    lst.on("selection_change", lambda e: log.append((e.selection, e.anchor, e.selection_source)))
    BUILT[id(lst)] = built
    window.root.add_child(lst)
    window.advance(16)
    return lst


def log(lst):
    return LOGS[id(lst)]


def row(lst, index):
    return BUILT[id(lst)][index]


def built_rows(lst):
    return len(lst.children())


def click(window, lst, index, **modifiers):
    window.simulate("click", node=row(lst, index), **modifiers)


def key(window, name, **modifiers):
    window.simulate("key_down", key=name, **modifiers)
    window.advance(16)


def focused_row(lst):
    return next(i for i, r in BUILT[id(lst)].items() if r.get("focused"))


def chosen(lst):
    return lst.get("selection")


# --- defaults: nothing changes ---------------------------------------------------------------


def test_the_defaults_choose_nothing_and_change_no_key():
    window = Window(width=800, height=600)
    lst = make_list(window, count=50, mode="none")
    assert lst.get("selection_mode") == "none"
    assert chosen(lst) == [] and lst.get("selection_anchor") is None
    assert lst.get("select_all_key") is False and lst.get("select_on_focus") is False
    assert lst.get("unselectable_keys") == []
    clicks = []
    row(lst, 0).on("click", lambda: clicks.append(1))
    click(window, lst, 0)
    click(window, lst, 1, shift=True)
    click(window, lst, 2, ctrl=True)
    row(lst, 0).focus()
    key(window, "space")  # a Space is still a click on the focused row
    key(window, "arrow_down")  # and an arrow still scrolls by 40
    assert lst.get("scroll_offset") == 40.0
    key(window, "a", ctrl=True)
    assert chosen(lst) == [] and log(lst) == [] and len(clicks) == 2


def test_a_new_list_takes_the_properties_at_creation():
    window = Window(width=800, height=600)
    lst = window.create(
        "virtual_list", item_count=10, item_extent=20, materialize=lambda i: window.create("box"),
        selection_mode="multiple", selection=[1, 2, 3], unselectable_keys=[5], select_all_key=True,
        select_on_focus=True, width=100, height=100,
    )
    assert lst.get("selection_mode") == "multiple"
    assert chosen(lst) == [1, 2, 3] and lst.get("selection_anchor") == 1
    assert lst.get("unselectable_keys") == [5]
    assert lst.get("select_all_key") and lst.get("select_on_focus")


# --- AC-SLT-1: clicks ------------------------------------------------------------------------


def test_clicks_choose_ranges_and_toggles_with_the_anchor_of_ac_slt_1():
    window = Window(width=800, height=600)
    lst = make_list(window)  # A..E = 0..4
    for index, modifiers, expected, anchor in [
        (1, {}, [1], 1),  # click B
        (3, {"shift": True}, [1, 2, 3], 1),  # Shift+click D
        (4, {"ctrl": True}, [1, 2, 3, 4], 4),  # Ctrl+click E
        (2, {"shift": True}, [2, 3, 4], 4),  # Shift+click C: E..C replaces
    ]:
        click(window, lst, index, **modifiers)
        assert chosen(lst) == expected
        assert lst.get("selection_anchor") == anchor
    assert [(s, a, src) for s, a, src in log(lst)] == [
        ([1], 1, "pointer"),
        ([1, 2, 3], 1, "pointer"),
        ([1, 2, 3, 4], 4, "pointer"),
        ([2, 3, 4], 4, "pointer"),
    ]


def test_ctrl_shift_click_adds_the_range_and_ctrl_click_toggles_off():
    window = Window(width=800, height=600)
    lst = make_list(window, count=5)
    click(window, lst, 0)
    click(window, lst, 2, ctrl=True)
    click(window, lst, 4, ctrl=True, shift=True)
    assert chosen(lst) == [0, 2, 3, 4]
    click(window, lst, 3, ctrl=True)
    assert chosen(lst) == [0, 2, 4]


def test_a_click_in_the_empty_part_clears_a_multiple_selection():
    window = Window(width=800, height=600)
    lst = make_list(window, count=3, height=100)  # rows end at 60
    click(window, lst, 1)
    window.simulate("click", node=lst, x=50, y=90)
    assert chosen(lst) == []
    assert log(lst)[-1][0] == []


def test_a_single_list_chooses_one_whatever_the_modifiers():
    window = Window(width=800, height=600)
    lst = make_list(window, mode="single")
    click(window, lst, 1)
    click(window, lst, 3, shift=True)
    assert chosen(lst) == [3]
    click(window, lst, 4, ctrl=True)
    assert chosen(lst) == [4]
    window.simulate("click", node=lst, x=50, y=99)  # an empty part clears nothing in a single list
    assert chosen(lst) == [4]


# --- AC-SLT-2, AC-SLT-3: keys ----------------------------------------------------------------


def test_shift_arrows_extend_from_the_anchor_per_ac_slt_2():
    window = Window(width=800, height=600)
    lst = make_list(window)
    click(window, lst, 1)  # B, chosen, the anchor
    row(lst, 1).focus()
    key(window, "arrow_down", shift=True)
    key(window, "arrow_down", shift=True)
    assert chosen(lst) == [1, 2, 3] and lst.get("selection_anchor") == 1
    key(window, "arrow_up", shift=True)
    assert chosen(lst) == [1, 2] and lst.get("selection_anchor") == 1
    assert focused_row(lst) == 2
    assert [src for _, _, src in log(lst)[1:]] == ["keyboard"] * 3


def test_ctrl_arrows_move_focus_and_space_toggles_per_ac_slt_3():
    window = Window(width=800, height=600)
    lst = make_list(window)
    click(window, lst, 0)
    row(lst, 0).focus()
    key(window, "arrow_down", ctrl=True)
    key(window, "arrow_down", ctrl=True)
    assert focused_row(lst) == 2 and chosen(lst) == [0]
    key(window, "space", ctrl=True)
    assert chosen(lst) == [0, 2]
    key(window, "space")
    assert chosen(lst) == [0], "Space toggles in a multiple list"

    single = make_list(window, mode="single")
    row(single, 1).focus()
    key(window, "space")
    key(window, "arrow_down")
    assert chosen(single) == [1], "an arrow moves focus only"
    key(window, "space")
    assert chosen(single) == [2], "Space chooses in a single list"


def test_space_toggles_without_a_click_and_enter_activates_without_choosing():
    window = Window(width=800, height=600)
    lst = make_list(window)
    clicks = []
    lst.on("click", lambda e: clicks.append(e.target.get("focused")))
    row(lst, 2).focus()
    key(window, "space")
    assert chosen(lst) == [2] and clicks == []
    key(window, "enter")
    assert chosen(lst) == [2] and len(clicks) == 1, "Enter opens: a click, selection unchanged"
    key(window, "space", shift=True)
    assert chosen(lst) == [2]

    single = make_list(window, mode="single")
    row(single, 3).focus()
    key(window, "enter")
    assert chosen(single) == [3], "Enter chooses in a single list"


def test_shift_space_chooses_the_range_from_the_anchor():
    window = Window(width=800, height=600)
    lst = make_list(window)
    click(window, lst, 0)
    row(lst, 3).focus()
    key(window, "space", shift=True)
    assert chosen(lst) == [0, 1, 2, 3]


def test_arrows_choose_only_with_select_on_focus_and_ctrl_still_moves_alone():
    window = Window(width=800, height=600)
    lst = make_list(window)
    row(lst, 0).focus()
    key(window, "arrow_down")
    assert focused_row(lst) == 1 and chosen(lst) == []
    lst.set(select_on_focus=True)
    key(window, "arrow_down")
    assert chosen(lst) == [2] and lst.get("selection_anchor") == 2
    key(window, "arrow_down", ctrl=True)
    assert focused_row(lst) == 3 and chosen(lst) == [2]
    key(window, "home")
    assert chosen(lst) == [0] and focused_row(lst) == 0


def test_a_list_takes_the_arrows_so_it_does_not_also_scroll():
    window = Window(width=800, height=600)
    lst = make_list(window, count=50)
    row(lst, 0).focus()
    key(window, "arrow_down")
    assert focused_row(lst) == 1 and lst.get("scroll_offset") == 0.0
    for _ in range(6):
        key(window, "arrow_down")
    assert focused_row(lst) == 7 and lst.get("scroll_offset") == 60.0, "scrolled by the least to show it"


def test_page_keys_move_focus_a_page_of_rows():
    window = Window(width=800, height=600)
    lst = make_list(window, count=100)
    row(lst, 0).focus()
    key(window, "page_down")
    assert focused_row(lst) == 4
    key(window, "page_up")
    assert focused_row(lst) == 0


def test_escape_never_clears_a_selection():
    window = Window(width=800, height=600)
    lst = make_list(window)
    click(window, lst, 1)
    click(window, lst, 3, shift=True)
    row(lst, 3).focus()
    key(window, "escape")
    assert chosen(lst) == [1, 2, 3]
    assert len(log(lst)) == 2


# --- AC-SLT-4: Ctrl+A ------------------------------------------------------------------------


def test_ctrl_a_chooses_all_only_when_the_list_asks_for_it():
    window = Window(width=800, height=600)
    lst = make_list(window)
    row(lst, 1).focus()
    key(window, "a", ctrl=True)
    assert chosen(lst) == [], "select_all_key is off"
    lst.set(select_all_key=True)
    key(window, "a", ctrl=True)
    assert chosen(lst) == [0, 1, 2, 3, 4]
    assert log(lst)[-1][2] == "keyboard"


def test_ctrl_a_in_a_text_input_selects_the_text_not_the_rows():
    window = Window(width=800, height=600)
    lst = make_list(window, select_all_key=True)
    field = window.create("text_input", text="hello", width=100, height=24)
    window.root.add_child(field)
    field.focus()
    key(window, "a", ctrl=True)
    assert field.get("selection") == (0, 5)
    assert chosen(lst) == []
    row(lst, 0).focus()
    key(window, "a", ctrl=True)
    assert chosen(lst) == [0, 1, 2, 3, 4]


def test_ctrl_a_in_a_text_input_inside_a_row_selects_the_text():
    window = Window(width=800, height=600)
    field = window.create("text_input", text="hello", width=100, height=20)
    lst = window.create(
        "virtual_list", item_count=3, item_extent=20, width=200, height=60,
        materialize=lambda i: field if i == 1 else window.create("box", width=100, height=20, focusable=True),
        selection_mode="multiple", select_all_key=True,
    )
    window.root.add_child(lst)
    window.advance(16)
    field.focus()
    key(window, "a", ctrl=True)
    assert field.get("selection") == (0, 5) and lst.get("selection") == []
    key(window, "arrow_down")
    assert lst.get("selection") == [], "the input keeps its arrows"


def test_ctrl_a_skips_unselectable_rows_and_a_single_list_has_none():
    window = Window(width=800, height=600)
    lst = make_list(window, select_all_key=True, unselectable_keys=[2])
    row(lst, 0).focus()
    key(window, "a", ctrl=True)
    assert chosen(lst) == [0, 1, 3, 4]
    single = make_list(window, mode="single", select_all_key=True)
    row(single, 0).focus()
    key(window, "a", ctrl=True)
    assert chosen(single) == []


# --- AC-SLT-5: unselectable rows -------------------------------------------------------------


def test_a_range_passes_unselectable_rows_and_focus_never_lands_on_them():
    window = Window(width=800, height=600)
    lst = make_list(window, count=6, height=120, unselectable_keys=[0, 2, 3])
    click(window, lst, 1)
    click(window, lst, 5, shift=True)
    assert chosen(lst) == [1, 4, 5]
    click(window, lst, 2)
    assert chosen(lst) == [1, 4, 5], "a click on one chooses nothing"
    row(lst, 1).focus()
    visited = []
    for name in ["arrow_down", "arrow_down", "arrow_up", "arrow_up", "arrow_up", "end", "home"]:
        key(window, name)
        visited.append(focused_row(lst))
    assert visited == [4, 5, 4, 1, 1, 5, 1]
    assert not {0, 2, 3} & set(visited)


def test_making_a_chosen_row_unselectable_unchooses_it():
    window = Window(width=800, height=600)
    lst = make_list(window)
    click(window, lst, 0)
    click(window, lst, 3, shift=True)
    lst.set(unselectable_keys=[1, 3])
    assert chosen(lst) == [0, 2]
    assert log(lst)[-1] == ([0, 2], 0, "api")


# --- AC-SLT-6: a hundred thousand rows -------------------------------------------------------


def test_shift_end_over_a_hundred_thousand_rows_builds_no_row_for_it():
    window = Window(width=800, height=600)
    lst = make_list(window, count=100_000, height=100)
    click(window, lst, 0)
    row(lst, 0).focus()
    key(window, "end", shift=True)
    window.advance(16)
    assert lst.get("selection") == list(range(100_000))
    event_selection, anchor, source = log(lst)[-1]
    assert len(event_selection) == 100_000 and anchor == 0 and source == "keyboard"
    shown = 100 // 20
    assert built_rows(lst) <= shown + 20, built_rows(lst)
    assert focused_row(lst) == 99_999, "focus went to the last row once it was built"
    key(window, "a", ctrl=True)
    lst.set(select_all_key=True)
    key(window, "a", ctrl=True)
    assert len(chosen(lst)) == 100_000
    assert built_rows(lst) <= shown + 20


def test_the_event_carries_the_count_and_the_ranges():
    window = Window(width=800, height=600)
    lst = make_list(window, count=100_000)
    seen = []
    lst.on("selection_change", lambda e: seen.append((e.count, e.ranges, e.anchor, e.selection_source)))
    click(window, lst, 2)
    click(window, lst, 4, ctrl=True)
    row(lst, 4).focus()
    key(window, "end", shift=True)
    assert seen == [
        (1, [(2, 3)], 2, "pointer"),
        (2, [(2, 3), (4, 5)], 4, "pointer"),
        (99_996, [(4, 100_000)], 4, "keyboard"),
    ]


# --- AC-SLT-13: press on a chosen row --------------------------------------------------------


def test_a_press_on_a_chosen_row_chooses_it_alone_unless_the_pointer_moves():
    window = Window(width=800, height=600)
    window.set(drag_start_px=4)
    lst = make_list(window)
    click(window, lst, 0)
    click(window, lst, 2, shift=True)
    assert chosen(lst) == [0, 1, 2]
    target = row(lst, 1)

    window.simulate("pointer_down", node=target, x=10, y=10)
    assert chosen(lst) == [0, 1, 2], "nothing changes on the press"
    window.simulate("pointer_move", node=target, x=12, y=11)
    window.simulate("pointer_up", node=target, x=12, y=11)
    assert chosen(lst) == [1], "released without going 4 px: a click"

    click(window, lst, 0)
    click(window, lst, 2, shift=True)
    window.simulate("pointer_down", node=target, x=10, y=10)
    window.simulate("pointer_move", node=target, x=14, y=10)
    window.simulate("pointer_up", node=target, x=14, y=10)
    assert chosen(lst) == [0, 1, 2], "4 px or more starts the app's drag: the set is left"
    window.simulate("pointer_down", node=target, x=10, y=10)
    window.simulate("pointer_move", node=target, x=15, y=10)
    window.simulate("pointer_move", node=target, x=10, y=10)
    window.simulate("pointer_up", node=target, x=10, y=10)
    assert chosen(lst) == [0, 1, 2], "once a drag, a drag, even back where it started"


def test_with_the_default_drag_start_a_press_and_release_on_a_chosen_row_is_always_a_click():
    window = Window(width=800, height=600)
    lst = make_list(window)
    click(window, lst, 0)
    click(window, lst, 2, shift=True)
    target = row(lst, 1)
    window.simulate("pointer_down", node=target, x=10, y=10)
    window.simulate("pointer_move", node=target, x=90, y=15)
    window.simulate("pointer_up", node=target, x=90, y=15)
    assert chosen(lst) == [1], "drag_start_px is 0: no move is a drag"


def test_a_modifier_press_on_a_chosen_row_acts_at_once():
    window = Window(width=800, height=600)
    window.set(drag_start_px=4)
    lst = make_list(window)
    click(window, lst, 0)
    click(window, lst, 2, shift=True)
    click(window, lst, 1, ctrl=True)
    assert chosen(lst) == [0, 2]


# --- the properties --------------------------------------------------------------------------


def test_setting_selection_replaces_the_set_and_fires_one_api_event():
    window = Window(width=800, height=600)
    lst = make_list(window, count=10)
    lst.set(selection=[7, 3, 4, 3])
    assert chosen(lst) == [3, 4, 7] and lst.get("selection_anchor") == 3
    assert log(lst) == [([3, 4, 7], 3, "api")]
    lst.set(selection=[3, 4, 7])
    assert len(log(lst)) == 1, "the same set again changes nothing"
    lst.set(selection=range(2, 5))
    assert chosen(lst) == [2, 3, 4]
    lst.set(selection=[])
    assert chosen(lst) == [] and lst.get("selection_anchor") is None
    assert log(lst)[-1] == ([], None, "api")


def test_a_bad_selection_raises_and_changes_nothing():
    window = Window(width=800, height=600)
    lst = make_list(window, count=10, unselectable_keys=[5])
    lst.set(selection=[1])
    for bad, text in [
        ([10], "unknown key 10"),
        ([-1], "unknown key -1"),
        ([2, 5], "unselectable_keys"),
        (["a"], "list of row keys"),
        ("12", "list of row keys"),
        ([True], "list of row keys"),
        (3, "list of row keys"),
    ]:
        with pytest.raises(ValueError, match=text):
            lst.set(selection=bad)
        assert chosen(lst) == [1]
    single = make_list(window, mode="single")
    with pytest.raises(ValueError, match="at most one key"):
        single.set(selection=[1, 2])
    none = make_list(window, mode="none")
    with pytest.raises(ValueError, match="selection_mode"):
        none.set(selection=[1])
    none.set(selection=[])


def test_selection_is_checked_against_the_rest_of_the_same_call():
    window = Window(width=800, height=600)
    lst = make_list(window, count=5, mode="none")
    lst.set(selection_mode="multiple", selection=[1, 2])
    assert chosen(lst) == [1, 2]
    lst.set(item_count=20, selection=[15])
    assert chosen(lst) == [15]
    lst.set(unselectable_keys=[3], selection=[2, 4])
    assert chosen(lst) == [2, 4]
    with pytest.raises(ValueError, match="unselectable_keys"):
        lst.set(unselectable_keys=[4], selection=[4])


def test_the_selection_mode_changes_what_is_kept():
    window = Window(width=800, height=600)
    lst = make_list(window)
    lst.set(selection=[1, 2, 3])
    lst.set(selection_mode="single")
    assert chosen(lst) == [1]
    lst.set(selection_mode="none")
    assert chosen(lst) == [] and lst.get("selection_anchor") is None
    assert log(lst)[-1] == ([], None, "api")
    with pytest.raises(ValueError, match="one of"):
        lst.set(selection_mode="several")


def test_a_shorter_list_drops_the_rows_it_lost():
    window = Window(width=800, height=600)
    lst = make_list(window, count=10)
    lst.set(selection=[1, 8], unselectable_keys=[7, 9])
    assert chosen(lst) == [1, 8]
    lst.set(item_count=5)
    assert chosen(lst) == [1] and lst.get("unselectable_keys") == []
    assert log(lst)[-1] == ([1], 1, "api")


def test_the_properties_apply_to_a_virtual_list_only():
    window = Window(width=800, height=600)
    box = window.create("box", width=10, height=10)
    for name, value in [
        ("selection_mode", "multiple"),
        ("select_all_key", True),
        ("select_on_focus", True),
        ("unselectable_keys", [1]),
    ]:
        with pytest.raises(ValueError, match="applies only to a virtual_list"):
            box.set(**{name: value})
    for name in ["selection_mode", "selection_anchor", "select_all_key", "select_on_focus", "unselectable_keys"]:
        with pytest.raises(ValueError, match="applies only to a virtual_list"):
            box.get(name)
    lst = make_list(window)
    with pytest.raises(ValueError, match="read-only"):
        lst.set(selection_anchor=1)
    with pytest.raises(ValueError, match="a bool"):
        lst.set(select_all_key=1)
    with pytest.raises(ValueError, match="applies only to a text, text_input or terminal"):
        box.get("selection")


# --- keys that belong to a node on the way ---------------------------------------------------


def test_a_row_with_its_own_key_down_listener_keeps_its_keys():
    window = Window(width=800, height=600)
    lst = make_list(window)
    row(lst, 1).on("key_down", lambda: None)
    row(lst, 1).focus()
    key(window, "arrow_down")
    key(window, "space")
    assert focused_row(lst) == 1 and chosen(lst) == []
    lst.on("key_down", lambda: None)
    row(lst, 0).focus()
    key(window, "arrow_down")
    assert focused_row(lst) == 1, "a listener on the list itself does not stop it"


def test_the_list_itself_can_hold_focus_and_take_the_keys():
    window = Window(width=800, height=600)
    lst = make_list(window, focusable=True)
    lst.focus()
    key(window, "arrow_down")
    assert focused_row(lst) == 0
    lst.focus()
    key(window, "end")
    assert focused_row(lst) == 4


# --- focus moves that wait for a row to be built ---------------------------------------------


def test_focus_events_fire_when_a_row_gets_the_focus_it_was_waiting_for():
    window = Window(width=800, height=600)
    lst = make_list(window, count=1000)
    seen = []
    lst.on("focus", lambda e: seen.append(e.target.get("focused")))
    row(lst, 0).focus()
    seen.clear()
    key(window, "end")
    assert focused_row(lst) == 999
    assert seen == [True]
    assert lst.get("scroll_offset") == 1000 * 20 - 100


def test_the_row_that_held_focus_is_released_once_focus_has_moved_off_it():
    window = Window(width=800, height=600)
    lst = make_list(window, count=1000)
    row(lst, 0).focus()
    key(window, "end")
    assert built_rows(lst) <= 100 // 20 + 1
