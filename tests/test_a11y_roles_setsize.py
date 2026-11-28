"""0.6.11 (UXREQ-003, UXREQ-004): `set_size`, `pos_in_set`, `checked="mixed"`, `orientation`, `modal` and the
18 new role names, headless: what `get`/`set` take and refuse. What a screen reader sees is in
`test_a11y_roles_setsize_live.py`; the AccessKit nodes are checked by the Rust unit tests."""

import pytest

from tess_re import Window

OLD_ROLES = [
    "button", "checkbox", "radio", "switch", "slider", "progressbar", "link", "textbox", "tab", "tablist",
    "tabpanel", "menu", "menuitem", "dialog", "alert", "list", "listitem", "tree", "treeitem", "heading", "img",
    "group", "none", "separator",
]
NEW_ROLES = [
    "status", "spinbutton", "radiogroup", "option", "listbox", "toolbar", "navigation", "complementary",
    "region", "grid", "row", "gridcell", "columnheader", "rowheader", "searchbox", "combobox", "tooltip",
    "scrollbar",
]


@pytest.fixture
def node():
    return Window().create("box", width=10, height=10)


def test_defaults(node):
    assert node.get("set_size") is None and node.get("pos_in_set") is None
    assert node.get("a11y_orientation") is None
    assert node.get("modal") is False
    assert node.get("checked") is None


@pytest.mark.parametrize("role", OLD_ROLES + NEW_ROLES)
def test_every_role_name_is_accepted_and_read_back(node, role):
    node.set(role=role)
    assert node.get("role") == role


def test_an_unknown_role_lists_the_valid_names(node):
    with pytest.raises(ValueError) as error:
        node.set(role="carousel")
    message = str(error.value)
    for name in OLD_ROLES + NEW_ROLES:
        assert name in message


@pytest.mark.parametrize("name", ["set_size", "pos_in_set"])
def test_set_size_and_pos_in_set_take_a_positive_int_or_none(node, name):
    node.set(**{name: 12})
    assert node.get(name) == 12
    node.set(**{name: None})
    assert node.get(name) is None
    for bad in (0, -1, 1.5, "3", True):
        with pytest.raises(ValueError):
            node.set(**{name: bad})
    assert node.get(name) is None


@pytest.mark.parametrize("value", [True, False, "mixed", None])
def test_checked_and_pressed_take_the_tri_state(node, value):
    node.set(checked=value)
    assert node.get("checked") is value or node.get("checked") == value
    node.set(pressed=value)
    assert node.get("pressed") is value or node.get("pressed") == value


@pytest.mark.parametrize("bad", ["yes", "Mixed", 1, 0.5, []])
def test_checked_refuses_anything_else(node, bad):
    with pytest.raises(ValueError):
        node.set(checked=bad)
    assert node.get("checked") is None


def test_orientation(node):
    for value in ("horizontal", "vertical", None):
        node.set(a11y_orientation=value)
        assert node.get("a11y_orientation") == value
    for bad in ("diagonal", "Horizontal", 1, True):
        with pytest.raises((ValueError, TypeError)):
            node.set(a11y_orientation=bad)
    assert node.get("a11y_orientation") is None


def test_modal(node):
    node.set(role="dialog", modal=True)
    assert node.get("modal") is True
    node.set(modal=False)
    assert node.get("modal") is False
    with pytest.raises((ValueError, TypeError)):
        node.set(modal="yes")


def test_a_bad_value_changes_nothing_in_the_same_call(node):
    with pytest.raises(ValueError):
        node.set(pos_in_set=4, set_size=0)
    assert node.get("pos_in_set") is None


def test_a_virtual_list_row_reads_back_only_what_was_set():
    window = Window(width=200, height=200)
    made = []

    def materialize(_index):
        row = window.create("box", fill=(0, 0, 0, 255), role="listitem")
        made.append(row)
        return row

    window.root.add_child(window.create("virtual_list", item_count=1000, item_extent=10.0,
                                        materialize=materialize, width=100, height=100))
    window.advance(0)
    assert made, "rows are built"
    # The engine derives the position for the screen reader; it does not store it on the row.
    assert all(row.get("pos_in_set") is None and row.get("set_size") is None for row in made)


MENU_ROLES = ["menubar", "menuitemcheckbox", "menuitemradio"]


@pytest.mark.parametrize("role", MENU_ROLES)
def test_the_menu_roles_of_0_6_14_are_accepted_and_read_back(node, role):
    node.set(role=role)
    assert node.get("role") == role


def test_a_menu_bar_title_exposes_haspopup_and_expanded_and_a_check_item_checked(node):
    node.set(role="menuitem", haspopup="menu", expanded=False)
    assert node.get("haspopup") == "menu" and node.get("expanded") is False
    node.set(haspopup=True)
    assert node.get("haspopup") == "menu"
    node.set(haspopup="dialog")
    assert node.get("haspopup") == "dialog"
    node.set(haspopup=None)
    assert node.get("haspopup") is None
    node.set(role="menuitemcheckbox", checked=True)
    assert node.get("checked") is True
    node.set(role="menuitemradio", checked=False)
    assert node.get("checked") is False


def test_haspopup_refuses_an_unknown_kind(node):
    with pytest.raises(ValueError):
        node.set(haspopup="popover")
