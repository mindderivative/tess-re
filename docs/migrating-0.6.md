# Upgrading to 0.6.x

0.6.0 renames the Python package from `tre` to `tess_re`, and the environment
variables from `TRE_*` to `TESS_RE_*`. Nothing else changed: every class,
function, property and event keeps its name, so a search and replace is the
whole migration. There is no alias and no fallback: `import tre` raises
`ModuleNotFoundError` on 0.6.0, and the old variables are ignored.

The names moved so that the Tesserae projects share one prefix and cannot
collide with other packages: `tess_re` (this engine), `tess_ui`, `tess_el`,
`tess_dl` and `tess_shell`.

The PyPI name does not change: `pip install tesserae-engine`.

## 0.6.0

### Renamed

- **The import name.** `import tre` becomes `import tess_re`, `from tre import
  Window` becomes `from tess_re import Window`, and so on for `tre._core`,
  `tre.snapshot` and the rest. A project that supports both 0.5.x and 0.6.x
  can write `try: import tess_re as tre` / `except ModuleNotFoundError: import
  tre`.
- **The environment variables.** Add `TESS_RE_` in place of `TRE_`; the rest of
  each name is unchanged.

  | Before | After |
  |---|---|
  | `TRE_GPU_DEVICES` | `TESS_RE_GPU_DEVICES` |
  | `TRE_GPU_MEMORY` | `TESS_RE_GPU_MEMORY` |
  | `TRE_GPU_RENDERERS` | `TESS_RE_GPU_RENDERERS` |
  | `TRE_DAMAGE_FULL` | `TESS_RE_DAMAGE_FULL` |
  | `TRE_DAMAGE_VERIFY` | `TESS_RE_DAMAGE_VERIFY` |
  | `TRE_SCROLL_BLIT_MIN_NODES` | `TESS_RE_SCROLL_BLIT_MIN_NODES` |
  | `TRE_SCROLL_BLIT_OFF` | `TESS_RE_SCROLL_BLIT_OFF` |
  | `TRE_FORBID_REMOVED` | `TESS_RE_FORBID_REMOVED` |

  The variables that only the test suite and the tools read changed the same
  way: `TRE_TEST_FORCE_FALLBACK`, `TRE_TEST_PARTIAL`, `TRE_TEST_SIZE`,
  `TRE_TEST_WINDOWS`, `TRE_LIVE_INPUT`, `TRE_LIVE_CLIPBOARD`,
  `TRE_LIVE_BOTTOM_INSET`, `TRE_PRIVATE_COMPOSITOR`, `TRE_RUN_BENCHMARK` and
  `TRE_ICD_DIR`.
- **Error and log text.** Messages that named the engine as `tre` (shader
  errors, GPU debug labels, the default window title) say `tess_re` now. Code
  that matches on that text needs the new word.
- **Exception module.** `UnsupportedPlatform.__module__` is `tess_re`.

### Not changed

- The PyPI distribution name, `tesserae-engine`.
- Every public name inside the package.
- The names of the engine's own threads (`tre-gpu-wake`, `tre-timer-wake`,
  `tre-appearance`, `tre-layer`), so a tool that looks them up by name keeps
  working.
- Earlier upgrade pages (0.3.x to 0.5.x) still say `tre` and `TRE_*`: that was
  right for those releases.

### Check your project

```sh
grep -rnE '\btre\b|TRE_' --include='*.py' --include='*.toml' --include='*.yml' --include='*.sh' .
```

## 0.6.1

### Changed

- **A callback that queues itself stops with the loop (#260).** Once `App.run()` has decided to end (the last
  window closed, or `LoopHandle.stop()`), queued `LoopHandle.call_soon` callbacks are no longer run, so one
  that queues itself again cannot keep `run()` from returning. On macOS (0.5.9 to 0.6.0) such a callback kept
  `run()` from returning after the last window closed. A callback that ran once more after the last window's
  `closed` event now does not. A loop with `keep_alive=True` and no window still runs them.

## 0.6.2

### Changed

- **An unsized text node measures to its content (#263).** Until 0.6.1 a text
  node with no `width` or `height` was zero wide and zero tall, so a centred
  label wrapped one word to a line and an effect over it cropped the text.
  `width="fit-content"` and `height="fit-content"` (0.5.7) were the opt-in. Now
  unset, `"auto"` and `"fit-content"` are the same thing on a text node, on both
  sides: the size of its text.
- **A fit width is the text plus 2 pixels.** A box exactly as wide as the
  measured text can cut it off or wrap its last word. The extra pixels apply
  whenever a text node fits its width, including an explicit `"fit-content"`
  from 0.5.7, so a label you already opted in is 2 px wider. The height is
  exact. A long label is still narrowed to the space its parent offers and wraps
  there; the 2 pixels never make it overflow.
- **`get` reads `"fit-content"`.** `label.get("width")` and `get("height")` return
  `"fit-content"` for a text side that fits, where an unsized one read `"auto"`
  before. A box still reads `"auto"`.
- **`window.measure_text` is unchanged.** It returns the exact size with no extra
  pixels, so code that sizes text by hand from it keeps its numbers.

### Fixed

- **A change to one window made in another window's callback is drawn (#264).** A node set in window B from a
  timer or listener of window A could wait for the next unrelated wake of the loop before it was drawn, in about
  half of all processes with two layer surfaces (the draw order differs per process). It is drawn at once now, so a
  queued no-op `call_soon` after every cross-window change is no longer needed. Nothing to change in your code.

### What may move

- A text node in a `Row` takes its text's width instead of 0. Its height is still
  the row's when the row stretches.
- A text node in a centring or start-aligned box takes its text's size instead of
  0 by 0. In a stretching box it still fills the box's width, as in CSS.
- A passthrough effect over padded text now draws it whole.

### To get the old behaviour

Give the side a number: `width=0, height=0`, or any size you want. A percentage
or a `calc(...)` size also takes that side off fit.

```python
label = window.create("text", text="Hi", width=0, height=0)   # the old unsized text
```

### Check your project

Search for text nodes you create without a size and look at the ones inside a
`Row`, a centring box or an anchored layer. The tests that pinned the old zero
(for example a label asserted to be `layout_width == 0`) need the new rule.

## 0.6.3

### Added

- **`PopupWindow(geometry=...)` (#266).** `geometry=(x, y, width, height)` names the popup proper inside a
  larger surface, so a transparent margin that holds a shadow is not placed, flipped or slid by the
  compositor. Nothing changes without it. See the
  [reference](reference/layer-window.md#popupwindow) and
  [Menus from a bar](guide/menus-from-a-bar.md).

## 0.6.4

Additions only, with one thing that can break (see Shader joins). Nothing else changes
without asking for it.

- **`transform_origin` (#269, TRE-REQ-013 step 1).** `scale` and `rotation_deg` turn about a point
  you choose: `node.set(transform_origin=(0, 0))` is the top-left corner, `(1, 1)` the bottom-right, as
  fractions of the box. The default `(0.5, 0.5)` is the center, as before, so nothing changes
  without it. See [Painting](guide/painting.md).
- **`corner_style` (#269, TRE-REQ-013 step 1).** `"concave"` (or a 4-tuple of `"round"` and
  `"concave"`, ordered as `corner_radius`) makes a box corner curve inward about the box's corner, so a
  small box is the fillet between two shapes. The default `"round"` is a plain rounded box, as before.
  See [Painting](guide/painting.md).
- **Shader joins (#270, TRE-REQ-013 step 2).** `shader.animate(name, to, duration_ms, easing)` eases a
  uniform with the engine's clock (no Python per frame); `hit_path` gives any node the shape a click
  hits, in the node's own pixels, so a shader's notches let clicks through; a shader may call the
  helpers `sd_circle`, `sd_box`, `sd_round_box`, `sd_round_union`, `sd_smooth_union` and `sd_coverage`; and
  `frame.scale` is the display scale. **One thing can break:** a shader that declares a function with one
  of the helper names is rejected, as for any other name tess_re's prelude takes; rename it. Nothing else
  changes. See [Shaders](reference/shader.md).
- **`morph` on a path (#271, TRE-REQ-013 step 3).** `morph="structure"` morphs two paths with the same
  commands number by number, so arcs stay arcs and every mid-frame is exact; `"strict"` raises for a pair
  that differs. The default `"resample"` is the morph as it was. See
  [Morphing paths](guide/animation.md#morphing-paths).
- **Parametric paths (#272, TRE-REQ-013 step 4).** A path's `data` with `{expression}` holes and
  `params={...}`, moved by `node.animate_param(name, to, duration_ms, easing)`, is filled in by the engine
  each tick. Plain data is unchanged; only a `{` in `data` makes it a template (a `{` was never valid path
  data). See [A path with parameters](guide/animation.md#a-path-with-parameters).
- **Fixed: an animation started by a timer.** A timer that called `animate` ran after the frame's tick,
  and the window, told nothing was animating, drew once and slept, so the animation did not move until
  something else woke the window. It now draws every frame it runs. (Found while measuring #272; present
  before 0.6.4.)

## 0.6.5

- **`PopupWindow(input_region=)` (#282, TRE-REQ-014).** A popup with a transparent shadow margin took clicks
  in that margin. `input_region=[(x, y, w, h), ...]`, `[]`, or `"geometry"` (the body) limits where it takes
  input, and `popup.set(input_region=...)` changes it; the default `None` is the whole surface, as before.
  See [`PopupWindow`](reference/layer-window.md#popupwindow).

## 0.6.6

- **`wrap` and `overflow` on a `text_input` (#285, TRE-REQ-016).** A multiline input takes `wrap="word"` or
  `"none"`; a single-line one takes `overflow="scroll"`, `"ellipsis"` or `"clip"`, and stays one line. The
  scroll follows the caret by the shaped text. Both default to `None`, which is exactly the 0.6.5 behavior (a
  single-line input wraps by word, a multiline one keeps each line whole), so nothing changes without them. Before,
  both properties raised `ValueError` on a text input. See [Long lines in a text input](guide/text.md#long-lines-in-a-text-input-066).

## 0.6.7

- **A text input's caret keys, Insert and double click (#288, TRE-REQ-017).** In a multiline input with a `wrap`
  (either value), Up, Down, Home, End, Page Up and Page Down go by the visual row, keep the column, and take Shift
  for selecting. Every text input gains Ctrl+Left/Right and Ctrl+Backspace/Delete by word, Ctrl+Home/End, an
  `overwrite` property toggled by the Insert key (with an `overwrite_change` event and a block caret), and double
  click (a word) and triple click (a string up to the spaces) selection. **Behavior changes, in every text input:**
  Ctrl+Left, Ctrl+Right, Ctrl+Backspace and Ctrl+Delete moved or deleted one character and now work by word; Ctrl+Home and
  Ctrl+End in a multiline input went to the start or end of the line and now go to the start or end of the text; and a
  double or triple click used to place the caret only and now selects, so code that clicks twice quickly (a test
  clicking one input twice) selects a word: advance the clock between clicks. A multiline input with no `wrap` keeps
  its 0.6.6 Up, Down, Home, End and Page keys. See
  [Keys, rows and clicks](guide/text.md#keys-rows-and-clicks-in-a-text-input-067).

## 0.6.8 (2026-11-15)

Nothing you have written breaks. A new text-input property, a new launcher call, and two fixes:

- **Setting a text input's `text` to the text it already holds changes nothing (T-037).** It used to move the caret to
  the end and drop the selection, so a framework that binds `text` two-way and sets it again after every keystroke
  threw the caret to the end whenever you typed in the middle. The caret and the selection now stay, and no `change`
  event fires. Setting a *different* text is as before: the caret goes to its end and the selection is cleared.

- **`Window.request_activation_token(callback, app_id=None)` (T-051, new, nothing to migrate).** A launcher asks for an
  `xdg_activation_v1` token and starts the program it launches with `XDG_ACTIVATION_TOKEN` set to it, so the
  compositor focuses the new window. Works on a `Window` and a `LayerWindow`; the callback gets the token, or `None`.
  See [Launching an app and giving it focus](guide/bars-and-panels.md#launching-an-app-and-giving-it-focus).

- **`tab_navigates` on a multiline `text_input` (T-018, TRE-REQ-018, new, nothing to migrate).** Set it and Tab and
  Shift+Tab move focus out of the input instead of typing a tab character, as from a single-line input. Off by
  default, so every existing multiline input behaves as before. See [Text input](guide/text.md#text-input).
- **A popup or layer window closed before it is placed now closes (T-019, TRE-REQ-019, a fix).** `close()` called in the
  same loop turn as `App.add_window`, before the compositor's first configure, used to be ignored: the surface appeared
  and stayed, and neither `close_requested` nor `closed` came. The close is now remembered and made when the surface
  exists, with the same events (`closed`, reason `"app"`). Code that worked around it by guarding its own close can
  drop the guard.

## 0.6.9 (2026-11-16)

Nothing you have written breaks. One new call and one fix:

- **`Window.announce(text, politeness="polite")` (TRE-REQ-021, new, nothing to migrate).** Raises one live announcement
  for a window through the accessibility tree with no node, on a `Window`, a `LayerWindow` and a `PopupWindow`. It works
  for a hidden module and for a repeated message, which a live node cannot do. A shell that announces through a hidden
  1x1 live box can use it instead. See [Announcing without a node](guide/accessibility.md#announcing-without-a-node).
- **A `LayerWindow.set(...)` made before the surface exists is now applied (TRE-REQ-020, a fix).** `set()` right after
  `App.add_window`, before the surface was created, was stored and read back by `get()` but never sent to the
  compositor: a launcher's `keyboard="exclusive"` could be lost, and so could an `input_region`, margin, anchor, size or
  exclusive zone. The stored configuration is now sent as soon as the surface exists, in the order it was made. Code
  that sends the setting again at the first `configure` to work around it can drop that.

## 0.6.10 (2026-11-20)

Nothing you have written breaks. New calls and fixes, listed by request:

- **`active_descendant` on any node (TRE-REQ-023, new, nothing to migrate).** `field.set(active_descendant=row)` tells
  assistive technology which node is the current item of a node that keeps keyboard focus (a search field and its
  result list); `None` clears it. Focus does not move. See
  [Active descendant](guide/accessibility.md#active-descendant).

- **`LayerWindow(size=("content", ...), min_size=, max_size=)` (TRE-REQ-022, new, nothing to migrate).** An axis of `size`
  can be `"content"`: the surface is the root node's outer size, padding included, and follows the content as it
  changes, with one request to the compositor per change and the new content held until the compositor answers.
  `min_size` and `max_size` clamp it. A fixed `size` behaves as before. See
  [A surface the size of its content](reference/layer-window.md#a-surface-the-size-of-its-content).

- **A click-away and keyboard-leave recipe for a panel that holds the keyboard (TRE-REQ-024, documentation and tests, no
  new API).** A `LayerWindow`'s `active` event is the compositor's keyboard enter and leave, and a transparent 1x1
  `PopupWindow(grab=True)` hung off the panel reports a click on another application as its `closed` (reason
  `"dismissed"`), with the panel keeping the keyboard. See
  [Knowing the keyboard left, or the user clicked elsewhere](reference/layer-window.md#knowing-the-keyboard-left-or-the-user-clicked-elsewhere).

- **`Window.announce` keeps call order (TRE-REQ-027, a fix).** Several `announce` calls made in one loop turn reached a
  screen reader in an arbitrary order in 0.6.9, because AccessKit announces the nodes one update adds in hash order.
  tre now sends one announcement per update, oldest first, and keeps drawing frames until the queue is empty, so calls
  are heard in the order they were made (six messages take six frames). Nothing to change.

- **Licence notices in the wheel (TRE-REQ-025, new, nothing to migrate).** `dist-info/licenses/third_party/` now holds
  `THIRD_PARTY_NOTICES.md` (the fonts the engine embeds, the five shared libraries `auditwheel` adds to the Linux
  wheels, and every Rust crate the engine links, with licence, authors and licence text) and the licence text of each
  font and library. The package's `License-Expression` stays `MIT`: that is the engine's own code; the notices file
  says what else is inside. Regenerate with `python3 tools/gen_third_party_notices.py` after the dependency tree changes
  (`--check` says whether the committed file is stale).

- **`App.on("connection_lost", fn)` and `supports("layer_shell", refresh=True)` (TRE-REQ-026, new).** When the compositor
  goes away mid-run, `fn()` is called once from inside `run()`, then each open window hears `closed` with reason
  `"connection_lost"` and `run()` returns normally. In 0.6.9 `run()` returned with nothing said and the windows heard no
  `closed`; code that treated "run() returned with windows still open" as a lost compositor keeps working, and can use the
  callback instead. `supports()` remembers its first answer (a "no" too); `refresh=True` asks again. See
  [`connection_lost`](reference/app.md#connection_lost-0610).

- **`Gradient(..., repeat=, spread=)` (TRE-REQ-006, new, nothing to migrate).** `Gradient.linear`, `radial` and `sweep`
  run their ramp `repeat` times across the span, `spread` being `"repeat"`, `"reflect"` or `"pad"`; the default is one
  pass, drawn as in 0.6.9. `gradient.repeat` and `gradient.spread` read them back, and `repeat` animates. See
  [Repeating a ramp](reference/paint.md#repeating-a-ramp-0610).

- **Timers, `call_soon` and `on_complete` run while every Wayland window is hidden (#247, a fix).** A lone bar under a
  fullscreen window, or windows on another workspace, no longer hold back timers, thread-posted `call_soon` work and
  animation completions until a window is shown again. Nothing is drawn meanwhile. See
  [Hidden windows](reference/window.md#hidden-windows).
- **A resizing window presents at most 250 times a second (#248, a fix).** A fullscreen change on KWin with RADV made an
  animating window commit about 10,000 times a second for 0.4 s. The resize still presents without a vsync barrier, now
  at least 4 ms apart. The issue stays open until the resize is checked by hand on KWin.
