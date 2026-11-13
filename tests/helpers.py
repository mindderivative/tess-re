"""Shared test helpers.

`add` creates a node with `window.create` and attaches it to the window's
root, so a test that just needs a node in the tree stays one line.
`CODE_EDITOR` is the props for a monospace, multiline text input that shows
whitespace.
"""

import os
from typing import Any

from tess_re import MONOSPACE_FONT_FAMILY, Node, Window

#: Props for a code-editor-style text input.
CODE_EDITOR: dict[str, Any] = {
    "font_family": MONOSPACE_FONT_FAMILY,
    "multiline": True,
    "show_whitespace": True,
}


def add(window: Window, kind: str, **props: Any) -> Node:
    """Creates a `kind` node with `props` and attaches it to the root."""
    node = window.create(kind, **props)
    window.root.add_child(node)
    return node


def software_adapter_pinned() -> bool:
    """Whether the environment asks for a software adapter (lavapipe, `tools/with_adapter.sh lvp`)."""
    icd = os.environ.get("VK_ICD_FILENAMES", "") + os.environ.get("VK_DRIVER_FILES", "")
    return bool(os.environ.get("TESS_RE_TEST_FORCE_FALLBACK")) or "lvp" in icd or "lavapipe" in icd


def on_tiling_compositor() -> bool:
    """Whether the session's compositor tiles new windows (sway, i3-style), which ignores the size, the
    decorations and the maximize a test asks of a toplevel. A private `tools/sway_live.sh` is one."""
    desktop = os.environ.get("XDG_CURRENT_DESKTOP", "").lower()
    return bool(os.environ.get("SWAYSOCK")) or "sway" in desktop
