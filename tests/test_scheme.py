"""0.6.11: the window's colour scheme -- `Window(scheme=...)`, `window.set(scheme=...)`,
`App(scheme=...)`, `tess_re.scheme_colors(...)` and `set(scheme_colors=...)`. Headless: pixels come
from `Window.snapshot`, the OS appearance from `simulate("color_scheme")`."""

from collections import Counter

import pytest

import tess_re
from tess_re import App, Window
from helpers import add

RED = (255, 0, 0, 255)
LIGHT_TEXT = (0x1D, 0x1B, 0x20, 255)  # on_surface, light
DARK_TEXT = (0xE6, 0xE0, 0xE9, 255)  # on_surface, dark


def shot(window):
    rgba, width, height = window.snapshot()
    return [tuple(rgba[i : i + 4]) for i in range(0, len(rgba), 4)], width, height


def solid(window):
    """The colours of the fully opaque pixels, most common first."""
    pixels, _, _ = shot(window)
    return Counter(p for p in pixels if p[3] == 255)


def big_text(window, **props):
    return add(window, "text", text="I", font_size=90, font_weight=700, **props)


def window_with_text(**kwargs):
    window = Window(width=120, height=120, **kwargs)
    window.root.set(padding=0)
    node = big_text(window)
    return window, node


def painted_colour(window):
    colours = solid(window)
    assert colours, "nothing opaque was drawn"
    return colours.most_common(1)[0][0]


# --- The properties ---


def test_a_window_follows_the_os_by_default_and_is_light_while_it_is_unknown():
    window = Window(width=100, height=100)
    assert window.get("scheme") == "auto"
    assert window.get("scheme_resolved") == "light"


@pytest.mark.parametrize("scheme", ["light", "dark", "auto"])
def test_the_scheme_is_given_to_the_constructor_and_read_back(scheme):
    window = Window(width=100, height=100, scheme=scheme)
    assert window.get("scheme") == scheme
    assert window.get("scheme_resolved") == ("dark" if scheme == "dark" else "light")


def test_the_scheme_is_set_live_and_the_dark_getter_stays_the_os_flag():
    window = Window(width=100, height=100)
    os_flag = window.get("dark")
    window.set(scheme="dark")
    assert (window.get("scheme"), window.get("scheme_resolved")) == ("dark", "dark")
    assert window.get("dark") == os_flag
    window.set(scheme="light")
    assert window.get("scheme_resolved") == "light"
    assert window.get("dark") == os_flag


def test_auto_follows_the_os_appearance_event():
    window = Window(width=100, height=100)
    window.simulate("color_scheme", dark=True)
    assert window.get("scheme") == "auto"
    assert window.get("scheme_resolved") == "dark"
    window.simulate("color_scheme", dark=False)
    assert window.get("scheme_resolved") == "light"
    # An explicit scheme does not follow it.
    window.set(scheme="light")
    window.simulate("color_scheme", dark=True)
    assert window.get("scheme_resolved") == "light"


@pytest.mark.parametrize("bad", ["Dark", "blue", 1, True])
def test_a_bad_scheme_raises_and_changes_nothing(bad):
    window = Window(width=100, height=100, scheme="dark")
    with pytest.raises(ValueError, match='"light", "dark" or "auto"'):
        window.set(scheme=bad)
    assert window.get("scheme") == "dark"
    with pytest.raises(ValueError, match='"light", "dark" or "auto"'):
        Window(scheme=bad)


def test_scheme_resolved_is_read_only():
    with pytest.raises(ValueError, match="read-only"):
        Window(width=100, height=100).set(scheme_resolved="dark")


# --- The app's default ---


def test_a_window_inherits_the_apps_scheme_until_it_sets_its_own():
    app = App(scheme="dark")
    assert app.get("scheme") == "dark"
    inheriting = Window(width=100, height=100)
    own = Window(width=100, height=100, scheme="light")
    app.add_window(inheriting)
    app.add_window(own)
    assert (inheriting.get("scheme"), inheriting.get("scheme_resolved")) == ("dark", "dark")
    assert (own.get("scheme"), own.get("scheme_resolved")) == ("light", "light")
    app.set(scheme="light")
    assert app.get("scheme") == "light"
    assert inheriting.get("scheme_resolved") == "light"
    app.set(scheme="dark")
    assert inheriting.get("scheme_resolved") == "dark"
    assert own.get("scheme_resolved") == "light", "a window's own scheme beats the app's"
    # `None` gives a window back to the app.
    own.set(scheme=None)
    assert own.get("scheme_resolved") == "dark"


def test_the_app_defaults_to_auto_and_checks_its_arguments():
    app = App()
    assert app.get("scheme") == "auto"
    with pytest.raises(ValueError, match='"light", "dark" or "auto"'):
        App(scheme="sepia")
    with pytest.raises(ValueError, match='"light", "dark" or "auto"'):
        app.set(scheme="sepia")
    with pytest.raises(ValueError, match="unknown app property"):
        app.set(theme="dark")
    with pytest.raises(ValueError, match="unknown app property"):
        app.get("theme")
    assert app.get("scheme") == "auto"


def test_a_window_added_after_the_app_changed_takes_its_scheme():
    app = App()
    app.set(scheme="dark")
    window = Window(width=100, height=100)
    app.add_window(window)
    assert window.get("scheme_resolved") == "dark"


# --- The role table ---


def test_the_baseline_tables_have_the_49_roles_and_the_published_values():
    light = tess_re.scheme_colors("light")
    dark = tess_re.scheme_colors("dark")
    assert len(light) == len(dark) == 49
    assert list(light) == list(dark)
    assert light["on_surface"] == LIGHT_TEXT and dark["on_surface"] == DARK_TEXT
    assert light["primary"] == (0x67, 0x50, 0xA4, 255) and dark["primary"] == (0xD0, 0xBC, 0xFF, 255)
    assert light["surface"] == (0xFE, 0xF7, 0xFF, 255) and dark["surface"] == (0x14, 0x12, 0x18, 255)
    assert light["on_surface_variant"] == (0x49, 0x45, 0x4F, 255)
    assert dark["on_surface_variant"] == (0xCA, 0xC4, 0xD0, 255)
    assert light["secondary_container"] == (0xE8, 0xDE, 0xF8, 255)
    assert all(len(c) == 4 and all(0 <= v <= 255 for v in c) for c in light.values())


def test_an_unknown_baseline_raises():
    with pytest.raises(ValueError, match="light, dark"):
        tess_re.scheme_colors("auto")


def test_a_windows_table_is_its_baseline_with_its_overrides():
    window = Window(width=100, height=100, scheme="dark")
    assert window.get("scheme_colors") == tess_re.scheme_colors("dark")
    window.set(scheme_colors={"on_surface": RED})
    table = window.get("scheme_colors")
    assert table["on_surface"] == RED
    assert table["primary"] == tess_re.scheme_colors("dark")["primary"]
    # The override survives a scheme switch; the rest follows it.
    window.set(scheme="light")
    table = window.get("scheme_colors")
    assert table["on_surface"] == RED
    assert table["primary"] == tess_re.scheme_colors("light")["primary"]
    # `None` and an empty dict clear the overrides.
    window.set(scheme_colors=None)
    assert window.get("scheme_colors") == tess_re.scheme_colors("light")
    window.set(scheme_colors={"primary": RED})
    window.set(scheme_colors={})
    assert window.get("scheme_colors") == tess_re.scheme_colors("light")


def test_an_unknown_role_raises_listing_the_valid_ones_and_changes_nothing():
    window = Window(width=100, height=100)
    window.set(scheme_colors={"primary": RED})
    with pytest.raises(ValueError, match=r'unknown role "on_surfaced" -- valid: primary, on_primary, .*scrim'):
        window.set(scheme_colors={"on_surface": RED, "on_surfaced": RED})
    assert window.get("scheme_colors")["primary"] == RED
    assert window.get("scheme_colors")["on_surface"] == LIGHT_TEXT
    with pytest.raises(ValueError, match="(r, g, b, a)"):
        window.set(scheme_colors={"primary": (1, 2, 3)})
    with pytest.raises(ValueError, match="dict"):
        window.set(scheme_colors=[("primary", RED)])


def test_the_scheme_is_set_together_with_other_properties_atomically():
    window = Window(width=100, height=100)
    with pytest.raises(ValueError):
        window.set(scheme="dark", title=5)
    assert window.get("scheme") == "auto"
    window.set(scheme="dark", scheme_colors={"primary": RED}, title="x")
    assert window.get("scheme_resolved") == "dark"
    assert window.get("scheme_colors")["primary"] == RED


# --- What the engine paints ---


def test_a_text_with_no_fill_is_on_surface_of_the_scheme():
    window, _ = window_with_text(scheme="light")
    assert painted_colour(window) == LIGHT_TEXT
    window.set(scheme="dark")
    assert painted_colour(window) == DARK_TEXT


def test_the_default_text_is_no_longer_black():
    window, text = window_with_text()
    assert painted_colour(window) == LIGHT_TEXT
    assert (0, 0, 0, 255) not in solid(window)
    assert text.get("fill") == LIGHT_TEXT


def test_a_text_that_sets_a_fill_keeps_it_in_both_schemes():
    window = Window(width=120, height=120)
    window.root.set(padding=0)
    big_text(window, fill=RED)
    for scheme in ("light", "dark"):
        window.set(scheme=scheme)
        assert painted_colour(window) == RED


def test_fill_none_gives_a_text_back_to_the_scheme():
    window, text = window_with_text(scheme="dark")
    text.set(fill=RED)
    assert text.get("fill") == RED
    assert painted_colour(window) == RED
    text.set(fill=None)
    assert text.get("fill") == DARK_TEXT
    assert painted_colour(window) == DARK_TEXT
    box = add(window, "box")
    with pytest.raises(ValueError, match="text or text_input"):
        box.set(fill=None)


def test_a_text_reads_its_scheme_colour_and_animates_from_it():
    window, text = window_with_text(scheme="dark")
    assert text.get("fill") == DARK_TEXT
    window.set(scheme="light")
    assert text.get("fill") == LIGHT_TEXT
    text.animate("fill", RED, 100)
    window.advance(0)
    assert text.get("fill") == LIGHT_TEXT, "an animation starts from what is on screen"
    window.advance(200)
    assert text.get("fill") == RED
    window.set(scheme="dark")
    assert painted_colour(window) == RED, "the animated fill is the text's own now"


def test_switching_the_scheme_at_run_time_repaints_every_following_node():
    window = Window(width=240, height=120, scheme="light")
    window.root.set(padding=0)
    big_text(window)
    big_text(window)
    assert painted_colour(window) == LIGHT_TEXT
    window.set(scheme="dark")
    colours = solid(window)
    assert DARK_TEXT in colours and LIGHT_TEXT not in colours
    window.simulate("color_scheme", dark=False)  # not followed: the window is explicit
    assert DARK_TEXT in solid(window)


def test_an_auto_window_repaints_when_the_os_appearance_changes():
    window, _ = window_with_text()
    assert painted_colour(window) == LIGHT_TEXT
    window.simulate("color_scheme", dark=True)
    assert painted_colour(window) == DARK_TEXT
    window.simulate("color_scheme", dark=False)
    assert painted_colour(window) == LIGHT_TEXT


def test_an_overridden_role_changes_the_default_text():
    window, _ = window_with_text(scheme="dark")
    window.set(scheme_colors={"on_surface": (0, 255, 0, 255)})
    assert painted_colour(window) == (0, 255, 0, 255)
    window.set(scheme_colors=None)
    assert painted_colour(window) == DARK_TEXT


def test_the_apps_scheme_repaints_its_windows():
    app = App(scheme="light")
    window, _ = window_with_text()
    app.add_window(window)
    assert painted_colour(window) == LIGHT_TEXT
    app.set(scheme="dark")
    assert painted_colour(window) == DARK_TEXT


def field(window, **props):
    return add(window, "text_input", font_size=70, font_weight=700, width=120, height=120, **props)


def test_a_text_inputs_text_placeholder_and_caret_follow_the_scheme():
    for scheme in ("light", "dark"):
        roles = tess_re.scheme_colors(scheme)
        window = Window(width=120, height=120, scheme=scheme)
        window.root.set(padding=0)
        node = field(window, text="I")
        assert painted_colour(window) == roles["on_surface"], scheme
        assert node.get("fill") == roles["on_surface"]

        window = Window(width=120, height=120, scheme=scheme)
        window.root.set(padding=0)
        field(window, placeholder="I")
        assert painted_colour(window) == roles["on_surface_variant"], scheme

        window = Window(width=120, height=120, scheme=scheme)
        window.root.set(padding=0)
        node = field(window, caret_width=12)
        node.focus()
        assert roles["primary"] in solid(window), scheme


def test_a_text_inputs_selection_is_primary_at_30_percent():
    for scheme in ("light", "dark"):
        roles = tess_re.scheme_colors(scheme)
        window = Window(width=120, height=120, scheme=scheme)
        window.root.set(padding=0)
        node = field(window, text="III")
        node.focus()
        window.simulate("key_down", key="a", ctrl=True)  # select all
        pixels, _, _ = shot(window)
        want = roles["primary"]
        # Over a transparent window the highlight is the primary role at alpha 0.3 (77 of 255).
        assert any(
            p[3] in range(75, 80) and all(abs(a - b) <= 3 for a, b in zip(p[:3], want[:3]))
            for p in pixels
        ), scheme


def test_a_text_input_with_a_fill_keeps_the_tint_derived_colours():
    window = Window(width=120, height=120, scheme="dark")
    window.root.set(padding=0)
    field(window, text="I", fill=RED)
    assert painted_colour(window) == RED
    assert tess_re.scheme_colors("dark")["on_surface"] not in solid(window)


def test_a_text_inputs_own_placeholder_caret_and_selection_colours_win():
    window = Window(width=120, height=120, scheme="dark")
    window.root.set(padding=0)
    node = field(window, placeholder="I", placeholder_fill=RED)
    assert painted_colour(window) == RED
    node.set(placeholder_fill=None)
    assert painted_colour(window) == tess_re.scheme_colors("dark")["on_surface_variant"]


def test_a_scroll_views_thumb_is_on_surface_variant_of_the_scheme():
    for scheme in ("light", "dark"):
        role = tess_re.scheme_colors(scheme)["on_surface_variant"]
        window = Window(width=100, height=100, scheme=scheme)
        window.root.set(padding=0)
        view = add(window, "scroll_view", width=100, height=100)
        view.add_child(window.create("box", width=100, height=500, flex_shrink=0))
        pixels, width, _ = shot(window)
        # The one-axis thumb: the role at 0x8C of 255, on nothing, so it is straight alpha.
        thumb = max(pixels[: 40 * width], key=lambda p: p[3])
        assert thumb[3] > 0, scheme
        assert all(abs(a - b) <= 2 for a, b in zip(thumb[:3], role[:3])), (scheme, thumb, role)
        assert abs(thumb[3] - 140) <= 2


def test_a_scroll_views_fill_beats_the_scheme():
    window = Window(width=100, height=100, scheme="dark")
    window.root.set(padding=0)
    view = add(window, "scroll_view", width=100, height=100, scrollbar_fill=RED)
    view.add_child(window.create("box", width=100, height=500, flex_shrink=0))
    assert RED in solid(window)


def terminal_window(scheme, **props):
    window = Window(width=120, height=60, scheme=scheme)
    window.root.set(padding=0)
    term = add(window, "terminal", shell="/bin/sh", cols=2, rows=1, font_size=40, **props)
    return window, term


def test_a_terminals_palette_defaults_to_the_scheme_and_none_gives_it_back():
    window, term = terminal_window("dark")
    palette = term.get("palette")
    assert palette["foreground"] is None and palette["cursor"] is None and palette["selection"] is None
    assert palette["background"] == (0, 0, 0, 0)
    term.set(palette={"foreground": RED, "cursor": RED})
    assert term.get("palette")["foreground"] == RED
    term.set(palette={"foreground": None})
    palette = term.get("palette")
    assert palette["foreground"] is None and palette["cursor"] == RED
    with pytest.raises(ValueError, match="tuple"):
        term.set(palette={"background": None})


# --- A tree with no window, and the layered kinds ---


def test_a_node_made_without_a_window_scheme_call_is_light():
    window = Window(width=100, height=100)
    text = add(window, "text", text="x")
    assert text.get("fill") == LIGHT_TEXT


def test_popup_and_layer_windows_take_the_argument_too():
    layer = tess_re.LayerWindow("test-scheme", size=(100, 40), scheme="dark")
    assert (layer.get("scheme"), layer.get("scheme_resolved")) == ("dark", "dark")
    layer.set(scheme="light")
    assert layer.get("scheme_resolved") == "light"
    popup = tess_re.PopupWindow(layer, size=(50, 50), anchor_rect=(0, 0, 10, 10), scheme="dark")
    assert popup.get("scheme_resolved") == "dark"
    popup.set(scheme_colors={"primary": RED})
    assert popup.get("scheme_colors")["primary"] == RED
