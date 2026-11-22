# `Window`

Owns one node tree and the OS window it's painted into. Add windows to an
[`App`](app.md), then call `App.run()`.

**`Window(width=480, height=200, title="tess_re v2", decorations=True, scheme=None, first_present=None, background=None)`** —
raises `ValueError` for a zero width or height. `decorations=False` (0.5.0)
opens the window without the OS's title bar and borders, for the framework
to draw its own; see [Window controls and state](#window-controls-and-state).
(0.6.11) `first_present` and `background` control what the user sees before and at the first frame; see
[First present and background](#first-present-and-background-0611).

```python
window = Window(width=640, height=400, title="Inbox")
```

A bar, panel or dock that the compositor places is a [`LayerWindow`](layer-window.md) (0.5.8), a
`Window` with a different Wayland role.

## Nodes

**`create(kind, **props) -> Node`** makes a detached node of `kind` —
`"box"`, `"text"`, `"text_input"`, `"image"`, `"path"`, `"svg"`,
`"canvas"`, `"scroll_view"`, `"virtual_list"`, or `"terminal"` — and sets `props`
atomically, as `Node.set` does: a bad property raises `ValueError` and
nothing is created. Every kind and property is on
[Nodes and Properties](properties.md).

**`build(spec) -> (root, nodes)`** (0.5.10) makes a whole detached subtree from one nested description and
**`apply(changes)`** changes properties on many nodes in one call. Both check everything first and change nothing
on a bad value. See [Building many nodes](properties.md#building-many-nodes-0510).

**`root`** is the tree's root node: a flex row with 16px padding and 16px
gaps, sized to the window. Attach nodes under it with `add_child`.

```python
card = window.create("box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=200, height=120,
                     corner_radius=12)
window.root.add_child(card)
```

## Layers

**`show_layer(node, anchor=None, placement="below", modal=False,
dismissible=True, offset=(0, 0), flip=True, viewport_margin=None, flip_threshold=None,
align="start", shift=None)`** shows `node` over the window's content, and
**`hide_layer(node)`** hides it — for menus, dialogs, tooltips, and sheets.
See [Layers](layers.md).

## Window controls and state

Since 0.5.0, a window can be run without the OS's title bar and borders
(`decorations=False`, or `set(decorations=...)` live), so the framework can
draw its own — and needs these to wire it up. See the
[custom-windowing design](../design/custom-windowing.md).

**`minimize()`**, **`maximize()`**, and **`restore()`** act on the open
window; before `App.run()` they set how it opens. **`close()`** closes the
window as if the user had: `close_requested` fires first and a listener can
cancel it. It happens on the loop's next turn, not during the call, and a
window that isn't open has nothing to close.

```python
minimize_button.on("click", lambda: window.minimize())
maximize_button.on("click", lambda: window.restore() if window.get("maximized") else window.maximize())
close_button.on("click", lambda: window.close())
window.on("maximized", lambda e: swap_icon(e.maximized))
window.on("active", lambda e: title_bar.set(opacity=1.0 if e.active else 0.6))
```

| Property | Set | Get |
| --- | --- | --- |
| `decorations` | Whether the OS draws the title bar and borders | The open window's answer |
| `fullscreen` | Borderless fullscreen on the window's monitor | Whether it is |
| `min_width`, `min_height` | The smallest size the user can resize to; 0 for none | The setting |
| `icon` | `(rgba, width, height)` — RGBA8 bytes, `width * height * 4` of them — or `None`; shown on Windows and X11 (Wayland and macOS take the app's icon from its desktop file or bundle) | — |
| `maximized`, `minimized` | — (use the methods above) | The open window's state, or before `App.run()` how it opens |
| `active` | — | Whether the window has the OS's focus |
| `occluded` | — | (0.5.10) Whether the window is completely hidden: the OS says so (X11 and macOS only) or it is minimized |
| `pause_when_occluded` | `bool`, default `True` | (0.5.10) Whether an occluded window draws nothing until it shows again. See [Hidden windows](#hidden-windows) |
| `layer_viewport_margin`, `layer_flip_threshold` | (0.6.12) The default viewport margin and flip threshold of the window's [layers](layers.md); see [Layer placement policy](properties.md#layer-placement-policy-0612) | The setting; `0` by default |
| `ime_offset` | (0.6.12) `(dx, dy)` added to the caret box the input method places its candidate window at | The setting; `(0, 0)` by default |
| `platform` | — | `"wayland"`, `"x11"`, `"windows"`, or `"macos"` |

Each settable one applies live to an open window, or when `App.run()` opens
it. A window smaller than its minimum is grown to it, when the minimum is
set and after any resize: on Wayland a minimum otherwise only limits what
the user can drag it to, and leaving fullscreen can restore a smaller size.
A maximized or fullscreen window isn't resized to its minimum. Nor is a
window the compositor holds at its size: on Wayland a compositor can report a
window as tiled, and a tiled window may not resize itself (COSMIC reports
every window this way). The minimum is still passed to the compositor as a
hint, but the window keeps its size, and `get("width")` and `get("height")`
give the real one, so don't assume a minimum was reached. Two window
events report state changes: **`maximized`** (`event.maximized`) when the
window is maximized or restored, and **`active`** (`event.active`) when it
gains or loses focus — each only when the value changes.

On Windows an undecorated window keeps its shadow. macOS is different; see
below.

### macOS: the overlay title bar

`winit` can't let the user resize an undecorated macOS window, so there
`decorations=False` doesn't remove the title bar: it makes it transparent
and runs the content up under it. The window keeps its shadow, native
resizing, and the traffic lights (close, minimize, zoom) in its top-left
corner, over the app's own title bar. `get("decorations")` is still
`False`. Two read-only properties say what's there, so one layout works on
every platform:

| Property | Get |
| --- | --- |
| `titlebar_inset` | `(height, width)` in logical pixels: the title-bar strip's height and the traffic lights' width with their margins. `(0.0, 0.0)` for a decorated window, in fullscreen (the traffic lights are hidden), and on every other platform |
| `native_controls` | Whether the OS shows its own window controls over the content: `True` only for an undecorated macOS window outside fullscreen. A framework hides its own minimize/maximize/close when it is |

The **`titlebar_inset`** window event (`event.titlebar_inset`) fires when
the inset changes — entering or leaving fullscreen, say.

```python
window = Window(decorations=False)
bar = window.create("box", height=40, window_region="drag")

def lay_out_bar(inset=None):
    height, width = inset or window.get("titlebar_inset")
    bar.set(padding_left=width)  # clear of the traffic lights
    for button in own_buttons:
        button.set(visible=not window.get("native_controls"))

window.on("titlebar_inset", lambda e: lay_out_bar(e.titlebar_inset))
lay_out_bar()
```

A drag region can be taller than macOS's own title-bar strip; all of it
moves the window. The resize border is off on macOS, where the OS resizes
the window itself. A double-click on the drag region does what the user
chose in System Settings (Desktop & Dock, "Double-click a window's title
bar to"): zoom, minimize, or nothing.

### Title bar and borders

An undecorated window has no title bar to move it by and no border to resize
it by (0.5.0). The framework marks its own; the OS does the moving and
resizing.

```python
window = Window(width=800, height=600, decorations=False)
window.set(resize_border=6, min_width=320, min_height=200)
bar = window.create("box", height=36, window_region="drag")
close = window.create("box", width=36, height=36, focusable=True)
close.on("click", lambda: window.close())
bar.add_child(close)                              # a button in the bar stays a button
bar.on("pointer_cancel", lambda: bar.set(opacity=1.0))
```

**The drag region.** A primary press on a node with `window_region="drag"`,
or on a node inside it that isn't interactive, moves the window. A node is
interactive when it's `focusable`, a text field or terminal, has a `click`
listener, or holds the pointer capture; only the nodes from the one pressed
up to the drag region are checked. `window_region="none"` turns dragging off
for a node and what's inside it, and `"drag"` on a node inside a button turns
it back on (a draggable icon, say). The press still delivers `pointer_down`
first, then — because the OS takes the pointer for the move — `pointer_cancel`
to the pressed node: no `pointer_up` or `click` follows, and any capture is
released.

**Double-click** on the drag region toggles maximize (on macOS, what the
user chose; see above). The two presses must fall within the system's
double-click time (Windows, macOS; 500 ms elsewhere) and 4 px of each other.
`set(titlebar_double_click=...)` (0.6.12) picks the action: `"maximize"`,
`"minimize"`, `"none"`, or `"event"`, which does nothing itself and fires the
window's `titlebar_double_click` event (`x`, `y`) for the app to act on (a
framework that rolls the window up, say). `None`, the default, is as before:
the user's setting on macOS, maximize elsewhere.

**The window menu** is opt-in: `window.set(system_menu=True)`, on any
platform. Then a secondary press on the drag region opens the OS's window
menu (Restore, Move, Size, Minimize, Maximize, Close) where the platform has
one — Windows, and Wayland compositors that offer one; elsewhere nothing
opens — and ends the press with `pointer_cancel`, so no `secondary_click`
follows. On Windows, Alt+Space on an undecorated window opens it too. Off,
the default, a secondary press on the drag region is an ordinary one, for
the framework's own menu.

**The resize border.** `set(resize_border=N)` makes a press within `N`
logical pixels of an edge resize the window from that edge, or from a corner
where two edges meet. Over the border the pointer shows the matching resize
cursor. A press there never reaches a node, and nor does its release. The
border is off for a decorated window (the OS has one), on macOS (the OS
resizes it), while maximized or fullscreen, and at `0`, the default.
`set(resize_cursors={...})` (0.6.12) changes the cursor of any of the eight
zones: keys `north`, `south`, `east`, `west`, `north_west`, `north_east`,
`south_west`, `south_east`, values a [cursor name](properties.md) (the same
names as a node's `cursor`; a `CursorImage` is not taken). Directions left out
keep the stock cursor (`ns_resize`, `ew_resize`, `nwse_resize` on the
north-west/south-east diagonal, `nesw_resize` on the other); a dict replaces
the previous one and `None` restores the stock map. `get` returns all eight.

| Property | Set | Get |
| --- | --- | --- |
| `resize_border` | Border width in logical pixels, a number `>= 0` | The setting |
| `system_menu` | Whether a secondary press on the drag region (and Alt+Space on Windows) opens the OS's window menu; `False` by default | The setting |
| `titlebar_double_click` | `None` (default: the OS's setting on macOS, maximize elsewhere), `"maximize"`, `"minimize"`, `"none"` or `"event"`; anything else raises `ValueError` (0.6.12) | The setting |
| `resize_cursors` | Dict of the eight directions to a cursor name, or `None` for the stock map (0.6.12) | All eight, as a dict |
| `undecorated_shadow` | Whether an undecorated window keeps the OS's shadow, a `bool`, default `True`. Windows only: applied to the live window and at creation; macOS and Linux accept and store it (there tess_re draws no shadow, the OS or compositor does) (0.6.12) | The setting |

In fullscreen the drag region and the border do nothing: their presses are
ordinary ones, delivered to the nodes as usual. The move, resize, and menu are the OS's and need a real
display; `simulate` exercises everything else — the rule, `pointer_cancel`,
double-click timing (with `advance`), and the border — without one.

## GPU health

Since 0.5.1, tess_re listens to the GPU, and three window events report what it
says. The device is polled each loop turn, and while GPU work is still running
an otherwise idle loop wakes about every 100 ms to poll; with the GPU idle it
costs nothing.

```python
window.on("gpu_lost", lambda e: log.error("GPU lost (%s): %s", e.reason, e.message))
window.on("gpu_error", lambda e: log.warning("GPU error: %s", e.message))
window.set(gpu_watchdog=10)   # opt in to gpu_stalled
window.on("gpu_stalled", lambda e: log.warning("a frame has run %.0f s", e.seconds))
```

- **A lost GPU ends the run.** A driver fault, or a hang the driver's own
  timeout caught (TDR on Windows, with Linux's and Metal's own timeouts),
  reaches tess_re as a lost device, and a lost device can't be restored. `gpu_lost`
  fires with `reason` (`"unknown"` for a fault, `"destroyed"` for a destroyed
  device) and the driver's `message`; then every window closes — a
  `close_requested` listener can't keep one open — and `App.run()` raises
  `RuntimeError("the GPU was lost: …; the run has ended")` instead of
  panicking or freezing. Nothing is rebuilt.
- **A GPU error doesn't panic.** `wgpu`'s default handler panics the process on
  any GPU error; tess_re replaces it. Each distinct error is logged once and fires
  `gpu_error` once, the draw that caused it is skipped, and the loop carries on.
- **The stall watchdog is off.** `window.set(gpu_watchdog=seconds)` turns it
  on (`None`, the default, turns it off): if a submitted frame hasn't completed
  after that many seconds, tess_re logs once and fires `gpu_stalled`. It only
  reports; stuck GPU work can't be cancelled. It matters most on a software
  adapter (the one CI uses), which has no driver timeout at all: a shader that
  never ends there is never reported lost.

`simulate("gpu_lost", reason=, message=)`, `simulate("gpu_error", message=)` and
`simulate("gpu_stalled", seconds=)` deliver each event to its listener with no GPU,
for tests.

## Window events and properties

**`on(event, handler)`** and **`off(event)`** listen to the window itself —
`resize`, `color_scheme`, `scale_factor`, `close_requested`, `closed`,
`dock_target`, `dock_drop`, (0.5.0) `maximized`, `active` and `titlebar_inset`,
and (0.5.1) `gpu_lost`, `gpu_error` and `gpu_stalled`; see
[Events and Listeners](events.md#window-listeners).

**`set(title=...)`** changes the title, live if the window is open.
**`set(partial_redraw=False)`** makes the window redraw all of itself every
frame. By default (`True`) each frame redraws only the parts of the window
that changed, and a frame where nothing visible changed draws nothing.
The pixels are the same either way; the switch is there for measuring and
for ruling partial redraw out when chasing a rendering bug. (0.5.8) Partial
redraw needs a kept copy of the last frame, as big as the window (4 bytes a pixel:
32 MiB at 3840 x 2160, 16 MiB measured for 2400 x 1400). A window opened with
`partial_redraw=False` never makes it, which is the cheap choice for a large surface
that redraws in full anyway; switching it on later makes the copy then and draws that
frame in full. Switching it off while the window is open stops using the copy and
frees it, though the driver may not hand the memory back at once (on one AMD driver the
process's video memory did not fall). A window whose
surface the platform won't let Tesserae Engine copy into always redraws in full, and
logs a warning saying so; `get("partial_redraw")` reports the setting, and
`get("partial_redraw_active")` whether it's in effect (`None` until
`App.run()` opens the window).

**`set(glyph_cache=True)`** (0.5.4) draws text from a glyph cache instead of
drawing each glyph's outline every time. A label costs about a quarter as much to
build (a 36-character label, 28 µs to 6 µs; a screen of labels, 18.8 ms to 5.8 ms),
which matters for a window with hundreds of text nodes that redraw. It is not
pixel-identical: edge pixels differ from the outline path by up to about 45 of 255
(80 at a scale factor of 2), and the glyph cache is marked experimental upstream.
So it is off by default, and `get("glyph_cache")` reports it. Switching it redraws
the whole window once. `snapshot()` follows the window's setting.

**Scrolling** (0.5.4) is the one change a partial redraw can carry as a block. When a
scroll view or virtual list scrolls and nothing else changes in or over it, the window
copies the view's kept pixels by the scroll distance and redraws only the strip the
scroll uncovered, its scrollbar, and anything that did not move with it, instead of the
whole view. It does this only when the copy is exact and worth it: the view has an opaque
plain background (no gradient, border or rounded corners), the scroll is a whole number of
device pixels, everything under it moved by the same distance, no other node paints over
it, nothing above it fades, blends or blurs, and at least 200 nodes move with it (a copy
costs about as much as redrawing fewer). Anything else redraws as before. Text edges drawn
by the copy can differ from a fresh draw by one level in a pixel or two, because a glyph
is rasterised from where it sits in the window; nothing else differs, and a later full
redraw re-draws them. How much it saves depends on the machine and the content: it
removes the redraw of the view, but not the walk over its nodes. Set the environment
variable `TESS_RE_SCROLL_BLIT_OFF=1` to turn it off while measuring or chasing a rendering bug.
**`set(transparent=True)`** (0.5.4) opens the window see-through: the OS gives it an alpha
channel, so a root `fill` with alpha below 255, or `(0, 0, 0, 0)`, shows the desktop
through it. With `decorations=False` that is a window of any shape: a rounded card,
a floating palette, a splash. It must be set before `App.run()`, because the OS fixes
it when the window is made (X11 can only choose it then); setting it on an open window
raises `ValueError`. Once the window is open, `get("transparent_active")` says whether
it took: `True` when the surface blends with the desktop, `False` where it can't
(there is no premultiplied or inherited alpha mode), and the window is then opaque, so
an app can fall back to a solid background. It is checked live on KDE under both
Wayland and X11, where a 50% red box over a green window behind it composites to exactly
`(128, 127, 0)`.

```python
window = Window(width=320, height=200, decorations=False)
window.set(transparent=True)
window.root.set(fill=(0, 0, 0, 0))                 # nothing of its own
card = window.create("box", width=300, height=180, x=10, y=10, position="absolute",
                     corner_radius=24, fill=(0x1C, 0x1B, 0x1F, 0xE6))   # 90% opaque, rounded
window.root.add_child(card)
```

**`set(blur_behind=True)`** asks the compositor to blur what is behind a transparent
window, live. It works only where the platform has it: Wayland with KDE's blur protocol,
and macOS; it is ignored on X11 and Windows, so treat it as a polish and keep the
window readable without it.

**`set(click_through=True)`** (0.5.4) makes the whole window ignore the pointer: clicks,
scrolls and hover go to whatever is behind it, and the window gets none. It takes effect
live, or at open if set before `App.run()`. The window is all or nothing: winit has no
per-region input shape, so a window whose opaque parts should still take clicks cannot
have its transparent parts pass them through. A platform that cannot do it raises
`ValueError`. A click-through window is for overlays and HUDs, so give the app a
keyboard shortcut or another window to turn it off again, since the window itself
can no longer be clicked.

**Placement (0.5.6).** `set(x=, y=)` puts the window's top-left corner at that place on the
desktop, in logical pixels; with only one of them, the other stays where the window is.
`get("x")`/`get("y")` read the window's place (before it opens, what was set), or `None`
where the system doesn't say. `center()` centres the open window on its monitor and
returns `False` if it can't. `set(always_on_top=True)` keeps the window above others,
`set(resizable=False)` stops the user resizing it, and `set(skip_taskbar=True)` keeps it
off the taskbar (Windows only; elsewhere it raises `ValueError`). All of them take effect
live, or at open if set before `App.run()`.

| | Windows | macOS | X11 | Wayland |
|---|---|---|---|---|
| `x`, `y`, `center()` | yes | yes | yes | no: the compositor places windows, and `get` returns `None` |
| `always_on_top` | yes | yes | a hint some window managers ignore | no |
| `resizable` | yes | yes | yes | yes |
| `skip_taskbar` | yes | no | no | no |

Window `parent`/`transient_for`, `modal` and centring on a parent are not provided: winit has
no modal windows on any platform, and a transient parent only on Windows.

Notes: draw straight-alpha colours as always; the renderer premultiplies for the
surface. An app that wants shadows draws them itself (the OS draws none around a
transparent undecorated window on every platform). A transparent window with partial
redraw still redraws exactly: cleared areas become transparent again.

## Timers (0.5.6)

**`window.after(ms, fn) -> TimerHandle`** calls `fn()` once, `ms` milliseconds from now.
**`window.every(ms, fn) -> TimerHandle`** calls it every `ms` milliseconds (at least 1)
until it is cancelled. `handle.cancel()` stops a timer and returns whether it was still
pending; `handle.active` says whether it is.

```python
toast = window.after(4000, dismiss_snackbar)
spinner = window.every(120, next_frame_of_spinner)
...
toast.cancel()
```

- **They run on the loop's thread**, with the window's other callbacks, in the frame, right
  after animations tick. A timer fires on the first frame at or after its time.
- **An idle window sleeps until the next timer.** A window with nothing animating and a
  snackbar's four-second timer redraws about once, when the timer fires, instead of sixty
  times a second to count down.
- **They run on the window's clock**, so `window.advance(ms)` moves them in tests: timers
  due inside the span run in order of time, each at its own moment, so `advance(1000)` with
  an `every(100)` runs ten times.
- **`every` keeps its own beat.** It does not drift by a frame per round. When the loop was
  too busy to run a tick on time, the tick is skipped, not replayed in a burst.
- **A timer set before `App.run()`** counts from when the window opens, with the time it had
  left kept. When the window closes its timers are dropped.
- **An exception in `fn`** is logged like any other callback's, and the timer goes on (an
  `every` keeps repeating; cancel it yourself to stop).
- A callback may cancel other timers, including ones due at the same moment, and may start
  new ones; a timer it starts runs from the next pass.

For a call from another thread, use `LoopHandle.call_soon`, which has no delay.

**`set(show_damage=True)`** makes each presented frame show what it
redrew: its damage rects tinted magenta, or, for a full redraw, the window's
edge outlined in orange. It's for seeing partial redraw work and for finding
redraws an app didn't mean to cause. The tint goes on the image the window
shows, never on the frame Tesserae Engine keeps, so it never changes what later frames
draw. Off by default.
**`set(present_mode=...)`** (0.5.4) chooses how frames are paced to the display.
`"vsync"`, the default, shows one frame per display refresh and lets the loop
wait for the display between frames, so a window with something animating uses a
few percent of a core. `"low_latency"` shows the newest frame at once
(`Mailbox`, where the surface has it, else vsync): a frame is never a refresh
old, but while something animates the loop renders as fast as it can and uses a
whole core. It takes effect at once, even in an open window. A window with
nothing changing costs nothing in either mode.

While a window is being **resized** (0.5.5), it is shown with the low-latency mode
whatever `present_mode` says, and goes back to the app's choice about 0.4 s after the
last resize, with one swapchain rebuild. A resize rebuilds the swapchain every frame,
and on Wayland a vsync present commits with a `wp_fifo_v1` barrier; KDE's compositor
then waited up to half a second before sending the next resize step, so an undecorated
window dragged by its edge trailed the pointer and kept moving after the button was
released. Nothing to set, and `present_mode` still means the steady-state choice.
**`set(dpi_scaling=True)`** (0.5.4) makes the window HiDPI-aware. The app
lays out in *logical* pixels and the engine draws at the display's
`scale_factor`: on a 2x screen a 100x60 box is 200x120 device pixels, and
text, borders and rounded corners are drawn at that resolution, so they stay
sharp instead of being stretched. `width`, `height`, the root's layout box,
pointer positions and pixel scroll deltas are all logical too, and shader
`frame.size` stays in device pixels. Off by default, since a framework that
already multiplies by `scale_factor` itself would be scaled twice; with it
off nothing changes. It takes effect live, and follows the window to a
monitor with a different scale. At a fractional scale such as 1.5, a node's layout
offset is rounded to a whole device pixel so its edges and glyphs sit on the grid.
This applies only to nodes whose ancestors add no scale of their own: under a node
with a paint `scale` (a hover zoom, say) offsets stay exact, so a child moves
smoothly as that scale animates instead of stepping by whole pixels.
**Colour scheme (0.6.11).** `Window(scheme=...)`, `LayerWindow(scheme=...)` and `PopupWindow(scheme=...)`, and
`window.set(scheme=...)`, take `"light"`, `"dark"`, `"auto"` (follow the OS flag `get("dark")`; light while unknown or
before the window opens) or `None` (follow the `App`'s, the default). `get("scheme")` is what the window follows,
`get("scheme_resolved")` (read-only) is `"light"` or `"dark"`. `set(scheme_colors={role: (r, g, b, a)})` overrides
single roles of the baseline (replacing earlier overrides; `None` or `{}` clears; an unknown role raises `ValueError`
listing the valid ones), and `get("scheme_colors")` is the whole resolved table; `tess_re.scheme_colors("light" | "dark")`
is the baseline. See [Colour scheme](../guide/painting.md#colour-scheme-0611) for what the scheme covers.

**`get(name)`** reads `width`, `height`, `title`, `scale_factor` (`1.0`
until `App.run()` opens the window), `dark` — the OS's current appearance,
or `None` where it can't say ([Window properties](events.md#window-properties))
— `partial_redraw`, `partial_redraw_active`, `glyph_cache`, `show_damage`, or the
[window's controls and state](#window-controls-and-state) above.

**`resize(width, height)`** sets the window's size from code, and the
root's layout box follows. It fires no `resize` event — that reports a
change the user made; `simulate("resize", width=, height=)` stands in for
one.

## `frame_stats`

**`frame_stats(reset=False)`** (0.5.4) says what the window's frames cost, so an app or
a framework can find its own bottlenecks. It is always on and costs a few clock reads
a frame. It returns a dict:

```python
stats = window.frame_stats()
stats["frames"]      # frames drawn since the window opened
stats["skipped"]     # passes that found nothing to draw (the loop slept through them)
stats["last"]        # the last frame, or None
stats["recent"]      # the last 240 frames
```

A frame (`last`, and each `frame` event's `stats`) has, in milliseconds, `tick_ms`
(animations and timers), `layout_ms`, `configure_ms` (0.5.5: rebuilding the swapchain
for a new window size, zero on a frame that did not resize; it used to be counted in
`layout_ms`), `prepare_ms` (working out what changed),
`acquire_ms` (waiting for the swapchain to hand over an image), `draw_ms` (building
the scene, rendering and submitting), `present_ms` and `total_ms`, plus `cpu_ms`
(`total_ms` without `acquire_ms` and `present_ms`: the app's own cost), `redraw`
(`"nothing"` when the kept frame was shown again, `"full"`, or `"partial"`),
`damage_rects` and `damage_area` (the fraction of the window redrawn), `shader_passes`,
`nodes` and `width`/`height`.

`recent` gives `count`, `fps` (frames a second over those frames), `total_ms` and
`cpu_ms` each as `mean`, `p95` and `max`, `stage_ms` (each stage's mean), and
`redraws`, how many of the frames were `nothing`, `full` or `partial`.

```python
r = window.frame_stats()["recent"]
print(f"{r['fps']:.0f} fps, {r['cpu_ms']['p95']:.2f} ms cpu at p95, "
      f"{r['redraws']['partial']} of {r['count']} frames partial")
```

`memory` (0.5.8, #222) is what the window's GPU device holds, read from `wgpu`'s allocator
when you call `frame_stats()` (not every frame): `allocated_bytes` (buffers and textures in
use), `reserved_bytes` (blocks taken from the driver, which is what the process holds),
`blocks`, `largest`, the five biggest allocations as `{"name", "bytes"}`, and `shared_device`
(0.5.9, #220): windows share one GPU device, so these numbers are the whole device's and every
window reads the same ones (`TESS_RE_GPU_DEVICES=per-window` gives each window its own). It is `None`
before `App.run()` opens the window and on a backend that cannot report. A `reserved_bytes`
far above `allocated_bytes` is the allocator's block size, not a leak
([VRAM per window](../design/vram-per-window.md)).

```python
m = window.frame_stats()["memory"]
if m: print(f"{m['allocated_bytes'] / 2**20:.1f} MiB used of {m['reserved_bytes'] / 2**20:.0f} MiB reserved")
```

Read the stages with the display in mind: with vsync on (the default, see
`present_mode`), `acquire_ms` is the loop waiting for the next refresh, so a frame's
`total_ms` is near the refresh interval even when `cpu_ms` is a small part of it.
`cpu_ms` is the number to watch against a frame budget. `reset=True` clears the
history after reading it, for measuring one interaction. Nothing is recorded before
`App.run()` opens the window.

For every frame as it happens, `window.on("frame", handler)` (`event.stats` is that
frame's dict); it fires each frame, so keep the handler cheap.

### GPU time

Where the adapter supports timestamp queries (`stats["gpu_timing"]` says), the GPU's time
is measured on one frame in sixteen (timing every frame costs the CPU about 0.3 ms a frame,
as much as a small frame's whole cost): `gpu_ms` on those frames (`None` on the rest), and
`recent["gpu_ms"]` as the mean over the frames that have it. The numbers are read back a few frames after the frame was drawn,
without waiting for the GPU, so the newest frame's `gpu_ms` is `None` until its result
arrives, and a frame that found every slot still in flight goes unmeasured rather than
stall the loop. It times the frame's own submission (the scene's render, the copy to the
screen, a scroll shift); a shader pass or an effect's offscreen render is submitted
separately and is not in it.

### Where the time went

`window.set(profile_nodes=True)` times each node the paint walk reaches, so
`stats["profile"]` can say where the scene-building time of the last frame went:

```python
window.set(profile_nodes=True)
...
p = window.frame_stats()["profile"]
p["ms"], p["reached"]                       # all the nodes' time, and how many were reached
p["by_kind"]["text"]                        # {"reached": 30, "drawn": 4, "ms": 0.42}
for s in p["slowest"]:                      # the ten slowest nodes, slowest first
    print(s["kind"], s["ms"], s["node"])    # the Node itself (None if it has since gone)
```

A node's time is its own (the walk's decision, its layers and its drawing, not its
children's, which are nodes of their own). `drawn` counts those that drew themselves; the
others were reached but fell outside what a partial redraw repaints. It is the CPU work of
encoding the scene, not the GPU's, and costs two clock reads a node, so it is off by
default; `stats["profile"]` is `None` while it is.

### A trace for other tools

`window.start_trace(path)` writes every frame the window draws from then on to `path`
in the Chrome Trace Event Format, until `window.stop_trace()` (which returns how many
frames it holds). Open the file at [ui.perfetto.dev](https://ui.perfetto.dev) or
`chrome://tracing`: each frame is a slice with its stages (tick, layout, prepare,
acquire, draw, present) inside it on the loop's track, and the GPU's time is a slice on a
second track. The file is written as it goes, so a viewer can read one a crash cut short.
`start_trace` raises `OSError` if the file can't be created and `ValueError` if a trace
is already running.

### From another thread

A `Window` belongs to the thread that made it. `window.stats_handle()` (made on that
thread) returns a `StatsHandle` that any thread can hold: `handle.read()` returns the same
dict as `frame_stats()`, except that `profile` is always `None` (it names `Node`s). It
does not wait for the event loop, so it works while the loop is busy.

## `snapshot`

**`snapshot(width=None, height=None, scale=None, time=0.0)`** (0.5.4) returns
what the window draws as `(rgba, width, height)`: straight-alpha RGBA8 bytes,
`width * height * 4` of them, top row first. It renders the window's tree
offscreen, so it works before `App.run()` and on a machine with no display
(it uses a software GPU adapter where there is no GPU), and it leaves the
window as it was.

`width` and `height` are logical pixels and default to the window's own;
`scale` multiplies them into the pixels returned, and defaults to the
window's (`1.0` before it opens, or with `dpi_scaling` off). `time` is the
clock, in seconds, that an animated shader sees as `frame.time`. Animations
are drawn at their current values; they are not advanced. A size or scale
that isn't greater than 0 raises `ValueError`; no GPU, or a size the GPU
can't render, raises `RuntimeError`.

`tess_re.write_png(path, rgba, width, height)` saves the pixels as a PNG, and
`tess_re.png_bytes(rgba, width, height)` returns the file's bytes, both with the
standard library alone:

```python
import tess_re

window = Window(width=320, height=200)
...  # build the tree
tess_re.write_png("home.png", *window.snapshot())
tess_re.write_png("home@2x.png", *window.snapshot(scale=2))
```

What it returns is what the window shows: the same renderer draws both, and
a live window and its snapshot are pixel for pixel the same. That makes it
usable for visual regression tests (compare against a stored image) and for
documentation screenshots.

## `measure_text`

**`measure_text(text, font_family="Roboto", font_size=16, font_weight=400,
font_style="normal", letter_spacing=0, line_height=None, max_width=None,
wrap="word", max_lines=None, overflow="clip") -> (width, height)`**

The size `text` takes, laid out exactly as a text node with those properties
paints it: wrapped within `max_width` when given, cut to `max_lines`, ended
with an ellipsis for `overflow="ellipsis"`. The width is the widest shown
line without its trailing whitespace. The numbers are exact: a text node that is
unsized (0.6.2) is this width plus 2 pixels, so give a hand-sized node a little
more than this if it must never wrap. This is how a widget sized by hand gets
its size. See [Text](../guide/text.md#sizing-text-to-its-content).

## Clipboard

**`read_clipboard() -> str | None`** returns the OS clipboard's text, or
`None` when it holds no text or can't be reached. **`write_clipboard(text) ->
bool`** puts `text` on it, `False` when it can't be reached. Neither raises:
a headless environment may have no clipboard service, which is logged. A
window keeps what it wrote on the clipboard for as long as it runs.

A focused text input handles Ctrl+C, Ctrl+X, Ctrl+V, and Ctrl+A itself —
copy, cut (which fires `change`), paste (typed input, so `input` fires), and
select all — and never copies from an `obscured` input.

## Testing without a display

**`simulate(event, node=None, **fields)`** delivers a synthetic event through
the same input pipeline as a live one — listeners, focus, text editing, a
focused terminal, the clipboard shortcuts, and docking drags all behave as
they would for a real mouse or keyboard:

```python
window.simulate("click", node=button)
window.simulate("pointer_move", node=card)          # hover
window.simulate("wheel", node=rows, delta_y=60)     # positive scrolls down
window.simulate("key_down", key="tab", shift=True)  # focus backward
window.simulate("input", text="hello")              # typing into the focused input
window.simulate("key_down", key="c", ctrl=True)     # copy its selection
```

Every event and field is in
[Events and Listeners](events.md#testing-without-a-display).

The `maximized` and `active` events fire only when the state changes, and before
`App.run()` `maximize()`, `restore()`, and `minimize()` change it without an
event, so `simulate("maximized", maximized=True)` straight after `maximize()` is
no change and fires nothing. Simulate the report on a fresh window, or leave the
state first ([Custom Title Bars](../guide/custom-title-bars.md#testing-without-a-display)).

**`advance(ms)`** moves this window's time forward by exactly `ms`
milliseconds, then runs animations, their `on_complete` callbacks, and
layout at the new time — deterministic time for tests, where `App.run()`
renders no frames:

```python
window.advance(0)            # pin the clock before starting animations
card.animate("opacity", 0.0, 200)
window.advance(100)
assert card.get("opacity") == 0.5
```

The first call pins the window's clock at the real current time; from then
on only `advance` moves it. Each window keeps its own time, and
`App.run()` returns every window it opens to the real clock.

## Docking

| Method | Purpose |
| --- | --- |
| `add_dock_zone(side, container, size)` | Registers `container` as `side`'s dock zone |
| `dock_panel(side, panel)` | Docks `panel` into `side`'s zone and shows it — moving it, if it's docked in another zone |
| `set_active_panel(side, index)` | Shows the zone's `index`th panel |
| `start_panel_drag(panel)` | Starts dragging a docked panel; the drag reports through the `dock_target`/`dock_drop` window events |
| `undock_panel(panel)` | Takes `panel` out of its zone and off the tree; `dock_panel` can dock it again |

`side` is one of `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"`.
See [Docking](../guide/docking.md) for a walkthrough.

## Hidden windows

**Wayland (0.5.10, #246).** The compositor says nothing about a hidden window, but it stops sending
the *frame callbacks* a surface draws on, and tess_re now draws a window only after the callback for its
last frame. A window on another workspace or minimized therefore stops drawing and costs nothing
(sway: 0 CPU ticks in 5 s with a looping animation, against 129 in 4 s visible; KWin 6.8: a minimized window drew 23
frames in 8 s against 255 before, one on another virtual desktop used about 0.5% of a core against 3.1%). A
`LayerWindow` under a fullscreen window stops drawing on sway; **KWin keeps showing it and keeps sending its frame
callbacks**, so it keeps drawing there (498 frames in 8 s, as when visible). `restore()` cannot bring a window back
from the minimized state under Wayland (the protocol has no such request); the user does. It no longer stalls the
others. Before this a hidden window left a vsync present
blocked while holding the one thread every window and Python thread shares, so **every other window
and every Python thread froze** until it was shown again. This needs no setting.

**X11 and macOS** report `occluded` (`window.get("occluded")`, the `occluded` event with
`event.occluded`). While the OS says so and `pause_when_occluded` is true (the default) the window draws
nothing and asks for no more frames, so the loop sleeps if every window is hidden. Frames that still happen
(an input, a timer, a wake) still run the animations and `on_complete`; values follow the clock, so they are
right when it shows, and it draws everything once it does. Set `pause_when_occluded=False` for a window that
must keep drawing while hidden. `get("occluded")` also reads `True` for a minimized window, but the pause
follows only what the OS reports. `Window.snapshot()` is not affected. **Windows** has no occlusion signal.
Nothing but Linux Wayland was run for this (sway and KWin); X11 and macOS are unverified.

**What a hidden Wayland window costs and does not do.** A window or layer surface the compositor is not
showing asks for nothing and draws nothing. Timers (`call_later`, `every`), thread-posted `call_soon` work and
`on_complete` callbacks still run while *every* window is hidden (a lone bar under a fullscreen window): since 0.6.10
the loop runs them on each wake and timer deadline without drawing, so a shell's bar keeps polling its state. Animations
tick only then (a timer or a wake), never by polling; their values follow the clock and draw when the window shows. `present_mode="low_latency"` no longer draws faster than the compositor
asks for frames on Wayland (a resize still presents without waiting, as before).

## First present and background (0.6.11)

On macOS and Windows an app can flash the OS's default background, the wrong light or dark, between the window
appearing and its first drawn frame (UX spec AC-TH-7 and AC-TH-8). These hold the first frame back until the app says it
is complete, and give every frame a clear colour that matches the app.

| Name | Type | Default | Meaning |
| --- | --- | --- | --- |
| `first_present` (constructor only) | `"immediate"`, `"on_ready"` or an `int` of milliseconds, 1 to 600000 | `"immediate"` | When the first frame is presented. `"immediate"` is as before: when it is drawn. `"on_ready"`: not until the app calls `window.ready()`. An `int`: `"on_ready"`, but at the latest that many milliseconds after the OS window was made, whether or not `ready()` was called |
| `background` | `(r, g, b, a)` of 0-255 ints, or `None` | `None`: the window scheme's `surface` role (`#FEF7FF` light, `#141218` dark) | The colour every frame is cleared to before the scene paints over it. Anything the app paints wins; it shows where the root and its children leave pixels bare, and in a window made see-through (`transparent=True`) it is what shows over the desktop. Settable before the window opens and live |
| `first_presented` | `bool` | | `get` only: whether the first frame has been presented |
| `ready()` | method | | Tells a held window its first frame is complete. A no-op on a window that presents immediately, for a window that is already presented, and for every call after the first. Callable from a listener, a timer or `LoopHandle.call_soon`, before or after the window opens |

```python
window = Window(width=900, height=600, first_present="on_ready", scheme="dark")
build_the_ui(window)
app.add_window(window)
app.thread_handle().call_soon(window.ready)     # or window.after(0, window.ready), or from your own handler
app.run()

Window(first_present=500)                      # shown when ready(), or after 500 ms, whichever is first
Window(background=(18, 18, 18, 255))            # the clear colour; None is the scheme's `surface`
window.set(background=None)
```

While the first present is held the engine still runs: timers and `call_soon` work run, the tree is laid out, and
`layout` events fire, held passes count as skipped frames in `frame_stats`, but nothing is drawn or committed to the surface, so the window cannot
show an unfinished frame. `ready()` wakes the loop and the first frame is drawn complete (the held frame is redrawn in
full, never from a partial redraw). `first_present` cannot be changed by `set` (`ValueError`): it describes how the
window opens. `Window.snapshot()` and the headless pipeline are unaffected.

**Per platform.** *Wayland*: a surface is mapped by the first buffer it commits, so the held window has no buffer and
the compositor shows nothing; verified on sway (the first `wl_surface.attach` with a buffer comes after `ready()`, or
after the deadline). *X11, macOS, Windows*: the OS window is also created hidden and shown right after the first
frame is presented, and the occlusion pause (`pause_when_occluded`) is ignored while the first frame is awaited.
**This is written but unverified here** (only Linux Wayland is testable); the hidden window must still deliver redraw
requests on those platforms for the first frame to be drawn. `background` is a clear colour, not a window-class brush:
before the first present the OS's own default shows for as long as the window is visible without a frame, which is
exactly what `"on_ready"` avoids.

`LayerWindow(first_present=..., background=...)` and `PopupWindow(first_present=..., background=...)` (0.6.12) take
the same arguments and `ready()` works on them. A layer surface and an `xdg_popup` are mapped by the first buffer they
commit, so holding the first present holds the mapping: until `ready()` (or the deadline) the compositor shows
nothing, reserves no `exclusive_zone` and gives the surface no pointer or keyboard input. The configure handshake is
not affected: the layer thread acknowledges every configure before the app hears of it, so the surface has its role and
its size, and a layout, `size="content"` settling and `set(...)` made meanwhile (the `keyboard` and `input_region` given
to the constructor included) go to the compositor as usual in buffer-less commits and take effect when the surface maps.
A deadline counts from the surface's first configure. `background` is a constructor argument too; a layer surface or
popup clears to nothing (see-through) unless it is set. See [layer-window.md](layer-window.md#first-present-and-ready)
for the popup case (grab, a parent that closes while the popup is held).

## Gesture, click and drag thresholds (0.6.12)

How quick, how near and how far a touch or a press must be to count as one thing and not another were fixed
numbers. They are window properties now (`Window.set` / `get`, and the same on `LayerWindow` and `PopupWindow`),
and each default is the value it always had, so a window that sets none behaves as before. Distances are layout
pixels (device independent, like `touch_slop`); a bad value (not a number, not finite, out of range) raises
`ValueError` naming the property and nothing is changed.

| Property | Type, range | Default | Meaning |
| --- | --- | --- | --- |
| `gesture_long_press_ms` | ms, more than 0 to 3600000 | `500` | How long a touch held in place (within `touch_slop`) becomes a `long_press` |
| `gesture_tap_ms` | ms, more than 0 to 3600000 | `300` | A touch released within this is a `tap`; held longer (and not a long press) it is nothing |
| `gesture_double_tap_ms` | ms, 0 to 3600000 | `300` | A second tap within this of the first is a double tap (`count` 2); `0` makes none |
| `gesture_double_tap_px` | px, 0 to 1e6 | `30` | ... and within this distance of the first |
| `gesture_swipe_speed` | px/s, 0 to 1e6 | `600` | A pan is a `swipe` if it lifts at least this fast ... |
| `gesture_swipe_px` | px, 0 to 1e6 | `40` | ... and has travelled at least this far |
| `gesture_rotate_slop` | radians, 0 to pi | `0.12` (about 7 degrees) | How far two fingers have to turn before it is a `rotate`; less is the wobble of a pinch |
| `multi_click_ms` | ms, 0 to 3600000 | `500` | Primary presses in text (static text, `text_input`, terminal) within this and `multi_click_px` of the one before count 2 and 3: a double click selects the word, a triple click the line. `0` makes none |
| `multi_click_px` | px, 0 to 1e6 | `4` | ... within this distance. Also how far apart the two presses of a title-bar double click (`window_region="drag"`) may be; its time stays the platform's |
| `drag_start_px` | px, 0 to 1e6 | `0` (off) | While the primary button is held, `pointer_move` listeners hear nothing until the pointer has moved this far from where it was pressed, and every move after that; release ends it. A press that never gets that far is a click, whatever the hand's wobble |

`touch_slop` (10 px) and `touch_target_slop` stay as they were. The thresholds are read at the next touch or
press, so changing one mid-gesture applies from the next event (a long press already waiting uses the new time at its
next poll).

```python
window.set(gesture_long_press_ms=400, gesture_double_tap_ms=250, gesture_double_tap_px=20)
window.set(multi_click_ms=400, multi_click_px=3)
window.set(drag_start_px=4)       # the Tesserae UX spec's drag start distance is 4 dp

moves = []
handle.on("pointer_down", lambda e: handle.capture_pointer())
handle.on("pointer_move", lambda e: moves.append(e.window_x))   # only once the pointer is 4 px from the press
```

`drag_start_px` gates what listeners hear, not what the engine does: hover, a scrollbar thumb or a text selection
drag follow the pointer from the first pixel, as they must to feel right. It applies to the pointer a finger stands
in for as well. (0.6.12) One thing the engine does decide with it: a press on a chosen row of a `virtual_list` with a
`selection_mode` chooses that row alone on its release, unless the pointer went `drag_start_px` first
([the drag rule](../guide/nodes-and-layout.md#selecting-rows-of-a-virtual-list)); with 0 no move is a drag.
The Tesserae UX spec asks for 4 on `drag_start_px` (its notes "Drag and Drop" and "Drag and Drop Pattern": a
press then movement of at least 4 dp starts a drag), 400 ms and 4 px for a double click, 500 ms for the long press
and 10 px for the slop (its note "11 Interaction"); a design system sets its values once at window creation. The defaults here are the engine's historical ones, not a promise of the spec.

