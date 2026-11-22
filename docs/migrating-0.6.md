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
  new API).** A `LayerWindow`'s `active` event is the compositor's keyboard enter and leave. *Correction (0.6.11):* this
  note first said a transparent 1x1 `PopupWindow(grab=True)` hung off the panel reports a click on another application
  as its `closed`, with the panel keeping the keyboard. That held on sway only: on KWin the grab takes the keyboard from
  the panel. Use a transparent output-sized `LayerWindow` with `keyboard="none"` under the panel instead. See
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

## 0.6.11

0.6.11 is mostly additive: new properties, each with the Tesserae UX specification's value as its default and `None` meaning
that default. A few defaults change; read **Changed defaults** first. **New properties** are grouped by area in the order they
were added; **Internal (Rust)** lists API changes that Python never sees.

### Changed defaults

- **A wheel notch scrolls 40 px, not 20 (changed, UXREQ-001).** The Tesserae UX note "Scrollbar and Scroll Container"
  sets `scroll.wheel.notch` to 40. It applies to every `"scroll_view"` and `"virtual_list"`, and to the pixels a
  `wheel` event reports for a wheel that sends lines. Deltas that arrive in pixels (a trackpad, a touch pan) are taken
  as they come. Code that tuned a count of notches for 20 px (an app that wrote `scroll_offset` from a wheel
  listener, a test that scrolls by one line and expects 20) takes half as many notches now.

- **Colour scheme: the engine's own default colours follow `Window(scheme=...)` (new; changes defaults; P4, tess-ui
  finding 5).** `scheme` (`"light"`, `"dark"`, `"auto"` = follow the OS, light while unknown) on `Window`, `LayerWindow`,
  `PopupWindow`, `window.set` and `App(scheme=...)` / `app.set`; `window.get("scheme_resolved")`,
  `tess_re.scheme_colors("light" | "dark")` (49 role colours) and `window.set(scheme_colors={...})` overrides.
  **Changed defaults:** a text's and a text input's unset `fill` is `on_surface` (`#1D1B20` light) instead of opaque
  black, so `get("fill")` reads that, and an animation of an unset `fill` starts from it; a text input with no `fill`
  paints its caret in `primary` and its selection in `primary` at 30% (were the text colour, and the text colour
  at 30%) and its placeholder in `on_surface_variant`; a scroll view's default thumb is `on_surface_variant`
  (`#49454F` light, `#CAC4D0` dark; it was `#CAC4D0` in both); a terminal's `palette` `foreground`, `cursor` and
  `selection` read `None` until set (the scheme's `on_surface`, `primary` at 50%, `primary` at 30%; were
  `#1C1B1F`-based). A node with a colour of its own is unchanged, and a text input with a `fill` keeps the derived
  placeholder/caret/selection. To keep the old look, set the colours. `fill=None` on a text or text input gives it
  back to the scheme; `palette` takes `None` for those three keys. See
  [Colour scheme](guide/painting.md#colour-scheme-0611).

- **`hit_testable` is on every node, and a plain `text` is hovered by default (changed, P2; tess-ui finding #4).**
  `node.get("hit_testable")` is `None` (the default: the kind's own answer), `True` or `False`; it used to be `True`
  or `False` with `True` the default read-back. A bare `text` label never took a pointer event before (unless
  `selectable` or a link), so a tooltip listener on a label did nothing; it now gets `pointer_enter` and
  `pointer_leave`, and the button around it stays hovered. A **press** on a plain label is still its parent's, exactly
  as before (a button's label is part of the button, and a press on the label that ends on the button's padding is
  still one click): `pointer_down`, `pointer_up` and `click` reach the label only when it opts in with
  `label.set(hit_testable=True)` (or is `selectable` or a link), and then `click` bubbles to the button once with
  `event.target` the label. `hit_testable=False` makes any node transparent to the pointer. A layer window's
  `input_region="auto"` still leaves a plain label out (a click-through layer stays click-through);
  `hit_testable=True` puts one in. Rust: `Node.hit_testable` and `Tree::set_hit_testable` take `Option<bool>`.

- **Other changed defaults, each with an opt-out (read with the list above).** A single-line `text_input` now centres its
  line vertically (`vertical_align="start"` restores the old top alignment). Selected text and a text input's selection
  highlight are the scheme's `primary` at 30%, and a scroll bar's thumb, a text input's placeholder and caret and a
  terminal's colours follow the scheme (set the colour to keep the old one). A plain window whose root paints nothing is
  cleared to the scheme's `surface` colour instead of transparent (`set(background=(0, 0, 0, 255))` restores the old look).
  The `alert` role is an assertive live region and `status` a polite one unless `live` is set. Ctrl+Shift+Escape no longer
  reaches a shell in a `terminal` (`escape_chord=None` restores it). The `emphasized` easing preset is `(0.2, 0, 0, 1)`, the
  Tesserae Motion note's value (it was `(0.3, 0, 0, 1)` in a pre-release build). Each is described in its own bullet below.

### New properties and calls

- **`virtual_list` keys, `scroll_to_index` and `scroll_into_view` (new; from the UX spec notes "List" and "Scrollbar and
  Scroll Container").** A focused virtual list, or a node inside one, now scrolls on Up/Down (40 px), Page Up/Page Down
  (the viewport less 10 percent) and Home/End, with the scroll-view rules (a `key_down` listener or a text input keeps
  its keys, Ctrl/Alt/Meta never scroll, a list that cannot move bubbles the key to the scroller around it). Focus
  moving to a built row, and `scroll_into_view()`, scroll the list by the least distance; the row holding focus stays
  built when it scrolls out of the window. `node.scroll_to_index(index, align="nearest")` reaches rows that are not
  built. A virtual list now fires `scroll` (it did not for the wheel either). Nothing to migrate unless an app handled
  these keys itself on a list or focused a row it expected to stay put: such an app now sees the list scroll as well.
  See [Keyboard scrolling](guide/nodes-and-layout.md#keyboard-scrolling).

- **`Window.set_defaults` and `App.set_defaults` (new; P12 of the visual-hooks audit).** A toolkit declares the baseline look
  of each node kind once (`window.set_defaults("text", font_size=14)`, `"*"` for every kind, or a dict of kind to
  properties for a whole theme) instead of passing it to every `create`. A default is merged into the props of `create`
  and of each `build` node before they are parsed and read at creation only: nodes that exist are not changed, and what
  `create` gets, then the window's defaults, then the app's, win in that order. `get_defaults`, `clear_defaults`. Nothing
  to migrate: with no defaults set, `create` is as before. See
  [Defaults per kind](reference/properties.md#defaults-per-kind-0611).

- **`outline_*` and `focus_outline_*` draw a ring outside the box (UXREQ-008, new, opt-in).** Every node takes
  `outline_width` (default 0, none), `outline_offset` (0; negative draws inside), `outline_color` (`None`: the window
  scheme's role `secondary`), `outline_gap_color` (`None`: a transparent gap) and `outline_radius` (`None`: the
  corner radius plus the offset, concentric), and the same five with the prefix `focus_outline_`, which replace them
  while the node has keyboard focus that is visible (the `focus_visible` a `focus` event reports; a `text_input` shows
  it for pointer focus too). The spec's focus ring is `focus_outline_width=3, focus_outline_offset=2`. The ring is
  above the node's own paint and its children, not clipped by the node's own `clip_children` or `mask`, and clipped by
  an ancestor's clip (keep padding of `offset + width`, or use a negative offset). Nothing to migrate: a node that sets
  none paints, hits and repaints exactly as before. See
  [Outline and focus ring](reference/properties.md#outline-and-focus-ring-0611).


One behaviour changes for every scroll view and virtual list: the wheel notch. The rest is new and opt-in.

- **`orientation="both"` (UXREQ-001, new, opt-in).** A `"scroll_view"` scrolls on both axes, with `scroll_offset` the
  vertical offset and the new `scroll_offset_x` the horizontal one (settable, readable, animatable). `"vertical"` and
  `"horizontal"` views are unchanged: the same offsets, thumb, hit area, keys (a Page Up is still a whole viewport)
  and `scroll` event, apart from the notch above. A `scroll` event now carries `x` and `y`, the offsets after the
  change, on every scroll view; `old_value` and `new_value` stay the main axis' offset. See
  [Two axes](guide/nodes-and-layout.md#two-axes-0611).

- **Scrollbar properties (UXREQ-001, new).** `scrollbar_hit_width`, `scrollbar_margin`, `scrollbar_min_length`,
  `scrollbar_gutter`, `scrollbar_corner`, `scrollbar_opacity`, `scrollbar_opacity_hover` and `scrollbar_opacity_drag`,
  next to `scrollbar_fill` and `scrollbar_width`. A `"both"` view defaults them to the spec (8 px thumb, 12 px hit strip,
  12 px gutter and corner, opacity 0.70 / 0.80 / 1.0); a one-axis view keeps its old look. `scrollbar_width` and the
  new ones take `None` for the default. See [Scrollbar](reference/properties.md#scrollbar-and-scroll-feel-0611).

- **`virtual_list` takes every scrollbar property, and shares the scroll view's scrollbar (P1, new, opt-in).** A
  virtual list's thumb used to be fixed (4 px, `#CAC4D0` at 55%, no emphasis) and every `scrollbar_*` property raised
  `ValueError` on it. All of them now work on a `virtual_list` (and on a `scroll_view`), with one implementation of the
  geometry, hit-testing, hover, drag and fade. A list that sets nothing looks and behaves as before; the error for a
  `scrollbar_*` property on another node now reads "applies only to a scroll_view or virtual_list node". See
  [Scrollbar and scroll feel](reference/properties.md#scrollbar-and-scroll-feel-0611).

- **`scrollbar_radius` (P1, new).** The thumb's corner radius, in pixels: default a pill (half the thickness) on a
  `"both"` view and 2 on a one-axis view and a list. `scrollbar_radius=0` is square.

- **`scrollbar_track_fill` and `scrollbar_track_opacity` (P1, new).** A track behind the thumb, as long as the lane and
  as thick as the thumb. Default none (the spec is track-less); opacity 1.0.

- **`scrollbar_fill_hover` and `scrollbar_fill_drag` (P1, new).** The thumb's colour with the pointer over it and while
  dragged, faded in and out like the opacity; unset, they are `scrollbar_fill`.

- **`scrollbar_side_v` and `scrollbar_side_h` (P1, new).** The edge of each bar: `"end"` (default: right, bottom),
  `"start"` (left, top), or physical (`"left"`, `"right"` for the vertical bar, `"top"`, `"bottom"` for the horizontal).
  `"start"` and `"end"` follow the layout direction; there is no right-to-left layout yet, so they are left-to-right,
  resolved in one place. The gutter and the corner follow the bar to its edge (a left bar makes the content start 12 px
  in).

- **`scrollbar_show`, `scrollbar_hide_delay_ms` (P1, new).** `"overflow"` (default, today's behaviour), `"always"` (a
  full-length thumb even without overflow, no gutter), `"auto_hide"` (fades out after the delay, default 1000 ms, and
  back in on scroll, hover and drag) or `"never"` (no bar and no gutter; the view still scrolls).

- **`scrollbar_fade_in_ms`, `scrollbar_fade_out_ms`, `scrollbar_hover_delay_ms`, `scrollbar_fade_easing` (P1, new).**
  The durations of the thumb's fades (default 100 ms each, replacing the fixed 100 ms), how long the pointer rests on
  the bar before the hover emphasis starts (default 0), and the curve: a preset name (`"linear"`, `"standard"` default,
  `"emphasized"`, `"ease_out"` and others) or a cubic bezier `(x1, y1, x2, y2)`.

- **`scrollbar_gutter_reserve` (P1, new).** `True` (default) reserves the gutter; `False` lets the thumb overlay the
  content.

- **`scrollbar_arrows`, `scrollbar_arrow_step` (P1, new).** `"both"` adds an arrow button at each end of the lane (a
  minimal triangle in the thumb's colour); a press scrolls the step (default 40 px) and repeats while held. Default
  `"none"`.

- **`scrollbar_lane_click` (P1, new).** `"page"` (default for a `"both"` view; the existing lane paging), `"jump"` (the
  thumb's middle goes to the press and the press carries on as a drag) or `"none"` (the default for a one-axis view and a
  list, as before: a press beside the thumb goes to the content).

- **`wheel_notch`, `key_line`, `page_fraction` (P1, new).** The wheel notch, the arrow-key step and the page, per scroll
  view and virtual list: defaults 40, 40 and 0.9 (a one-axis scroll view keeps a whole-viewport page, 1.0). A list's page
  was a constant 0.9 of the viewport and is now this property.

- **`scroll_smooth_ms`, `scroll_easing` (P1, new).** With `scroll_smooth_ms` above 0 (default 0: keys and wheel jump,
  as the spec says) the wheel and keys ease over that many milliseconds with the easing (default `"standard"`); notches
  add up. Drags, lane and arrow presses and `scroll_offset=` stay direct.

- **Named easings everywhere an easing is taken (P7, new).** `animate`, `animate_param`, `Shader.animate`,
  `LayerWindow.animate_margin` (and so a path's `trim_*`/`data` and the layout properties) now take a preset name too,
  next to `"linear"`, a bezier tuple and the springs: `"linear"`, `"standard"`, `"standard_decelerate"`,
  `"standard_accelerate"`, `"emphasized"`, `"emphasized_decelerate"`, `"emphasized_accelerate"`, `"ease"`, `"ease_in"`,
  `"ease_out"`, `"ease_in_out"` (curves in [the animation guide](guide/animation.md#easing)); a name outside the table
  raises `ValueError` listing them. The first seven are the table of the Tesserae UX note "Motion". **Changed
  (unreleased P1 preset):** `"emphasized"` was `(0.3, 0, 0, 1)`, a value in no MD3 table; it is now the note's
  `(0.2, 0, 0, 1)`, the same curve as `"standard"`.

- **`scroll_snap_settle_ms`, `scroll_snap_duration_ms`, `scroll_snap_easing` (P7, new).** The wait before a snapping
  `scroll_view` settles (default 120 ms), how long the settle takes (200 ms; 0 under reduced motion) and its curve
  (`"standard"`), until now constants of the engine.

- **`fling_tau_ms`, `fling_stop_speed`, `fling_min_speed` (P7, new).** A touch flick's decay time constant (default
  350 ms), the speed at which the coast ends (30 px/s) and the speed below which a flick does not coast (150 px/s), until
  now constants of the engine. A fling is not changed by reduced motion (it never was).

- **`page_repeat_delay_ms`, `page_repeat_ms` (P1, new).** The held lane or arrow press repeats after 400 ms, then every
  100 ms (were constants).

- **`Window.set(reduced_motion=...)` (P1, new).** `True`/`False` overrides, `None` (default) follows the OS value
  `window.get("reduced_motion")` already reported; `get` returns the window's own say, else the OS's. While it is true
  the scrollbar's fades, the smooth scroll and the scroll snap settle take no time (a snap settle took 200 ms before
  whatever the OS said): this closes the "reduced motion is not wired" limit of the two-axis scroll view.

- **`vertical_align` on `text` and `text_input` (new, P3, UXREQ-005); a single-line `text_input` centres its line
  (changed).** `vertical_align` is `"start"`, `"center"` or `"end"`: where the lines sit in the content box when it is
  taller than they are. A `text` defaults to `"start"` (unchanged). A `text_input` defaults to `None`, which is
  `"center"` for a single-line input and `"start"` for a multiline one: text in a 32 px field with padding 4 no longer
  sits high. An app that sized an input to its text sees no change; one that relied on the top alignment in a taller box
  sets `vertical_align="start"`. The caret, selection rectangles, `caret_rect()`, `text_rects()`, the IME's candidate
  area and a click's caret position all follow the offset. See [Text](reference/properties.md#text).

- **`letter_spacing` and `line_height` on `text_input` (new, P3).** As on `text`: pixels after each character
  (default `0`) and a multiple of the font size (default `None`, the font's own), shaped with the same layout the
  caret, selection and clicks use.

- **Accessibility: sets, mixed, `a11y_orientation`, `modal` and 18 more roles (UXREQ-003, UXREQ-004, new, opt-in).**
  `set_size` and `pos_in_set` (positive `int` or `None`, default `None`) say a node's place in a set; AccessKit keeps the
  size on the container, so set `set_size` on the list and `pos_in_set` on each item. A `virtual_list` needs neither: it
  says its `item_count` and each built row its index plus one (a value you set wins; `get` returns only what you set).
  `checked` now also takes `"mixed"` (it was `bool` or `None`; `pressed` still wins where both are set).
  `a11y_orientation` (`"horizontal"`, `"vertical"` or `None`) is the axis of a slider, scrollbar, separator or toolbar; it is
  not called `orientation` because a `scroll_view` already has an `orientation` for its scroll axes. `modal` (`bool`,
  default `False`) marks a dialog. `role` accepts `status`, `spinbutton`, `radiogroup`, `option`, `listbox`, `toolbar`,
  `navigation`, `complementary`, `region`, `grid`, `row`, `gridcell`, `columnheader`, `rowheader`, `searchbox`, `combobox`,
  `tooltip` and `scrollbar`. One behaviour change: a node with `role="alert"` and no `live` is now an assertive live
  region (AccessKit gave it no politeness; a reader may now interrupt for it), and `role="status"` is polite; set `live`
  to override. Nothing else to migrate. See [Accessibility](guide/accessibility.md#roles).

- **`escape_chord`, `escape_chord_reverse` on `terminal`, and the `terminal_escape` event (new; UXREQ-006, WCAG 2.1.2).**
  A focused terminal sent every key to the shell, so a keyboard user could not leave it. `escape_chord` (default
  `"ctrl+shift+escape"`; `None` disables) is checked before the key reaches the shell: the key is not sent, focus moves to
  the next focusable node, `terminal_escape` fires (`key` the chord, `related_target` the new focus) and `focus_visible`
  is on. `escape_chord_reverse` (default `None`) moves backward. Chords are `ctrl`/`shift`/`alt`/`meta` plus one key, matched
  exactly; a bad one raises `ValueError`. Behaviour change: Ctrl+Shift+Escape no longer reaches a shell (set
  `escape_chord=None` to keep it). See [Text](guide/text.md#terminal).

- **`hover_layer`, `pressed_layer`, `focus_layer`, `focus_layer_any_modality`, `state_layer_ms`, `state_layer_easing`
  (new, P13, UX ADR-0009; opt-in).** The engine draws and fades state layers: a colour whose alpha is the opacity
  (`hover_layer=(29, 27, 32, 20)` is 8 %), over the node's fill and border, under its children, clipped to its shape,
  for the hovered node and its ancestors (hover), a held primary press (pressed) and keyboard focus (focus; pointer
  focus too with `focus_layer_any_modality=True`). One layer at a time: pressed, then focus, then hover. The fade is
  `state_layer_ms` (default 100) on `state_layer_easing` (default `"standard"`), instant under reduced motion, and a
  disabled node draws none. All are `None` or off by default, so nothing changes for an app that sets none: no
  migration. An app that drew these states itself from `pointer_enter`/`pointer_leave`/`focus` can drop that code.
  See [State layers](reference/properties.md#state-layers-0611).

- **`Window(first_present=...)` and `window.ready()` (P5, new, opt-in).** `first_present` is `"immediate"` (the default,
  as before), `"on_ready"` (the first frame is presented only after the app calls `window.ready()`) or a number of
  milliseconds (`"on_ready"` with a deadline, counted from when the OS window was made). While it is held the engine
  lays out and runs timers but draws and commits nothing, so the user never sees an unfinished first frame, and on
  macOS and Windows the OS's default background cannot flash the wrong light or dark (UX spec AC-TH-7, AC-TH-8). It is
  given to the constructor only; `set(first_present=...)` raises `ValueError`. `ready()` is a no-op on a window that
  presents immediately and after the first call. `LayerWindow` and `PopupWindow` took `first_present` in 0.6.11 but only
  `"immediate"` (anything else raised `ValueError`); 0.6.12 lifts that (see `## 0.6.12`). Verified on Wayland (sway) only;

- **`stroke_cap`, `stroke_join`, `stroke_miter_limit`, `stroke_dash`, `stroke_dash_offset` (new, opt-in; `path` nodes, P9).**
  A path's stroke was always round-capped and round-joined and could not be dashed. Now `stroke_cap` (`"round"` default,
  `"butt"`, `"square"`), `stroke_join` (`"round"` default, `"miter"`, `"bevel"`), `stroke_miter_limit` (at least 1, default
  `4`), `stroke_dash` (a list of on/off lengths in pixels; `None` or `[]`, the default, is solid) and `stroke_dash_offset`
  (default `0`) are properties of a `path` node, taken by `create`, `set` and `set_defaults("path", ...)`; `None` restores the
  default and a bad value raises `ValueError`. **No default changed.** A `hit_shape="painted"` path is hit by the styled stroke
  and a path's shadow follows it. Setting them on another kind raises `ValueError`.

- **`image_sampling` (new, opt-in; `image` nodes, P9).** `"linear"` (the default, bilinear, as every image was drawn),
  `"nearest"` or `"cubic"`; `None` is `"linear"`. **No default changed.** An svg's embedded images and shader textures stay
  linear.

- **`shadows` on every kind (verified, P9).** Headless pixel tests (`engine-render` `paint_hooks.rs`, `tests/test_paint_hooks.py`)
  show `shadows` already works on a box (so a circle or ring, which are boxes with a corner radius, casts a round shadow),
  `image`, `svg`, `canvas`, `terminal`, `scroll_view` and `path` (since #339); nothing needed a fix and nothing changed.
  the X11, macOS and Windows paths (window created hidden, shown after the first present) are written but unverified. See
  [First present and background](reference/window.md#first-present-and-background-0611).

- **`Window(background=...)` / `set(background=...)` (P5, new; changes what unpainted pixels look like).** The colour every
  frame is cleared to before the scene paints, an `(r, g, b, a)` tuple, or `None`, the default. **`None` is now the window
  scheme's `surface` role** (`#FEF7FF` light, `#141218` dark) for an ordinary window: before, pixels no node painted
  were cleared to transparent, which an opaque surface shows as black, so an app that never painted a root background had a
  black window and now has the scheme's surface. A see-through window (`transparent=True`), a `LayerWindow` and a
  `PopupWindow` still clear to nothing unless `background` is given. An app that paints its own root background sees no
  change; to keep the old black, `window.set(background=(0, 0, 0, 255))`.

### Internal (Rust)

- **Internal (Rust):** `ScrollViewState::scrollbar_width` is a `Prop::Width` setting of `bar` (`ScrollbarState`, shared with `VirtualListState`; the per-kind `thumb_drag_anchor`, `level`, `scrollbar_fill` and `scrollbar` fields are gone); `ScrollbarOverrides` and `SCROLLBAR_PROPS` are the `ScrollProp` table; `grabs_virtual_list_thumb` and `grabs_scroll_view_thumb` are `Tree::grabs_thumb`;

- **Internal (Rust):** `ScrollViewState::scrollbar_width` is `scrollbar.thickness: Option<f64>`;
  `Tree::take_scroll_changes` returns `ScrollChange` values, not tuples; `grabs_scroll_view_thumb` returns the
  `ScrollAxis` it grabbed. The Python API has no such change.

## 0.6.12
- **`selection_mode`, `selection`, `selection_anchor`, `select_all_key`, `select_on_focus`, `unselectable_keys` and the
  `selection_change` event on a `virtual_list` (new, opt-in; no default changes; UX note "Selection", AC-SLT-1 to 6 and 13).**
  The engine had no selection for lists: `selected` was an accessibility flag the toolkit set by hand and every framework
  wrote its own Shift and Ctrl rules. A `virtual_list` can now choose its rows (`selection_mode` `"single"` or
  `"multiple"`; the default `"none"` leaves every click and key exactly as it was, Space still a `click`, the arrows
  still scrolling by 40). A row's key is its index; the set is stored as ranges, so Shift+End or Ctrl+A over 100 000 rows
  builds no row. Clicks (Ctrl toggles, Shift ranges, Ctrl+Shift adds, empty space clears, a press on a chosen row of
  several chooses it alone on release unless the pointer went `drag_start_px`) and keys (Up, Down, Home, End, Page keys
  move focus to a selectable row, Shift extends from the anchor, Ctrl moves focus alone, Space and Ctrl+Space toggle,
  Ctrl+A with `select_all_key`, Escape never clears) are the engine's; `selection_change` carries `selection`, `ranges`,
  `count`, `anchor` and `source`. **Behaviour to know:** with a `selection_mode` a list takes the arrow, Home, End and Page
  keys itself (focus moves to the next row and the list scrolls by the least distance) instead of scrolling by 40 and the
  page; with the default none of that happens. New `Event` fields `selection`, `ranges`, `anchor`, `source`. Not built:
  container children as items, the `selected` and `multiselectable` accessibility flags set from the set, and the
  announcement of the count (the next change). See [Selecting rows of a virtual list](guide/nodes-and-layout.md#selecting-rows-of-a-virtual-list).
- **`titlebar_double_click`, `resize_cursors`, `undecorated_shadow` (new `Window` properties, P11; no default changes).**
  `titlebar_double_click` is what a double-click on a `window_region="drag"` node does: `None` (default, as before: the
  user's setting on macOS, maximize elsewhere), `"maximize"`, `"minimize"`, `"none"`, or `"event"` (nothing happens;
  the window fires a new `titlebar_double_click` event with `x`, `y`). `resize_cursors` maps the eight `resize_border`
  zones (`north`, `south`, `east`, `west`, `north_west`, `north_east`, `south_west`, `south_east`) to cursor names;
  `None`, the default, is the stock `ns_resize`/`ew_resize`/`nwse_resize`/`nesw_resize` map. `undecorated_shadow`
  (`bool`, default `True`) keeps or drops the OS's shadow around an undecorated window; Windows only, stored and
  ignored on macOS and Linux (tess_re draws no shadow of its own). Bad values raise `ValueError`; `LayerWindow` and
  `PopupWindow` refuse all three like `system_menu`. Skipped: separate corner size for the resize border (one
  `resize_border` serves edges and corners), double-click time/slop (the OS value, and P6's multi-click settings).
  See [Custom title bars](reference/window.md#window-controls-and-state).
- **`gesture_long_press_ms`, `gesture_tap_ms`, `gesture_double_tap_ms`, `gesture_double_tap_px`, `gesture_swipe_speed`,
  `gesture_swipe_px`, `gesture_rotate_slop`, `multi_click_ms`, `multi_click_px`, `drag_start_px` (new, opt-in; window
  properties, P6).** The touch recogniser's long press (500 ms), tap (300 ms), double tap (300 ms and 30 px), swipe
  (600 px/s and 40 px) and rotate (0.12 rad) thresholds, the text multi-click's 500 ms and 4 px, and a drag start
  distance were fixed numbers beside the existing `touch_slop`. They are now `Window.set` / `get` properties (also on
  `LayerWindow` and `PopupWindow`; milliseconds, pixels, px/s, radians), each default the old value, so no default
  changes. `drag_start_px` is new behaviour with an off default (0): while the primary button is held, `pointer_move`
  listeners hear nothing until the pointer is that far from the press (the engine's own hover, thumb and selection
  drags are not held back), because tre had no drag threshold at all and the Tesserae spec wants 4 dp.
  `multi_click_px` is also the title-bar double click's press distance (it was a separate fixed 4). Bad values raise
  `ValueError` naming the property. See [Gesture, click and drag thresholds](reference/window.md#gesture-click-and-drag-thresholds-0612).
- **Terminal appearance: `font_family`, `line_height`, `caret_shape`, `caret_width`, `caret_blink`, `caret_unfocused`,
  `bold_weight`, `dim_opacity`, `underline_offset`, `underline_thickness`, `selection_foreground`, `selection_radius` on
  a `terminal` (new, opt-in; no default changes).** The audit and the UX note "Terminal" listed the font family, line
  height, cursor shape and blink, bold weight, dim opacity and underline geometry as fixed in the engine. Each is now a
  property whose default is what the terminal drew: the bundled Hack Nerd Font Mono, the font's own line height, a
  `"block"` cursor that does not blink and is not drawn without focus, bold 700, dim 0.6, an underline at 0.85 of the cell
  and 0.06 of it thick, each cell's own colour in a selection and square corners. `caret_shape`, `caret_width` and
  `caret_blink` are the text input's, so a terminal's `caret_shape` reads `"block"` where a text input's reads `"bar"`;
  `caret_blink` is steady under reduced motion. A terminal's `font_family` and `line_height` resize its box as `font_size`
  does. Bad values raise `ValueError`; `bold_weight`, `dim_opacity`, `underline_*` and `caret_unfocused` raise on any
  other kind. Not done: the 256-colour cube and grey ramp (the xterm values, not a design choice; the 16 ANSI colours are
  `palette`). `TerminalState` (Rust) gained the matching fields and a `cell_size` helper; it no longer derives `Debug` and
  `PartialEq` and its `Clone` is by hand (its history bar has animated state).
- **The terminal history scrollbar (UXREQ-006 second half; new, opt-in).** A `terminal` takes the `scrollbar_*`
  properties of a scroll view and `page_fraction`, `page_repeat_delay_ms`, `page_repeat_ms`, with the spec's measures,
  but **`scrollbar_show` defaults to `"never"`**: no existing terminal draws a bar until it sets `"overflow"`, `"always"`
  or `"auto_hide"`. The bar's content is the history and the screen; a thumb drag, a lane click or an arrow moves the
  view through the history in lines, and the wheel still does. `scrollbar_gutter`, `scrollbar_gutter_reserve`,
  `scrollbar_corner`, `scrollbar_side_h` and the wheel, key, smooth-scroll, snap and fling properties do not apply and
  raise `ValueError` on a terminal. The bar draws over the grid's right edge and reserves no space. The error for the
  scrollbar properties on any other kind still reads "applies only to a scroll_view or virtual_list node". Rust:
  `NodeKind::Terminal` is a `Scroller` (`ScrollDefaults::Terminal`; `TerminalState::{bar, history_len, history_back,
  line_px, scroll}`, `set_history`, `history_back_wanted`), `Tree::scrollbar_pressed`. See
  [Terminal](guide/text.md#terminal).
- **`selection_fill` on a static `text`; `selection_foreground` and `selection_radius` on `text` and `text_input` (new,
  opt-in; no default changes).** The UX note "Text Selection" has the highlight as `primary` at 30 % with the selected text
  keeping its own colour and a square corner. A `text_input` took `selection_fill`; a `text` had no way to set its
  highlight, its text colour, or its corner. `selection_fill` is now settable on a `text` (`None`, the default, is the
  scheme's `primary` at 30 %, or the text colour at 30 % with a `fill` of its own: what was painted), `selection_foreground`
  is the colour of the glyphs the selection covers (`None`: each glyph keeps its colour, as before) and `selection_radius`
  the highlight's corner radius in pixels (a number from `0`, the default: square, as before). Bad values raise `ValueError`.
  `caret_width` (a `text_input`'s, 1.5 px by default) is unchanged. `TextOptions` gained `selection_fill`,
  `selection_foreground` and `selection_radius` and `TextFieldState` gained `selection_foreground` and `selection_radius`
  (Rust). See [Selection appearance](guide/text.md#selection-appearance-0612).
- **Escape ends a drag (UXREQ-007; behaviour change).** While a gesture is held — a node holds the pointer capture, a
  scrollbar thumb is dragged, or a dock drag runs — Escape is **consumed**: the capture holder (else the pressed node)
  gets one `pointer_cancel`, no `pointer_up` and no `click` follow, and the key goes no further (no `key_down`, no byte to
  a focused terminal, no text edit, no `dismiss_on_escape` overlay closing, no keyboard scroll). A thumb drag restores its
  start offset. Without a gesture Escape is unchanged. An app that closed a popup with Escape mid-drag now sees the drag
  end first and needs a second Escape. A node that holds the capture still gets no `key_down`.
- **`dock_cancel` (new window event).** Escape during a dock drag leaves the panel in its zone, fires `dock_target` with
  `side=None` if a zone was highlighted (so the highlight goes within the frame), then `dock_cancel` (`event.panel`);
  `dock_drop` never fires. Add `window.on("dock_cancel", ...)` next to your `dock_drop` handler to clear drag state.
- **`edge_scroll`, `edge_scroll_band`, `edge_scroll_speed` (new, opt-in; `scroll_view` and `virtual_list`).** While a
  gesture is held, a pointer within `edge_scroll_band` dp (24) of the view's edge scrolls it, from `edge_scroll_speed[0]`
  (8) dp per frame at 60 Hz at the inner edge of the band to `[1]` (32) at the edge. Default `edge_scroll=False`, so
  nothing changes until set.

- **A path node's `shadows` follow the path, holes included (a fix).** Every node's shadow was its box, blurred, so a
  `path` ring's hole filled with solid shadow. A path now casts the silhouette of what it paints (the fill under the
  non-zero rule, plus the trimmed stroke), offset, grown by `spread` and blurred by the same `blur / 2` as a box.
  No new property; a negative `spread` shrinks the silhouette. Other kinds are unchanged. An app that leaned on the
  old box shadow under a path (a drop shadow of an icon's bounding box) now sees the icon's shape instead: put the
  shadow on a `box` behind it for the old look.

- **`hover_layer_ms`, `pressed_layer_ms`, `focus_layer_ms`, `hover_layer_easing`, `pressed_layer_easing`,
  `focus_layer_easing` (new, opt-in; per-state state-layer timings).** The UX spec's state fades differ by state
  (State Layers: hover 15 ms linear, press fade-in 105 ms; Motion `short3` 150 ms for a control state colour), but
  `state_layer_ms` (100) and `state_layer_easing` were shared. Each per-state property is a number of milliseconds
  (0 to an hour; `0` is at once) or an easing (preset name or bezier), default `None` = the shared value, so nothing
  changes unless you set one. The state being entered decides a fade in (and a change from one layer to another:
  hover to pressed uses `pressed_layer_ms`); the state being left decides a fade out to nothing (leaving hover uses
  `hover_layer_ms`). Reduced motion stays instant. Bad values raise `ValueError` as for `state_layer_ms`.
  `StateLayer` (Rust) gained the matching `*_ms`/`*_easing` fields, `active` and `timing(LayerState)`;
  `StateLayer::target` returns `Option<(LayerState, Color)>` and `sync` takes it. See
  [State layers](reference/properties.md#state-layers-0611).
- **`first_present` and `ready()` on `LayerWindow` and `PopupWindow`, and a `background=` constructor argument on both
  (new, opt-in; a popup of a closing parent is now told).** In 0.6.11 only a toplevel `Window` could hold its first present;
  a layer window or popup raised `ValueError` ("not supported on LayerWindow yet") for anything but `"immediate"`. They now
  take `"on_ready"` or a number of milliseconds (1 to 600000): the surface has its role, size and acknowledged configure but
  commits no buffer, so the compositor maps it (a bar, a panel, a menu) only at `window.ready()` or the deadline (counted
  from the first configure), and the user never sees an unfinished first frame. The default is `"immediate"`, unchanged.
  Settings made before then (`keyboard`, `input_region`, `set(...)`) still reach the compositor and apply when it maps; a held
  surface reserves no `exclusive_zone` and takes no input until it is mapped. `background=` (`(r, g, b, a)`, default `None`:
  clear to nothing) is accepted by the constructors; `set(background=...)` already worked. **Behaviour change:** when a layer
  surface or popup closes (`close()`, the compositor, an output going away), the popups hanging off it now get `closed` with
  reason `"dismissed"` and are destroyed before it; before, they were left without a notice on a destroyed surface. See
  [First present and ready](reference/layer-window.md#first-present-and-ready).

- **`layer_viewport_margin`, `layer_flip_threshold` (`Window.set`/`get`) and `viewport_margin`, `flip_threshold`, `align`,
  `shift` on `show_layer`, with `layer_*` node properties (new, opt-in; P10 placement policy).** The UX "Anchored
  Placement" note has a layer keep a viewport margin from the window edge (design choice 8 dp) and a flip threshold;
  the engine had margin 0 and slack 0 fixed. A margin is a number or `(top, right, bottom, left)`, the threshold a
  number of pixels, both default `0` (what layers always did): `window.set(layer_viewport_margin=8)` once, or per layer
  `show_layer(menu, anchor=b, viewport_margin=4, flip_threshold=12, align="end")`, later `menu.set(layer_viewport_margin=...)`
  (`None` follows the window). The margin also counts in the room that decides a flip. `align` (`"start"` default,
  `"center"`, `"end"`) lines the layer up with the anchor's edge; `shift=False` stops the shift (`shift=None` follows
  `flip`, so `flip=False` still pins both as before). Nothing changes unless you set one; invalid values raise
  `ValueError`. Rust: `OverlayMeta.pinned` is replaced by `no_flip` and `no_shift` (and gained `align`,
  `viewport_margin`, `flip_threshold`), `Tree::layer_defaults`/`set_layer_defaults`/`update_layer` are new, and
  `engine_core::overlay::place` is the pure placement function. See
  [Layer placement policy](reference/properties.md#layer-placement-policy-0612).
- **`PopupWindow(edge_margin=...)` (new, opt-in, constructor only).** The UX "Popup Window" note keeps the body 8 dp from
  the output edge; the compositor flipped and slid it to the edge. `edge_margin` (a whole number of pixels or `(top, right,
  bottom, left)`, default `None` = 0) grows the window geometry the compositor places by that much on each side, as far as
  the transparent room around `geometry` allows (`get("edge_margin_applied")`; a popup without `geometry` has none), and
  moves the anchor rectangle so the body attaches where it did. `position` and `configure`'s `x`, `y` stay the body's.
  `PopupConfig` (Rust) gained `edge_margin` and `edge_fit()`. See [Edge margin](reference/layer-window.md#edge-margin).
- **`ime_offset` (`Window.set`/`get`; new, opt-in).** `(dx, dy)` layout pixels added to the caret box the input method puts its
  candidate window at; default `(0, 0)`, the caret box itself as before.
