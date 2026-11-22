# Contributing

## Development setup

From a fresh machine (Linux shown; macOS and Windows need only Rust and Python):

```bash
# 1. System packages (Debian/Ubuntu names; the same libraries exist elsewhere).
#    The list CI uses is in .github/workflows/ci.yml; see Installation for details.
#    python3-dev is needed here, not in CI: `cargo test` links engine-py against libpython.
sudo apt-get install -y build-essential pkg-config git python3 python3-dev python3-venv \
    libxkbcommon-dev libwayland-dev libx11-dev libxrandr-dev libxi-dev libxcursor-dev \
    libxinerama-dev libudev-dev libfontconfig1-dev mesa-vulkan-drivers libgl1-mesa-dri

# 2. Rust: rustup installs the toolchain pinned in rust-toolchain.toml by itself.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. The code, a venv with the Python tools, and the extension built into it.
git clone https://github.com/mindderivative/tess-re.git
cd tess_re
git checkout 0.5.6                 # or main; see "Branches and releases" below
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements-dev.txt
maturin develop --release

# 4. Check that everything works.
tools/verify.sh
```

`tools/verify.sh` is the whole check chain in one script (below), and
`tools/verify.sh --cross` adds the Windows and macOS clippy runs and the MSRV
check. The live-window tests run only with a display (a desktop session, X11 or
Wayland) and skip without one. On a machine with no GPU, Mesa's software Vulkan
(`mesa-vulkan-drivers`, lavapipe) is enough: set
`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json`.

See [Installation](installation.md) for the Rust toolchain requirement
and portable-wheel build notes.

## Branches and releases

Each minor line has its own branch cut from `main` (`0.5.5`, `0.5.6`, ...); its
work is committed there, one commit per finished item with the issue number
(`0.5.6: what it does (#166)`), and the version in `Cargo.toml`, `pyproject.toml`
and `Cargo.lock` is bumped on the branch. Nothing is released until it is told to:

1. Push the branch and open a pull request into `main`; CI (`ci.yml`) runs on it. Bumping the
   version makes it the [full tier](#ci-tiers), so the release PR gets the whole run.
2. Merge it. The merge also deploys the docs to GitHub Pages (`docs.yml`).
3. Tag the merge commit `vX.Y.Z` (annotated) and push the tag. That starts
   `wheels.yml`: it builds the wheels and sdist, creates the GitHub Release, and
   then waits at the **`pypi` environment** for a reviewer's approval before it
   uploads to PyPI (trusted publishing, no token). PyPI never accepts the same
   version twice, so check the version before tagging.
4. After approval, `post-publish.yml` runs by itself when the Wheels run has finished
   (`workflow_run`). It waits until PyPI serves the version, installs it in a clean venv
   with `--no-cache-dir` on Ubuntu, macOS and Windows with Python 3.12 and 3.14, checks that
   the installed version is the tag's and that `tess_re` imports and builds a `Window`, then
   checks the PyPI files and the GitHub Release (wheels for each platform, one sdist, the
   standalone `.so`, a non-empty body) and writes the result to the job summary. It only
   reads: it never tags, approves or publishes, so steps 3 and the `pypi` approval stay with
   the maintainer. To check an already published version, run **Post-publish** by hand
   (`Actions` > `Post-publish` > `Run workflow`, with the `version`). It does not open a
   window, so a real compositor, a GPU, macOS and Windows hardware are still checked by hand.

## The GitHub project

Work is tracked on the GitHub project **Tesserae Rendering Engine** (project 3 of
`mindderivative`). An item moves **Backlog -> Ready -> In progress -> In review ->
Done**: an issue is drafted into Backlog with a scope comment (what goes in, what
is left out and why, size, risks); once the scope is approved it goes to Ready;
In progress while it is built; In review when it is committed, with a comment of
what was checked and what was not; Done when the work is approved. Findings,
failures and fixes go in the issue's comments.

[Tesserae](https://github.com/mindderivative/tesserae) (the UI framework built on
`tess_re`) files its requests as issues here. For each one, the scope is agreed with
the Tesserae side before the work starts, and a local wheel is handed over for it
to test before anything is published.

Picking those requests up on a timer is written down in
[`tools/intake-routine.md`](https://github.com/mindderivative/tess-re/blob/main/tools/intake-routine.md):
the process and the prompt to run in Claude Code, so it can be restarted on a new
machine.

### What has not been checked on real hardware

Open issues #47, #48, #91, #118 and #132-#134 are checks that need a Mac, a
Windows PC, touch hardware or real GPUs. CI runs on software GPUs, and nothing
else is verified on real hardware yet, notably: an IME (East Asian input) on any
platform, screen-reader output for the accessibility states, the feel of scroll
snapping with a real wheel or touch fling, window placement outside Wayland, and
`skip_taskbar` on Windows.

## CI tiers

`ci.yml` does not run everything on every pull request. A small job, `plan`
(`tools/ci_plan.py`), reads what the change touches and picks one of three tiers; **`CI result`**
is the one job that says whether it passed (a job the tier does not use is skipped, which is fine).
The plan is also printed in the run's summary, with the reason.

| Tier | When | What runs |
|---|---|---|
| **docs** | Only documentation changed: `docs/`, any `*.md`, `mkdocs.yml`, `.claude/`. | `mkdocs build --strict`, the generated Python API page is current, `CLAUDE.md` still only imports `AGENTS.md`, the version agrees. No Rust, no Python tests, no examples. |
| **partial** | Code changed: the standard run. | `cargo fmt --check`; clippy and `cargo test` for the **changed crates and every crate that depends on them** (a change to `engine-core` reaches all four, one to `engine-render` reaches `engine-render` and `engine-py`); then, when Python is reached, the extension is built, `mypy --strict`, stubtest and the API page are checked, and the **Python tests that cover the change** run, with **the examples that relate to it** (the ones that name what changed or share a word with the file's name, such as `gradient.rs` and `gradients.py`) and three broad ones (`showcase.py`, `layout.py`, `animation.py`). Linux only. |
| **full** | A release (the workspace `version` in `Cargo.toml` moved, such as 0.5.7 to 0.5.8); a build-configuration change (`Cargo.lock`, `rust-toolchain.toml`, the root `Cargo.toml`, `ci.yml`); the `full-ci` label on the PR; or a manual run (`Actions` > `CI` > `Run workflow`). | Everything: Linux (build, test, clippy, fmt, mypy, stubtest, the whole pytest suite, **every example**), Windows, macOS and the MSRV check. |

Which Python tests a partial run takes. The crates are layered: `engine-core` is under
`engine-render` and `engine-platform`, which are under `engine-py`, the Python surface. A change to
a lower crate reaches Python indirectly, and its Rust names are not in the Python tests, so it runs
the **whole** pytest suite. A change to `engine-py` or `python/` runs the tests that name what
changed (the same match `tools/tests_for.py` prints) plus a small smoke set, and the whole suite
when a changed file is named by no test or the picks are over 40% of the suite. That match is by
name, not coverage data, so it can miss a test that reaches the code another way; add the `full-ci`
label to a PR that deserves the whole run. The pytest picks, and the live window tests that skip on
a runner with no display, are listed in the job's log.

To see the plan for your branch before pushing: `python3 tools/ci_plan.py --base origin/main`.

## Running the checks

`tools/verify.sh` runs all of these in order and stops at the first failure.
Each on its own:

```bash
cargo test --workspace --release
cargo clippy --workspace --all-targets --release -- -D warnings
cargo fmt --all --check
cargo +1.90 check --workspace --all-targets   # MSRV
python -m pytest tests/
mypy --strict python/tess_re
python -m mypy.stubtest tess_re --allowlist tools/stubtest_allowlist.txt
mkdocs build --strict
```

The damage tracker walks only the nodes that changed (see
`crates/engine-render/src/damage.rs`). `TESS_RE_DAMAGE_VERIFY=1` makes every
tracker also walk the whole tree and panic if the two disagree, so run the
suites that way after touching anything that writes to a node, the layout or
the tracker, and CI does: `TESS_RE_DAMAGE_VERIFY=1 cargo test --workspace`. The
randomised `damage_incremental` tests drive it with thousands of edits.
`TESS_RE_DAMAGE_FULL=1` turns the shortcut off, to compare timings.

`stubtest` compares the type stub with the built extension, so run it after
`maturin develop`; CI runs it and `mypy --strict` on Linux.

Platform-only code (`#[cfg(target_os = ...)]`, such as the macOS title bar
and the Windows double-click time) can be type-checked from Linux without
the other OS's SDK:

```bash
rustup target add aarch64-apple-darwin x86_64-pc-windows-msvc
PYO3_CROSS_PYTHON_VERSION=3.12 CARGO_TARGET_DIR=target/macos \
  cargo clippy --workspace --all-targets --target aarch64-apple-darwin -- -D warnings
PYO3_CROSS_PYTHON_VERSION=3.12 CARGO_TARGET_DIR=target/windows \
  cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

This only compiles; CI's macOS and Windows jobs run the tests.

Building on Linux needs a few system packages; see
[Installation](installation.md#option-3-build-from-source).

Every real engine capability has its own headless, GPU-backed pixel test
under `crates/engine-render/tests/`, proving it actually paints what it
claims to, not just that the code compiles.

## Live tests that touch the desktop

**Safety first.** `tests/conftest.py` keeps a test run away from your real session: on Linux, unless
`TESS_RE_PRIVATE_COMPOSITOR=1` (`tools/sway_live.sh` and `tools/kwin_live.sh` set it) or `TESS_RE_ALLOW_REAL_SESSION=1` is in the
environment, it removes `DISPLAY` and `WAYLAND_DISPLAY` for the whole run, so every test that opens a window or
uses the clipboard skips ("no display reachable") instead of acting on the desktop. **The maintainer sets
`TESS_RE_ALLOW_REAL_SESSION=1` for a by-hand run on the desktop** (the commands under "Tests that need hardware or
KWin"), and nobody else's session should. A private compositor does not isolate kernel devices (`/dev/uinput`),
the X server or the session bus; `tools/sway_live.sh` unsets `DISPLAY` and `DBUS_SESSION_BUS_ADDRESS` for that reason.

Tests marked live open real windows on the compositor you are running, and skip without one. Two kinds
need to be asked for, because they act on the real session: `TESS_RE_LIVE_INPUT=1` (a virtual pointer or
keyboard from `/dev/uinput`, which moves the real pointer) and `TESS_RE_LIVE_CLIPBOARD=1`
(`tests/test_clipboard_layer_live.py` overwrites the clipboard, saving the previous text in memory and
putting it back; with no clipboard manager that text is lost when the test ends). Under
`tools/sway_live.sh` the compositor is private, so `TESS_RE_PRIVATE_COMPOSITOR=1` is set and the clipboard
test runs without being asked.

## Live tests in a private KWin

The desktop runs KWin; CI and `tools/sway_live.sh` run a tiling wlroots compositor, and the two differ
(where a new window goes, who gets the keyboard when a popup grabs it, how a held popup's grab behaves).
**`tools/kwin_live.sh -- CMD`** runs CMD inside a private, headless `kwin_wayland --virtual`, the same way
`sway_live.sh` does for sway. It is **local only**: CI has no KWin. It starts

* a private session bus (`dbus-run-session` with a config of its own: no service activation, no system bus)
  in a fresh `XDG_RUNTIME_DIR` (mode 700 under the temporary folder), with private `HOME` and `XDG_*`
  folders, so KWin reads and writes none of the desktop's settings;
* `kwin_wayland --virtual --socket pkwin --no-lockscreen --width 1280 --height 720` in that bus.

CMD gets `WAYLAND_DISPLAY=pkwin`, the private `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS`,
`TESS_RE_PRIVATE_COMPOSITOR=1`, `TESS_RE_COMPOSITOR=kwin`, `TESS_RE_KWIN_LOG` (KWin's log, where a KWin
script prints) and no `DISPLAY`. The sandbox (`tests/kwin_sandbox.py`, standard library only, also usable as
`with private_kwin() as kwin:`) **refuses to run** when the bus or the runtime directory it would hand out
is the real one or lies outside the temporary folder; at the end it stops KWin and the bus by their process
groups and kills whatever still carries the private runtime directory in its environment (never by process
name), then removes the folder. It never uses `/dev/uinput`: pointer and keys go in through KWin's own
`org_kde_kwin_fake_input`, which any client of a `--virtual` KWin may use.

```bash
# once: a venv with the extension (the wheel is built once per commit) and the one extra library
uv venv ~/.cache/tre-kwin-venv --python /usr/bin/python3.14
uv pip install --python ~/.cache/tre-kwin-venv/bin/python pytest jeepney "$(~/.orca/bots/orchestrator/bin/tre-wheel)"
# then, from the repository (PATH puts the venv's python first)
tools/kwin_live.sh -- tools/with_adapter.sh lvp -- python -m pytest -m live_kwin tests/test_popup_geometry_live.py -q -rs
tools/kwin_live.sh -- tools/with_adapter.sh lvp -- python -m pytest tests/test_first_present_live.py -q -rs   # the [kwin] variants
```

`jeepney` (pure Python) is for D-Bus with file descriptors: KWin's screenshot interface
(`org.kde.KWin.ScreenShot2`, which the private KWin allows any client) and the KWin script that lists
windows. Without it the KWin variants skip with that reason.

**Writing a test for both lanes.** Ask for the `compositor` fixture (`pytestmark =
pytest.mark.usefixtures("compositor")`): the test runs once, as `[sway]` in `sway_live.sh` and as `[kwin]` in
`kwin_live.sh`, and the other variant skips with its reason (the `[kwin]` variant carries the `live_kwin`
marker, which skips everywhere that is not the KWin lane, CI included; use the marker alone for a test that
only makes sense on KWin). Ask the compositor through `tests/live_compositor.py` and not through `swaymsg` or
`grim`: `screenshot()`, `pixel(x, y)`, `output_rect()`, `toplevel(title)` (the mapped window, or None while it
is held) and `input_device()` (pointer `move`/`click`/`press`/`release` and `key("Escape")`), each answered by
sway's tools in one lane and by KWin's D-Bus, a script in its log and its fake input in the other. Where the
two compositors really differ, write both answers into the test (a dict keyed by `compositor`, with the reason
in a comment), so a change on either side shows. The protocol-log helpers of `test_first_present_live.py` read
both libwayland timestamp formats.

**The pull-request rule:** the body of every PR states the live-compositor result in one line, `sway: x
passed, KWin(virtual): y passed`, or `... / not applicable` with the reason (no live test touches the
change). Run the live tests the change touches in both lanes before opening it.

### What the KWin lane covers

`[kwin]` variants of: `test_first_present_live.py` (window: hold, `ready()`, deadline two-sided bound,
background colour), `test_first_present_layers_live.py` (layer surface and popup: hold, `ready()`,
deadline, configure ack while held, settings made while held, parent closed while held),
`test_popup_geometry_live.py` and (with PR #346) `test_popup_edge_margin_live.py` (placement by the body,
flip by the body, edge margin), `test_popup_grab_live.py` (grab, click away, keyboard, Escape, a held
popup), `test_hidden_window_live.py`, `test_resize_present_cap_live.py` and `test_timer_animation_live.py`
(a window hidden by minimizing, a window covered by a fullscreen one, a size change; through
`live_compositor.window_action`), `test_compositor_differences_live.py`; and the pointer-driven tests of
`test_layer_window_live.py` (through `live_compositor.LayoutPointer`; it also gives a sway without
`pywayland` a virtual pointer, so they run there too). `test_kwin_lane_live.py` checks the sandbox itself from
inside. `test_window_controls.py` and `test_window_titlebar.py`, which are listed below as hand-run on a
floating compositor, pass in this lane unchanged.

### What differs in KWin

Each row is pinned by a test, so it stays true.

| What | KWin (virtual) | sway (headless) | Test |
|---|---|---|---|
| A new toplevel | keeps the size it asked for (200x200), centred, with a title bar of its own | tiled over the whole output, size ignored | `test_compositor_differences_live.py` |
| Keyboard while a popup holds a grab (the bar below asked for `keyboard="none"` or `"on_demand"`) | goes to the popup: the focused node gets `key_down`, Escape included (the TRE-REQ-024 finding: an `xdg_popup` grab owns the keyboard) | stays with the bar (or nobody): the popup never hears the key | `test_popup_grab_live.py` |
| Escape with a grabbing popup | the compositor does not end the popup; the key is the popup's | same: nobody ends it | `test_popup_grab_live.py`. The `PopupWindow` docs used to say Escape dismisses; it does not on either compositor (corrected in `layer-window.md` and the stub), an app closes the popup from a `key_down` handler |
| Click away while a popup is **held** (`first_present`) | ignored: the grab is inert until the popup is mapped; `ready()` shows it | dismisses the unmapped popup (`closed`, reason `dismissed`); it is never shown | `test_popup_grab_live.py`. The grab has to be requested with the role (a grab cannot be asked for after the first commit), so tre cannot defer it |
| Same everywhere (no difference found) | first present hold/`ready()`/deadline, layer surface and popup mapping, popup placement by the body and flip by the body, edge margin | same | the `[kwin]`/`[sway]` pairs |

### What could not be ported, and why

* **IME (`ime_offset`, the input-method tests of `test_layer_window_live.py`):** they drive a fake
  `zwp_input_method_v2` client (`tests/wlr_input_method.py`); KWin does not offer `zwp_input_method_manager_v2`
  (checked in `test_kwin_lane_live.py`).
* **Output hot-plug (7 tests in `test_layer_window_live.py`):** they add and remove outputs with `swaymsg create_output`;
  a `--virtual` KWin takes `--output-count` at start and has no command to add or remove one while it runs.
* **`test_a_transparent_bar_shows_the_desktop_behind_it`:** written against `grim -o` and sway tiling a window
  behind the bar.
* **The a11y tests, `test_layer_focus_live.py`, `test_clipboard_layer_live.py`, `test_connection_lost_live.py`:** need
  `at-spi`/`dbus-fast`, a sway-only layout (two exclusive layers), a data-control clipboard tool, and two sways of
  their own; not run in the lane.
* **#248 (the 10,000 commits a second after a fullscreen change on RADV):** `test_resize_present_cap_live.py` passes in
  the lane on lavapipe and on RADV, so it does not reproduce there: the burst needs the real KWin (DRM output,
  dmabuf feedback) and stays a by-hand check.
* **Real hardware:** a virtual KWin draws on the CPU and has one output; DP-1/HDMI-A-1 placement, real GPU
  pacing and the `TESS_RE_LIVE_INPUT=1` tests stay by hand (see below).

## Tests that need hardware or KWin

CI and a private headless sway cannot run every live test. The `live-sway` CI job runs the suite
inside `tools/sway_live.sh` with lavapipe, so layer windows, popups, clipboard, hidden-window and the
other compositor tests run on every pull request that runs Python tests. These skip there, each with
its reason, and run **only by hand** (CI has no display, no real GPU and no KWin; there is no
self-hosted runner). Run each group before a release, on a desktop you can watch, on the second
monitor (HDMI-A-1). Each command opens windows.

| Group | Tests | Why it cannot run in CI | Command |
|---|---|---|---|
| Floating compositor (KWin) | `test_window_controls.py::test_a_live_maximize_and_restore_fire_maximized`, `test_window_titlebar.py::test_live_decorations_and_inset` (2) | sway tiles the window and ignores its size, decorations and maximize. They also pass in a private KWin (`tools/kwin_live.sh`, above); run them on the desktop for the real one | `TESS_RE_ALLOW_REAL_SESSION=1 .venv/bin/python -m pytest tests/test_window_controls.py tests/test_window_titlebar.py -q -rs` |
| Real GPU, video memory | `test_gpu_memory_live.py` (3), `test_gpu_shared_live.py` (3) | `drm-total-vram` is a real driver's per-process figure; lavapipe has none | `TESS_RE_ALLOW_REAL_SESSION=1 env -u VK_ICD_FILENAMES .venv/bin/python -m pytest tests/test_gpu_memory_live.py tests/test_gpu_shared_live.py -q -rs` |
| Real GPU, pacing | `test_present_mode.py::test_an_animating_window_is_paced_by_default` (1) | lavapipe draws on the CPU, so CPU time says nothing about pacing | `TESS_RE_ALLOW_REAL_SESSION=1 env -u VK_ICD_FILENAMES .venv/bin/python -m pytest tests/test_present_mode.py -q -rs` |
| Synthetic input | `test_layer_window_live.py` (3 marked `live_input`) | `/dev/uinput` makes a system-wide device the **real** compositor reads: it moves the real pointer, clicks and types on your session. A private sway does not read it, so they must never run unattended or in CI | `TESS_RE_ALLOW_REAL_SESSION=1 TESS_RE_LIVE_INPUT=1 .venv/bin/python -m pytest tests/test_layer_window_live.py -q -rs -k "real_pointer or keyboard_none or negative_margin"` (watch the screen; keep other windows closed) |
| Benchmark | `test_virtual_list_benchmark.py` (1) | a timing run, opt-in by design | `TESS_RE_ALLOW_REAL_SESSION=1 TESS_RE_RUN_BENCHMARK=1 .venv/bin/python -m pytest tests/test_virtual_list_benchmark.py -v -s` |

One more check is the maintainer's by hand, and has no test: whether KWin actually **focuses** a window launched with
a token from `request_activation_token` (the token round-trip itself is tested in `tests/test_activation_token.py`).
Launch an app from a `LayerWindow` button with the token in `XDG_ACTIVATION_TOKEN` and see that its window is
focused, then without the token and see that KWin holds it back.

These 13 are an explicit exception to "every test runs somewhere automatically": run them by hand and
say in the release notes that you did, with the machine's GPU and compositor.

## Runners

The Linux CI jobs run on `ubuntu-24.04`, not `ubuntu-latest`: on 26.04 the `engine-render` GPU tests
(llvmpipe) segfaulted and a restored cargo cache pinned pyo3 to the old image's Python 3.12 (a
`-lpython3.12` link error), which turned `main` red on 2026-11-02. Move the pin when 26.04 passes a full run.
`docs.yml` and `wheels.yml` still use `ubuntu-latest` (no GPU tests there).

## Helper scripts

Small scripts in `tools/` take over the routine parts of the workflow. All of them read before
they write, and the ones that change something (`pr_chain.py`, `board.py move`/`create`,
`restamp_version.py`) do nothing until told to (`--yes`, `--apply`) or show a dry run first.

| Script | What it does |
|---|---|
| `tools/status.sh` | Branch, the chain of local branches, unpushed work, version, the newest `LOG.md` section and the board, in one screen. |
| `tools/branches.py` | The local branches as a tree: commits over the parent, merged, pushed. |
| `tools/tests_for.py` | The clippy, cargo and pytest commands that cover the files you changed (by name match; it runs nothing). |
| `tools/with_adapter.sh` | Runs a command on one GPU adapter: `radv`, `lvp` (llvmpipe) or `fallback`. |
| `tools/sway_live.sh` | Runs a command inside a private headless sway, for the live `LayerWindow` tests, and shuts it down. |
| `tools/kwin_live.sh` | The same for a private headless KWin (own bus and runtime directory; local only): [Live tests in a private KWin](#live-tests-in-a-private-kwin). |
| `tools/pr_chain.py` | Status, push, open, retarget (with the close and reopen that start CI), wait, merge and cleanup for a stack of branches. |
| `tools/board.py` | The project board: `status`, `move` (Done needs `--approved`) and `create` (an issue into Backlog or Ready). |
| `tools/check_version.py` | Every place the version lives agrees; `--expect V --pypi` before a release. |
| `tools/restamp_version.py` | Re-stamps "X.Y.Z (#N)" notes written for a version that was released without that work. |
| `tools/wrapup.py` | What `LOG.md`, the docs and the board still need after a piece of work. |
| `tools/bench_tree_build.py` | Times building a tree through the Python API: `add_child` at 1k, 5k and 20k siblings, `create`, `set`, a 3,100-node tree, and `build`, `apply` and `set_children` (medians, run by hand; no CI assertion). |
| `tools/vram_probe.py` | How much GPU memory a window holds: one, two or five windows, a bar, a full-screen overlay, each in its own process on the current Wayland session. |
| `tools/xdnd_source.py` | A ctypes XDND drag source, to drive X11 file drops with nobody holding the mouse. |
| `tools/check_shaders_live.py` | A live check of the shader pipeline on the machine it runs on (#91): a video-style input repainting every frame, an effect across real resizes, and idle and animated CPU; prints PASS/FAIL. |
| `tools/check_agent_docs.py` | `CLAUDE.md` only imports `AGENTS.md` (`--fix` restores it); a `verify.sh` step. |
| `tools/graphify_agents.py`, `tools/graphify_refresh.sh` | Write the graphify section of `AGENTS.md` after a graphify upgrade; refresh the code graph when sources are newer (the `Stop` hook). |
| `tools/gen_python_api.py`, `tools/gen_rust_api.py` | Regenerate `docs/api/python.md` (from `python/tess_re/_core.pyi`) and `docs/api/rust.md` (from rustdoc JSON); the CI docs job fails when either page is stale. |

## Checking transparent windows on a new platform

Transparency, blur-behind and click-through depend on the OS and compositor, and
the test suite can only prove them on the machine it runs on. On Windows, macOS,
GNOME or any other desktop, run `python tools/check_transparency.py`: it opens
a frameless transparent window, steps through three phases (normal,
click-through, normal) and prints a report to paste into the issue. The lines
marked LOOK need your eyes.

## Building this documentation site

```bash
pip install mkdocs mkdocs-material
mkdocs serve   # live preview at http://127.0.0.1:8000
mkdocs build   # static site in site/
```

!!! note
    Don't use `pip install '.[docs]'` for this — installing extras from a
    local `pyproject.toml` always builds the base package too (Tesserae Engine's
    real Rust extension), which needs a full Rust toolchain and
    `fontconfig` dev headers you don't need just to build docs. The
    plain install above only ever runs `mkdocs`, never imports `tess_re`.

## Project discipline

This project holds itself to one discipline end to end for every real
change: investigate → plan → implement → test → document → commit. See
the GitHub project [Tesserae Rendering Engine](https://github.com/users/mindderivative/projects/3) (0.5.0 on; its
README describes how a step is recorded),
[`BUILD_TRACKER_ARCHIVE_0.4.md`](https://github.com/mindderivative/tess-re/blob/main/BUILD_TRACKER_ARCHIVE_0.4.md), and
[`BUILD_TRACKER_ARCHIVE_0.3.md`](https://github.com/mindderivative/tess-re/blob/main/BUILD_TRACKER_ARCHIVE_0.3.md) for the
milestone-by-milestone history this discipline has produced, and
[Architecture](architecture.md) for the design principles that shape it.

Commit messages match the existing history's style (see `git log`); real,
empirically-verified findings are preferred over assumptions — a claim
about behavior is checked by actually running the code, not inferred
from reading it alone.

## The API pages

[`docs/api/rust.md`](api/rust.md) and [`docs/api/python.md`](api/python.md) are
generated, not written by hand. Regenerate them when a public item changes:

```bash
# Rust: rustdoc's JSON, then the page (descriptions come from doc comments;
# tools/rust_api_descriptions*.json replaces any that read badly)
for c in engine-core engine-render engine-platform; do
  RUSTC_BOOTSTRAP=1 cargo rustdoc -p $c --lib -- -Z unstable-options --output-format json
done
python tools/gen_rust_api.py

# Python: from python/tess_re/_core.pyi (the Node and Window property and event
# tables are at the top of the script)
python tools/gen_python_api.py
```

Write a doc comment's first sentence as the one-line description a reader
wants: it is what the page shows.
