# `LayerWindow`

A [`Window`](window.md) whose Wayland role is a **layer surface** (`zwlr_layer_shell_v1`):
a bar, a panel, a dock, an OSD or a wallpaper that the compositor places, not the user.
It anchors to screen edges, sits on a stacking layer, can reserve screen space so
maximised windows stop short of it, takes the keyboard in the way you choose, and can be
bound to one output. A `LayerWindow` is a `Window` in every way that makes sense (`root`,
`create`, `show_layer`, `after`, `every`, `on`, `simulate`, `advance`, `snapshot`,
`close`), and `App.run()` opens it like one. How to build one is in
[Bars and panels](../guide/bars-and-panels.md).

> **Two senses of "layer".** tess_re's [layers](layers.md) (`show_layer`) are nodes that
> float over a window's own content, drawn inside it. A layer *surface* is a separate
> window, with its own stacking position among everything on the desktop. A
> `LayerWindow`'s `layer` setting is the second kind; it keeps the protocol's word. A
> `LayerWindow` can use `show_layer` too, but its layers are clipped to the bar's own
> small surface.

```python
from tess_re import App, LayerWindow

bar = LayerWindow("tesserae-shell.bar", layer="top", anchor=("top", "left", "right"),
                  size=(0, 32), exclusive_zone=32)
app = App()
app.add_window(bar)
app.run()
```

## The settings

**`LayerWindow(namespace, *, layer="top", anchor=None, size=(0, 0), min_size=None,
max_size=None, exclusive_zone=0, margin=0, keyboard="none", output=None,
input_region=None)`**

| Setting | Meaning |
|---|---|
| `namespace` | What the surface is for (`"tesserae-shell.bar"`); the protocol's namespace. Required and not empty. Constructor only. |
| `layer` | Where it stacks: `"background"`, `"bottom"`, `"top"` or `"overlay"`. Windows sit between `"bottom"` and `"top"`; a fullscreen window may cover `"top"`. |
| `anchor` | The edges it is attached to: a sequence of `"top"`, `"bottom"`, `"left"`, `"right"` (or one name). Nothing anchored floats, centred by the compositor. |
| `size` | `(width, height)` in logical pixels, each a number or `"content"` (below). A `0` stretches between the two edges anchored on that axis, so a `0` width needs `"left"` and `"right"` in `anchor`, and a `0` height needs `"top"` and `"bottom"`. |
| `min_size`, `max_size` | (unreleased) `None`, or `(width, height)` with `None` or a whole number of pixels above 0 for each axis: a floor and a ceiling for a `"content"` axis. A minimum above its maximum raises `ValueError`. |
| `exclusive_zone` | Pixels to reserve along the anchored edge (for a corner, the `exclusive_edge`), so maximised windows stop short of it. `0` reserves none; `-1` ignores other surfaces' zones and takes the whole output. A positive zone counts only for a surface anchored to one edge, or to one edge and both edges across it (a bar anchored `"top"`, `"left"`, `"right"`); with any other `anchor` the compositor treats it as `0`, and a `warning` event says so. |
| `margin` | `(top, right, bottom, left)` in logical pixels, or one number for all four. Negative is accepted and moves the surface past the screen edge. |
| `keyboard` | `"none"`: never gets key events. `"on_demand"`: gets them when the user focuses it (a click). `"exclusive"`: takes the keyboard on opening, and nothing else gets keys until it closes. |
| `output` | An [`Output`](outputs.md) (from `App.outputs()`) or a connector name (`"DP-1"`, `"HDMI-A-1"`) to bind it to. `None` picks the output at position (0, 0), or the first by name if none is there, the same on every run (the compositor's own choice alternates). A surface meant for another screen names it. Constructor only. |
| `first_present`, `background` | (0.6.12) When the first frame is presented, and what frames are cleared to; see [First present and ready](#first-present-and-ready). |
| `input_region` | Where the compositor delivers pointer and touch input. `None`: the whole surface. `[]`: nothing, so clicks fall through to the window below. A list of `(x, y, width, height)` rectangles in logical pixels from the surface's top-left, rounded outward, at most 256: input lands only inside them. Live through `set()`. `click_through=True` is `input_region=[]` on a `LayerWindow` (and `False` gives the whole surface back); giving both with different meanings raises `ValueError`. `"auto"`: the union of the shapes of drawn, hit-testable nodes (a box with a fill, gradient or border; any image, path, svg, canvas, text field or terminal; a selectable or linked text, or one with `hit_testable=True` (0.6.11): a plain label takes pointer events but does not hold the region open, so a label on a click-through layer stays click-through), with a box's rounded corners and a canvas's circle hit shape counted, and (0.5.9) a path set to `hit_shape="painted"` counted as what it paints, its fill and its stroke, so a ring leaves its middle open and a concave corner leaves its notch open ([hit testing a path](paint.md#hit-testing-a-path)); a path left at `hit_shape="box"` still counts as its box. A transparent root or a node with `hit_testable=False` adds nothing, so clicks there fall through. It follows layout, transforms and animation: while something animates it only grows, then snaps to the exact shape. A shape is cut into horizontal bands of 2 px or more (at most 256 rectangles, coarser if a shape needs more); it may cover a pixel or two beyond a curve, never less. The region reaches the compositor with a commit of its own, so an idle surface needs no frame. |

**`exclusive_edge`** says which edge a corner-anchored surface's `exclusive_zone` is for: a clock
in the top-right corner (`anchor=("top", "right")`) that reserves 28 pixels from the top takes
`exclusive_edge="top"`; with `"right"` it reserves them from the right instead. Without it a
corner surface reserves nothing (the compositor cannot tell which of the two edges was meant, and
a `warning` event says so). It must be one of the anchored edges. A surface anchored to one edge,
or to one edge and both edges across it (a bar), needs none: the compositor deduces it, and giving
the same edge is allowed. It is live through `set()`; the protocol has no request that clears an
edge once set, so `set(exclusive_edge=None)` on a surface that has one raises `ValueError` (move it
to the other edge, or make a new window). It needs layer-shell version 5 (KWin and recent wlroots
compositors have it); on a compositor with version 4 the surface still opens and a `warning` event
(`key` `"exclusive_edge"`) says the edge is ignored. tess_re binds the layer shell itself, at the
highest version the compositor offers.

**Compositor support.** sway (wlroots) and KDE's KWin 6.8.1 (version 5) both reserve the zone on
the named edge of a corner surface, and reserve nothing for a corner surface with no edge (checked
live on both, 2026-10-25).

### A surface the size of its content

(Unreleased, TRE-REQ-022.) `size=("content", "content")`, or one axis, `(480, "content")`, makes the surface
the size of what is in it and keeps it that size as the content changes (a result list that grows as the
user types):

```python
panel = LayerWindow("launcher", anchor=("bottom",), size=(480, "content"),
                    max_size=(None, 400))
panel.root.set(padding=16, flex_direction="vertical")   # 16 around 200 x 80 of content: 232 x 112
```

- The size is the **root node's outer size, padding included**: padding 16 around a 200 by 80 box makes the
  surface 232 by 112. The root is laid out unconstrained on a `"content"` axis (or up to `max_size`), so
  give it no size of its own there.
- Sizes are whole logical pixels, rounded up so a fractional scale never clips content, then clamped by
  `min_size` and `max_size`. Content past `max_size` is clipped: put it in a scroll view.
- tre asks the compositor for the new size when the content's size changes, **once per change**, and holds
  the frame that shows the new content until the compositor has answered (the `configure` event), so the
  new content is never drawn at the old size. Steady content asks for nothing, so there is no resize loop.
  Before the first layout the surface is 1 pixel (or `min_size`) and nothing is drawn.
- The edge the surface is anchored to stays where it is, so a bottom-anchored panel grows upward; an
  unanchored one stays centred. A compositor may lay the surface out at the new size one frame before the new
  buffer reaches it, so for that one frame the old content sits where the new size puts it (half a step off
  centre for a centred surface). It is the protocol's order, not something an app can avoid.
- `get("size")` gives what was set, with `"content"` kept as the word; `configure` gives the size the compositor
  set. `set(size=...)`, `set(min_size=...)` and `set(max_size=...)` work while the surface is open, in either
  direction. Headless, `get("layout_width")` and `get("layout_height")` of the root read the content size.

**`set(**props)`** changes `layer`, `anchor`, `size`, `min_size`, `max_size`, `exclusive_zone`, `margin` and
`keyboard` live, in one commit, and the keys `Window.set` has that mean something to a
placed surface (`touch_slop`, `partial_redraw`, `present_mode`, ...). A bad value raises
`ValueError` and changes nothing, the layer keys included. The window keys that mean
nothing to a surface the compositor places raise too, rather than being ignored:
`title`, `decorations`, `maximized`, `minimized`, `fullscreen`, `resize_border`,
`system_menu`, `min_width`, `min_height`, `icon`, `x`, `y`, `always_on_top`,
`resizable`, `skip_taskbar` and `blur_behind`. `namespace` and `output` can only be given
to the constructor.

**`animate(property, to, duration_ms=0, easing=None, on_complete=None)`** moves one margin
(`"margin_top"`, `"margin_right"`, `"margin_bottom"` or `"margin_left"`) to `to` pixels from where
it is now, with the same `duration_ms`, `easing` (linear, a cubic bezier, or a spring) and
`on_complete` as `Node.animate`. The engine runs it: one small commit per step, no buffer
change, and no Python until `on_complete` (called once, when the duration has elapsed). The
window's root gets `animation_end` (`property`, `finished`). Starting another animation on the
same margin retargets from the current value (the first one's `on_complete` is not called);
`set(margin=...)` while one runs cancels it and snaps (`finished=False`). Negative values slide
a surface past the screen edge. Any other property raises `ValueError`: surface size and
`exclusive_zone` are step changes, not animations. Headless, `advance(ms)` runs margin
animations and their callbacks with no compositor. Every window's animations in one loop turn
read the same time, but Wayland cannot commit two surfaces atomically, so surfaces animating
together can differ by up to a frame on screen. A margin takes whole pixels (the animation is
rounded to them).

**`get(name)`** reads those settings (`anchor` as a tuple of names, `margin` as a
4-tuple), and anything `Window.get` reads.

**`close()`** closes the surface on the loop's next turn: `close_requested` first (a
listener may keep it open with `event.cancel()`), then `closed` with reason `"app"`.

## Errors

The protocol's mistakes are `ValueError`s that name the key, raised before anything is
sent to the compositor (a bad request would end the connection): a `0` size with the two
anchors it needs missing, an unknown `layer`, `keyboard` or edge name, a `margin` that is
not four numbers, an `exclusive_zone` below `-1`. A node's `window_region="drag"` (a title bar that moves its window) raises on a `LayerWindow`'s nodes: the compositor places the surface, so there is no window to drag.

`LayerWindow(...)` raises **`UnsupportedPlatform`** (a `RuntimeError`) where there is no
layer shell: an X11 session, GNOME's Mutter, Windows and macOS. It does not quietly turn
into a plain window: a bar without its exclusive zone is worse than an error. An app that
wants a fallback catches it and builds one from `Window`. `tess_re.supports("layer_shell")`
answers the question without making a window. The first answer is remembered, a "no" included;
`tess_re.supports("layer_shell", refresh=True)` (0.6.10) asks the compositor again, so a program
started before its compositor can poll until it is there, and one that lost its compositor can
wait for the next (see [`connection_lost`](app.md#connection_lost-0610)).

With no display at all (a CI run) a `LayerWindow` can be built and driven headless, like
a `Window`, but not opened. A stretched axis is `1920` or `1080` logical pixels there
until `resize(width, height)` says otherwise.

If an `output` names no output, `App.run()` raises `RuntimeError` listing the ones the
compositor has.

### First present and ready

(0.6.12) **`first_present`** is `"immediate"` (the default, and what every layer window did before), `"on_ready"` or a
number of milliseconds (1 to 600000), exactly as for [`Window`](window.md#first-present-and-background-0611), and
`window.ready()` ends the hold. A layer surface is mapped by the first buffer it commits, so a held surface has a role, a
size and a place in the compositor's configure handshake but **no buffer**: the compositor shows nothing, reserves no
`exclusive_zone`, and sends it no pointer or keyboard input, until `ready()` or, for a number, that many milliseconds
after the surface's first configure, whichever comes first.

```python
bar = LayerWindow("shell.bar", anchor=("top", "left", "right"), size=(0, 32), exclusive_zone=32,
                  first_present="on_ready", background=(0x14, 0x12, 0x18, 255))
build_bar(bar)                       # nodes, text, icons: nothing is drawn yet
app.add_window(bar)
app.thread_handle().call_soon(bar.ready)   # or bar.ready() from any listener or timer
```

* The configure handshake is complete while it is held (every configure is acknowledged before the app hears of it), so
  the compositor's state stays consistent; layout runs, `configure` and `resize` fire, a `size="content"` surface settles
  to its content, and timers and listeners run. Only the draw and the commit of a buffer wait. The first buffer is drawn
  in full when it is let through.
* Settings made before the first buffer still apply: `keyboard` and `input_region` given to the constructor go with the
  role-setting commit, and `set(...)` made while held (also before the surface exists) is committed without a buffer, so it
  is in force the moment the surface maps.
* `background` is the colour every frame is cleared to; the default for a layer surface is nothing (see-through), so a
  rounded bar shows what is behind it. Set one to clear to a colour. `set(background=...)` works while open.
* `first_present` is fixed at construction (`set` raises `ValueError`). `ready()` is a no-op for an immediate surface, one
  already presented and every call after the first. `window.get("first_presented")` says whether the first frame is out.
* An app run with `app.run(max_frames=N)` counts a held pass as a frame, as for a window.

## Events

Window events apply, with these additions (see [Events](events.md)):

| Event | Fires | Fields |
|---|---|---|
| `configure` | The compositor assigned a size: when the surface first has one, and whenever it changes (a repeat of the same size, which sway sends, is not a change). `resize` fires too. | `width`, `height` in logical pixels |
| `closed` | The surface is gone | `reason`: `"compositor"`, `"output_removed"` or `"app"` (a `PopupWindow`: `"dismissed"` or `"failed"`) |
| `output_changed` | The surface entered an output | `output`: its connector name |
| `warning` | A setting the protocol accepts but ignores | `key` (the setting), `message` |
| `moved` | A margin drag ended (see below) | `margin`: the final `(top, right, bottom, left)` |
| `active` | The keyboard entered or left the surface: the compositor's keyboard enter and leave (see [Knowing the keyboard left, or the user clicked elsewhere](#knowing-the-keyboard-left-or-the-user-clicked-elsewhere)) | `active`: `True` when the surface has the keyboard |

After `closed` with reason `"compositor"` or `"output_removed"` the window is dead; make a
new `LayerWindow` for a new output.

## Knowing the keyboard left, or the user clicked elsewhere

A panel that holds the keyboard exclusively (a launcher: `keyboard="exclusive"`) cannot be left by clicking
elsewhere the way a menu is, and wants to close itself when that happens. There are two signals, and
what tre gives for each (TRE-REQ-024):

**The keyboard leaves: the `active` event.** A `LayerWindow` is told by the compositor when it gains and
loses the keyboard, and `window.on("active", ...)` hears it (`event.active`). With another exclusive surface
taking the keyboard (a lock screen, a second panel) it is `False`; when that surface closes it is `True`
again. Checked in a private sway with two exclusive surfaces: `True`, `False`, then `True` on the first as the
second opened and closed. A click elsewhere does **not** fire it while the surface holds the keyboard
exclusively, because the compositor does not take the keyboard away for a click; with `keyboard="on_demand"` a
click on another window does move focus away, and `active` is `False`.

**A press lands outside: a transparent catcher surface, no new event.** Put a fully transparent
`LayerWindow` the size of the output **under** the panel (layer `"top"` when the panel is on `"overlay"`)
with `keyboard="none"`, open it together with the panel, and hide the panel when it is pressed:

```python
catcher = LayerWindow("click-away", anchor=("top", "bottom", "left", "right"), size=(0, 0),
                      exclusive_zone=-1, layer="top", keyboard="none", output=panel_output)
catcher.root.set(fill=(0, 0, 0, 0))
catcher.root.on("click", lambda e: hide_launcher())     # a press anywhere not on the panel
app.add_window(catcher)        # from a handler, with the launcher open; close it with the launcher
```

The catcher never takes the keyboard (`keyboard="none"`), so the panel keeps it on every compositor.
It costs a full-output surface for as long as the panel is open (about 210 MB of VRAM fixed per window
plus the output's buffer, measured in #207): open it on demand and close it with the panel.

!!! warning "Do not use a grabbing `PopupWindow` for this"
    A `PopupWindow(parent, ..., grab=True)` hung off the panel was documented here in 0.6.10 as the
    recipe. It is **not portable**: an `xdg_popup` grab gives the popup the keyboard by protocol, and on
    **KWin** the panel loses it as soon as the popup is shown (`active` `False` on the panel, `True` on the
    popup, the `wl_keyboard` focus moves to the popup's surface), so the panel's text field stops receiving
    keys. It only appeared to work on sway/wlroots, which keep focus on an exclusive-keyboard layer surface
    (checked in a private sway only). A popup with `grab=False` does not take the keyboard but is not
    dismissed by an outside click either, so it cannot be a catcher. Checked in a private `kwin_wayland
    --virtual`, 2026-11-21.

What to know before relying on the catcher:

- **The press is the catcher's, not the application's below.** Wayland gives a client no way to see a press on
  another client's surface without taking it, so the click that hides the panel is not delivered to what is
  under it. The user clicks again. This holds for any such mechanism, which is why tre has no `outside_press`
  option: it could only wrap this.
- **It lives as long as you keep it.** Close the catcher when the panel hides, and open a new one with the
  next panel; do not leave a full-output surface open.
- **A click on the panel** goes to the panel: the catcher is stacked below it (a lower layer, or earlier on the
  same layer), so only a press elsewhere reaches the catcher.
- **Status:** tess-shell uses this on KWin (its tests on a private KWin with a fake pointer) and it holds the
  keyboard there. tre's own tests of this recipe do not exist yet; the sway test of the 0.6.10 grab popup was
  removed from the claims here (see the warning above).

## Dragging a surface by its margin

A movable widget (a desktop clock, a floating panel) anchors to an edge or a corner and moves
by changing its margins. Give a node `window_region="drag_margin"` and a primary press on it,
or on a non-interactive node inside it, starts a drag: the engine follows the pointer, one
small commit per input frame, with no Python in the loop, and when the button goes up the
window gets `moved` with the final `margin`, which the app can save. The usual exclusions apply
as for `"drag"`: a press on a focusable node, a text input or terminal, a node with a `click`
listener, or one holding the pointer capture, met before the drag node, is an ordinary press;
`window_region="none"` on a node keeps a part of the handle from dragging.

- Only the anchored edges move: a surface anchored `"top"` and `"left"` changes `margin_top`
  and `margin_left`; one anchored `"bottom"` and `"right"` changes the other two, in the
  opposite direction. An axis anchored to both edges (a bar across the top) or to neither does
  not move on that axis.
- The margins are clamped so the surface stays on its output: never negative, never past the
  far edge. (Other surfaces' exclusive zones do not limit it.)
- The deltas come from the compositor's relative-pointer protocol, so they are exact however
  the surface lags behind the pointer. An absolute pointer (a tablet, a virtual machine's,
  a remote desktop's, and KWin's reading of a `/dev/uinput` absolute device) sends no relative
  motion; the drag then follows the pointer's surface-local position, which can overshoot a
  little while a commit is still in flight. The same fallback runs where the compositor has no
  relative-pointer protocol.
- A running margin animation is stopped where it is when a drag starts. A node's
  `window_region="drag_margin"` raises on an ordinary `Window`, and `"drag"` still raises here.

## What it is like to draw on

- **Layout is in logical pixels, drawn at the surface's scale** (`dpi_scaling` is on, and
  follows fractional scaling). The root has **no padding or gap**, unlike a `Window`'s,
  and the window is **transparent**, so a rounded or floating bar needs no backdrop.
- **Input** is as a window's: pointer, buttons and scroll; keys (named keys, text, the
  clipboard shortcuts, and key repeat from the compositor's own rate; copy and paste go through the
  compositor's data-control protocol, so they work on a session with no X server); touch where the
  seat has it; the cursor shape a node asks for. Text arrives as typed (compose and dead
  keys work through the keymap); an **input method** (CJK composition, a candidate window) works
  too, through `zwp_text_input_v3`: while a text input of the surface has keyboard focus the
  input method is enabled, the candidate window is placed at the caret, a password field says
  so, and the node gets `compose_start`, `compose_update` and `compose_end` and the committed
  text as `input` and `change`, as in an ordinary window. It needs a compositor with the
  text-input protocol (KWin, sway) and an input method running; otherwise the surface types
  through the keymap alone. The surface needs keyboard focus (`"on_demand"` or `"exclusive"`). A click on a bar does not focus another window's keyboard: with
  `"on_demand"` the bar keeps keys until the user clicks elsewhere.
- **Idle costs nothing.** A surface draws only when something changed, and an idle bar
  wakes the loop for nothing but its own timers.
- It shares the loop, and one `App`, with ordinary windows.

## Accessibility

A layer surface is published to screen readers like a window (0.5.8). Linux has no toolkit-wide
accessibility layer to hook, so tess_re gives each surface an AT-SPI adapter of its own (AccessKit's
`accesskit_unix`, the backend a window's adapter wraps): the surface appears as one more window of
the application, its nodes carry the `role`, `label` and state you set on them, the tree follows the
nodes as they change, a screen reader's action on a node (a click, a focus) reaches the app as the
usual `a11y_action` and activation events, and keyboard focus entering or leaving the surface tells
the reader whether the window is active. Nothing is built until a screen reader asks. Under Wayland
an application cannot know where its surfaces are on screen, so the reader gets the tree but no
screen position for a layer surface. See [Accessibility](../guide/accessibility.md) for what a node
should declare.

## `PopupWindow`

A menu or flyout the compositor places next to a rectangle of its parent: a window whose Wayland
role is `xdg_popup`, parented to a `LayerWindow` (a tray menu off a bar) or to another
`PopupWindow` (a submenu). It is menu-sized, so it costs a bar's worth of memory, not an output's,
and the compositor, which knows the screen edges and other panels' reserved areas, flips and slides
it to stay on screen (0.5.8, #223).

```python
menu = PopupWindow(bar, size=(180, 120), anchor_rect=(100, 0, 60, 32),
                   anchor="bottom_left", gravity="bottom_right",
                   constraint_adjustment=("flip_y", "slide_x"))
menu.root.set(fill=(0x2B, 0x29, 0x30, 0xFF))
menu.on("closed", lambda e: print(e.reason))   # "dismissed" on an outside click or Escape
app.add_window(menu)                           # from a click handler, with `bar` already open
```

| Argument | Meaning |
|---|---|
| `parent` | An open `LayerWindow` or `PopupWindow`. Otherwise `closed` arrives with reason `"failed"`. |
| `size` | `(width, height)` in logical pixels, fixed for the popup's life. |
| `anchor_rect` | `(x, y, width, height)` in the parent's coordinates: the rectangle to hang off (the button). |
| `anchor`, `gravity` | The point on that rectangle the popup attaches to, and the direction it extends: `"none"`, `"top"`, `"bottom"`, `"left"`, `"right"`, `"top_left"`, `"bottom_left"`, `"top_right"`, `"bottom_right"`. |
| `constraint_adjustment` | What the compositor may do to keep it on screen: `"slide_x"`, `"slide_y"`, `"flip_x"`, `"flip_y"`, `"resize_x"`, `"resize_y"`. A name or a sequence; the default is `("flip_y", "slide_x")`, and `()` asks for none. |
| `offset` | A fixed `(x, y)` shift after placement. |
| `grab` | `True` (the default) gives the popup the pointer and keyboard. |
| `geometry` | (0.6.3) `(x, y, width, height)` in the popup's own pixels: the popup proper, when the surface is bigger. The compositor places and constrains this rectangle and ignores the rest. Default: the whole surface. Must lie inside `size`. |
| `edge_margin` | (0.6.12) A whole number of pixels, or `(top, right, bottom, left)`, zero or more: the clearance the compositor keeps between the body and the output's edge when it flips or slides the popup (UX Popup Window: 8 dp). Default `None`: 0, what popups always did. Constructor only; `get("edge_margin")` reads it and `get("edge_margin_applied")` what fit. See [Edge margin](#edge-margin). |
| `first_present`, `background` | (0.6.12) As for a `LayerWindow` (see [First present and ready](#first-present-and-ready)); a popup's hold is described under **First present** below. |
| `input_region` | (0.6.5) Where input is delivered, in the popup's own logical pixels: `None` (the default, the whole surface), `[]`, a list of `(x, y, width, height)` rectangles (at most 256), or `"geometry"` for the body. Not `"auto"`. The one popup setting `set` can change. |

**Where it landed.** `configure` carries `width`, `height` and, for a popup, `x` and `y`: where the
compositor put it, in its parent's coordinates, after any flip or slide (`y` is negative when a
bottom bar's menu flipped above it). `popup.get("position")` reads the same, or `None` until placed. With a `geometry` the position is the body's.

**A shadow without the space (0.6.3, #266).** A shadow is drawn inside the popup's own surface, in a
transparent margin, because a surface cannot draw outside itself. Left to itself the compositor places
that whole surface, so the body lands a margin off the anchor and a popup near a screen edge flips a
margin early. Give the body as `geometry` and the compositor places, flips and slides the body alone:

```python
m = 20                                   # the shadow's reach
menu = PopupWindow(bar, size=(180 + 2 * m, 120 + 2 * m), geometry=(m, m, 180, 120),
                   anchor_rect=(100, 0, 60, 32))
body = menu.create("box", width=180, height=120, position="absolute", x=m, y=m,
                   fill=(0x2B, 0x29, 0x30, 0xFF), corner_radius=8,
                   shadows=[((0, 0, 0, 90), 0, 4, 16, 0)])
```

`anchor`, `gravity`, `offset` and `constraint_adjustment` then refer to the body, and `configure`'s
`x` and `y` are the body's. `width` and `height` in `configure` are the surface's (body plus margins).
If a compositor resizes the popup (`resize_x`, `resize_y`) it resizes the body, and the surface follows
with the same margins.

## Edge margin

**Edge margin (0.6.12).** The compositor flips and slides a popup until it is inside the output, with no
space left to the edge. The protocol has no margin for that, so `edge_margin` uses the one thing the compositor
does constrain: the window geometry. The geometry the compositor places is the body grown by the margin on each
side, and the anchor rectangle moves so the body still attaches where it would without a margin, flipped or
not. The margin is therefore the transparent room the surface keeps around the body (`geometry`): a side with
less room gets as much as there is (`get("edge_margin_applied")`, `(top, right, bottom, left)`), and a popup
with no `geometry` has none, since its whole surface is the body. A menu with a 10 px shadow margin and
`edge_margin=8` gets all 8. `position` and `x`, `y` in `configure` stay the body's.
Where it is inexact: an anchor rectangle thinner than the two margins on an axis where the popup is placed
outside it (`"bottom_left"` anchor with `"bottom_right"` gravity: below) is treated as a point in its middle;
`anchor="none"` with an edge gravity is not corrected, and a `"none"` gravity centres the body on the point
moved by half the difference of the margins.

```python
m = 20
menu = PopupWindow(bar, size=(180 + 2 * m, 120 + 2 * m), geometry=(m, m, 180, 120),
                   edge_margin=8, anchor_rect=(100, 0, 60, 32))
```

**Clicks in the margin (0.6.5, #282).** `geometry` places the body; it does not say where input goes,
and the transparent margin still takes clicks that were aimed at the window below. Give the popup an
`input_region` of the body and they pass through:

```python
menu = PopupWindow(bar, size=(180 + 2 * m, 120 + 2 * m), geometry=(m, m, 180, 120),
                   input_region="geometry", anchor_rect=(100, 0, 60, 32))
menu.set(input_region=[(m, m, 180, 60)])        # or rectangles, in the popup's own pixels; [] is nothing
```

With `grab=True` a click in the margin is outside the popup. If what lies under the margin is another
application's window or the bare desktop, the click ends the popup (`closed`, reason `"dismissed"`).
If it is **your own** window, the bar the popup hangs off included, that window gets the click and
the popup stays open (the protocol's rule for a grab, see Dismissal below): listen for the click there
and call `close()`. The region is not moved or scaled if a
compositor resizes the popup (`resize_x`, `resize_y`), so set it again from the `configure` size if you use those.

**Dismissal.** With `grab=True`, a click outside the application, or Escape, ends the popup:
`closed` with reason `"dismissed"`, and the click is the popup's. A click on **another window of
the same application** is delivered to that window as usual (that is the protocol's rule for a
grab), so a menu that should close on it listens for the click and calls `close()`. Closing a
popup closes the submenus hanging off it.

**First present (0.6.12).** `PopupWindow(..., first_present="on_ready" | <ms>, background=...)` and `popup.ready()`
work as for a `LayerWindow`: an `xdg_popup` is mapped by its first buffer, so a held popup is placed (`configure` reports
its `x`, `y` and size) and the configure is acknowledged, but nothing is shown until `ready()` or the deadline (counted
from the first configure). Build the menu's content while it is held, then call `ready()`: the popup appears complete.

* **The parent is not blocked.** The parent keeps drawing and taking input; a held popup is just an unmapped surface.
* **A grab and a hold.** With `grab=True` the grab is the compositor's to start when the popup is mapped. Checked on
  sway: a held popup with a grab is not dismissed while it is held, and maps normally at `ready()`. (A dismissal by a
  click outside after it maps is the existing behaviour, see Dismissal above; not re-tested with a hold.)
* **A parent that closes while the popup is held (or open) ends the popup first:** `closed` arrives on the popup with
  reason `"dismissed"` (the reason for a popup the compositor ends because its parent went), the popup's surface is
  destroyed before the parent's, and any popups hanging off it go the same way, deepest first. `popup.ready()` after
  that is a no-op. (Before 0.6.12 the popup of a closed parent was never told.) The same applies when the parent closes
  because the compositor closed it or its output went.
* A popup is not held because its parent is: it commits as it was made. Whether a compositor shows a popup whose parent
  layer surface is still unmapped is its own call (sway committed it); call `parent.ready()` first if the popup is to be
  shown with it.

The popup is a window like any other: its nodes, events, listeners, animation and input method
work as in a `LayerWindow`. Its own settings cannot be changed after it is made. Errors raise
`ValueError` before anything is sent; `UnsupportedPlatform` where there is no Wayland compositor
with the layer shell.

See [Menus from a bar](../guide/menus-from-a-bar.md).

## Not yet

Nothing in the `LayerWindow` and `PopupWindow` plan is outstanding.

## When the surface is hidden

A layer surface under a fullscreen window, or on an output that is off, gets no frame callbacks from compositors
that stop sending them (sway does; KWin keeps showing it), and draws nothing meanwhile, without stalling the app.
`window.get("occluded")` stays `False` for a layer window (no OS signal; X11 and macOS report it for ordinary
windows only). See [Hidden windows](window.md#hidden-windows).
