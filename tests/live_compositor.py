"""What the live tests ask of the compositor they run in, the same way for a private sway and a private KWin.

Two lanes run the live tests (docs/contributing.md, "Live tests in a private compositor"):

  * `tools/sway_live.sh`: a headless sway; pixels from `grim`, windows from `swaymsg -t get_tree`;
  * `tools/kwin_live.sh`: a headless `kwin_wayland --virtual` in a private bus (local only, CI has no
    KWin); pixels from KWin's `org.kde.KWin.ScreenShot2` on the private bus, windows from a KWin script
    that prints `workspace.windowList()` into KWin's log, pointer and keys from KWin's own
    `org_kde_kwin_fake_input` (no uinput, nothing reaches the real session).

`active()` says which one this run is in (`TESS_RE_COMPOSITOR=kwin` is exported by `kwin_live.sh`, a
`SWAYSOCK` by `sway_live.sh`). The KWin side needs `jeepney` (pure Python, `uv pip install jeepney` into
the venv of the KWin lane; not a dependency of anything else) for D-Bus with file descriptors.
"""

from __future__ import annotations

import array
import contextlib
import json
import os
import select
import shutil
import socket
import struct
import subprocess
import tempfile
import threading
import time
import uuid
from dataclasses import dataclass


def active() -> str | None:
    """`'kwin'`, `'sway'` or None: the private compositor this test run is in."""
    if os.environ.get("TESS_RE_COMPOSITOR") == "kwin" and os.environ.get("TESS_RE_KWIN_LOG"):
        # Only the sandbox of kwin_sandbox.py sets these; check it is private all the same (a leftover
        # TESS_RE_COMPOSITOR in a shell must never point the tests at a real session).
        tmp = os.path.realpath(tempfile.gettempdir()) + os.sep
        runtime = os.path.realpath(os.environ.get("XDG_RUNTIME_DIR", "/"))
        bus = os.environ.get("DBUS_SESSION_BUS_ADDRESS", "")
        if (
            runtime.startswith(tmp)
            and f"path={runtime}/" in bus
            and os.environ.get("WAYLAND_DISPLAY") == "pkwin"
            and os.environ.get("TESS_RE_PRIVATE_COMPOSITOR")
            and not os.environ.get("DISPLAY")
        ):
            return "kwin"
        return None
    if os.environ.get("SWAYSOCK"):
        return "sway"
    return None


def skip_reason(name: str) -> str | None:
    """Why a test written for compositor `name` cannot run here, or None when it can."""
    if name == "kwin":
        if not shutil.which("kwin_wayland"):
            return "kwin_wayland is not installed"
        if active() != "kwin":
            return "needs a private KWin (tools/kwin_live.sh; TESS_RE_COMPOSITOR=kwin)"
        try:
            import jeepney  # noqa: F401
        except ImportError:
            return "needs jeepney for KWin's D-Bus (uv pip install jeepney)"
        return None
    if name == "sway":
        if active() != "sway":
            return "needs a private sway (tools/sway_live.sh)"
        if not (shutil.which("swaymsg") and shutil.which("grim")):
            return "needs swaymsg and grim"
        return None
    raise ValueError(name)


# -- pixels ----------------------------------------------------------------------------------


@dataclass
class Shot:
    """An RGB screenshot of the whole output layout (origin at the top-left of the first output)."""

    width: int
    height: int
    rgb: bytes  # width * height * 3

    def pixel(self, x: int, y: int) -> tuple[int, int, int]:
        i = (y * self.width + x) * 3
        return tuple(self.rgb[i : i + 3])  # type: ignore[return-value]

    def find(self, colour: tuple[int, int, int]) -> tuple[int, int, int, int] | None:
        """The bounding box (left, top, right, bottom) of every pixel of `colour`, or None."""
        needle = bytes(colour)
        xs, ys = [], []
        for y in range(self.height):
            row = self.rgb[y * self.width * 3 : (y + 1) * self.width * 3]
            at = row.find(needle)
            while at != -1:
                if at % 3 == 0:
                    xs.append(at // 3)
                    ys.append(y)
                at = row.find(needle, at + 1)
        return (min(xs), min(ys), max(xs) + 1, max(ys) + 1) if xs else None


def _ppm(data: bytes) -> Shot:
    header, rest = data.split(b"\n255\n", 1)
    width, height = (int(v) for v in header.split()[1:3])
    return Shot(width, height, rest)


def screenshot() -> Shot:
    """The whole output layout, from the active compositor."""
    name = active()
    if name == "sway":
        out = subprocess.run(["grim", "-t", "ppm", "-"], capture_output=True, timeout=20, check=True)
        return _ppm(out.stdout)
    if name == "kwin":
        return _kwin_screenshot()
    raise RuntimeError("no private compositor")


def pixel(x: int, y: int) -> tuple[int, int, int]:
    """The colour at output coordinates (x, y)."""
    if active() == "sway":
        out = subprocess.run(
            ["grim", "-g", f"{x},{y} 1x1", "-t", "ppm", "-"], capture_output=True, check=False
        ).stdout
        return tuple(out[-3:])  # type: ignore[return-value]
    return screenshot().pixel(x, y)


def output_rect() -> tuple[int, int, int, int]:
    """(x, y, width, height) of the first output."""
    if active() == "sway":
        outputs = json.loads(
            subprocess.run(["swaymsg", "-t", "get_outputs", "-r"], capture_output=True, check=True).stdout
        )
        rect = outputs[0]["rect"]
        return rect["x"], rect["y"], rect["width"], rect["height"]
    width, height = os.environ["TESS_RE_KWIN_SIZE"].split("x")
    return 0, 0, int(width), int(height)


# -- windows ---------------------------------------------------------------------------------


def _walk(node):
    yield node
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        yield from _walk(child)


def toplevel(title: str) -> dict | None:
    """The mapped toplevel named `title`: {'x', 'y', 'width', 'height'} of its content in output
    coordinates, or None while the compositor has no window of that name (a held first present)."""
    if active() == "sway":
        tree = subprocess.run(["swaymsg", "-t", "get_tree"], capture_output=True, text=True, check=False).stdout
        node = next((n for n in _walk(json.loads(tree)) if n.get("name") == title), None)
        if node is None:
            return None
        rect, inner = node["rect"], node["window_rect"]
        return {
            "x": rect["x"] + inner["x"],
            "y": rect["y"] + inner["y"],
            "width": inner["width"],
            "height": inner["height"],
        }
    for window in kwin_windows():
        if window["caption"] == title:
            return window["client"]
    return None


# -- KWin ------------------------------------------------------------------------------------


def _kwin_connection():
    from jeepney.io.blocking import open_dbus_connection

    return open_dbus_connection(bus=os.environ["DBUS_SESSION_BUS_ADDRESS"], enable_fds=True)


def _kwin_screenshot() -> Shot:
    from jeepney import DBusAddress, MessageType, new_method_call
    from jeepney.fds import FileDescriptor

    address = DBusAddress(
        "/org/kde/KWin/ScreenShot2", bus_name="org.kde.KWin.ScreenShot2", interface="org.kde.KWin.ScreenShot2"
    )
    read_end, write_end = os.pipe()
    chunks: list[bytes] = []
    want = [float("inf")]  # bytes to read, known once KWin has answered

    def drain() -> None:
        got = 0
        while got < want[0]:
            ready, _, _ = select.select([read_end], [], [], 0.05)
            if not ready:
                continue
            data = os.read(read_end, 1 << 20)
            if not data:
                return
            chunks.append(data)
            got += len(data)

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    try:
        with contextlib.closing(_kwin_connection()) as connection:
            # jeepney closes the descriptor once it has been sent
            message = new_method_call(
                address, "CaptureWorkspace", "a{sv}h", ({"native-resolution": ("b", True)}, FileDescriptor(write_end))
            )
            reply = connection.send_and_get_reply(message, timeout=20)
        if reply.header.message_type != MessageType.method_return:
            raise RuntimeError(f"ScreenShot2.CaptureWorkspace failed: {reply.body}")
        info = reply.body[0]
        width, height, stride = info["width"][1], info["height"][1], info["stride"][1]
        want[0] = stride * height
        reader.join(20)
    finally:
        os.close(read_end)
    data = b"".join(chunks)
    if len(data) < stride * height:
        raise RuntimeError(f"ScreenShot2 sent {len(data)} of {stride * height} bytes")
    # QImage::Format_RGBX8888 (16) / RGBA8888 (17, 18): R, G, B, then X or A; (4, 5, 6) are BGRA.
    fmt = info["format"][1]
    rgb = bytearray(width * height * 3)
    order = (0, 1, 2) if fmt in (16, 17, 18) else (2, 1, 0)
    for y in range(height):
        row = data[y * stride : y * stride + width * 4]
        for k, o in enumerate(order):
            rgb[y * width * 3 + k : (y + 1) * width * 3 : 3] = row[o::4]
    return Shot(width, height, bytes(rgb))


def kwin_script(source: str, timeout: float = 10.0):
    """Runs a one-shot KWin script and returns what it passed to `emit(value)` (JSON-able). The script
    is loaded through KWin's `org.kde.kwin.Scripting` on the private bus; `print` goes to KWin's log
    (`QT_LOGGING_RULES` in the sandbox), where the marker line is read back."""
    from jeepney import DBusAddress, new_method_call

    log = os.environ["TESS_RE_KWIN_LOG"]
    token = uuid.uuid4().hex
    marker = f"TRE_OUT {token} "
    folder = os.environ.get("XDG_CACHE_HOME") or os.path.dirname(log)
    path = os.path.join(folder, f"tre-script-{token}.js")
    with open(path, "w") as handle:
        handle.write(f'function emit(v) {{ print("{marker}" + JSON.stringify(v)); }}\n' + source)
    scripting = DBusAddress("/Scripting", bus_name="org.kde.KWin", interface="org.kde.kwin.Scripting")
    with contextlib.closing(_kwin_connection()) as connection:
        loaded = connection.send_and_get_reply(
            new_method_call(scripting, "loadScript", "ss", (path, f"tre-{token}")), timeout=10
        )
        runner = DBusAddress(
            f"/Scripting/Script{loaded.body[0]}", bus_name="org.kde.KWin", interface="org.kde.kwin.Script"
        )
        connection.send_and_get_reply(new_method_call(runner, "run"), timeout=10)
        try:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                with open(log, errors="ignore") as text:
                    for line in text:
                        if marker in line:
                            return json.loads(line.split(marker, 1)[1])
                time.sleep(0.05)
        finally:
            connection.send_and_get_reply(
                new_method_call(scripting, "unloadScript", "s", (f"tre-{token}",)), timeout=10
            )
            os.unlink(path)
    raise RuntimeError("the KWin script emitted nothing")


_WINDOWS = """
emit(workspace.windowList().map(function (w) {
    var c = w.clientGeometry, f = w.frameGeometry;
    return {caption: String(w.caption), cls: String(w.resourceClass), layer: Number(w.layer), active: !!w.active,
            fullscreen: !!w.fullScreen, minimized: !!w.minimized,
            client: {x: c.x, y: c.y, width: c.width, height: c.height},
            frame: {x: f.x, y: f.y, width: f.width, height: f.height}};
}));
"""


def kwin_windows(timeout: float = 10.0) -> list[dict]:
    """Every window of the private KWin (`caption`, `cls`, `layer`, `active`, `fullscreen`, `minimized`,
    `client` and `frame` rectangles)."""
    return kwin_script(_WINDOWS, timeout)


# sway's command for each action, and the property a KWin script sets
_ACTIONS = {
    "fullscreen": ("fullscreen enable", "w.fullScreen = true"),
    "unfullscreen": ("fullscreen disable", "w.fullScreen = false"),
    "hide": ("move scratchpad", "w.minimized = true"),
    "show": ("scratchpad show", "w.minimized = false"),
}


def window_action(title: str, action: str) -> None:
    """Fullscreens, unfullscreens, hides (sway: scratchpad; KWin: minimizes) or shows the window `title`."""
    sway_command, kwin_statement = _ACTIONS[action]
    if active() == "sway":
        out = subprocess.run(
            ["swaymsg", f'[title="{title}"] {sway_command}'], capture_output=True, text=True, check=False
        )
        if out.returncode != 0 or '"success": true' not in out.stdout:
            raise RuntimeError(f"swaymsg {sway_command}: rc {out.returncode} {out.stdout} {out.stderr}")
        return
    done = kwin_script(
        f"var n = 0; workspace.windowList().forEach(function (w) {{ if (String(w.caption) == {json.dumps(title)}) "
        f"{{ {kwin_statement}; n++; }} }}); emit(n);"
    )
    if done != 1:
        raise RuntimeError(f"KWin found {done} windows titled {title!r}")


def is_fullscreen(title: str) -> bool:
    if active() == "sway":
        tree = subprocess.run(["swaymsg", "-t", "get_tree"], capture_output=True, text=True, check=False).stdout
        node = next((n for n in _walk(json.loads(tree)) if n.get("name") == title), None)
        return bool(node and node.get("fullscreen_mode"))
    return any(w["caption"] == title and w["fullscreen"] for w in kwin_windows())


def region(x: int, y: int, width: int, height: int) -> bytes:
    """The pixels of a rectangle of the output, as comparable bytes."""
    shot = screenshot()
    return b"".join(shot.rgb[((y + r) * shot.width + x) * 3 : ((y + r) * shot.width + x + width) * 3] for r in range(height))


class _Wayland:
    """A hand-written Wayland client, just enough to bind one global and send it requests (the wire format
    is small, so no pywayland): the compositor's own virtual-input protocols are all the tests need."""

    def __init__(self) -> None:
        runtime, display = os.environ["XDG_RUNTIME_DIR"], os.environ["WAYLAND_DISPLAY"]
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.connect(os.path.join(runtime, display))
        self.next_id = 2
        self.registry = self._new()
        self._send(1, 1, struct.pack("<I", self.registry))  # wl_display.get_registry
        self.globals = {}
        for obj, opcode, payload in self._roundtrip():
            if obj == self.registry and opcode == 0:  # global(name, interface, version)
                name, length = struct.unpack_from("<II", payload)
                version = struct.unpack_from("<I", payload, 8 + (length + 3) // 4 * 4)[0]
                self.globals[payload[8 : 8 + length - 1].decode()] = (name, version)

    def bind(self, interface: str, version: int) -> int:
        """The new object's id, for a global the compositor offers."""
        if interface not in self.globals:
            raise RuntimeError(f"this compositor offers no {interface}")
        name, offered = self.globals[interface]
        obj = self._new()
        self._send(
            self.registry,
            0,
            struct.pack("<I", name) + self._string(interface) + struct.pack("<II", min(version, offered), obj),
        )
        return obj

    def _new(self) -> int:
        self.next_id += 1
        return self.next_id - 1

    @staticmethod
    def _string(text: str) -> bytes:
        raw = text.encode() + b"\0"
        return struct.pack("<I", len(raw)) + raw + b"\0" * (-len(raw) % 4)

    def _send(self, obj: int, opcode: int, payload: bytes, fd: int | None = None) -> None:
        data = struct.pack("<IHH", obj, opcode, 8 + len(payload)) + payload
        if fd is None:
            self.sock.sendall(data)
        else:
            self.sock.sendmsg([data], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [fd]))])

    def request(self, obj: int, opcode: int, payload: bytes = b"", fd: int | None = None) -> None:
        """Send a request and wait until the compositor has handled it; a protocol error raises."""
        self._send(obj, opcode, payload, fd)
        for target, code, data in self._roundtrip():
            if target == 1 and code == 0:  # wl_display.error
                raise RuntimeError(f"wayland protocol error: {data!r}")

    def _roundtrip(self) -> list[tuple[int, int, bytes]]:
        callback = self._new()
        self._send(1, 0, struct.pack("<I", callback))  # wl_display.sync
        events, buffer = [], b""
        while True:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise RuntimeError("the compositor closed the connection")
            buffer += chunk
            while len(buffer) >= 8:
                obj, word = struct.unpack_from("<II", buffer)
                size, opcode = word >> 16, word & 0xFFFF
                if len(buffer) < size:
                    break
                events.append((obj, opcode, buffer[8:size]))
                buffer = buffer[size:]
                if obj == callback and opcode == 0:
                    return events

    def close(self) -> None:
        self.sock.close()

    def __enter__(self):
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


BTN_LEFT = 0x110
#: key names the tests use and their evdev codes
KEYS = {"Escape": 1, "a": 30}


class KWinInput(_Wayland):
    """Pointer and keys for the private KWin through its `org_kde_kwin_fake_input` (any client of a
    `--virtual` KWin may use it; no uinput). plasma-wayland-protocols' fake-input.xml: request order = opcode."""

    AUTHENTICATE, BUTTON, MOTION_ABSOLUTE, KEY = 0, 2, 9, 10

    def __init__(self) -> None:
        if active() != "kwin":
            raise RuntimeError("KWinInput needs the private KWin")
        super().__init__()
        self.device = self.bind("org_kde_kwin_fake_input", 5)
        self.request(self.device, self.AUTHENTICATE, self._string("tre live tests") + self._string("input for the private KWin lane"))

    def move(self, x: float, y: float) -> None:
        self.request(self.device, self.MOTION_ABSOLUTE, struct.pack("<ii", int(x * 256), int(y * 256)))

    def click(self, x: float | None = None, y: float | None = None) -> None:
        if x is not None and y is not None:
            self.move(x, y)
            time.sleep(0.1)
        self.request(self.device, self.BUTTON, struct.pack("<II", BTN_LEFT, 1))
        self.request(self.device, self.BUTTON, struct.pack("<II", BTN_LEFT, 0))

    def press(self) -> None:
        self.request(self.device, self.BUTTON, struct.pack("<II", BTN_LEFT, 1))

    def release(self) -> None:
        self.request(self.device, self.BUTTON, struct.pack("<II", BTN_LEFT, 0))

    def key(self, name: str) -> None:
        """A key press and release, by name (`KEYS`)."""
        code = KEYS[name]
        self.request(self.device, self.KEY, struct.pack("<II", code, 1))
        self.request(self.device, self.KEY, struct.pack("<II", code, 0))


class LayoutPointer:
    """`tests/wlr_pointer.Pointer`'s interface (`move(x, y, layout_width, layout_height)`, `press`, `release`,
    `click`, `escape`, `close`) over the private compositor's own input (`input_device()`), for the
    pointer-driven tests of `test_layer_window_live.py` that were written against a headless sway."""

    def __init__(self) -> None:
        self.device = input_device()

    def move(self, x, y, layout_width=None, layout_height=None):
        self.device.move(x, y)  # layout pixels; the extent is the output's, known to the device
        time.sleep(0.1)

    def press(self):
        self.device.press()
        time.sleep(0.1)

    def release(self):
        self.device.release()
        time.sleep(0.1)

    def click(self):
        self.device.click()
        time.sleep(0.1)

    def escape(self):
        self.device.key("Escape")

    def close(self):
        self.device.close()


class SwayInput(_Wayland):
    """The same for the private sway: a persistent virtual pointer (`zwlr_virtual_pointer_v1`) and a persistent
    virtual keyboard (`zwp_virtual_keyboard_v1`, keymap from `xkbcli`), which are also what give the headless
    seat a pointer and a keyboard for the whole test (a `wtype` per key would add and remove the keyboard)."""

    CREATE_VIRTUAL_POINTER = 0
    MOTION_ABSOLUTE, BUTTON, FRAME = 1, 2, 4
    CREATE_VIRTUAL_KEYBOARD = 0
    KEYMAP, KEY = 0, 1

    def __init__(self) -> None:
        if active() != "sway":
            raise RuntimeError("SwayInput needs the private sway")
        if not shutil.which("xkbcli"):
            raise RuntimeError("needs xkbcli for a virtual keyboard's keymap in the private sway")
        super().__init__()
        seat = self.bind("wl_seat", 1)
        manager = self.bind("zwlr_virtual_pointer_manager_v1", 2)
        self.pointer = self._new()
        # create_virtual_pointer(seat: object?, id: new_id)
        self.request(manager, self.CREATE_VIRTUAL_POINTER, struct.pack("<II", seat, self.pointer))
        keyboards = self.bind("zwp_virtual_keyboard_manager_v1", 1)
        self.keyboard = self._new()
        self.request(keyboards, self.CREATE_VIRTUAL_KEYBOARD, struct.pack("<II", seat, self.keyboard))
        keymap = subprocess.run(
            ["xkbcli", "compile-keymap", "--layout", "us"], capture_output=True, check=True, timeout=20
        ).stdout + b"\0"
        fd = os.memfd_create("tre-keymap")
        try:
            os.write(fd, keymap)
            self.request(self.keyboard, self.KEYMAP, struct.pack("<II", 1, len(keymap)), fd)  # xkb_v1
        finally:
            os.close(fd)
        _, _, width, height = output_rect()
        self.extent = (width, height)
        self.started = time.monotonic()

    def _time(self) -> int:
        return int((time.monotonic() - self.started) * 1000) & 0xFFFFFFFF

    def move(self, x: float, y: float) -> None:
        self.request(
            self.pointer,
            self.MOTION_ABSOLUTE,
            struct.pack("<IIIII", self._time(), int(x), int(y), *self.extent),
        )
        self.request(self.pointer, self.FRAME)

    def click(self, x: float | None = None, y: float | None = None) -> None:
        if x is not None and y is not None:
            self.move(x, y)
            time.sleep(0.1)
        for state in (1, 0):
            self.request(self.pointer, self.BUTTON, struct.pack("<III", self._time(), BTN_LEFT, state))
            self.request(self.pointer, self.FRAME)

    def press(self) -> None:
        self.request(self.pointer, self.BUTTON, struct.pack("<III", self._time(), BTN_LEFT, 1))
        self.request(self.pointer, self.FRAME)

    def release(self) -> None:
        self.request(self.pointer, self.BUTTON, struct.pack("<III", self._time(), BTN_LEFT, 0))
        self.request(self.pointer, self.FRAME)

    def key(self, name: str) -> None:
        """A key press and release, by name (`KEYS`)."""
        for state in (1, 0):
            self.request(self.keyboard, self.KEY, struct.pack("<III", self._time(), KEYS[name], state))


def input_device() -> KWinInput | SwayInput:
    """Pointer and keys for the active private compositor (a context manager: `with input_device() as d`)."""
    if active() == "kwin":
        return KWinInput()
    if active() == "sway":
        return SwayInput()
    raise RuntimeError("no private compositor")
