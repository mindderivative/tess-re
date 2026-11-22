"""Choosing the children of a box, container or scroll view (UX spec "Selection", AC-SLT-1 to AC-SLT-5 and
AC-SLT-8): the same properties as a `virtual_list` -- `selection_mode`, `selection`, `selection_anchor`,
`select_all_key`, `select_on_focus` -- with the children as items (`selection` is a list of child nodes), a
child marked unselectable with `item_selectable=False`, and the `selection_change` event. Also the follow-ups
of both kinds of list: Tab and a click do not land on an unselectable item, and a right click chooses an
unchosen item alone. Everything runs headless through `Window.simulate`.
"""

import pytest

from tess_re import Window

LOGS = {}


def make_box(window, count=5, kind="box", mode="multiple", **props):
    """A 200 wide, 100 tall column of `count` focusable 20 dp children, with a `selection_change`
    recorder, `log(host)`. A scroll view has one child, its content, so its items are the children of
    that: a column box. Returns `(host, children)`."""
    layout = {"flex_direction": "vertical"}
    if kind == "scroll_view":
        layout["orientation"] = "vertical"
    host = window.create(kind, width=200, height=100, selection_mode=mode, **layout, **props)
    parent = host
    if kind == "scroll_view":
        parent = window.create("box", width=200, flex_direction="vertical")
        host.add_child(parent)
    kids = [window.create("box", width=200, height=20, flex_shrink=0, focusable=True) for _ in range(count)]
    for kid in kids:
        parent.add_child(kid)
    LOGS[id(host)] = log = []
    host.on("selection_change", lambda e: log.append((e.selection, e.ranges, e.count, e.anchor, e.selection_source)))
    window.root.add_child(host)
    window.advance(16)
    return host, kids


def log(host):
    return LOGS[id(host)]


def click(window, kid, **modifiers):
    window.simulate("click", node=kid, **modifiers)


def key(window, name, **modifiers):
    window.simulate("key_down", key=name, **modifiers)
    window.advance(16)


def index_of(kids, node):
    return None if node is None else kids.index(node)


def chosen(host, kids):
    return [kids.index(n) for n in host.get("selection")]


def focused(kids):
    return next((i for i, k in enumerate(kids) if k.get("focused")), None)


@pytest.fixture(params=["box", "scroll_view"])
def kind(request):
    return request.param


# --- defaults: nothing changes ---------------------------------------------------------------


def test_the_defaults_choose_nothing_and_change_no_key(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, kind=kind, mode="none")
    assert host.get("selection_mode") == "none"
    assert host.get("selection") == [] and host.get("selection_anchor") is None
    assert host.get("select_all_key") is False and host.get("select_on_focus") is False
    assert host.get("selection_announce") is None and host.get("selection_announce_ms") == 300
    assert all(k.get("item_selectable") is True for k in kids)
    clicks = []
    kids[0].on("click", lambda: clicks.append(1))
    click(window, kids[0])
    click(window, kids[1], shift=True)
    kids[0].focus()
    key(window, "space")  # a Space is still a click on the focused child
    key(window, "arrow_down")  # an arrow is nobody's here: focus stays
    key(window, "a", ctrl=True)
    assert host.get("selection") == [] and log(host) == [] and len(clicks) == 2
    assert focused(kids) == 0


def test_a_plain_box_has_no_selection_state_to_read_but_the_defaults():
    window = Window(width=800, height=600)
    box = window.create("box", width=10, height=10)
    assert box.get("selection_mode") == "none" and box.get("selection") == []


def test_a_new_container_takes_the_properties_at_creation(kind):
    window = Window(width=800, height=600)
    host = window.create(
        kind, selection_mode="multiple", select_all_key=True, select_on_focus=True,
        selection_announce="{count} chosen", selection_announce_ms=50,
    )
    assert host.get("selection_mode") == "multiple"
    assert host.get("select_all_key") and host.get("select_on_focus")
    assert host.get("selection_announce") == "{count} chosen" and host.get("selection_announce_ms") == 50
    with pytest.raises(ValueError, match="not a child"):
        window.create(kind, selection_mode="multiple", selection=[window.create("box")])


# --- AC-SLT-1: clicks ------------------------------------------------------------------------


def test_clicks_choose_ranges_and_toggles_with_the_anchor_of_ac_slt_1(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, kind=kind)  # A..E = 0..4
    for index, modifiers, expected, anchor in [
        (1, {}, [1], 1),  # click B
        (3, {"shift": True}, [1, 2, 3], 1),  # Shift+click D
        (4, {"ctrl": True}, [1, 2, 3, 4], 4),  # Ctrl+click E
        (2, {"shift": True}, [2, 3, 4], 4),  # Shift+click C: E..C replaces
    ]:
        click(window, kids[index], **modifiers)
        assert chosen(host, kids) == expected
        assert host.get("selection_anchor") is kids[anchor] or host.get("selection_anchor") == kids[anchor]
    assert [([kids.index(n) for n in s], ranges, count, kids.index(a), src) for s, ranges, count, a, src in log(host)] == [
        ([1], [(1, 2)], 1, 1, "pointer"),
        ([1, 2, 3], [(1, 4)], 3, 1, "pointer"),
        ([1, 2, 3, 4], [(1, 5)], 4, 4, "pointer"),
        ([2, 3, 4], [(2, 5)], 3, 4, "pointer"),
    ]


def test_the_event_carries_live_child_nodes():
    window = Window(width=800, height=600)
    host, kids = make_box(window)
    heard = []
    host.on("selection_change", lambda e: heard.append(e))
    click(window, kids[2])
    (event,) = heard
    assert event.selection == [kids[2]] and event.anchor == kids[2] and event.target == host
    assert event.count == 1 and event.ranges == [(2, 3)] and event.selection_source == "pointer"


def test_ctrl_click_toggles_off_and_an_empty_part_clears(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, count=3, kind=kind)  # children end at 60 of 100
    click(window, kids[0])
    click(window, kids[2], ctrl=True)
    assert chosen(host, kids) == [0, 2]
    click(window, kids[2], ctrl=True)
    assert chosen(host, kids) == [0]
    window.simulate("click", node=host, x=50, y=90)
    assert host.get("selection") == [] and log(host)[-1][2] == 0


def test_a_single_container_chooses_one_whatever_the_modifiers():
    window = Window(width=800, height=600)
    host, kids = make_box(window, mode="single")
    click(window, kids[1])
    click(window, kids[3], shift=True)
    assert chosen(host, kids) == [3]
    click(window, kids[4], ctrl=True)
    assert chosen(host, kids) == [4]
    window.simulate("click", node=host, x=50, y=99)  # the gap clears nothing in a single container
    assert chosen(host, kids) == [4]


def test_a_press_inside_a_child_chooses_that_child():
    window = Window(width=800, height=600)
    host, kids = make_box(window)
    inner = window.create("box", width=10, height=10)
    kids[1].add_child(inner)
    window.advance(16)
    window.simulate("click", node=inner)
    assert chosen(host, kids) == [1]


# --- AC-SLT-2, AC-SLT-3: keys ----------------------------------------------------------------


def test_shift_arrows_extend_from_the_anchor_and_ctrl_arrows_only_move(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, kind=kind)
    click(window, kids[1])
    kids[1].focus()
    key(window, "arrow_down", shift=True)
    key(window, "arrow_down", shift=True)
    assert chosen(host, kids) == [1, 2, 3] and host.get("selection_anchor") == kids[1]
    key(window, "arrow_up", shift=True)
    assert chosen(host, kids) == [1, 2] and focused(kids) == 2
    key(window, "arrow_down", ctrl=True)
    key(window, "arrow_down", ctrl=True)
    assert focused(kids) == 4 and chosen(host, kids) == [1, 2]
    key(window, "space", ctrl=True)
    assert chosen(host, kids) == [1, 2, 4]
    key(window, "space")
    assert chosen(host, kids) == [1, 2], "Space toggles in a multiple container"
    assert [src for *_, src in log(host)[1:]] == ["keyboard"] * 5


def test_single_space_chooses_and_an_arrow_moves_focus_only():
    window = Window(width=800, height=600)
    host, kids = make_box(window, mode="single")
    kids[1].focus()
    key(window, "space")
    key(window, "arrow_down")
    assert chosen(host, kids) == [1] and focused(kids) == 2
    key(window, "space")
    assert chosen(host, kids) == [2]


def test_select_on_focus_follows_the_arrow_keys():
    window = Window(width=800, height=600)
    host, kids = make_box(window, mode="single", select_on_focus=True)
    kids[0].focus()
    key(window, "arrow_down")
    key(window, "arrow_down")
    assert chosen(host, kids) == [2]


def test_ctrl_a_is_opt_in_and_skips_unselectable_children(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, kind=kind, select_all_key=True)
    kids[1].set(item_selectable=False)
    kids[3].set(visible=False)
    kids[0].focus()
    key(window, "a", ctrl=True)
    assert chosen(host, kids) == [0, 2, 4]
    assert log(host)[-1][4] == "keyboard"
    other, other_kids = make_box(window, select_all_key=False)
    other_kids[0].focus()
    key(window, "a", ctrl=True)
    assert other.get("selection") == []


def test_page_keys_move_by_a_page_of_children():
    window = Window(width=800, height=600)
    host, kids = make_box(window, count=30, kind="scroll_view")
    kids[0].focus()
    key(window, "page_down")
    down = focused(kids)
    assert 3 <= down <= 5
    key(window, "page_up")
    assert focused(kids) == 0


# --- AC-SLT-5: unselectable children ---------------------------------------------------------


def test_a_range_passes_unselectable_children_and_focus_never_lands_on_them(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, count=6, kind=kind)
    for i in (0, 2, 3):
        kids[i].set(item_selectable=False)
    click(window, kids[1])
    click(window, kids[5], shift=True)
    assert chosen(host, kids) == [1, 4, 5]
    click(window, kids[2])
    assert chosen(host, kids) == [1, 4, 5], "a click on one chooses nothing"
    assert focused(kids) != 2, "and focuses nothing"
    kids[1].focus()
    visited = []
    for name in ["arrow_down", "arrow_down", "arrow_up", "arrow_up", "arrow_up", "end", "home"]:
        key(window, name)
        visited.append(focused(kids))
    assert visited == [4, 5, 4, 1, 1, 5, 1]


def test_tab_passes_over_unselectable_children():
    window = Window(width=800, height=600)
    host, kids = make_box(window, count=4)
    kids[1].set(item_selectable=False)
    kids[0].focus()
    key(window, "tab")
    assert focused(kids) == 2
    key(window, "tab", shift=True)
    assert focused(kids) == 0
    host.set(selection_mode="none")
    key(window, "tab")
    assert focused(kids) == 1, "with no mode Tab visits every child"


def test_making_a_chosen_child_unselectable_or_invisible_unchooses_it():
    window = Window(width=800, height=600)
    host, kids = make_box(window)
    click(window, kids[0])
    click(window, kids[3], shift=True)
    kids[1].set(item_selectable=False)
    assert chosen(host, kids) == [0, 2, 3]
    assert log(host)[-1][4] == "api"
    kids[3].set(visible=False)
    assert chosen(host, kids) == [0, 2]


def test_a_hidden_child_cannot_be_chosen_by_click_or_range():
    window = Window(width=800, height=600)
    host, kids = make_box(window)
    kids[2].set(visible=False)
    click(window, kids[0])
    click(window, kids[4], shift=True)
    assert chosen(host, kids) == [0, 1, 3, 4]


# --- the right click ------------------------------------------------------------------------


def test_a_right_click_chooses_an_unchosen_child_alone_and_keeps_the_set_of_a_chosen_one(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, kind=kind)
    click(window, kids[0])
    click(window, kids[2], shift=True)
    window.simulate("secondary_click", node=kids[1])
    assert chosen(host, kids) == [0, 1, 2], "on a chosen child the set stays"
    window.simulate("secondary_click", node=kids[4])
    assert chosen(host, kids) == [4], "on an unchosen one it is chosen alone"
    assert log(host)[-1][4] == "pointer"


def test_a_right_click_on_a_virtual_list_row_follows_the_same_rule():
    window = Window(width=800, height=600)
    built = {}

    def materialize(i):
        built[i] = window.create("box", width=100, height=20, focusable=True)
        return built[i]

    lst = window.create(
        "virtual_list", item_count=6, item_extent=20, materialize=materialize, width=200, height=120,
        flex_shrink=0, selection_mode="multiple",
    )
    window.root.add_child(lst)
    window.advance(16)
    window.simulate("click", node=built[0])
    window.simulate("click", node=built[2], shift=True)
    window.simulate("secondary_click", node=built[1])
    assert lst.get("selection") == [0, 1, 2]
    window.simulate("secondary_click", node=built[4])
    assert lst.get("selection") == [4]


def test_tab_and_a_click_skip_an_unselectable_virtual_list_row():
    window = Window(width=800, height=600)
    built = {}

    def materialize(i):
        built[i] = window.create("box", width=100, height=20, focusable=True)
        return built[i]

    lst = window.create(
        "virtual_list", item_count=6, item_extent=20, materialize=materialize, width=200, height=120,
        flex_shrink=0, selection_mode="multiple", unselectable_keys=[1, 2],
    )
    window.root.add_child(lst)
    window.advance(16)
    built[0].focus()
    key(window, "tab")
    assert built[3].get("focused") and not built[1].get("focused")
    key(window, "tab", shift=True)
    assert built[0].get("focused")
    window.simulate("click", node=built[4])
    window.simulate("click", node=built[2])
    assert lst.get("selection") == [4]
    assert built[4].get("focused") and not built[2].get("focused")


# --- `selection` get and set -----------------------------------------------------------------


def test_selection_is_set_from_child_nodes_and_replaces_the_whole_set(kind):
    window = Window(width=800, height=600)
    host, kids = make_box(window, kind=kind)
    host.set(selection=[kids[3], kids[1]])
    assert host.get("selection") == [kids[1], kids[3]]
    assert host.get("selection_anchor") == kids[1]
    assert log(host)[-1][4] == "api" and log(host)[-1][2] == 2
    host.set(selection=[])
    assert host.get("selection") == [] and host.get("selection_anchor") is None
    host.set(selection=[kids[0]])
    host.set(selection=None)
    assert host.get("selection") == []
    n = len(log(host))
    host.set(selection=[])
    assert len(log(host)) == n, "no change, no event"


def test_a_selection_that_names_a_bad_node_changes_nothing():
    window = Window(width=800, height=600)
    host, kids = make_box(window)
    stranger = window.create("box")
    other_window = Window(width=100, height=100)
    foreign = other_window.create("box")
    kids[1].set(item_selectable=False)
    kids[2].set(visible=False)
    host.set(selection=[kids[0]])
    for bad, match in [
        ([stranger], "not a child"),
        ([kids[1]], "item_selectable"),
        ([kids[2]], "not visible"),
        ([foreign], "different Window"),
        ([kids[0], kids[3]][0], "list of child nodes"),
        ("x", "list of child nodes"),
        ([1], "list of child nodes"),
    ]:
        with pytest.raises(ValueError, match=match):
            host.set(selection=bad)
    assert host.get("selection") == [kids[0]]
    host.set(selection_mode="single")
    with pytest.raises(ValueError, match="at most one"):
        host.set(selection=[kids[0], kids[3]])
    host.set(selection_mode="none")
    with pytest.raises(ValueError, match="selection_mode"):
        host.set(selection=[kids[0]])


def test_mode_and_selection_in_one_call_and_a_mode_none_clears():
    window = Window(width=800, height=600)
    host, kids = make_box(window, mode="none")
    host.set(selection_mode="multiple", selection=[kids[0], kids[1]])
    assert chosen(host, kids) == [0, 1]
    host.set(selection_mode="single")
    assert len(host.get("selection")) == 1
    host.set(selection_mode="none")
    assert host.get("selection") == [] and host.get("selection_anchor") is None
    assert log(host)[-1][2] == 0


def test_the_properties_validate_their_values():
    window = Window(width=800, height=600)
    host, _ = make_box(window)
    with pytest.raises(ValueError, match="read-only"):
        host.set(selection_anchor=None)
    with pytest.raises(ValueError, match="a bool"):
        host.set(select_all_key=1)
    with pytest.raises(ValueError, match="one of"):
        host.set(selection_mode="many")
    with pytest.raises(ValueError, match="item_selectable"):
        host.set(unselectable_keys=[1])
    with pytest.raises(ValueError, match="item_selectable"):
        host.get("unselectable_keys")
    for bad in [1, "", "   ", b"x"]:
        with pytest.raises(ValueError, match="selection_announce"):
            host.set(selection_announce=bad)
    for bad in [-1, 61000, float("nan"), "x", True]:
        with pytest.raises(ValueError, match="selection_announce_ms"):
            host.set(selection_announce_ms=bad)
    with pytest.raises(ValueError, match="bool"):
        kids = host.children()
        kids[0].set(item_selectable=3)
    host.set(selection_announce="{count} chosen", selection_announce_ms=0)
    assert host.get("selection_announce") == "{count} chosen" and host.get("selection_announce_ms") == 0
    host.set(selection_announce=None)
    assert host.get("selection_announce") is None


def test_the_properties_apply_to_a_container_or_virtual_list_only():
    window = Window(width=800, height=600)
    canvas = window.create("canvas", width=10, height=10, draw=lambda c: None)
    for name, value in [("selection_mode", "multiple"), ("select_all_key", True), ("selection_announce", "x")]:
        with pytest.raises(ValueError, match="applies only to a virtual_list, box, container or scroll_view"):
            canvas.set(**{name: value})
    with pytest.raises(ValueError, match="applies only to a virtual_list, box, container or scroll_view"):
        canvas.get("selection_mode")
    with pytest.raises(ValueError, match="applies only to a text"):
        canvas.get("selection")


# --- reorder and removal keep the choice by index ---------------------------------------------


def test_a_removed_last_child_leaves_the_set_and_the_choice_stays_by_index():
    window = Window(width=800, height=600)
    host, kids = make_box(window, count=4)
    click(window, kids[3])
    assert chosen(host, kids) == [3]
    kids[3].destroy()
    window.advance(16)
    assert host.get("selection") == []
    assert log(host)[-1][2] == 0 and log(host)[-1][4] == "api"
