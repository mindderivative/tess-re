"""The KWin lane itself (`tools/kwin_live.sh`): what the sandbox promises, checked from inside it.

Runs only there (`live_kwin`). The sandbox must be private (own bus and runtime directory, no X server, no
way to the real session), KWin must show the globals the other live tests lean on, and the probes
(`live_compositor`: screenshot, window list, input) must work. The IME tests of `test_layer_window_live.py`
need a wlroots input-method protocol that KWin does not offer: that is checked here so the reason stays true.
"""

import os
import tempfile

import live_compositor
import pytest

pytestmark = pytest.mark.live_kwin


def test_the_environment_is_the_sandboxs_and_not_the_desktops():
    runtime = os.path.realpath(os.environ["XDG_RUNTIME_DIR"])
    assert runtime.startswith(os.path.realpath(tempfile.gettempdir()) + os.sep)
    assert not runtime.startswith(f"/run/user/{os.getuid()}")
    assert os.environ["WAYLAND_DISPLAY"] == "pkwin"
    assert f"path={runtime}/bus" in os.environ["DBUS_SESSION_BUS_ADDRESS"]
    assert not os.environ.get("DISPLAY") and not os.environ.get("SWAYSOCK")
    assert os.environ["TESS_RE_PRIVATE_COMPOSITOR"] == "1" and os.environ["TESS_RE_COMPOSITOR"] == "kwin"
    assert not os.environ.get("TESS_RE_ALLOW_REAL_SESSION") and not os.environ.get("TESS_RE_LIVE_INPUT")
    assert os.environ["HOME"].startswith(os.path.dirname(runtime)), "private HOME: no desktop settings are read"


def test_the_probes_work_on_an_empty_desktop():
    assert live_compositor.kwin_windows() == []
    shot = live_compositor.screenshot()
    assert (shot.width, shot.height) == live_compositor.output_rect()[2:]
    with live_compositor.KWinInput() as device:
        device.move(10, 10)  # authenticates and reaches the compositor without a protocol error


def test_kwin_offers_what_the_live_tests_use_and_no_wlroots_input_method():
    with live_compositor._Wayland() as client:
        names = set(client.globals)
    assert {"zwlr_layer_shell_v1", "xdg_wm_base", "org_kde_kwin_fake_input", "wp_fractional_scale_manager_v1"} <= names
    # The IME tests drive a fake zwp_input_method_v2 (tests/wlr_input_method.py); KWin has only the v1 one.
    assert "zwp_input_method_manager_v2" not in names, sorted(names)
    assert "zwlr_virtual_pointer_manager_v1" not in names, "KWin takes input through its fake input, not wlroots'"
