# Nodes and Properties

Every node comes from `window.create(kind, **props)`, and every property is read and
written through [`node.set`, `node.get`, and `node.animate`](node.md#set-get-and-focus).
This page lists them all. Paint, paths, shadows, and easing have their own page,
[Paint, Paths, and Animation](paint.md); events are in
[Events and Listeners](events.md).

## Kinds

| Kind | Required | What it is |
| --- | --- | --- |
| `"box"` | — | A layout container with an optional fill, stroke, corners, and shadows |
| `"text"` | `text` | Shaped, non-editable text |
| `"text_input"` | — | Editable text: selection, clipboard, IME, syntax spans, folding, obscuring |
| `"image"` | `rgba`, `pixel_width`, `pixel_height` | Decoded RGBA8 pixels; video is repeated `set(rgba=..., ...)` |
| `"path"` | `data` | A vector path — see [Paths](paint.md#paths) |
| `"svg"` | `svg` | A whole SVG document — see [SVG documents](paint.md#svg-documents) |
| `"canvas"` | `draw` | Immediate-mode drawing through a [painter](painter.md) |
| `"scroll_view"` | — | Clips and scrolls one child |
| `"virtual_list"` | `item_count`, `materialize`, and `item_extent` or `size_hint` | Builds only the rows its viewport shows |
| `"terminal"` | `shell`, `cols`, `rows` | A PTY-backed terminal emulator |

```python
title = window.create("text", text="Inbox", font_size=22, font_weight=500)
field = window.create("text_input", placeholder="Search", width=240)
```

`create` makes a **detached** node: attach it with `add_child` or
`insert_child`. Until it's attached, it's freed once no handle points into it
(see [Lifetime](node.md#lifetime)). A text's and a text input's `fill` have no value of their own until set:
the window's [colour scheme](../guide/painting.md#colour-scheme-0611) gives them `on_surface` (`#1D1B20` in the
light scheme, `#E6E0E9` in the dark one); every other color defaults to transparent.

### Building many nodes (0.5.10)

`window.build(spec)` makes a whole detached subtree from one nested description and returns the root and
a dict of the nodes that have an `id`; `window.apply(changes)` changes properties on many nodes in one call.
Both check everything first, as `create` and `Node.set` do for one node: a bad description or entry raises
`ValueError` naming where, and nothing is made or changed. (A callback that raises while they run, such as a
canvas `draw`, is not a bad value: it keeps its own error, `build` frees what it had made, and `apply` stops where it was.)

```python
root, nodes = window.build({
    "kind": "box", "id": "row",
    "props": {"flex_direction": "horizontal", "gap": 8},
    "children": [
        {"kind": "text", "id": "battery", "props": {"text": "Battery 87%", "font_size": 14}},
        {"kind": "text", "props": {"text": "Charging"}},
    ],
})
parent.add_child(root)
window.apply([(nodes["battery"], {"text": "Battery 86%"}), (clock, {"text": "14:06"})])
```

A node is a dict with `kind` (required), `props` (what `create(kind, **props)` takes), `children` (a list
of nodes) and `id` (a unique non-empty `str`); any other key, a bad kind or property, a duplicate `id` or
nesting over 256 levels raises `ValueError` naming the path (`spec.children[2].props`). `build` parses
every node first and makes them only when all are valid, so a spec with a `draw` canvas and a typo later
calls no `draw` and starts no terminal shell (terminals are made last); a shell that cannot start raises the
`OSError` `create` raises and frees the rest. A call takes at most 1,000,000 nodes. At most one entry of an `apply`
may set `shader`. `apply` takes `(node, props)` pairs
and applies them in order once all are checked (the same node twice: the later wins). **Speed:** these save
Python-to-Rust calls, not time: layout already runs once per frame however many nodes change, and on a
3,100-node tree `build` took 5.3 ms against 4.9 ms of per-node calls, `apply` of 1,000 entries 0.33 ms
against 0.33 ms (`tools/bench_tree_build.py`; llvmpipe, medians). The reason to use them is the all-or-nothing check.
`set_children` is cheap for appends and removals; a reorder moves nodes one at a time and is not (2,000 reversed: 0.63 ms).
Appending a child no longer gets slower with every sibling already there: 0.12 µs a call at 20,000
siblings, 0.13 at 1,000 (it was 10.9 and 0.63).

## Size and position

Every node.

| Property | Value |
| --- | --- |
| `width`, `height` | A number of pixels, `"auto"`, or a percentage such as `"50%"`; (0.5.7) a percentage plus pixels such as `"calc(100% - 16)"` ([Mixed units](../guide/nodes-and-layout.md#flex-layout)); on a text node also `"fit-content"`, the size of its text |
| `min_width`, `min_height`, `max_width`, `max_height` | A number of pixels, `"auto"`, or a percentage such as `"50%"` |
| `aspect_ratio` | Width over height, or `None` |
| `position` | `"relative"` (in the flow) or `"absolute"` |
| `x`, `y` | The left and top offset: a number, a percentage, or `"auto"`/`None`. The same as `left` and `top` |
| `left`, `top`, `right`, `bottom` | (0.5.7) An inset from that edge of the parent: a number, a percentage, or `"auto"`/`None`. On an absolute node with both edges of an axis set and that side's size `auto`, the node is stretched between them. See [Absolute positioning](../guide/nodes-and-layout.md#absolute-positioning) |
| `inset` | (0.5.7) All four insets at once; reads back as one value, or `(top, right, bottom, left)` when they differ |
| `anchor` | (0.5.7) A node of the same window to place this node against, or `None` (the default). While set the node is absolute and the tree places it after each layout. See [Anchored layout](../guide/nodes-and-layout.md#anchored-layout) |
| `placement` | (0.5.7) With an `anchor`: `"below"` (default), `"above"`, `"start"`, `"end"`, or a point of the anchor to centre on: `"top_start"`, `"top_end"`, `"bottom_start"`, `"bottom_end"`, `"center"`. Reads back `None` with no anchor |
| `offset` | (0.5.7) With an `anchor`: `(dx, dy)` moves the point the node attaches to, in the anchor's own coordinates |

## Flex layout

On a node with children:

| Property | Value |
| --- | --- |
| `flex_direction` | `"horizontal"` or `"vertical"` |
| `flex_wrap` | `"no_wrap"` or `"wrap"` |
| `align_items` | `"start"`, `"end"`, `"flex_start"`, `"flex_end"`, `"center"`, `"baseline"`, `"stretch"` |
| `justify_content` | `"start"`, `"end"`, `"flex_start"`, `"flex_end"`, `"center"`, `"stretch"`, `"space_between"`, `"space_around"`, `"space_evenly"` |
| `gap` | Between children: a number or a percentage |
| `padding`, `padding_top`, `padding_right`, `padding_bottom`, `padding_left` | A number or a percentage |

As a flex child, any node:

| Property | Value |
| --- | --- |
| `flex_grow`, `flex_shrink` | Non-negative numbers |
| `flex_basis` | Like `width` |
| `align_self` | One of `align_items`' values, or `None` to follow the parent |
| `margin`, `margin_top`, `margin_right`, `margin_bottom`, `margin_left` | A number, a percentage, or `"auto"` |

`padding` and `margin` read back as one value while all four sides agree, and as
`(top, right, bottom, left)` once they don't.

## Grid layout

0.4.2. On a node with `display="grid"`:

| Property | Value |
| --- | --- |
| `display` | `"flex"` (the default) or `"grid"` |
| `grid_template_columns`, `grid_template_rows` | A track list, as a string or a list: pixels, a percentage, `"auto"`, `"1fr"`, `"min_content"`, `"max_content"`, `"minmax(min, max)"`, `"fit_content(limit)"`, and `"repeat(n, tracks)"` with `n` a count, `"auto_fill"`, or `"auto_fit"` -- e.g. `"200 1fr auto"`, `[120, "1fr"]`, or a bare number, one track of that many pixels (`96`, 0.4.3) |
| `grid_auto_columns`, `grid_auto_rows` | The size of tracks made for children placed past the template: a track list without `repeat`, or a bare number (`grid_auto_rows=96`, 0.4.3) |
| `grid_auto_flow` | `"row"` (the default), `"column"`, `"row dense"` (or `"dense"`), or `"column dense"` |
| `row_gap`, `column_gap` | Between rows or columns: a number or a percentage; `gap` sets both |
| `justify_items` | `align_items`' values, across each child's cell |
| `align_content` | `justify_content`'s values, for the rows as a whole when they don't fill the height |

As a grid child, any node:

| Property | Value |
| --- | --- |
| `grid_column`, `grid_row` | A line (`2`, or `-1` counting from the end), `"span 2"`, `"auto"`, or `"start / end"` such as `"1 / 3"` |
| `justify_self` | One of `align_items`' values, or `None` to follow the parent |

Track lists and placements read back as strings in this syntax. Named grid
lines and template areas aren't supported yet.

## Transform, visibility, and order

Every node.

| Property | Value |
| --- | --- |
| `translate_x`, `translate_y` | Pixels (animatable) |
| `scale` | A factor about `transform_origin` (animatable) |
| `rotation_deg` | Degrees clockwise about `transform_origin` (animatable) |
| `transform_origin` | (0.6.4) `(x, y)`, fractions of the box that `scale` and `rotation_deg` turn about; `(0.5, 0.5)`, the center, by default. Set, not animated; see [Painting](../guide/painting.md) |
| `visible` | `False` hides the node and its subtree: not painted, not hit, no layout space, not in the accessibility tree or tab order |
| `z_index` | Paint and hit order among siblings: higher is on top; equal values keep child order |
| `clip_children` | `True` clips children to the node's rounded box |
| `hit_path` | (0.6.4) SVG path data in the node's own pixels, or `None`: pointer events and an `input_region="auto"` follow that shape in place of the box (how a [shader](shader.md) node says where it is); see [Painting](../guide/painting.md) |
| `mask` | `None`, `"circle"`, `{"rounded": radius}`, or `{"path": data, "view_box": (x, y, w, h)}` (0.5.6) — clips the node and its subtree to the shape; see [Masks](paint.md#masks-056) |
| `sticky` | (0.5.4) a number of pixels, or `None`: holds that far from the start edge of the scroll view it is in ([Sticky headers](../guide/nodes-and-layout.md#sticky-headers)) |

Each transform part animates on its own, so easing one never disturbs another.

## Text

On `text` and `text_input` unless noted.

| Property | Value |
| --- | --- |
| `text` | The content. Setting it on a text input puts the caret at the end, and fires no `change` event: that's for edits the user makes |
| `font_family` | A family name — see [`register_font`](../api/python.md#module) |
| `font_weight` | 1–1000 |
| `font_size` | Pixels (also on a terminal) |
| `line_height` | A multiple of the font size, or `None` for the font's own (`text`; and `text_input` since 0.6.11) |
| `fill` | The glyph colour, `(r, g, b, a)` or a `Gradient`. (0.6.11) `None` (the default for a node that was never given one) is the scheme's `on_surface`: `#1D1B20` light, `#E6E0E9` dark, resolved at paint time, so a window that switches scheme repaints it. `get("fill")` reads back the colour in force (the scheme's, until set). A fill you set is never changed by the scheme |
| `text_align` | `"start"`, `"center"`, or `"end"` (`text` only) |
| `font_style` | `"normal"` or `"italic"` — slanted when the family has no italic face (`text` only) |
| `letter_spacing` | Extra pixels after each character, default `0` (`text`; and `text_input` since 0.6.11) |
| `vertical_align` | (0.6.11) Where the lines sit in the content box when it is taller than they are: `"start"`, `"center"` or `"end"`. `text`: default `"start"` (as before). `text_input`: `None` (default) is `"center"` for a single-line input and `"start"` for a multiline one; text in a 32 px field with padding 4 no longer sits high. Text taller than the box starts at the top. The caret, selection rectangles, `caret_rect()`, `text_rects()`, the IME's area and a click's caret position all follow the offset. Example: `field.set(vertical_align="start")` for the old look |
| `wrap` | `"word"` wraps within the node's width; `"none"` keeps each paragraph on one line. On a text input (0.6.6) it is for a multiline one: see [Text input](#text-input) |
| `max_lines` | The most lines shown, or `None` (`text` only) |
| `overflow` | `"clip"` cuts text past the box or the line limit; `"ellipsis"` ends each cut line with "…". On a text input (0.6.6) it is for a single-line one, and takes `"scroll"` too: see [Text input](#text-input) |
| `spans` | (0.5.4) `[(start, end, style), ...]`: UTF-8 byte ranges styled with a dict of `color`, `weight`, `italic`, `underline`, `strikethrough`, `font_size`, `font_family`, `link` (`text` only; later spans win where they overlap) |
| `selectable` | (0.5.4) `True` lets a press and drag select the text and Copy take it (`text` only; default `False`) |
| `fill` | The glyph color (animatable) |

A text node with no `width` or `height` is the size of its text (0.6.2, #263; `"auto"`
and `"fit-content"` mean the same). To size it by hand, give it a number; or use
[`window.measure_text`](window.md#measure_text), which lays text out exactly as
a text node with the same properties paints it, with no extra pixels:

```python
style = dict(font_size=14, max_lines=1, overflow="ellipsis")
width, height = window.measure_text("A long list item title", max_width=180, **style)
title = window.create("text", text="A long list item title", width=180, height=height, **style)
```

A text node's explicit `width`, `min_width` and `max_width` are laid out rounded
**up** to a whole pixel (0.5.3), so a width from `measure_text` always fits its
text; `get` reads back the value you set and `layout_width` the rounded one.

**`padding`** makes a content box, and a node's own content draws inside it
(0.5.1): a text node's text, a text input's text and caret, a terminal's cells,
an image (fitted into it), a path (its view box fitted into it), and a canvas
(whose painter coordinates start at the padding). The node's `width` and
`height` include the padding, as for a box, and text wraps in what's left. The
node's own background, border and rounded corners stay on its whole box, and
its children are laid out inside the padding, as before. Pointer events' `x`
and `y` stay relative to the node's own corner, so subtract the padding to
match a canvas's painter coordinates. Clicks and hit tests land on what is
painted.

## Text input

| Property | Value |
| --- | --- |
| `multiline` | `True` for several lines |
| `overwrite` | (0.6.7) `True` for overwrite mode: a typed character replaces the one after the caret. The Insert key toggles it and fires `overwrite_change`. Default `False`. See [Keys, rows and clicks](../guide/text.md#keys-rows-and-clicks-in-a-text-input-067) |
| `wrap` | (0.6.6) multiline: `"word"` breaks lines at the width, `"none"` keeps each whole and scrolls sideways, `None` (default) is as before: no wrap, and a monospace scroll. Means nothing to a single-line input |
| `overflow` | (0.6.6) single-line: `"scroll"` keeps the caret in view, `"ellipsis"` cuts a long line with "…" until the input is focused, `"clip"` cuts it at the box edge; any of them makes it one line. `None` (default) is as before: the line wraps by word. Means nothing to a multiline input. See [Long lines](../guide/text.md#long-lines-in-a-text-input-066) |
| `selection` | `(start, end)` byte offsets; `start == end` is a caret. Given with `text`, it selects in the new text |
| `show_whitespace` | Draws spaces and tabs as visible marks |
| `syntax_spans` | A list of `(start, end, color)` byte ranges |
| `folded_ranges` | A list of `(start, end)` byte ranges drawn as an ellipsis |

Plus the colors on [Paint, Paths, and Animation](paint.md#text-inputs-scroll-views-and-terminals). (0.6.11) A
text input's `fill` is its text colour, and until it is set the window's
[colour scheme](../guide/painting.md#colour-scheme-0611) decides it, with the placeholder, caret and selection:
`on_surface`, `on_surface_variant` and `primary`. `set(fill=None)` gives a set fill back to the
scheme.

## Image

| Property | Value |
| --- | --- |
| `rgba`, `pixel_width`, `pixel_height` | Straight-alpha RGBA8 pixels and their size — always set together |
| `fit` | `"fill"`, `"contain"`, or `"cover"` |

## Canvas

| Property | Value |
| --- | --- |
| `draw` | `draw(painter)`, called when the canvas is created, when `draw` is set, and on `node.redraw()` |

## Scroll view

| Property | Value |
| --- | --- |
| `orientation` | `"vertical"` (default), `"horizontal"` or (0.6.11) `"both"`. A `"vertical"` or `"horizontal"` view scrolls along one axis; `scroll_offset` on a vertical view is its vertical offset, and a sideways strip needs `orientation="horizontal"` (chosen on the scroll view, not by its content's `flex_direction`). A `"both"` view scrolls along both and has the scrollbar of the Tesserae UX spec "Scrollbar and Scroll Container" (see [Scrollbar](#scrollbar-and-scroll-feel-0611)); its `scroll_offset` stays the vertical offset |
| `scroll_snap` | (0.5.6) `"none"` (default), `"start"`, `"center"` or `"end"`: when the user stops scrolling, the view settles on the item nearest that alignment. See [Scrolling](../guide/nodes-and-layout.md#scrolling) |
| `scroll_offset` | Pixels scrolled; on a scroll view or (0.5.6) a virtual list; any other node raises `ValueError` (animatable, so a carousel can ease to a snap point). `set` and `animate` clamp it to the view's range at once — past the end reads back as the end (0.4.3); an offset given to `create`, before the content is attached, is clamped at the first layout. On a `"both"` view it is the vertical offset |
| `scroll_offset_x` | (0.6.11) Pixels scrolled to the right, on a scroll view; settable, readable and animatable like `scroll_offset`, and clamped the same way. Only a `"both"` view has a horizontal range: on a `"vertical"` or `"horizontal"` view it is always `0` (a horizontal view's offset is `scroll_offset`) |

### Scrollbar and scroll feel (0.6.11)

Every aspect of a scrollbar, and of how a scroll view feels, is a property of a `scroll_view` **and** of a
`virtual_list`: one implementation of the geometry, hit-testing, hover, drag and fade serves both (a list's bar used
to be fixed). The tree draws what you set; nothing is a callback. Each takes a value of the type below, or `None` for
the default, and `get` reads the value in force (a colour or a side reads what was set, `None`: the default). Set one on
a node, or on a family of nodes from your own defaults table: every property is per node.

The defaults are the Tesserae UX note "Scrollbar and Scroll Container" for a `"both"` view. A `"vertical"` or
`"horizontal"` view and a virtual list keep the scrollbar they had before (the *one axis / list* column) so nothing
changes for them unless you set a property. A durations column is milliseconds; a number of pixels is a layout pixel.

**Measures and colours**

| Property | Type | Default, `"both"` | Default, one axis / list | Meaning, example |
| --- | --- | --- | --- | --- |
| `scrollbar_fill` | colour `(r, g, b, a)` | `#CAC4D0` opaque (the role `on_surface_variant`) | `#CAC4D0` at 55% | The thumb's colour; its own alpha is multiplied by the opacity below. `scrollbar_fill=(202, 196, 208, 255)` |
| `scrollbar_fill_hover` | colour | = `scrollbar_fill` | = `scrollbar_fill` | The colour with the pointer in the hit area (fades from the rest colour). `scrollbar_fill_hover=(255, 255, 255, 255)` |
| `scrollbar_fill_drag` | colour | = `scrollbar_fill` | = `scrollbar_fill` | The colour while the thumb is dragged. `scrollbar_fill_drag=(103, 80, 164, 255)` |
| `scrollbar_width` | px, ≥ 0 | 8 | 4 | The thumb's thickness, across its axis. `scrollbar_width=6` |
| `scrollbar_radius` | px, ≥ 0 | 4 (a pill, the thickness ÷ 2) | 2 | The thumb's and the track's corner radius. `scrollbar_radius=0` is square |
| `scrollbar_hit_width` | px, ≥ 0 | 12 | the thickness plus 12 | The pointer strip across the axis, centred on the thumb: a press in it grabs the thumb, one along the lane outside the thumb is a lane click. `scrollbar_hit_width=24` |
| `scrollbar_margin` | px, ≥ 0 | 2 | 2 | Between the thumb and the view's edge, and the thumb's end and the lane's end. `scrollbar_margin=4` |
| `scrollbar_min_length` | px, ≥ 0 | 32 | 32 | The shortest the thumb gets along its axis. `scrollbar_min_length=24` |
| `scrollbar_corner` | px, ≥ 0 | 12 | 0 | How far each lane stops short of the end where the other axis has a bar, so the bars never meet. `scrollbar_corner=0` |
| `scrollbar_opacity` | 0 to 1 | 0.70 | 1.0 | The thumb's opacity at rest. `scrollbar_opacity=0.5` |
| `scrollbar_opacity_hover` | 0 to 1 | 0.80 | = rest | …with the pointer in the hit area. `scrollbar_opacity_hover=0.9` |
| `scrollbar_opacity_drag` | 0 to 1 | 1.0 | = rest | …while dragged. `scrollbar_opacity_drag=1.0` |
| `scrollbar_track_fill` | colour | `None`: no track (the spec is track-less) | `None` | A track behind the thumb, as long as the lane and as thick as the thumb. `scrollbar_track_fill=(231, 224, 236, 255)` |
| `scrollbar_track_opacity` | 0 to 1 | 1.0 | 1.0 | Multiplies the track fill's alpha; nothing is painted until `scrollbar_track_fill` is set. `scrollbar_track_opacity=0.4` |

**Where, and when it shows**

| Property | Type | Default | Meaning, example |
| --- | --- | --- | --- |
| `scrollbar_side_v` | `"end"`, `"start"`, `"left"`, `"right"` | `"end"` | The edge of the vertical bar. `"end"` and `"start"` follow the layout direction: today every layout is left-to-right, so they are the right and the left edge until a direction property exists (one place resolves them, `BarSide::edge`). `scrollbar_side_v="start"` |
| `scrollbar_side_h` | `"end"`, `"start"`, `"bottom"`, `"top"` | `"end"` | The edge of the horizontal bar: `"end"` and `"bottom"` are the bottom, `"start"` and `"top"` the top. `scrollbar_side_h="top"` |
| `scrollbar_show` | `"overflow"`, `"always"`, `"auto_hide"`, `"never"` | `"overflow"` | `"overflow"`: whenever the axis overflows (today's behaviour). `"always"`: also without overflow, then a thumb as long as the lane (nothing to drag; no gutter is reserved). `"auto_hide"`: when it overflows, fading out `scrollbar_hide_delay_ms` after the last scroll, hover or drag, and back in on the next; the faded bar still takes the pointer, so hovering its edge shows it. `"never"`: no bar and no gutter; the view still scrolls. `scrollbar_show="auto_hide"` |
| `scrollbar_hide_delay_ms` | ms, 0 to an hour | 1000 | How long `"auto_hide"` waits. `scrollbar_hide_delay_ms=1500` |
| `scrollbar_fade_in_ms` | ms | 100 | The duration of a fade toward more emphasis or visibility (hover, drag, `"auto_hide"` showing). Replaces the engine's fixed 100 ms. `scrollbar_fade_in_ms=150` |
| `scrollbar_fade_out_ms` | ms | 100 | …toward less (leaving the hit area, `"auto_hide"` hiding). `scrollbar_fade_out_ms=300` |
| `scrollbar_hover_delay_ms` | ms | 0 | How long the pointer stays in the hit area before the hover emphasis starts. `scrollbar_hover_delay_ms=200` |
| `scrollbar_fade_easing` | preset name or `(x1, y1, x2, y2)` | `"standard"` | The curve of those fades. A [preset name](../guide/animation.md#easing) (`linear`, `standard`, `standard_decelerate`, `standard_accelerate`, `emphasized`, `emphasized_decelerate`, `emphasized_accelerate`, `ease`, `ease_in`, `ease_out`, `ease_in_out`) or a bezier `(x1, y1, x2, y2)` with `x1` and `x2` from 0 to 1 (CSS `cubic-bezier`); the same forms `animate` takes, minus the springs. `scrollbar_fade_easing="ease_out"` |
| `scrollbar_gutter` | px, ≥ 0 | 12 (`"both"`), 0 | The room an overflowing axis reserves at its bar's edge ([how](../guide/nodes-and-layout.md#two-axes-0611)). `scrollbar_gutter=16` |
| `scrollbar_gutter_reserve` | bool | `True` | `False`: no room is reserved and the thumb overlays the content. `scrollbar_gutter_reserve=False` |

**Buttons and lane clicks**

| Property | Type | Default | Meaning, example |
| --- | --- | --- | --- |
| `scrollbar_arrows` | `"none"`, `"both"` | `"none"` | `"both"`: a square arrow button, as thick as the thumb, at each end of the lane (the lane and so the thumb get 2 × thickness shorter). A press scrolls `scrollbar_arrow_step` and repeats while held (`page_repeat_delay_ms`, `page_repeat_ms`). The arrows are minimal: a triangle in the thumb's colour, with no background, hover state or disabled look. `scrollbar_arrows="both"` |
| `scrollbar_arrow_step` | px, ≥ 0 | 40 | What one arrow press scrolls. `scrollbar_arrow_step=20` |
| `scrollbar_lane_click` | `"page"`, `"jump"`, `"none"` | `"page"` (`"both"`), `"none"` (one axis, list: a press beside the thumb has always gone to the content) | What a press on the lane beside the thumb does. `"page"`: one page (`page_fraction` of the viewport) toward it, repeating while held until the thumb reaches the press. `"jump"`: the thumb's middle goes to the press and the press goes on as a drag. `"none"`: nothing. `scrollbar_lane_click="jump"` |

**Scroll feel** (the wheel, the keys and the page; scroll views and virtual lists)

| Property | Type | Default | Meaning, example |
| --- | --- | --- | --- |
| `wheel_notch` | px, ≥ 0 | 40 | What one notch of a wheel that reports lines scrolls (pixel deltas, a trackpad or a touch pan, are taken as they come). `wheel_notch=60` |
| `key_line` | px, ≥ 0 | 40 | What an arrow key scrolls. `key_line=24` |
| `page_fraction` | 0.01 to 1 | 0.9 (a `"vertical"` or `"horizontal"` view keeps a whole viewport, 1.0) | The share of the viewport Page Up/Down and a lane click scroll. `page_fraction=0.75` |
| `scroll_smooth_ms` | ms | 0 (the spec: keys and wheel jump) | When above 0 the wheel and the keys ease over this long; a second notch goes on from where the first was heading. A drag, a lane or arrow press and `scroll_offset=` stay direct. `scroll_smooth_ms=150` |
| `scroll_easing` | preset name or bezier | `"standard"` | The curve of a smooth scroll. `scroll_easing="ease_out"` |
| `page_repeat_delay_ms` | ms | 400 | How long a held lane or arrow press waits before it repeats. `page_repeat_delay_ms=300` |
| `page_repeat_ms` | ms, 1 to an hour | 100 | The interval of the repeat. `page_repeat_ms=50` |

**Scroll snap and fling** (0.6.11; the engine's own timings, until now constants; scroll views and virtual lists)

| Property | Type | Default | Meaning, example |
| --- | --- | --- | --- |
| `scroll_snap_settle_ms` | ms, 0 to an hour | 120 | How long a view with `scroll_snap` waits after its last input (wheel, key, finger, fling) before it settles on a snap point. `scroll_snap_settle_ms=250` |
| `scroll_snap_duration_ms` | ms, 0 to an hour | 200 | How long the settling takes; 0 while reduced motion is on, whatever this says. `scroll_snap_duration_ms=300` |
| `scroll_snap_easing` | preset name or bezier | `"standard"` (0.2, 0, 0, 1) | The curve of the settle. `scroll_snap_easing="emphasized_decelerate"` |
| `fling_tau_ms` | ms, 1 to an hour | 350 | How quickly a touch flick slows: the time constant of its exponential decay (the speed is `v0 · e^(-t/τ)`). Larger coasts further and longer: the distance is about `v0 · τ`. `fling_tau_ms=700` |
| `fling_stop_speed` | px/s, 0.1 to 1 000 000 | 30 | The coast ends when its speed has fallen to this. A flick no faster than this does not coast. `fling_stop_speed=60` |
| `fling_min_speed` | px/s, 0 to 1 000 000 | 150 | A flick slower than this does not coast at all (the finger lifted too slowly). `fling_min_speed=400` |

```python
# A design system's scrollbar: a thin pill that shows on hover or scroll, with arrows and a track.
view.set(scrollbar_width=6, scrollbar_show="auto_hide", scrollbar_fade_out_ms=250,
         scrollbar_track_fill=(231, 224, 236, 255), scrollbar_track_opacity=0.5,
         scrollbar_arrows="both", scrollbar_side_v="start", scroll_smooth_ms=120)
```

**Reduced motion.** `Window.set(reduced_motion=True)` (`None`, the default, follows the OS, the value
`window.get("reduced_motion")` and the `reduced_motion` event report) makes the fades, `scroll_smooth_ms` and the
scroll snap settle (`scroll_snap_duration_ms`) take no time. Any one scroller can still be given its own durations of `0`.
A touch fling is not changed by it: it is the finger's own momentum, not a transition, and carries on at the
speed the finger left it; set `fling_min_speed` very high (or listen to `reduced_motion` and set it) to end flings.

## Virtual list

```python
def row(index):
    return window.create("text", text=items[index])

rows = window.create("virtual_list", item_count=len(items), item_extent=32,
                     materialize=row, width=320, height=400)
```

| Property | Value |
| --- | --- |
| `item_count` | How many rows |
| `item_extent` | Every row's height, or `None` once `size_hint` is set |
| `size_hint` | `size_hint(index)` → that row's height, for rows of different heights |
| `materialize` | `materialize(index)` → a node this window made, for that row |

Tesserae Engine keeps exactly the rows the viewport shows built: whenever layout runs —
each frame, `window.advance`, `window.simulate`, and any `layout_*` read — it
calls `materialize` for each newly visible row, gives the row the list's width
and its own height, and detaches rows scrolled away. A detached row is freed
unless you keep a handle to it to reuse. Changing `item_count`, `item_extent`,
`size_hint`, or `materialize` rebuilds every row.

A virtual list scrolls by wheel, thumb drag and (0.6.11) keys: Up and Down by 40
pixels, Page Up and Page Down by the viewport less 10 percent, Home and End to the
ends (the [keyboard scrolling](../guide/nodes-and-layout.md#keyboard-scrolling)
rules apply). `node.scroll_to_index(index, align="nearest")` scrolls to a row by
index, built or not; `scroll_into_view()` and focus reveal a built row by the least
distance; `scroll` fires for every change. A row holding focus is kept built when it
scrolls out of the window.

(0.6.11) A virtual list takes every [scrollbar and scroll-feel property](#scrollbar-and-scroll-feel-0611) a scroll
view does (`scrollbar_*`, `wheel_notch`, `key_line`, `page_fraction`, `scroll_smooth_ms`, `scroll_easing`,
`page_repeat_*`, `fling_*`; `scroll_snap_*` are accepted and read back but do nothing there, a list has no snap), and draws, hit-tests, hovers, drags and fades its bar with the same code. A list that sets none keeps
the bar it had (4 px, 2 px radius, no hover or drag emphasis, no lane click); the keys and the page (nine tenths of the
viewport) are the spec's.

## Terminal

| Property | Value |
| --- | --- |
| `cols`, `rows` | The grid size, 1–1000 each; the terminal's box follows its grid and font |
| `selection` | `(start_row, start_col, end_row, end_col)`, or `None` |
| `shell`, `scrollback_lines` | Given to `create` only |
| `text` | Read-only: the visible grid, one line per row, trailing blanks trimmed |

Plus `font_size` and the `palette` on
[Paint, Paths, and Animation](paint.md#text-inputs-scroll-views-and-terminals). (0.6.11) The palette's `foreground`,
`cursor` and `selection` read `None` until set, which is the window's
[colour scheme](../guide/painting.md#colour-scheme-0611): `on_surface`, `primary` at 50% alpha and `primary` at 30% (the selection).

## Accessibility (0.6.11)

Every node takes these; they change what assistive technology is told, not what is drawn. The full list of
accessibility properties (`role`, `label`, `checked`, ...) is on [Node](node.md#set-get-and-focus), the guide is
[Accessibility](../guide/accessibility.md#sets-mixed-orientation-and-modal-0611).

| Property | Type | Default | Unit | Example |
| --- | --- | --- | --- | --- |
| `set_size` | positive `int` or `None` | `None` (a `virtual_list` says its `item_count`) | items | `listbox.set(set_size=12)` |
| `pos_in_set` | positive `int` or `None` | `None` (a `virtual_list` row says its index plus one) | 1-based place | `item.set(pos_in_set=3)` |
| `checked` | `True`, `False`, `"mixed"`, `None` | `None` | | `parent.set(checked="mixed")` |
| `a11y_orientation` | `"horizontal"`, `"vertical"`, `None` | `None` (the platform's default for the role) | | `slider.set(a11y_orientation="horizontal")` |
| `modal` | `bool` | `False` | | `dialog.set(role="dialog", modal=True)` |
| `role` | 42 names, 18 new: `status`, `spinbutton`, `radiogroup`, `option`, `listbox`, `toolbar`, `navigation`, `complementary`, `region`, `grid`, `row`, `gridcell`, `columnheader`, `rowheader`, `searchbox`, `combobox`, `tooltip`, `scrollbar` | `"none"`-like (unset) | | `box.set(role="status")` |

`status` is a polite live region and `alert` an assertive one unless `live` is set.

## Read-only

| Property | Value |
| --- | --- |
| `kind` | The node's kind, by the name `create` takes — `"box"`, `"text"`, `"text_input"`, ... |
| `focused` | Whether the node has keyboard focus |
| `layer_placement` | A shown [layer](layers.md)'s side or point of its anchor — `"below"`, `"above"`, `"start"`, `"end"`, or (0.5.7) `"top_start"`, `"top_end"`, `"bottom_start"`, `"bottom_end"`, `"center"` — or `None` |
| `layout_x`, `layout_y` | The node's position in the window, as drawn — its own and its ancestors' transforms included |
| `layout_width`, `layout_height` | Its computed size, before any `scale` |

Reading a `layout_*` value runs any pending layout first, so it always matches
the tree as it is now. A detached subtree is laid out on its own, at its content
size — so you can measure one before attaching it.
