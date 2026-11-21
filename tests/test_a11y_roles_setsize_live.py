"""0.6.11 (UXREQ-003, UXREQ-004): what a screen reader sees of `set_size`/`pos_in_set`, `checked="mixed"`,
`a11y_orientation`, `modal` and the new roles, read over a private AT-SPI bus (`atspi_env`).

Needs a Wayland compositor (run it with `tools/sway_live.sh`), `dbus-daemon`, `busctl`, `at-spi2-registryd`
and `dbus-fast`; skips without them. AT-SPI role numbers are `ATSPI_ROLE_*`, state numbers `ATSPI_STATE_*`.
"""

import json
import os
import subprocess
import sys
import time

import pytest

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux") or not os.environ.get("TESS_RE_PRIVATE_COMPOSITOR"),
    reason="needs a private compositor: tools/sway_live.sh -- pytest ...",
)

CHILD = """
import json, os, threading, time
from tess_re import App, Window
app = App()
w = Window(width=400, height=500, title="a11y-roles")
w.root.set(flex_direction="vertical")

def mk(**props):
    node = w.create("box", width=300, height=24, fill=(60, 60, 60, 255), **props)
    w.root.add_child(node)
    return node

mk(role="status", label="Saved")
mk(role="option", label="Mars", pos_in_set=3)
mk(role="checkbox", label="Some", checked="mixed", focusable=True)
mk(role="checkbox", label="Yes", checked=True, focusable=True)
mk(role="slider", label="Volume", value=0.5, value_min=0, value_max=1, a11y_orientation="horizontal")
mk(role="scrollbar", label="Track", value=0.5, value_min=0, value_max=1, a11y_orientation="vertical")
mk(role="dialog", label="Rename", modal=True)
mk(role="dialog", label="Plain dialog")
for role, label in (
    ("spinbutton", "Count"), ("radiogroup", "Group"), ("listbox", "Box"), ("toolbar", "Tools"),
    ("navigation", "Nav"), ("complementary", "Aside"), ("region", "Reg"), ("grid", "Grid"), ("row", "Row"),
    ("gridcell", "Cell"), ("columnheader", "ColH"), ("rowheader", "RowH"), ("searchbox", "Find"),
    ("combobox", "Combo"), ("tooltip", "Tip"), ("alert", "Alert!"),
):
    mk(role=role, label=label)
w.root.add_child(w.create(
    "virtual_list", item_count=1000, item_extent=20.0, width=300, height=60, role="list", label="Planets",
    materialize=lambda i: w.create("box", role="listitem", label="Planet %d" % i, width=300, height=20,
                                   fill=(90, 90, 90, 255)),
))
app.add_window(w)
threading.Thread(
    target=lambda: (time.sleep(2.0), print(json.dumps({"kind": "ready"}), flush=True), time.sleep(20), os._exit(0)),
    daemon=True,
).start()
app.run()
"""

# ATSPI_ROLE_*
STATUS_BAR, SPIN_BUTTON, PANEL, LIST_ITEM, LIST_BOX, TOOL_BAR, LANDMARK = 54, 52, 39, 32, 98, 63, 110
TABLE, TABLE_ROW, TABLE_CELL, COLUMN_HEADER, ROW_HEADER, ENTRY, COMBO_BOX = 55, 90, 56, 10, 47, 79, 11
TOOL_TIP, SCROLL_BAR, NOTIFICATION, CHECK_BOX, SLIDER, DIALOG = 64, 48, 101, 7, 51, 16
# ATSPI_STATE_*
CHECKED, HORIZONTAL, MODAL, VERTICAL, INDETERMINATE, CHECKABLE = 4, 14, 16, 29, 32, 41


@pytest.fixture(scope="module")
def seen():
    sys.path.insert(0, os.path.dirname(__file__))
    import atspi_env

    if not atspi_env.available():
        pytest.skip("needs dbus-daemon, busctl, at-spi2-registryd and dbus-fast (pip install dbus-fast)")
    env = atspi_env.AtspiEnvironment()
    child = subprocess.Popen(
        [sys.executable, "-c", CHILD], env=env.env(), stdout=subprocess.PIPE, text=True,
    )
    try:
        assert json.loads(child.stdout.readline())["kind"] == "ready"
        env.enable()
        root = ("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root")
        found = {}
        deadline = time.time() + 15
        while time.time() < deadline and not found:
            for destination, path in env.children(*root):
                tree = env.tree(destination, path)
                if any(node[2] == "Saved" for node in tree):
                    for depth, role, name, d, p in tree:
                        found[name] = {
                            "role": role,
                            "states": env.states(d, p),
                            "attributes": env.attributes(d, p),
                            "depth": depth,
                            "path": p,
                        }
            time.sleep(0.5)
        assert found, "the window's tree was never published on the a11y bus"
        yield found
    finally:
        child.kill()
        env.close()


@pytest.mark.parametrize(
    "name, role",
    [
        ("Saved", STATUS_BAR), ("Count", SPIN_BUTTON), ("Group", PANEL), ("Mars", LIST_ITEM), ("Box", LIST_BOX),
        ("Tools", TOOL_BAR), ("Nav", LANDMARK), ("Aside", LANDMARK), ("Reg", LANDMARK), ("Grid", TABLE),
        ("Row", TABLE_ROW), ("Cell", TABLE_CELL), ("ColH", COLUMN_HEADER), ("RowH", ROW_HEADER),
        ("Find", ENTRY), ("Combo", COMBO_BOX), ("Tip", TOOL_TIP), ("Track", SCROLL_BAR),
        ("Alert!", NOTIFICATION),
    ],
)
def test_the_new_roles_reach_a_reader_as_the_documented_role(seen, name, role):
    assert seen[name]["role"] == role, seen[name]


def test_an_item_with_a_position_says_it(seen):
    # AT-SPI's `posinset` is one-based; AccessKit's zero-based number is made from the 1-based property.
    assert seen["Mars"]["attributes"].get("posinset") == "3", seen["Mars"]


def test_a_virtual_list_says_each_built_rows_place_without_the_app_setting_it(seen):
    assert seen["Planet 0"]["attributes"].get("posinset") == "1", seen["Planet 0"]
    assert seen["Planet 1"]["attributes"].get("posinset") == "2", seen["Planet 1"]
    # The size is the container's `item_count` (1000), not the 2-3 rows that are built.
    assert seen["Planet 0"]["attributes"].get("setsize") == "1000", seen["Planet 0"]


def test_mixed_is_indeterminate_and_true_is_checked(seen):
    assert INDETERMINATE in seen["Some"]["states"] and CHECKED not in seen["Some"]["states"], seen["Some"]
    assert CHECKABLE in seen["Some"]["states"]
    assert CHECKED in seen["Yes"]["states"] and INDETERMINATE not in seen["Yes"]["states"]


def test_orientation_is_a_horizontal_or_vertical_state(seen):
    assert HORIZONTAL in seen["Volume"]["states"] and VERTICAL not in seen["Volume"]["states"]
    assert VERTICAL in seen["Track"]["states"] and HORIZONTAL not in seen["Track"]["states"]


def test_modal_is_the_modal_state_of_a_dialog(seen):
    assert seen["Rename"]["role"] == DIALOG and MODAL in seen["Rename"]["states"], seen["Rename"]
    assert MODAL not in seen["Plain dialog"]["states"]
