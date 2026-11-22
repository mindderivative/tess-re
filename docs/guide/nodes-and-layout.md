# Nodes and Layout

Everything on screen is a node in a window's tree. You create nodes, attach
them under one another, and set their layout properties; Tesserae Engine lays the tree
out with flexbox or CSS Grid ([taffy](https://github.com/DioxusLabs/taffy)) whenever it
changes, and paints it every frame.

## Windows and the app

```python
from tess_re import App, Window

window = Window(width=640, height=400, title="Inbox")
# ... build the tree under window.root ...

app = App()
app.add_window(window)
app.run()
```

A `Window` owns one tree. Its `root` is a flex row with 16px of padding and
16px gaps, sized to the window; restyle it like any node, with
`window.root.set(...)`. `App.run()` opens every added window and blocks until
all of them close. `run(max_frames=N)` stops each window after `N` frames,
which is how the examples exit on their own.

A window's size follows the OS window: listen for the `resize` window event
to react when the user resizes it. `window.resize(width, height)` changes it
from code.

## Creating and attaching nodes

`window.create(kind, **props)` makes a node and sets its properties in one
atomic step: a bad property raises `ValueError` and nothing is created.

| Kind | What it is |
| --- | --- |
| `"box"` | A layout container that can paint a fill, a stroke, rounded corners, and shadows |
| `"text"` | Shaped text — see [Text](text.md) |
| `"text_input"` | Editable text — see [Text](text.md#text-input) |
| `"image"` | Decoded RGBA8 pixels — see [Painting](painting.md#images) |
| `"path"` | A vector path — see [Painting](painting.md#paths) |
| `"canvas"` | Immediate-mode drawing — see [Painting](painting.md#canvas) |
| `"scroll_view"` | A clipped, scrollable viewport — see [below](#scrolling) |
| `"virtual_list"` | A list that builds only its visible rows — see [below](#virtual-lists) |
| `"terminal"` | A terminal emulator running a shell — see [Text](text.md#terminal) |

A new node is **detached**. Attach it with `add_child`, which appends, or
`insert_child(index, child)`:

```python
panel = window.create("box", flex_direction="vertical", gap=8, padding=12, width=240)
title = window.create("text", text="Inbox", font_size=22, width=200, height=28)
search = window.create("text_input", placeholder="Search", width=216, height=32)
panel.add_child(title)
panel.add_child(search)
window.root.add_child(panel)
```

Every property is on [Nodes and Properties](../reference/properties.md).

To give every node of a kind the same baseline look, declare it once with `window.set_defaults("text", font_size=14)` (or
`app.set_defaults(...)` for every window): nodes created afterwards start from it, and what `create` gets wins. See
[Defaults per kind](../reference/properties.md#defaults-per-kind-0611).

## Flex layout

A node lays out its children along `flex_direction` — `"horizontal"` (the
default) or `"vertical"` — separated by `gap` and inset by `padding`.
`align_items` places them across that axis and `justify_content` along it:

```python
toolbar = window.create("box", width="100%", height=48, padding=8, gap=8,
                        align_items="center", justify_content="space_between")
```

Sizes are pixels, `"auto"`, or a percentage of the parent: `width="50%"`. A text
node with no size (or `"auto"`, or `"fit-content"`) is the size of its text (see
[Sizing text to its content](text.md#sizing-text-to-its-content)).

**Mixed units (0.5.7).** `width` and `height` also take a percentage plus
pixels, written `"calc(100% - 16)"`, `"100% - 16"` or `"33% + 10"` (a space
around each `+` and `-`, as in CSS; `px` is optional): the whole width except a
strip, a third and ten pixels. The percentage is taken of the same box a plain
`"50%"` is: the parent's content box for a node in the flow, its padding box for
an absolute one. It follows the parent when the parent is resized, and reads back
as `"calc(100% - 16px)"`.

```python
bar = window.create("box", width="100% - 16", height=40)
```

The tree resolves it after laying out the parent and then lays out again, so it
costs a second layout pass on the frames where a parent changed size, and only
for trees that use it. Limits: it is for `width` and `height`, not `min_*` or
`max_*`; in a grid it is taken of the grid container, not the item's cell; and a
size its own parent depends on (a row as wide as its children, with a child that
is `"100% - 16"` of the row) has no answer, so it is left `auto` and a warning is
logged. For a strip stretched between two edges of an absolute node, `left` and
`right` (below) say it without a calculation.
As a flex child, a node can grow into spare room with `flex_grow`, shrink
with `flex_shrink`, and start from `flex_basis`; `align_self` overrides its
parent's `align_items` for it alone. A sidebar that keeps its width while the
content takes the rest:

```python
sidebar = window.create("box", width=200, flex_shrink=0)
content = window.create("box", flex_grow=1)
```

`padding` and `margin` take one value for every side, or set sides one at a
time with `padding_top`, `margin_left`, and so on. A `margin` of `"auto"`
takes up the free space on that side, which pushes a node to the far end of
its row.

`min_width`, `max_width`, `min_height`, and `max_height` bound a size, and
`aspect_ratio` fixes width over height. `flex_wrap="wrap"` lets children
flow onto more lines, and `align_content` spaces those lines. The
[property reference](../reference/properties.md) lists every layout
property and its values.

## Grid layout

`display="grid"` lays a node's children out in rows and columns instead of
one line, so their edges line up across rows — a settings form of labels and
fields, a gallery of equal cells, a dashboard of tiles. The columns and rows
are track lists: pixels, a percentage, `"auto"`, a share of the free space
(`"1fr"`), `"minmax(min, max)"`, and `"repeat(n, tracks)"`, as a string or a list;
a bare number is one track of that many pixels (`grid_auto_rows=96`):

```python
form = window.create("box", display="grid", width="100%",
                     grid_template_columns="auto 1fr", row_gap=8, column_gap=12)
for label, field in rows:
    form.add_child(label)   # column 1
    form.add_child(field)   # column 2, as wide as the rest of the form
```

Children fill the cells in order, a row at a time (`grid_auto_flow="column"`
fills columns instead). A child can be placed with `grid_column` and
`grid_row`: a start line (`2`; negative lines count from the end, so `-2`
is the last column's), a span (`"span 2"`), or
both ends (`"1 / 3"`). A gallery whose cells wrap to fit the width:

```python
gallery = window.create("box", display="grid", width="100%", gap=8,
                        grid_template_columns="repeat(auto_fill, minmax(96, 1fr))")
```

`row_gap` and `column_gap` space the rows and columns apart (`gap` sets both),
and `justify_items`/`justify_self` place a child across its cell's width the
way `align_items`/`align_self` do across its height. The full list is in
[Node properties](../reference/properties.md#grid-layout).

## Absolute positioning

`position="absolute"` takes a node out of the flow and places it at `x`/`y`
from its parent's padding box — for a badge over a corner, a drag preview, or
a scrim that fills its parent:

```python
badge = window.create("box", position="absolute", x=28, y=-4, width=16, height=16,
                      corner_radius=8, fill=(0xB3, 0x26, 0x1E, 0xFF))
icon_button.add_child(badge)
```

**Insets (0.5.7).** `left`, `top`, `right` and `bottom` (and `inset`, all four
at once) place the node from each edge of its parent; `x` and `y` are `left` and
`top`. Set both edges of an axis and leave that size `auto`, and the node is
stretched between them: a strip 8 pixels in from each side, or a scrim that fills
its parent:

```python
bar = window.create("box", position="absolute", left=8, right=8, bottom=8, height=40)
scrim = window.create("box", position="absolute", inset=0, fill=(0, 0, 0, 0x80))
```

The insets are measured from the parent's edge, not its padding, and they follow
the parent when it is resized. A margin shrinks the stretched node. A size that is
not `auto` wins over the second inset (`left=8, right=8, width="100%"` is the
parent's whole width). On a node in the flow the insets offset it from where it
was laid out, as in CSS (`right=8` moves it 8 pixels left).

Content that floats over the whole window — menus, dialogs, tooltips — goes
in a [layer](layers.md) instead, which also handles placement, input, and
focus.

## Anchored layout

**Placing a node against another node (0.5.7).** To put a badge on the corner of
a distant button, a label beside a field, or a marker on a chart point, give the
node an `anchor`:

```python
badge = window.create("box", width=16, height=16, corner_radius=8,
                      anchor=button, placement="top_end", offset=(-2, 2))
header.add_child(badge)   # any parent: the button need not be near it
```

After each layout the tree puts the node at `placement` of the anchor's box, in
the same pass, so it follows the anchor with no lag, through scrolling, scaling and
other transforms above either of them. The node stays an ordinary child of its own
parent, unlike a [layer](layers.md): it is clipped by `clip_children`, ordered by
`z_index` with its siblings, is in the parent's Tab order, and its events bubble to
its parent.

`placement` is `"below"` (the default), `"above"`, `"start"` or `"end"` (outside
the anchor, aligned to its start edge), or `"top_start"`, `"top_end"`,
`"bottom_start"`, `"bottom_end"` or `"center"`: the node's **centre** on that point
of the anchor. `offset=(dx, dy)` moves the point the node attaches to, in the
anchor's own coordinates (+x toward the end, +y down; a scaled anchor scales it).
`anchor=None` puts the node back in the flow.

Rules: setting an anchor makes the node absolute, and the tree writes its insets
after every layout, so `x`, `right` and the like on it have no lasting effect.
Unlike a layer it is never flipped or shifted to fit. A node cannot be anchored to
itself or to a node inside it (a `ValueError`); two nodes anchored to each other
have no answer, so they are put back in the flow and a warning is logged; a chain
(A to B to C) works. If the anchor is removed the node stays where it was. The
node's own `translate_*`, `scale` and `rotation_deg` apply on top of the place it
is put. The anchor's box is the bounds of its laid-out box through its transform.
`placement` and `offset` need an `anchor`, in the same call or before.

## Reading the layout

**The `layout` event (0.5.7).** To be told when a node's size changes, rather
than polling each frame, listen for `layout`. It fires when the node is first laid
out and whenever its laid-out size changes, whatever the cause: a `set`, the
parent being resized, a [calc size](#flex-layout) resolving, new text in a
`"fit-content"` label. `event.width` and `event.height` are the new size;
`event.old_value` is `None` the first time and the old `(width, height)` after.
Only a node with a listener is watched, so it costs nothing elsewhere. It doesn't
bubble, and arrives once a frame, after that frame's layout.

```python
panel.on("layout", lambda e: chart.set(width=e.width - 32, height=e.height - 32))
```

`layout_x`, `layout_y`, `layout_width`, and `layout_height` read a node's
computed box in window coordinates. Reading one runs any pending layout first,
so it always matches the tree as it is now:

```python
window.root.add_child(panel)
print(panel.get("layout_x"), panel.get("layout_width"))
```

A detached subtree is laid out on its own at its content size, so you can
measure a node before attaching it. `layout_x` and `layout_y` include the
node's transforms (`translate_x`, `scale`, ...) and its ancestors', so they
say where it's drawn; `layout_width` and `layout_height` are its size before
scaling. A transform never moves any other node.

## Changing the tree

| Method | Does |
| --- | --- |
| `add_child(child)` / `insert_child(index, child)` | Attach `child`, moving it if it's attached elsewhere |
| `set_children(children)` | (0.5.10) Replace the node's children in one call: listed nodes move here, the rest are detached |
| `children()` / `parent()` | Walk the tree |
| `remove()` | Detach the node; it stays alive while you hold a handle to it |
| `destroy()` | Free the node and its subtree now |

Moving a node keeps everything about it — its handle, listeners, focus, and
any running animation — so a list reorders by moving rows rather than
rebuilding them:

```python
for index, key in enumerate(new_order):
    list_box.insert_child(index, rows[key])
```

To make many nodes at once, `window.build(spec)` makes a whole detached subtree from nested dicts and
`window.apply([(node, props), ...])` changes many nodes in one call; both check everything first, so a typo
makes nothing and changes nothing (0.5.10; see [Building many nodes](../reference/properties.md#building-many-nodes-0510)
and `examples/build_tree.py`).

Switching screens is `old.remove()` then `window.root.add_child(new)`. The
removed screen keeps its scroll offsets, text, and selection, so attaching it
again brings it back as it was. A detached node you no longer hold is freed
automatically; `destroy()` frees one now. See
[Lifetime](../reference/node.md#lifetime).

## Visibility and stacking

`visible=False` hides a node and its subtree: it isn't painted, takes no
layout space, can't be hit or focused, and leaves the accessibility tree.
Siblings paint in order, later on top; `z_index` changes that without
reordering. `clip_children=True` clips a node's children to its rounded box.

## Scrolling

A `"scroll_view"` clips its content and scrolls it along its `orientation`,
`"vertical"`, `"horizontal"` or (0.6.11) `"both"`. Give it one content child that holds the rest:

```python
view = window.create("scroll_view", width=300, height=200)
content = window.create("box", flex_direction="vertical", width=300, gap=4)
for name in names:
    content.add_child(window.create("text", text=name, width=280, height=20))
view.add_child(content)
```

### Two axes (0.6.11)

`orientation="both"` scrolls the content on both axes, with the scrollbar of the Tesserae UX note
"Scrollbar and Scroll Container":

```python
view = window.create("scroll_view", orientation="both", width=400, height=300)
canvas = window.create("box", width=1600, height=1200)        # or no size: see below
view.add_child(canvas)
view.set(scroll_offset=120, scroll_offset_x=300)               # down and right; both animate
view.on("scroll", lambda e: print(e.x, e.y))                   # the offsets after the change
```

- **Offsets.** `scroll_offset` is the vertical offset, as on a `"vertical"` view; `scroll_offset_x` is the
  horizontal one. Both can be set, read and animated (`view.animate("scroll_offset_x", ...)`), and both are
  clamped to the range layout measures. A `"vertical"` or `"horizontal"` view behaves exactly as before:
  its `scroll_offset_x` stays `0`, and a horizontal view's offset is still `scroll_offset`.
- **The content is measured on both axes.** A content box with no size of its own is as large as what it
  holds, wide and tall (a row of wide children is as wide as they are together); a size you give it is the
  size the view scrolls.
- **One thumb per overflowing axis.** An axis shows its thumb if and only if its content is longer than the
  view, measured; the two axes are independent. The vertical thumb is at the right edge, the horizontal at
  the bottom. When both show, each lane stops 12 px short of the end (`scrollbar_corner`), so the thumbs never
  overlap, and the 12 × 12 corner is left empty. Thumb length is `max(32, lane × viewport ÷ content)`.
- **Defaults** are the spec's: thumb 8 px thick and pill-shaped, a 12 px hit strip centred on it, 2 px margin,
  at 0.70 opacity at rest, 0.80 with the pointer in the hit strip, 1.0 while dragged (fading over 100 ms), in the
  colour of the role `on_surface_variant` (`#49454F` in the light scheme, `#CAC4D0` in the dark one: the
  [colour scheme](painting.md#colour-scheme-0611)); each is a property (`scrollbar_fill`, `scrollbar_width`,
  `scrollbar_hit_width`, `scrollbar_margin`, `scrollbar_min_length`, `scrollbar_opacity`,
  `scrollbar_opacity_hover`, `scrollbar_opacity_drag`, `scrollbar_gutter`, `scrollbar_corner`; see
  [Scrollbar](../reference/properties.md#scrollbar-and-scroll-feel-0611)), so a project can theme it.
- **The gutter.** The 12 px at the end edge of an overflowing axis belongs to the thumb, so content does not
  sit under it. The engine does this where it can: the view's layout reserves the gutter on that side (taffy's
  scrollbar gutter, switched on for an axis once it overflows and off when it does not), so a content box sized
  by a percentage or an inset ends before the thumb, as it would with end padding. A content box with a size
  of its own is yours: if it is as wide as the view, its last 12 px are under the vertical thumb, so give it
  `width="100%"`, no width, or 12 px less. The gutter is read from the previous layout, so content that wraps
  differently once the gutter takes its width can take a layout or two to settle.
- **Wheel.** A notch is 40 px (0.6.11; it was 20, for every scroll view and virtual list; pixel deltas from a
  trackpad are taken as they come). The wheel moves the vertical axis; Shift+wheel, or a sideways wheel or
  trackpad, moves the horizontal; a diagonal one moves both. What the view cannot use (an axis at its end, or
  with nothing to scroll) goes on to the view around it.
- **Keys.** Up and Down move 40 px vertically, Left and Right 40 px horizontally; Page Up and Page Down move
  the viewport less 10% vertically, and with Shift horizontally; Home and End go to the start and end of the
  vertical axis. Everything else about keys is as in [Keyboard scrolling](#keyboard-scrolling): a key a
  `key_down` listener or a text input uses is not scrolled, Ctrl, Alt and Meta never scroll, and a key at the end
  of an inner view goes to the view around it. A `"vertical"` or `"horizontal"` view still pages by a whole
  viewport.
- **Pointer.** Pressing the thumb's hit strip grabs it, and the offset follows the pointer linearly,
  `start + Δ × (content − viewport) ÷ (lane − thumb)`, also once the pointer is outside the strip. Pressing the
  lane beside the thumb scrolls one page (the viewport less 10%) toward the press, and again after 400 ms and
  every 100 ms while it is held (the engine's own timer, so it needs a frame loop, or `Window.advance`), until the
  thumb reaches the press or the button is released.
- **`scroll_into_view`** (and focus) move each axis by the least that shows the node.
- **`scroll_snap`** settles the vertical (main) axis only.
- **Accessibility.** The thumbs are paint, not nodes: they are not in the accessibility tree and not a
  Tab stop. The scroll view is as before.
- A `virtual_list` is still vertical only, and takes the same scrollbar properties ([below](#theming-the-scrollbar-and-the-scroll-feel-0611)).

### Theming the scrollbar and the scroll feel (0.6.11)

Nothing about a scrollbar is fixed: every number, colour, side, timing and behaviour is a property of the scroll view or
virtual list, `None` for the spec's default. The full table, with types, defaults and units, is
[Scrollbar and scroll feel](../reference/properties.md#scrollbar-and-scroll-feel-0611). A `scroll_view` and a
`virtual_list` share one implementation, so the same properties give the same bar on both:

```python
for node in (view, rows):                      # a scroll view and a virtual list
    node.set(scrollbar_width=6, scrollbar_radius=3,
             scrollbar_fill=(202, 196, 208, 255), scrollbar_fill_hover=(255, 255, 255, 255),
             scrollbar_show="auto_hide", scrollbar_hide_delay_ms=800, scrollbar_fade_out_ms=250,
             scrollbar_side_v="start",           # the left edge in today's left-to-right layouts
             scrollbar_arrows="both", scrollbar_lane_click="jump",
             wheel_notch=60, key_line=24, scroll_smooth_ms=120)
window.set(reduced_motion=None)                # None follows the OS; True/False overrides it
```

- **Sides.** `scrollbar_side_v` puts the vertical bar on the right (`"end"`, the default, or `"right"`) or the left
  (`"start"`, `"left"`); `scrollbar_side_h` the horizontal one on the bottom (`"end"`, `"bottom"`) or the top
  (`"start"`, `"top"`). A bar's gutter is on its own edge: with the bar on the left or the top, the content starts
  that far in. The lane of one axis stops `scrollbar_corner` short at the end where the other bar is.
- **Showing.** `scrollbar_show` is `"overflow"` (the default), `"always"` (a full-length thumb even when nothing
  overflows), `"auto_hide"` (fades after `scrollbar_hide_delay_ms`, back in on scroll, hover and drag) or `"never"`
  (no bar and no gutter, still scrollable). `scrollbar_gutter_reserve=False` lets the thumb overlay content.
- **Arrows and lane clicks.** `scrollbar_arrows="both"` adds a button at each end of the lane that scrolls
  `scrollbar_arrow_step`, repeating while held; `scrollbar_lane_click` is `"page"`, `"jump"` (the thumb's middle goes
  to the press and the press carries on as a drag) or `"none"`.
- **Feel.** `wheel_notch`, `key_line` and `page_fraction` are the notch, the arrow key and the page; with
  `scroll_smooth_ms` above 0 the wheel and the keys ease (`scroll_easing`), and a held press repeats after
  `page_repeat_delay_ms` every `page_repeat_ms`.
- **Reduced motion.** `Window.set(reduced_motion=True)` takes the fades, the smooth scroll and the snap settle to 0 ms.
  `None` (the default) follows the OS.
- **One bar, two kinds.** A list that sets none of this keeps the bar it had, but every property works on it, with
  hover, drag and the lane click, exactly as on a scroll view.

### Snapping (0.5.6)

For a carousel, a tab strip or a picker, `scroll_snap` settles the view on an item:

```python
strip = window.create("scroll_view", orientation="horizontal", width=300, height=80,
                      scroll_snap="start")        # or "center" / "end" / "none"
row = window.create("box", flex_direction="horizontal")
strip.add_child(row)
for card in cards:
    row.add_child(card)                            # the items: children of the content box
cards[0].set(snap_align="center")                  # this one lines up differently
```

- The **snap points** are the content box's children, each lined up with the view's start,
  middle or end (its own `snap_align` if it has one), clamped to the scrollable range.
- The view **settles 120 ms after the last input** (wheel, touch pan, key, scrollbar drag) and
  once a touch fling has stopped, animating about 200 ms to the nearest point. More input
  during the wait puts it off; a new input during the settle takes it over.
- Your own `set(scroll_offset=)` and `animate("scroll_offset", ...)` are **not** snapped: they
  go where you say. `scroll_snap="none"` turns snapping off.
- It always settles ("mandatory"); a "proximity" mode and snapping a `virtual_list` are not
  provided. A virtual list does have a `scroll_offset` (settable, readable, animatable) to
  settle on a row yourself.
- Over a horizontal view a plain vertical wheel turn still scrolls the page around it (0.4.4);
  Shift+wheel, or a sideways wheel or trackpad, scrolls the strip.

The mouse wheel scrolls it, 40 px a notch (0.6.11; it was 20) — the nearest scroll view under the pointer that
can move the wheel's way. A view the wheel can't move passes it on to the one
outside it (scroll chaining, 0.4.4): a plain wheel over a horizontal carousel
scrolls the page around it, and so does a wheel over an inner list that's
already at its end, or has nothing to scroll. Shift+wheel scrolls a
horizontal view (0.4.3). So does setting `scroll_offset`, which also
animates — how a carousel eases to a snap point:

```python
view.animate("scroll_offset", 400, 300, easing=(0.2, 0.0, 0.0, 1.0))
```

An offset past the end is the end: `set` and `animate` clamp it to how far
the content reaches past the view (0.4.3), so it reads back right at once
and an animation eases all the way there.

### Keyboard scrolling

Keys scroll the nearest scroll view around the focused node (0.4.2): the arrow
keys by 40 pixels, Page Up and Page Down by a viewport, Home and End to either
end. Up, Down, Page Up, and Page Down move a vertical view, Left and Right a
horizontal one. A view that can't move a key's way — across its axis, at that
end, or with nothing to scroll — passes it to the next one out (0.4.4), so
Page Down at the end of an inner list moves the page. Keys the focused node uses itself don't scroll: a node with a
`key_down` listener between the focused node and the view keeps every key, and
a text input keeps all but Page Up and Page Down. With nothing focused, keys
scroll nothing. A key pressed with Ctrl, Alt, or Meta held is a shortcut, not a
scroll (0.4.3) — Ctrl+Page Down or Alt+Left stays yours — while Shift still
scrolls. Keyboard scrolls jump rather than ease, unless `scroll_smooth_ms` is set (0.6.11); the line (`key_line`)
and the page (`page_fraction`) are properties of the view too.

A `"virtual_list"` takes the same keys (0.6.11) with the same exclusions and the
same chaining, vertically only: Up and Down by 40 pixels (a list's rows may be any
height), Page Up and Page Down by the viewport less 10 percent, Home and End to
offset 0 and the furthest offset. Besides the keys and the wheel, its scrollbar
thumb can be dragged. The keys move the offset, not a selection or an active
row: the framework chooses which row is current. A focused row that has scrolled
out of the window stays built until focus moves off it, so the keys keep working.

### Revealing a node

`node.scroll_into_view()` scrolls every scroll view around `node`, innermost
first, by the least that shows it; a node longer than a view is aligned to its
start. Focus does the same, so Tab never lands on a node out of sight, and so
does an assistive technology's `scroll_into_view` request.

```python
rows[40].scroll_into_view()
```

A virtual list is one of those scrollers (0.6.11): focus moving to one of its
built rows, or `scroll_into_view()` on it, scrolls the list by the least distance
that shows the row. A row that is not built cannot be asked; use
`scroll_to_index`.

### Scrolling a virtual list to a row

`list.scroll_to_index(index, align="nearest")` (0.6.11) scrolls a virtual list so
row `index` shows, whether or not the row is built, at once; it returns whether
the offset changed. `align` is `"nearest"` (the least scroll that shows the whole
row; none if it is already shown; a row taller than the list goes to its top),
`"start"` (row top at the list's top), `"center"` or `"end"` (row bottom at the
list's bottom); the offset is clamped to the list's range. An `index` outside
`0..item_count`, an unknown `align`, or a node that is not a virtual list raises
`ValueError`. The offsets are exact: `index * item_extent`, or the sum of the
`size_hint` of the rows before it (`size_hint` is called for every row when the
list is first laid out, so a very long variable list pays that once).

```python
rows.scroll_to_index(5000, align="center")
```

### The `scroll` event

A scroll view or (0.6.11) virtual list fires `scroll` whenever its offset changes, whatever changed it:
the wheel, a key, `scroll_into_view`, focus, `set`, or an animation, once a
frame. `event.old_value` and `event.new_value` are the offsets before and
after. It doesn't bubble.

```python
view.on("scroll", lambda e: status.set(text=f"{e.new_value:.0f} px down"))
```

See `examples/scroll_keys.py`.

### Sticky headers

`node.set(sticky=inset)` (0.5.4) makes a node hold the start edge of the scroll view
around it, as CSS's `position: sticky` does: it scrolls with its content until it
reaches `inset` pixels from the top (the left, in a horizontal view), then stays
there while its own parent scrolls past, and is pushed out by the end of that parent.
Put a header in each section and give it `sticky=0`:

```python
for title, items in sections:
    section = window.create("box", flex_direction="vertical")
    header = window.create("box", height=32, fill=HEADER, z_index=1, sticky=0)
    section.add_child(header)
    ...
    content.add_child(section)
```

The node moves at paint time only: its layout position, and so its siblings', never
change, and hit testing and accessibility bounds follow where it is drawn. As in CSS,
content that comes later in the section paints over a stuck header unless the header
is above it, so give it a `z_index`. Details: it sticks within the nearest scroll view
or virtual list (a direct child of the view is the content itself and can't stick);
transforms between it and the view are not accounted for, and a sticky node inside
another sticky node ignores the outer one's movement. `sticky=None` makes it ordinary
again. See `examples/sticky_headers.py`.

## Virtual lists

A `"virtual_list"` has `item_count` rows but builds only the ones its viewport
shows. `materialize(index)` returns a node for a row as it scrolls into view;
rows scrolled away are released.

```python
def row(index):
    return window.create("text", text=items[index], height=32)

rows = window.create("virtual_list", item_count=len(items), item_extent=32,
                     materialize=row, width=320, height=400)
```

Every row is `item_extent` tall, or give `size_hint(index)` for rows of
different heights. A `materialize` or `size_hint` that raises is logged, and
its row stays empty. Changing `item_count` (or either callback) rebuilds the
visible rows. A virtual list scrolls by wheel, keys, thumb drag,
`scroll_to_index` and `scroll_into_view`, and fires `scroll`; see Keyboard
scrolling above. See `examples/scrollable_list.py` and
`examples/variable_height_list.py`.

### Selecting rows of a virtual list

(0.6.12) A virtual list can choose its own rows, so every list built on it has one set of rules for Shift, Ctrl
and the anchor (the Tesserae UX note "Selection", AC-SLT-1 to AC-SLT-6 and AC-SLT-13). It is off until you ask:
`selection_mode` is `"none"` by default, and then no click, key or Ctrl+A is looked at and nothing below applies.

```python
lst = window.create("virtual_list", item_count=100_000, item_extent=32, materialize=row,
                    width=320, height=400, selection_mode="multiple", select_all_key=True)
lst.on("selection_change", lambda e: print(e.count, e.anchor, e.selection_source))
lst.get("selection")          # [] -- the chosen rows' keys, ascending
lst.set(selection=[3, 4, 9])  # replaces the whole set; selection_change, source "api"
```

- **A row's key is its index** (`materialize(index)`); a virtual list has no other identity for a row. When the
  data changes under the list (an insert, a delete), the framework sets `selection` again: the engine keeps indices.
- **`selection_mode`**: `"single"` (at most one row) or `"multiple"`. `selection` (get and set), `selection_anchor`
  (get only: the row a range starts from, `None` when there is none), `select_all_key` (`False`), `select_on_focus`
  (`False`) and `unselectable_keys` (a list of keys, empty) are properties of the list; the table of every property
  is in the [Properties reference](../reference/properties.md#virtual-list). Setting `selection` checks every key
  (`ValueError` for a key outside `0..item_count`, an unselectable key, a second key in a `"single"` list, or any key
  in a `"none"` list) and changes nothing when it raises; it is checked against the rest of the same `set` call.
  Setting `selection_mode` to `"none"` empties the set; to `"single"` keeps one row (the anchor if it is chosen, else the first).
  A smaller `item_count` drops the rows past the end.
- **The set is stored as ranges**, not as a flag per row. Shift+End and Ctrl+A over a hundred thousand rows build no
  row: only the rows in the viewport (and the one holding focus) exist, and `event.count` is exact (AC-SLT-6). The
  engine draws nothing for a chosen row; the framework styles the rows it builds (it reads `selection`, and asks
  again on `selection_change`).

**Pointer** (`"multiple"`; Shift and Ctrl are the keys held when the primary button goes down; Cmd counts as Ctrl):

| Input | Result |
| --- | --- |
| Click | choose only this row; it is the anchor |
| Ctrl+click | toggle this row; it is the anchor |
| Shift+click | choose the rows from the anchor to this one, replacing the set; the anchor stays |
| Ctrl+Shift+click | add the rows from the anchor to this one |
| Click in the list's empty part (below the last row) | clear the set |
| Press on a chosen row of several, released without going `drag_start_px` | choose only this row, on the release |
| Press on a chosen row of several, and the pointer goes `drag_start_px` or more before the release | the set is left as it is: the app starts its drag with the whole selection |

A `"single"` list chooses the row clicked, whatever is held. Only the primary button chooses; a secondary press
(a context menu) does not.

**The drag rule (AC-SLT-13).** The pointer going `drag_start_px` (a [window property](../reference/window.md#gesture-click-and-drag-thresholds-0612))
from the press makes it a drag, and stays one even if the pointer comes back. With the default `drag_start_px=0`
no move is a drag, so a press on a chosen row always chooses that row alone on its release: set `drag_start_px=4`
(the spec's value) on a window whose rows start drags. The release must be on the row that was pressed, else the set
stays.

**Keys.** While focus is on a row of the list, or on the list itself (a row needs to be `focusable` for focus to be
in it; a list whose rows are not can be focusable itself and the engine keeps the row the keys are on), a
`"multiple"` list takes these, and does not also scroll by them (the list scrolls by the least distance that
shows the row focus lands on). A row not built yet is scrolled to, and takes focus as soon as it is built:
`focus` and `unfocus` fire then.

| Keys | Result |
| --- | --- |
| Up, Down, Home, End, Page Up, Page Down | move focus to the previous, next, first, last row, a page up or down; the set is unchanged (with `select_on_focus`, that row is chosen alone) |
| Ctrl + those keys | move focus only, in every case |
| Shift + those keys | move focus and choose the rows from the anchor to the focused row, replacing the set; the anchor stays (with none, the row focus was on) |
| Ctrl+Shift + those keys | the same, adding to the set |
| Space | toggle the focused row (it is the anchor if now chosen); no `click` |
| Ctrl+Space | the same |
| Shift+Space | choose the rows from the anchor to the focused row (Ctrl+Shift+Space adds) |
| Enter | not changed: a `click` on the focused row (it opens), the set stays |
| Ctrl+A | with `select_all_key`, choose every selectable row; not when focus is in a text input (that selects the input's text) |
| Escape | never clears the set (UX note "11 Interaction"): the key is not the list's |

A `"single"` list: Space and Enter choose the focused row (Enter is still the `click`); the arrows move focus
(and choose, with `select_on_focus`); Shift and Ctrl change nothing; there is no select all. Keys with Alt or Meta held
are shortcuts and are not the list's.

**Unselectable rows.** The rows of `unselectable_keys` cannot be chosen by a click, a key, a range or Ctrl+A (a range
passes them), and Up, Down, Home, End and the Page keys never put focus on one (AC-SLT-5); a chosen row that is made
unselectable leaves the set. The list cannot stop Tab or a click from focusing such a row: a framework that disables a
row also makes it not `focusable`.

**Keys that belong to a node on the way.** A text input (inside a row, say) keeps its keys, and so does a row with
its own `key_down` listener (a slider in a row), as for [keyboard scrolling](#keyboard-scrolling). A `key_down`
listener on the list itself does not stop the engine: it hears the keys of its rows and the engine still chooses.

**`selection_change`** fires on the list (it does not bubble) once for each key press, click or `set` that changed
the set, with the whole new set: `event.selection` (the keys, ascending), `event.ranges` (the same set as
half-open `(start, end)` pairs, which stays small for any size), `event.count`, `event.anchor` and `event.selection_source`
(`"pointer"`, `"keyboard"` or `"api"`). A call that leaves the set as it was fires nothing.
