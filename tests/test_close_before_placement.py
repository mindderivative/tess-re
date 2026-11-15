"""T-019 (TRE-REQ-019): a `PopupWindow` or `LayerWindow` closed before the compositor has made it
must close, not appear a moment later and stay with no owner.

`close()` in the same loop turn as `App.add_window` used to be ignored: the popup was configured and
shown, and neither `close_requested` nor `closed` was ever sent. Now the close is remembered and made
the moment the surface exists, with the same events as any other close (`closed` reason `"app"`).

Needs a Wayland compositor with the layer shell: runs in a private sway (`tools/sway_live.sh`).
"""

import json
import os
import subprocess
import sys
import textwrap

import pytest

SCRIPT = """
import json, os, threading, time
from tess_re import App, LayerWindow, PopupWindow

def say(kind, **kw):
    print(json.dumps({"kind": kind, **kw}), flush=True)

app = App()
bar = LayerWindow("close-early", anchor=("top", "left", "right"), size=(0, 40), exclusive_zone=40, keyboard="none")
bar.root.set(fill=(30, 30, 30, 255))
app.add_window(bar)
%(make)s
for ev in ("configure", "close_requested", "closed"):
    victim.on(ev, lambda e, ev=ev: say(ev, reason=getattr(e, "reason", None)))
handle = app.thread_handle()

def act():
    %(open)s
    victim.close()
    %(second)s

threading.Thread(
    target=lambda: (time.sleep(2.0), handle.call_soon(act), time.sleep(3.0), say("done"), os._exit(0)),
    daemon=True,
).start()
app.run()
"""

POPUP = dict(
    make=(
        'victim = PopupWindow(bar, size=(120, 80), anchor_rect=(100, 0, 40, 40), anchor="bottom_left", '
        'gravity="bottom_right", grab=False)'
    ),
    open="app.add_window(victim)",
    second="",
)
POPUP_TWICE = dict(POPUP, second="victim.close()")
LAYER = dict(
    make='victim = LayerWindow("close-early-2", anchor=("bottom", "left", "right"), size=(0, 30), keyboard="none")',
    open="app.add_window(victim)",
    second="",
)


def run(spec):
    if not os.environ.get("WAYLAND_DISPLAY"):
        pytest.skip("needs a Wayland compositor with the layer shell (tools/sway_live.sh)")
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(SCRIPT) % spec], capture_output=True, text=True, timeout=60
    )
    return [json.loads(line) for line in done.stdout.splitlines() if line.startswith("{")], done.stderr


@pytest.mark.parametrize("spec", [POPUP, LAYER], ids=["popup", "layer_window"])
def test_a_close_before_placement_closes_with_the_usual_events(spec):
    events, err = run(spec)
    kinds = [e["kind"] for e in events]
    assert "closed" in kinds, (events, err)
    assert kinds.index("close_requested") < kinds.index("closed"), events
    assert [e["reason"] for e in events if e["kind"] == "closed"] == ["app"], events


def test_a_second_close_is_harmless():
    events, err = run(POPUP_TWICE)
    assert [e["kind"] for e in events].count("closed") == 1, (events, err)
