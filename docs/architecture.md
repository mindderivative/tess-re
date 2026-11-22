# Architecture

A short orientation. The full design reference is
[`ARCHITECTURE.md`](https://github.com/mindderivative/tess-re/blob/main/ARCHITECTURE.md)
in the repository root.

## Four crates

```
engine-py        PyO3 classes, the per-frame loop, listeners, callbacks
   │
   ├── engine-platform   winit event loop, input translation, AccessKit adapter, layer surfaces
   └── engine-render     scene building, damage tracking, persistent target, text
         │
         └── engine-core  node tree, Animated<T>, layout, dispatch, focus, layers
```

`engine-core` depends on none of the others and has no Python, windowing,
or GPU code, so it's tested on its own. Only `engine-py` imports `pyo3`, and
its Python surface is the one stability contract. `engine-platform` and
`engine-render` both build on `engine-core`. See
[Rust API](api/rust.md) for each crate's types.

## How a frame is made

1. **Input** from `winit` (or `window.simulate`) is hit-tested, moves focus,
   edits text, and reaches your listeners, bubbling from the target. A press
   on a drawn title bar or resize border then goes to the OS, to move or
   resize the window ([Custom Title Bars](guide/custom-title-bars.md)).
2. Callbacks queued with **`LoopHandle.call_soon`** run.
3. **One central tick** advances every running animation; completion
   callbacks run after it.
4. **Layout** runs through `taffy`'s cache — paint-only changes never
   dirty it — and virtual lists build their visible rows.
5. **Paint** walks the tree into a `vello_gpu` scene, culling what's off
   screen. A damage tracker finds what changed since the last frame by
   re-checking only the nodes written to, so its cost follows the changes, not
   the size of the tree, and only the changed rects (at most four, or the whole window) are rendered into
   the window's persistent target, which is then copied to the screen.
   `window.set(partial_redraw=False)` redraws in full and keeps no such target, and
   `show_damage=True` tints what was redrawn. The **AccessKit** tree is
   built from the same node tree.

When nothing changed and nothing animates, the loop sleeps until the next
input or `call_soon`.

**Pacing on Wayland (0.5.10).** A window is drawn only after the compositor's frame callback for its last frame
(`pre_present_notify`; a layer surface asks for its own `wl_surface.frame`), and the loop waits for that callback
instead of polling. A surface the compositor is not showing gets none, so it stops drawing and costs nothing: before this
its blocked vsync present held the GIL and the one loop thread and froze every other window. On X11 and macOS the OS
reports a fully hidden window (`occluded`) and `pause_when_occluded` stops its drawing. See
[Occlusion](design/occlusion.md) and the [threading guide](guide/threading.md#hidden-windows-wayland).

## Layer surfaces and popups

On Linux, a [`LayerWindow`](reference/layer-window.md) (a bar, panel or OSD) and a
[`PopupWindow`](reference/layer-window.md#popupwindow) (a menu off a bar) are windows
that `winit` cannot make, because their Wayland role is `zwlr_layer_shell_v1` or `xdg_popup`.
`engine-platform` runs them on a **second Wayland event queue of winit's own connection**,
dispatched on a thread of its own that hands events to the same loop through the event-loop
proxy, so a layer surface is drawn, laid out, animated and made accessible by the same code
as any window (`WindowRenderer` does not know which kind of window it draws into). The thread
owns what the protocol makes it own: the surface's configure and close, outputs and their
hot-plug, the keyboard (through `xkbcommon-dl`, loaded at run time), input methods
(`zwp_text_input_v3`) and a per-surface AccessKit adapter. tess_re binds the layer shell itself,
at the highest version the compositor offers. The design and what was measured are in
[the LayerWindow design note](design/layer-window.md) and the
[spike findings](design/layer-surface-spike.md).

Each turn the loop also polls the GPU device. A GPU error becomes a
`gpu_error` event instead of a panic, and a lost GPU fires `gpu_lost` and ends
the run; while GPU work is still running the loop wakes about every 100 ms to
poll, so a hang in the last frame is still seen
([GPU health](reference/window.md#gpu-health)).

A node may carry a WGSL [shader](guide/shaders.md). Before the scene renders,
the renderer runs one pass per drawn shader node into a texture the size of
its box, and the scene draws that texture in the node's place; an effect
shader first renders the node's own subtree offscreen to read it. These passes
run only when something changed, and a window drawing an `animated` shader keeps
running like any animation.

## Everything visual is adjustable from Python

Tesserae Engine is mechanism only: every visual element, default and behaviour is a property
(a number, an enum, a colour or a named easing) that Python sets on a node, never a callback run
per frame, and each default is the Tesserae UX spec's value. A project that sets nothing gets the
spec. Properties added under this rule so far (0.6.11):

| Property | Default | Where |
| --- | --- | --- |
| `selection_fill` (`text` from 0.6.12, `text_input`), `selection_foreground`, `selection_radius` (`text`, `text_input`; 0.6.12), `caret_width` (`text_input`) | `None`: the scheme's `primary` at 30% (the text colour at 30% with a `fill` of its own); `None`: each glyph keeps its colour; `0`: square; `1.5` px | [Selection appearance](guide/text.md#selection-appearance-0612) |
| `font_family`, `line_height` (`terminal`), `caret_shape`, `caret_width`, `caret_blink` (`terminal`), `caret_unfocused`, `bold_weight`, `dim_opacity`, `underline_offset`, `underline_thickness`, `selection_foreground`, `selection_radius` (0.6.12) | the bundled monospace face; the font's line height; `"block"`, `1.5`, steady; `"hidden"`; `700`; `0.6`; `0.85` of the cell and `0.06` of it; each cell's own colour; square | [Terminal](reference/properties.md#terminal) |
| `scrollbar_show` and the rest of the `scrollbar_*` properties (`terminal`; 0.6.12, UXREQ-006) | `"never"`: the history scrollbar is opt-in; when shown it is the scroll view's spec bar over the history | [Terminal](guide/text.md#terminal) |
| `hit_testable` (every node; `True`, `False` or `None`) | `None`: every kind takes hits, a plain `text` label included | [Node](reference/node.md), [Events](reference/events.md) |
| `vertical_align` (`text`, `text_input`; `"start"`, `"center"`, `"end"`) | `text`: `"start"`; `text_input`: `None`, which is `"center"` single-line and `"start"` multiline | [Text](reference/properties.md#text) |
| `scheme` (`Window`, `LayerWindow`, `PopupWindow`, `App`; `"light"`, `"dark"`, `"auto"`) | `"auto"`: the OS flag, light while unknown. Selects the baseline colour of every engine default (text, text input text/placeholder/caret/selection, scroll thumb, terminal foreground/cursor/selection) | [Colour scheme](guide/painting.md#colour-scheme-0611) |
| `outline_width`, `outline_offset`, `outline_color`, `outline_gap_color`, `outline_radius` and the `focus_outline_*` five (every node) | width 0 (none); offset 0; colour the scheme role `secondary`; gap transparent; radius the corner radius plus the offset | [Outline and focus ring](reference/properties.md#outline-and-focus-ring-0611) |
| `scheme_colors` (window; `{role: (r, g, b, a)}`), `tess_re.scheme_colors(scheme)` | the 49 Material 3 baseline roles of the UX spec "Colour Roles" | [Colour scheme](guide/painting.md#colour-scheme-0611) |
| `fill=None` (`text`, `text_input`), `palette` `foreground`/`cursor`/`selection` `None` | the scheme's colour | [Properties](reference/properties.md#text) |
| `letter_spacing`, `line_height` (now on `text_input` too) | `0`, `None` (the font's own) | [Text](reference/properties.md#text) |
| `Window.set_defaults` / `App.set_defaults` (`kind` or `"*"`, any `create` property) | none set: the engine's own defaults; read at node creation, never per frame | [Defaults per kind](reference/properties.md#defaults-per-kind-0611) |
| `escape_chord`, `escape_chord_reverse` (`terminal`; a chord string or `None`) | `"ctrl+shift+escape"`, `None`: leaves a focused terminal toward the next/previous node without sending the key to the shell (WCAG 2.1.2); fires `terminal_escape` | [Terminal](reference/properties.md#terminal) |
| `hover_layer`, `pressed_layer`, `focus_layer` (every node; colour, alpha is the opacity), `focus_layer_any_modality`, `state_layer_ms`, `state_layer_easing`, and per state (0.6.12) `hover_layer_ms`/`pressed_layer_ms`/`focus_layer_ms` and `*_layer_easing` (`None`: the shared value; the entered state times a fade in, the left state a fade out) | `None` (nothing drawn), `False`, `100`, `"standard"`: the engine tracks hover, press and focus, draws the layer over the fill and under the children and fades it (pressed, focus, hover; none when disabled; instant under reduced motion) | [State layers](reference/properties.md#state-layers-0611) |
| `first_present` (`Window`, and from 0.6.12 `LayerWindow` and `PopupWindow`; `"immediate"`, `"on_ready"` or milliseconds), `window.ready()` | `"immediate"`: the first frame is presented when drawn. Held, nothing is drawn or committed until the app is ready (or the deadline passes) | [First present and background](reference/window.md#first-present-and-background-0611) |
| `titlebar_double_click` (`Window`; `None`, `"maximize"`, `"minimize"`, `"none"`, `"event"`), `resize_cursors` (dict of eight directions to a cursor name), `undecorated_shadow` (bool; Windows) | `None`: OS setting on macOS, maximize elsewhere; the stock `ns`/`ew`/`nwse`/`nesw` resize cursors; `True` (0.6.12) | [Window](reference/window.md#window-controls-and-state) |
| `background` (`Window`, `LayerWindow`, `PopupWindow`, `set`; `(r, g, b, a)` or `None`) | `None`: the scheme's `surface` role; nothing for a see-through window, a `LayerWindow` and a `PopupWindow` | [First present and background](reference/window.md#first-present-and-background-0611) |
| `gesture_long_press_ms`, `gesture_tap_ms`, `gesture_double_tap_ms`, `gesture_double_tap_px`, `gesture_swipe_speed`, `gesture_swipe_px`, `gesture_rotate_slop`, `multi_click_ms`, `multi_click_px`, `drag_start_px` (`Window`, `LayerWindow`, `PopupWindow`; `set`/`get`; numbers) | 500 ms, 300 ms, 300 ms, 30 px, 600 px/s, 40 px, 0.12 rad, 500 ms, 4 px, 0 (off): the values they had, so nothing changes until one is set | [Window thresholds](reference/window.md#gesture-click-and-drag-thresholds-0612) |
| `stroke_cap`, `stroke_join`, `stroke_miter_limit`, `stroke_dash`, `stroke_dash_offset` (`path`; 0.6.12) | `"round"`, `"round"`, `4`, none (solid), `0`: the stroke as it always was | [Paths](reference/paint.md#stroke-style-of-a-path-0612) |
| `image_sampling` (`image`; `"linear"`, `"nearest"`, `"cubic"`; 0.6.12) | `"linear"` (bilinear, as every image was drawn) | [Image](reference/properties.md#image) |
| `shadows` on every kind (0.6.12 audit, no new property) | already worked on box, circle, ring, image, svg, canvas, terminal, text, scroll_view, virtual_list (a rounded box) and path (its silhouette) | [Shadows of every kind](reference/paint.md#shadows-of-every-kind-0612) |

**Scrollbar and scroll feel (0.6.11, P1).** A `scroll_view` and a `virtual_list` share one scrollbar: one table of
properties (`engine_core::ScrollProp`), one geometry (`ScrollbarMetrics::thumbs`), one hit test, hover, drag and fade
(`ScrollbarState`), one painter. The final properties, all on both kinds (reference:
[Scrollbar and scroll feel](reference/properties.md#scrollbar-and-scroll-feel-0611)):

| Group | Properties (default) |
| --- | --- |
| Measures | `scrollbar_width` (8; 4 one-axis/list), `scrollbar_radius` (pill; 2), `scrollbar_hit_width` (12; width + 12), `scrollbar_margin` (2), `scrollbar_min_length` (32), `scrollbar_gutter` (12; 0), `scrollbar_corner` (12; 0), `scrollbar_gutter_reserve` (`True`) |
| Colour | `scrollbar_fill` (`#CAC4D0`), `scrollbar_fill_hover`, `scrollbar_fill_drag` (= fill), `scrollbar_track_fill` (none), opacities `scrollbar_opacity` (0.70; 1.0), `_opacity_hover` (0.80), `_opacity_drag` (1.0), `scrollbar_track_opacity` (1.0) |
| Place and visibility | `scrollbar_side_v`, `scrollbar_side_h` (`"end"`), `scrollbar_show` (`"overflow"`), `scrollbar_hide_delay_ms` (1000) |
| Motion | `scrollbar_fade_in_ms`, `scrollbar_fade_out_ms` (100), `scrollbar_hover_delay_ms` (0), `scrollbar_fade_easing` (`"standard"`) |
| Buttons and lane | `scrollbar_arrows` (`"none"`), `scrollbar_arrow_step` (40), `scrollbar_lane_click` (`"page"`; `"none"` one-axis/list) |
| Feel | `wheel_notch` (40), `key_line` (40), `page_fraction` (0.9; 1.0 one-axis), `scroll_smooth_ms` (0), `scroll_easing` (`"standard"`), `page_repeat_delay_ms` (400), `page_repeat_ms` (100) |
| Snap and fling | `scroll_snap_settle_ms` (120), `scroll_snap_duration_ms` (200), `scroll_snap_easing` (`"standard"`), `fling_tau_ms` (350), `fling_stop_speed` (30 px/s), `fling_min_speed` (150 px/s) |
| Edge auto-scroll | `edge_scroll` (`False`), `edge_scroll_band` (24 dp), `edge_scroll_speed` (`(8, 32)` dp per frame at 60 Hz) |

The named easings live in one table (`engine_core::EASINGS`: `linear`, `standard`, `standard_decelerate`,
`standard_accelerate`, `emphasized`, `emphasized_decelerate`, `emphasized_accelerate` (the Tesserae Motion note) and the
CSS `ease`, `ease_in`, `ease_out`, `ease_in_out`), and one parser (`engine-py/src/easing.rs`) takes a name from it, a
bezier tuple or (for `animate`) a spring wherever an easing is taken from Python: `animate`, `animate_param`,
`Shader.animate`, `LayerWindow.animate_margin` and the easing properties of a scroller. There is no per-window motion
scale: durations start in many places (Python calls, the scrollbar fades, the smooth scroll, the snap settle), so a
scale would be one multiply per site rather than one in the animation core; `reduced_motion` is the one switch.
`Window.set(reduced_motion=...)` (default: follow the OS) collapses the fades, the smooth scroll and the snap settle to
0 ms.

## Design principles

- **Rust owns every frame; Python owns intent.** Python never runs during
  layout, paint, or interpolation — only inside callbacks.
- **One animation mechanism.** Every animatable value is the same
  `Animated<T>`, ticked by one pass.
- **Mechanism in the engine, meaning and look in the framework.** Tesserae Engine
  reports what it can know — pointer, focus, whether focus came from the
  keyboard — and draws only what it's told.
- **Data in, not files.** Pixels, fonts, and tree content arrive as data;
  the framework owns every format.

## History

Tesserae Engine is a from-scratch second iteration of an earlier Vulkan engine,
archived under
[`archive/`](https://github.com/mindderivative/tess-re/tree/main/archive) with
its lessons learned.
The GitHub project [Tesserae Rendering Engine](https://github.com/users/mindderivative/projects/3)
records the work from 0.5.0 on,
[`BUILD_TRACKER_ARCHIVE_0.4.md`](https://github.com/mindderivative/tess-re/blob/main/BUILD_TRACKER_ARCHIVE_0.4.md) the 0.4.x line, and
[`BUILD_TRACKER_ARCHIVE_0.3.md`](https://github.com/mindderivative/tess-re/blob/main/BUILD_TRACKER_ARCHIVE_0.3.md)
every earlier milestone, including the 0.3.5 program that moved Material
Design 3 and the declarative layer out to the framework.
