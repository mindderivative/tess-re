"""A private, headless KWin for the live tests (`tools/kwin_live.sh`, the `live_kwin` pytest marker).

The owner's desktop is KWin and a private sway is not: new windows are placed differently, an
`xdg_popup` grab takes the keyboard, layer-shell details differ. CI has no KWin, so this lane is local
only. `private_kwin()` starts

  * a private session bus (`dbus-run-session` with a config of its own, no service activation, no
    system bus), in a fresh `XDG_RUNTIME_DIR` (mode 700 under the temporary folder), with private
    `HOME` and `XDG_*` folders, so KWin reads and writes none of the desktop's settings;
  * `kwin_wayland --virtual --socket pkwin --no-lockscreen` inside that bus, a virtual output and no
    input devices.

Nothing here touches the real session: the environment the sandbox gives out is scrubbed of the real
`WAYLAND_DISPLAY`, `DISPLAY`, bus and runtime directory, the sandbox REFUSES to go on when after setup
the bus or the runtime directory is the real one or lies outside the temporary folder, and the end is
by the process groups it started plus a sweep by runtime directory (a process whose environment names
the private `XDG_RUNTIME_DIR`), never by a process name. No uinput.

Standard library only. Adapted from the sandbox the tess-shell project uses (no dependency on it).
As a script: `python tests/kwin_sandbox.py [--width W] [--height H] -- COMMAND...` runs COMMAND in the
private KWin and returns its exit status.
"""

from __future__ import annotations

import argparse
import contextlib
import os
import re
import select
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from collections.abc import Callable, Iterator, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

#: variables a sandbox never inherits: they name the real session, bus, compositor or sound server
SCRUBBED = (
    "DBUS_SESSION_BUS_ADDRESS",
    "DBUS_STARTER_ADDRESS",
    "DBUS_STARTER_BUS_TYPE",
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "DISPLAY",
    "SWAYSOCK",
    "I3SOCK",
    "NOTIFY_SOCKET",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_ID",
    "XDG_SESSION_PATH",
    "XDG_SESSION_TYPE",
    "XDG_ACTIVATION_TOKEN",
    "DESKTOP_STARTUP_ID",
    "JOURNAL_STREAM",
    "INVOCATION_ID",
    "MANAGERPID",
    "SYSTEMD_EXEC_PID",
    "PULSE_SERVER",
    "PULSE_RUNTIME_PATH",
    "PULSE_COOKIE",
    "PULSE_SINK",
    "PULSE_SOURCE",
    "PIPEWIRE_REMOTE",
    "PIPEWIRE_RUNTIME_DIR",
    "XDG_STATE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "TESS_RE_ALLOW_REAL_SESSION",
    "TESS_RE_LIVE_INPUT",
)

SOCKET = "pkwin"


class SandboxUnavailable(RuntimeError):
    """A tool the sandbox needs is not installed."""


class SandboxError(RuntimeError):
    """A sandbox that could not be started or did not answer."""


class SandboxRefused(RuntimeError):
    """The environment after the setup names the REAL bus or runtime directory: nothing runs."""


# -- the environment -------------------------------------------------------------------------


@dataclass(frozen=True)
class RealSession:
    """What the real session looks like, read from the environment of the process that made the
    sandbox (a sandbox never changes this process's environment)."""

    bus: str
    runtime: str
    uid_runtime: str

    @classmethod
    def current(cls) -> RealSession:
        return cls(
            bus=os.environ.get("DBUS_SESSION_BUS_ADDRESS", ""),
            runtime=os.environ.get("XDG_RUNTIME_DIR", ""),
            uid_runtime=f"/run/user/{os.getuid()}",
        )


def scrubbed_environ(base: Mapping[str, str] | None = None) -> dict[str, str]:
    """`base` (default: this process) without the variables that name the real session."""
    env = dict(os.environ if base is None else base)
    for name in SCRUBBED:
        env.pop(name, None)
    return env


def refuse_real(env: Mapping[str, str], real: RealSession | None = None) -> None:
    """Raise `SandboxRefused` when `env` holds the real bus or runtime directory, or when the bus or
    the runtime directory is missing or outside the temporary folder, or when it names a display."""
    real = RealSession.current() if real is None else real
    runtime = env.get("XDG_RUNTIME_DIR", "")
    bus = env.get("DBUS_SESSION_BUS_ADDRESS", "")
    real_runtimes = {os.path.realpath(p) for p in (real.runtime, real.uid_runtime) if p}
    if not runtime or not os.path.isabs(runtime):
        raise SandboxRefused(f"XDG_RUNTIME_DIR is not a private absolute path: {runtime!r}")
    resolved = os.path.realpath(runtime)
    tmp = os.path.realpath(tempfile.gettempdir())
    if resolved in real_runtimes or any(resolved.startswith(r + os.sep) for r in real_runtimes):
        raise SandboxRefused(f"XDG_RUNTIME_DIR {runtime} is the real runtime directory")
    if not resolved.startswith(tmp + os.sep):
        raise SandboxRefused(f"XDG_RUNTIME_DIR {runtime} is not under {tmp}")
    if not bus:
        raise SandboxRefused("DBUS_SESSION_BUS_ADDRESS is not set")
    if real.bus and bus == real.bus:
        raise SandboxRefused(f"the bus {bus} is the real session bus")
    if real.uid_runtime in bus or (real.runtime and real.runtime in bus):
        raise SandboxRefused(f"the bus {bus} lives in the real runtime directory")
    match = re.search(r"unix:path=([^,;]+)", bus)
    if not match or not os.path.realpath(match.group(1)).startswith(resolved + os.sep):
        raise SandboxRefused(f"the bus {bus} is not a socket inside {runtime}")
    if env.get("DISPLAY"):
        raise SandboxRefused(f"DISPLAY={env['DISPLAY']} names an X server")
    display = env.get("WAYLAND_DISPLAY", "")
    if display and (display.startswith("/") or display != SOCKET):
        raise SandboxRefused(f"WAYLAND_DISPLAY={display!r} is not the private socket {SOCKET!r}")


def force_remove(path: Path) -> None:
    """`rm -rf` that first makes every folder writable."""
    if not path.exists() and not path.is_symlink():
        return
    for root, dirs, files in os.walk(path):
        for name in (*dirs, *files):
            full = os.path.join(root, name)
            with contextlib.suppress(OSError):
                if not os.path.islink(full):
                    os.chmod(full, stat.S_IRWXU if os.path.isdir(full) else stat.S_IRUSR | stat.S_IWUSR)
    with contextlib.suppress(OSError):
        os.chmod(path, stat.S_IRWXU)
    shutil.rmtree(path, ignore_errors=True)


def unmount_under(path: Path) -> list[str]:
    """Lazily unmount every mount below `path` (a gvfs daemon a private bus starts mounts
    `$XDG_RUNTIME_DIR/gvfs`; the mount outlives it and blocks removal)."""
    prefix = str(path) + "/"
    done: list[str] = []
    try:
        with open("/proc/self/mountinfo") as handle:
            lines = handle.read().splitlines()
    except OSError:
        return done
    points = [line.split()[4] for line in lines if len(line.split()) > 4]
    for point in sorted((p for p in points if p.startswith(prefix)), reverse=True):
        for tool in ("fusermount3", "fusermount"):
            if shutil.which(tool):
                subprocess.run([tool, "-u", "-z", point], capture_output=True, check=False, timeout=15)
                break
        else:
            subprocess.run(["umount", "-l", point], capture_output=True, check=False, timeout=15)
        done.append(point)
    return done


def processes_with_runtime(runtime: Path) -> list[int]:
    """Pids (other than this one) whose environment names `runtime` as `XDG_RUNTIME_DIR`."""
    needle = f"XDG_RUNTIME_DIR={runtime}".encode()
    found: list[int] = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit() or int(entry) == os.getpid():
            continue
        try:
            with open(f"/proc/{entry}/environ", "rb") as handle:
                data = handle.read()
        except OSError:
            continue
        if any(item == needle for item in data.split(b"\0")):
            found.append(int(entry))
    return found


def kill_strays(runtime: Path, *, grace: float = 3.0) -> list[int]:
    """Kill every process that still carries `runtime` in its environment (TERM, then KILL).
    Returns the pids that had to be killed."""
    found = pids = processes_with_runtime(runtime)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for pid in pids:
            with contextlib.suppress(OSError):
                os.kill(pid, sig)
        deadline = time.monotonic() + grace
        while time.monotonic() < deadline and processes_with_runtime(runtime):
            time.sleep(0.05)
        pids = processes_with_runtime(runtime)
        if not pids:
            break
    return found


def wait_until(predicate: Callable[[], bool], timeout: float, what: str, *, interval: float = 0.05) -> None:
    """Poll `predicate` until it is true; `SandboxError` on timeout."""
    deadline = time.monotonic() + timeout
    while True:
        if predicate():
            return
        if time.monotonic() > deadline:
            raise SandboxError(f"timed out after {timeout:g} s waiting for {what}")
        time.sleep(interval)


def need(*tools: str) -> None:
    missing = [t for t in tools if not (shutil.which(t) or os.path.exists(t))]
    if missing:
        raise SandboxUnavailable(f"not installed: {', '.join(missing)}")


def stop_group(process: subprocess.Popen[Any], *, timeout: float = 5.0) -> None:
    """Terminate a process started with `start_new_session=True` and its whole group."""
    with contextlib.suppress(OSError):
        os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(OSError):
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
    with contextlib.suppress(OSError):
        os.killpg(process.pid, signal.SIGKILL)  # whatever else stayed in the group


# -- the sandbox -----------------------------------------------------------------------------

# No <standard_session_servicedirs/>: a name nobody owns is not activated, so no portal, no kded, no
# gvfs is started inside the sandbox (the sway lane has none either). KWin 6.8 hosts what it needs.
BUS_CONF = (
    "<!DOCTYPE busconfig PUBLIC "
    '"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" '
    '"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">\n'
    "<busconfig><type>session</type><listen>unix:path={runtime}/bus</listen><auth>EXTERNAL</auth>"
    '<policy context="default"><allow send_destination="*" eavesdrop="true"/>'
    '<allow eavesdrop="true"/><allow own="*"/></policy></busconfig>\n'
)


@dataclass
class Compositor:
    """A private KWin in a private bus. `env` is the environment for any child that should live in
    it: the private `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS`, no `DISPLAY`,
    `TESS_RE_PRIVATE_COMPOSITOR=1`, `TESS_RE_COMPOSITOR=kwin`, `TESS_RE_KWIN_LOG`, `TESS_RE_KWIN_SIZE`."""

    root: Path
    runtime: Path
    env: dict[str, str]
    log: Path
    width: int
    height: int
    real: RealSession
    bus_process: subprocess.Popen[str] | None = None
    process: subprocess.Popen[bytes] | None = None
    leftovers: list[int] = field(default_factory=list)

    @property
    def display(self) -> str:
        return self.env["WAYLAND_DISPLAY"]

    def run(self, argv: Sequence[str], timeout: float = 60, **kw: Any) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(argv),
            env={**self.env, **kw.pop("env", {})},
            capture_output=True,
            text=True,
            timeout=timeout,
            stdin=subprocess.DEVNULL,
            check=False,
            **kw,
        )

    def name_owned(self, name: str) -> bool:
        done = self.run(
            [
                "dbus-send",
                "--session",
                "--dest=org.freedesktop.DBus",
                "--print-reply",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus.NameHasOwner",
                f"string:{name}",
            ],
            timeout=10,
        )
        return done.returncode == 0 and "boolean true" in done.stdout

    def kwin_log(self) -> str:
        return self.log.read_text(errors="ignore")

    def processes(self) -> list[int]:
        return processes_with_runtime(self.runtime)


@contextlib.contextmanager
def private_kwin(*, width: int = 1280, height: int = 720) -> Iterator[Compositor]:
    """A private bus and a real `kwin_wayland --virtual` on socket `pkwin` inside it; waits until the
    socket exists and `org.kde.KWin` is on the private bus. Everything is stopped and removed at the
    end, also on failure."""
    need("kwin_wayland", "dbus-run-session", "dbus-send")
    real = RealSession.current()
    root = Path(tempfile.mkdtemp(prefix="tre-kwin-", dir=tempfile.gettempdir()))
    runtime = root / "run"
    runtime.mkdir(mode=0o700)
    os.chmod(runtime, 0o700)
    env = scrubbed_environ()
    env["XDG_RUNTIME_DIR"] = str(runtime)
    for name, sub in (
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_STATE_HOME", "state"),
        ("HOME", "home"),
    ):
        (root / sub).mkdir()
        env[name] = str(root / sub)
    env["DBUS_SYSTEM_BUS_ADDRESS"] = f"unix:path={root}/no-system-bus"
    conf = root / "session.conf"
    conf.write_text(BUS_CONF.replace("{runtime}", str(runtime)))
    log = root / "kwin.log"
    sandbox = Compositor(root, runtime, env, log, width, height, real)
    handle = None
    try:
        sandbox.bus_process = subprocess.Popen(
            ["dbus-run-session", f"--config-file={conf}", "--", "sh", "-c", 'echo "$DBUS_SESSION_BUS_ADDRESS"; exec cat'],
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            start_new_session=True,
        )
        assert sandbox.bus_process.stdout is not None
        ready, _, _ = select.select([sandbox.bus_process.stdout], [], [], 20)
        line = sandbox.bus_process.stdout.readline().strip() if ready else ""
        if not line:
            raise SandboxError("dbus-run-session printed no bus address")
        env["DBUS_SESSION_BUS_ADDRESS"] = line
        refuse_real(env, real)
        wait_until(lambda: sandbox.name_owned("org.freedesktop.DBus"), 20, "the private bus")

        handle = open(log, "wb")  # noqa: SIM115 - lives as long as KWin
        # `KWIN_SCREENSHOT_NO_PERMISSION_CHECKS`: org.kde.KWin.ScreenShot2 is restricted to clients with a
        # desktop file; in the private KWin the test process may take screenshots
        # (`live_compositor.screenshot()`). Scripts print to kwin.log through the logging rules.
        sandbox.process = subprocess.Popen(
            ["kwin_wayland", "--virtual", "--socket", SOCKET, "--no-lockscreen", "--width", str(width), "--height", str(height)],
            env={
                **env,
                "KWIN_SCREENSHOT_NO_PERMISSION_CHECKS": "1",
                "QT_LOGGING_RULES": "kwin_scripting.debug=true;js.debug=true",
                "QT_FORCE_STDERR_LOGGING": "1",
            },
            stdin=subprocess.DEVNULL,
            stdout=handle,
            stderr=handle,
            start_new_session=True,
        )

        def up() -> bool:
            assert sandbox.process is not None
            if sandbox.process.poll() is not None:
                raise SandboxError(f"kwin_wayland exited early: {log.read_text(errors='ignore')[-600:]}")
            return (runtime / SOCKET).exists() and sandbox.name_owned("org.kde.KWin")

        wait_until(up, 40, "the private KWin")
        env["WAYLAND_DISPLAY"] = SOCKET
        env["TESS_RE_PRIVATE_COMPOSITOR"] = "1"
        env["TESS_RE_COMPOSITOR"] = "kwin"
        # What `tests/live_compositor.py` needs to read this KWin: its log (script output), its output size.
        env["TESS_RE_KWIN_LOG"] = str(log)
        env["TESS_RE_KWIN_SIZE"] = f"{width}x{height}"
        refuse_real(env, real)
        yield sandbox
    finally:
        if sandbox.process is not None:
            stop_group(sandbox.process)
        if handle is not None:
            handle.close()
        if sandbox.bus_process is not None:
            with contextlib.suppress(OSError):
                if sandbox.bus_process.stdin is not None:
                    sandbox.bus_process.stdin.close()
            stop_group(sandbox.bus_process)
        sandbox.leftovers = kill_strays(runtime)
        unmount_under(runtime)
        force_remove(root)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run a command inside a private headless KWin.")
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        print("kwin_sandbox: no command given (put it after --)", file=sys.stderr)
        return 2
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))  # still run the cleanup below
    try:
        with private_kwin(width=args.width, height=args.height) as kwin:
            print(
                f"kwin_live: WAYLAND_DISPLAY={kwin.display} XDG_RUNTIME_DIR={kwin.runtime} "
                f"bus={kwin.env['DBUS_SESSION_BUS_ADDRESS']}",
                file=sys.stderr,
            )
            try:
                status = subprocess.run(command, env=kwin.env, check=False).returncode
            except FileNotFoundError as error:
                print(f"kwin_sandbox: {error}", file=sys.stderr)
                status = 127
        if kwin.leftovers:
            print(f"kwin_live: killed stray processes of the sandbox: {kwin.leftovers}", file=sys.stderr)
        return status
    except SandboxUnavailable as error:
        print(f"kwin_live.sh: {error}", file=sys.stderr)
        return 2
    except (SandboxError, SandboxRefused) as error:
        print(f"kwin_live.sh: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
