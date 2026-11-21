# Painting

Tesserae Engine paints what you set and nothing else: no default borders, no focus
rings, no theme. Every node shares one set of paint properties; paths,
images, and canvases add their own. Everything here can be set with
`create` or `set`, and most of it [animates](animation.md).

## Color

A color is an `(r, g, b, a)` tuple of 0–255 ints, straight (not
premultiplied) alpha: `(255, 0, 0, 128)` is half-transparent red. Colors
default to transparent, except a text's and a text input's `fill`, which are
the window's [colour scheme](#colour-scheme-0611) until you set them. `get`
returns exactly the tuple you set.

## Boxes: fill, stroke, corners

```python
card = window.create("box", width=240, height=120,
                     fill=(0xFF, 0xFF, 0xFF, 0xFF),
                     stroke_color=(0xCA, 0xC4, 0xD0, 0xFF), stroke_width=1,
                     corner_radius=12)
```

- `fill` — the box's color, or a `Gradient` (see [Gradients](../reference/paint.md#gradients)).
  On a text it's the glyph color, and on a path the shape's fill.
- `stroke_color`, `stroke_width` — a border drawn inside the box, so it never
  changes layout.
- `corner_radius` — one radius, or `(top_left, top_right, bottom_right,
  bottom_left)`:

```python
sheet.set(corner_radius=(28, 28, 0, 0))  # rounded top, square bottom
```

- `corner_style` (0.6.4) — `"round"` (the default) or `"concave"`, or four of them in the
  same order as `corner_radius`. A concave corner curves inward: the box with a quarter
  disc about its own corner cut away, of the corner's `corner_radius`. A small box with one
  concave corner is the fillet that fills the inside corner where two shapes meet, and its
  `corner_radius` animates like any other. The fill, the border, the clip of an image or a
  shader, and the pointer's hit test and a layer window's input region all follow the notch;
  a click in it falls through. A shadow still takes its shape from the rounded corners
  alone, as it does for corners of differing radii.

```python
fillet = window.create("box", width=12, height=12, fill=ink,
                       corner_radius=(0, 0, 12, 0),
                       corner_style=("round", "round", "concave", "round"))
```

- `hit_path` (0.6.4) — SVG path data in the node's own pixels, or `None` (the default: the
  box). Pointer events, and an `input_region="auto"`, follow that shape in place of the box and
  let clicks through everywhere else, inside the node's box. A shader paints where the engine
  cannot see, so this is how a shader node says where it is:
  `join.set(shader=shape, hit_path="M0,0 H100 V30 H30 V100 H0 Z")`. It is set, not animated; a
  `path` node uses `hit_shape` instead.

## Outline and focus ring (0.6.11)

`stroke_color` and `stroke_width` draw inside the box, and two shadows cannot make a ring with a transparent gap. The
`outline_*` properties draw a ring **outside** the box instead: `outline_width` thick, `outline_offset` away from the
border box, in `outline_color` (default: the window scheme's `secondary`), following the corner radius.

```python
button = window.create("box", width=120, height=40, fill=fill, corner_radius=8, focusable=True,
                       # Shown only for keyboard focus; 3 px wide, 2 px off the control.
                       focus_outline_width=3, focus_outline_offset=2)
```

Tab to the button and the ring appears in the same frame; click it with the mouse and it does not. That is the
`focus_visible` of a `focus` event (see [Events and input](events-and-input.md)) applied by the engine, so a toolkit
sets the three numbers once (on every focusable node it creates, or through its defaults) and runs no code on focus.

- The gap between the box and the ring is transparent; `outline_gap_color` fills it (the surface colour, say).
- `outline_offset` may be negative: the ring then sits inside the box, which is how a control that touches the edge
  of a clipping container keeps a whole ring.
- The ring is above the node's own paint and its children, and no layout, hit test or accessibility node follows it.
- A clipping ancestor (`clip_children`, a `scroll_view`, a `virtual_list`) cuts the ring where it cuts everything
  else: give the container padding of at least `outline_offset + outline_width`, or use the inward ring.

Every property and the exact geometry: [Outline and focus ring](../reference/properties.md#outline-and-focus-ring-0611).

## Shadows

`shadows` is a list of `(color, offset_x, offset_y, blur, spread)` —
CSS `box-shadow`'s model, first listed on top. Two soft shadows give a
raised card:

```python
card.set(shadows=[((0, 0, 0, 77), 0, 1, 2, 0), ((0, 0, 0, 38), 0, 2, 6, 2)])
```

With no blur and a positive spread, a shadow is a ring outside the box —
a focus indicator that never shifts layout:

```python
button.set(shadows=[((0x67, 0x50, 0xA4, 0xFF), 0, 0, 0, 3)])
```

## Opacity and transforms

`opacity` fades a node **and its whole subtree together**, as one layer, so
overlapping children don't show through each other. `translate_x`,
`translate_y`, `scale` (about the center), and `rotation_deg` (clockwise
about the center) move what's painted without moving any other node — the pointer
still finds the node where it's drawn. Each animates on its own. `transform_origin`
(0.6.4) moves the point `scale` and `rotation_deg` turn about: a pair of fractions of
the node's box, `(0.5, 0.5)` (the center) by default, `(0, 0)` its top-left corner,
`(1, 1)` its bottom-right. A piece that grows from the corner it touches is
`scale=0.0` animated to `1.0` with `transform_origin=(0, 0)`. It is set, not
animated, and a pair outside 0 to 1 is a point outside the box.

## SVG documents

An `"svg"` node paints a whole SVG file, parsed once and fitted into the node's box without stretching. It is
what an icon set or a logo is, with gradients, clips, masks, dashes, group opacity and text:

```python
icon = window.create("svg", svg=open("battery.svg").read(), width=24, height=24,
                     svg_color=(0xE6, 0xE1, 0xE5, 0xFF))     # what currentColor resolves to
```

`svg_color` makes a monochrome icon follow your theme. Pictures inside the document come from the framework, not
a file the engine opens (`svg_images`). **From 0.5.10 a document's SMIL animation plays too**: `<animate>`, `<set>`
and `<animateTransform>` are sampled at `svg_time`, a node property you animate like any other, so a loading
spinner or a draw-on check mark needs no Python per frame. See [Animating an SVG](animation.md#animating-an-svg),
the [reference](../reference/paint.md#svg-documents) and `examples/svg_document.py`, `examples/svg_smil.py`.

## Paths

A `"path"` node draws SVG path data — the `d` attribute. With a `view_box`,
the data is fitted into the node's box the way an SVG `viewBox` is, so an
icon from any icon set drops straight in:

```python
check = window.create("path", data="M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4z",
                      view_box=(0, 0, 24, 24), width=24, height=24,
                      fill=(0x1D, 0x1B, 0x20, 0xFF))
```

A path has a `fill`, a `stroke_color` and `stroke_width` (in pixels however
the view box scales), and `trim_start`/`trim_end`, which draw only part of
the stroke. Animating `trim_end` from 0 to 1 draws a line on; moving both
ends together makes a spinner:

```python
ring = window.create("path", data="M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20",
                     view_box=(0, 0, 24, 24), width=48, height=48,
                     stroke_color=(0x67, 0x50, 0xA4, 0xFF), stroke_width=4,
                     trim_end=0.0)
ring.animate("trim_end", 1.0, 1000)
```

Animating `data` morphs one shape into another — see
[Animation](animation.md#morphing-paths).

## Images

An `"image"` node shows straight-alpha RGBA8 pixels you've already decoded.
Tesserae Engine never reads image files, so any decoder works:

```python
from PIL import Image

img = Image.open("photo.jpg").convert("RGBA")
photo = window.create("image", rgba=img.tobytes(), pixel_width=img.width,
                      pixel_height=img.height, width=200, height=150, fit="cover")
```

`fit` is `"fill"` (stretch), `"contain"` (letterbox), or `"cover"` (crop).
Video is the same node with each frame set as it arrives:
`photo.set(rgba=frame, pixel_width=w, pixel_height=h)`.

## Canvas

A `"canvas"` node draws with code. Its `draw` callback gets a
[`Painter`](../reference/painter.md) and runs when the canvas is created,
when `draw` is set, and whenever you call `canvas.redraw()` — never on its
own, so redraw when your data changes:

```python
def draw(painter):
    painter.fill_rect(0, 0, 200, 100, (0xF7, 0xF2, 0xFA, 0xFF))
    for i, value in enumerate(samples):
        painter.fill_circle(10 + i * 20, 100 - value, 4, (0x67, 0x50, 0xA4, 0xFF))
    painter.stroke_path([[10, 90], [100, 20, 190, 90]], (0x1D, 0x1B, 0x20, 0xFF), 2.0)

chart = window.create("canvas", width=200, height=100, draw=draw)
```

`stroke_path` takes a start point, then lines (`[x, y]`), quadratic curves
(`[cx, cy, x, y]`), and cubic curves (`[c1x, c1y, c2x, c2y, x, y]`). A canvas
is hit as its whole box unless its painter narrows that with
`set_hit_test_circle` or `set_hit_test_path` — so a click on a chart's point
can land on that point alone.

## Redrawing

A window redraws only what changed each frame (0.4.0): Tesserae Engine compares the
frame with the last one and repaints the changed areas, or the whole window
when most of it changed. Nothing in your code has to say what changed.
`window.set(partial_redraw=False)` redraws the whole window every frame
instead, and `window.get("partial_redraw_active")` says whether the open
window really redraws partially (`None` before `App.run()` opens it).
`window.set(show_damage=True)` (0.4.1) tints each frame's redrawn areas, for
debugging. See [Window](../reference/window.md).

## Colour scheme (0.6.11)

The few colours the engine draws by itself are not constants: they are roles of the Tesserae UX specification's
colour table (the Material 3 baseline, seed `#6750A4`), and the window's **scheme** says which of the two baselines,
`"light"` or `"dark"`, they come from.

```python
window = Window(scheme="dark")          # "light", "dark", or "auto" (the default: follow the OS)
window.set(scheme="light")              # live: the window repaints
App(scheme="dark")                      # the default for every window that sets none
window.get("scheme_resolved")           # "light" or "dark": what "auto" came to
window.get("dark")                      # still the OS flag, unchanged
base = tess_re.scheme_colors("dark")    # {"on_surface": (230, 224, 233, 255), ...}: all 49 roles
window.set(scheme_colors={"on_surface": (255, 255, 255, 255)})   # override single roles
```

`"auto"` follows the OS appearance flag (`window.get("dark")`, kept up to date by the `color_scheme` event) and is
light while the platform cannot say, and for a window that is not open yet. A window with no scheme of its own takes
the `App`'s (`App(scheme=...)`, `app.set(scheme=...)`, default `"auto"`); `window.set(scheme=None)` gives a window back
to the app. A `PopupWindow` follows the app's scheme like any other window: give it its parent's with `scheme=`.

**What the scheme covers** (each only while the node sets no colour of its own):

| Default | Role | Light | Dark |
| --- | --- | --- | --- |
| a `text`'s glyphs (`fill` never set) | `on_surface` | `#1D1B20` | `#E6E0E9` |
| a `text_input`'s text (`fill` never set) | `on_surface` | `#1D1B20` | `#E6E0E9` |
| its placeholder (no `placeholder_fill`) | `on_surface_variant` | `#49454F` | `#CAC4D0` |
| its caret (no `caret_color`) | `primary` | `#6750A4` | `#D0BCFF` |
| its selection (no `selection_fill`), and a selected static `text` | `primary` at 30% alpha | `#6750A4` | `#D0BCFF` |
| a `scroll_view`'s thumb (no `scrollbar_fill`) | `on_surface_variant` | `#49454F` | `#CAC4D0` |
| a terminal's foreground (no `palette["foreground"]`) | `on_surface` | `#1D1B20` | `#E6E0E9` |
| its cursor (no `palette["cursor"]`), at 50% alpha so the glyph under it shows | `primary` | `#6750A4` | `#D0BCFF` |
| its selection (no `palette["selection"]`) | `primary` at 30% alpha | `#6750A4` | `#D0BCFF` |

**What stays explicit:** a colour you set (`fill`, `placeholder_fill`, `caret_color`, `selection_fill`,
`scrollbar_fill`, `palette`) is never changed by the scheme. A text input that has a `fill` of its own keeps the
older derived defaults (placeholder the text colour at 60%, caret the text colour, selection the text colour at 30%),
because they follow its colour. The terminal's 16 ANSI colours and its default background (clear, so the node's own
fill shows) are the same in both schemes. Everything else the engine paints has no default of its own: boxes, paths,
strokes, shadows and the window itself are transparent until you set them, so a window has no background colour
until you give its root a `fill`. Start from `tess_re.scheme_colors(...)` or `window.get("scheme_colors")` (the
baseline with your overrides): `surface`, `surface_container` and the other roles are there for a toolkit to use.

`set(fill=None)` on a `text` or `text_input` gives a set colour back to the scheme; animating `fill` starts from the
colour in force. `set(scheme_colors={...})` replaces the window's overrides (`None` or `{}` clears them) and raises
`ValueError`, listing the valid role names, for an unknown one; the overrides stay across a change of scheme.

## State layers (0.6.11)

A toolkit's hover, pressed and keyboard-focus feedback is a translucent overlay in the content colour over the control.
Declare it once and the engine draws and fades it:

```python
ON_SURFACE = (29, 27, 32)
button.set(hover_layer=(*ON_SURFACE, 20),    # 8 %
           pressed_layer=(*ON_SURFACE, 26),  # 10 %
           focus_layer=(*ON_SURFACE, 26))    # 10 %, keyboard focus only
```

The overlay is drawn over the node's fill and border and under its children, inside its rounded corners; one layer shows
at a time (pressed, then focus, then hover) and changes fade over `state_layer_ms` (default 100) on `state_layer_easing`
(default `"standard"`), instantly under reduced motion. A disabled node draws none. All the rules and defaults are in
[State layers](../reference/properties.md#state-layers-0611).

## What Tesserae Engine leaves to you

The colors Tesserae Engine draws for you all have properties: a text's and a text input's `fill`, a text input's
`placeholder_fill`, `caret_color`, and `selection_fill`; a scroll view's or virtual list's
`scrollbar_fill` (and its hover, drag and track colours), `scrollbar_width`, `scrollbar_radius` and the other
`scrollbar_*` measures; a terminal's `palette`; and their defaults come from the window's
[colour scheme](#colour-scheme-0611). Hover and
press feedback, focus rings, ripples, and disabled styling are yours to draw
from [events](events-and-input.md) — [Building a Widget](building-a-widget.md)
shows how.
