# Accessibility

Every frame, Tesserae Engine builds an [AccessKit](https://github.com/AccessKit/accesskit)
tree from the node tree and hands it to the platform's screen reader. What
a node tells assistive technology is what you set on it: Tesserae Engine knows boxes
and paths, not buttons and switches, so a widget declares what it is. A `LayerWindow` (a bar, a
panel, an overlay menu) is published the same way, as one more window of the application; see
[the LayerWindow reference](../reference/layer-window.md#accessibility).

## Role, name, and state

```python
save = window.create("box", width=96, height=40, corner_radius=20,
                     fill=(0x67, 0x50, 0xA4, 0xFF),
                     role="button", label="Save", focusable=True)
```

- `role` — what the node is: `"button"`, `"checkbox"`, `"switch"`,
  `"slider"`, `"textbox"`, `"menu"`, `"menuitem"`, `"dialog"`, `"tab"`,
  `"heading"`, `"list"`, `"listitem"`, and the rest listed on
  [`Node`](../reference/node.md#set-get-and-focus).
- `label` — the name read aloud. A text node's words aren't exposed on their
  own, so label the widget that shows them.
- State — `checked`, `selected`, `expanded`, `disabled`, and a heading's
  `level`. Keep them current as the widget changes: a switch sets
  `checked` each time it toggles. They are for assistive technology:
  `disabled` doesn't stop the node taking focus, clicks, or keys — the widget
  does that.
- Value — `value` (a string or number), with `value_min`, `value_max`, and
  `value_step` for a range such as a slider or progress bar.
- More state (0.5.6) — `pressed` (a toggle button, with `"mixed"`), `invalid`
  with a `description` of why, `busy`, and `current` (`"page"`, `"step"`, ...)
  for the current item of a set. A range can also say its value in words with
  `value_text`.
- Relations (0.5.6) — `describedby` and `controls` take the nodes that
  describe this one and the nodes it controls (a button and the panel it
  opens). A node that is hidden or removed is left out for assistive
  technology.
- Active descendant (unreleased) — `active_descendant` names one node, or
  `None`: the current item of a node that keeps the keyboard, such as the
  row selected in a list under a search field. See
  [Active descendant](#active-descendant) below.

A text input and a terminal are `"textbox"` from the start, and a text
input's value is its text.

## Focus

Screen readers follow keyboard focus, so everything a keyboard user can
operate should be `focusable=True` — see
[Focus and the keyboard](events-and-input.md#focus-and-the-keyboard). An
assistive technology's request to focus a node focuses it exactly as Tab
would, and `focus_visible` is `True` for it. However focus arrives — Tab,
a click, `node.focus()`, or an assistive technology — the scroll views
around the node scroll just enough to show it (0.4.2), so a screen reader
never lands on a node that's out of sight. See
[Revealing a node](nodes-and-layout.md#revealing-a-node).

## Actions

The actions a node offers follow from its role and state:

- focusable nodes offer focus;
- button-like roles (button, checkbox, radio, switch, link, menu item, tab,
  tree item) offer activation;
- a slider, or any node with a value range, offers increment, decrement, and
  set-value;
- a node with `expanded` set offers expand and collapse.

Activation arrives as an ordinary `click`, so a widget that handles clicks
already handles it. The rest arrive as `a11y_action`:

```python
volume = 0.5

def set_volume(value):
    global volume
    volume = round(min(1.0, max(0.0, value)), 2)
    slider.set(value=volume)          # keep what's announced current

def on_action(event):
    if event.action == "increment":
        set_volume(volume + 0.1)
    elif event.action == "decrement":
        set_volume(volume - 0.1)
    elif event.action == "set_value":
        set_volume(float(event.value))

slider.on("a11y_action", on_action)
```

`event.action` is `"increment"`, `"decrement"`, `"expand"`, `"collapse"`,
`"scroll_into_view"`, or `"set_value"` (with `event.value`). Tesserae Engine answers
`"scroll_into_view"` itself, after your listeners run: it scrolls every
scroll view around the node just enough to show it, as
[`node.scroll_into_view()`](nodes-and-layout.md#revealing-a-node) does, so a
widget needs no listener for it.

## Text, selection and links

A text node that is `selectable`, has a selection, or has a span with a `link`
(0.5.4) is exposed as a text container: if its `role` is still the default it
reads as a label, and its content is a run of text for each line and each stretch
between link boundaries, with each link a `link` node holding its run and the span's `link`
string as its URL. A selection (`selection`, or the user's) is reported as a text
selection, so a screen reader can read it and move through the text, and it can
set the selection (`SetTextSelection`), which selects the text as the pointer
would. Following a link (the `click` action on the link node) fires the text
node's `link` event with `href`, as a mouse click does.

Each shaped line is its own run, with its bounds and the position and width of
every character, and the start of each word, so a screen reader can place its
cursor, move by line or word, and a magnifier can follow it. A link that wraps is
one link node holding a run per line. The lines come from the window's text
renderer when the accessibility tree is built, so a tree built with no renderer
(a headless test) has one run per link segment over the node's box. Positions
assume left-to-right text. Plain text with none of these is unchanged: no
children, and the role you set. This is built and checked against AccessKit's
own consumer; it has not been tried with NVDA, VoiceOver or Orca.

## Announcing and hiding

`live="polite"` or `"assertive"` makes a node a live region: a screen reader
announces changes to it — a snackbar's message, a form's error. `a11y_hidden`
hides a node from assistive technology while it stays on screen, for purely
decorative parts; `visible=False` hides it from everyone.

### Announcing without a node

A live node announces only when its name changes or when it enters the tree. So it
cannot say that the last window closed once its module is hidden, it says nothing the
second time the same message comes, and two nodes with one message say it twice.
`window.announce(text, politeness="polite")` raises one announcement for the window with
no node of yours (0.6.9):

```python
bar.announce("Connected")                       # polite: waits for the reader to finish
bar.announce("Battery critical", "assertive")   # assertive: interrupts
bar.announce("Connected")                       # said again: a new call is a new announcement
```

Each call is one announcement, so removing duplicates is yours. Calls are heard in the order they
were made (0.6.10), even several in one loop turn: tre sends one per accessibility update and the
window draws a frame for each, so six messages take six frames (about 100 ms at 60 Hz). The queue
holds 16; the newest are kept. It works on a `Window`, a
`LayerWindow` and a `PopupWindow`, and for a module that is hidden from the accessibility tree.
An empty text or any politeness other than `"polite"` and `"assertive"` raises `ValueError`.
Nothing is heard unless a screen reader is connected, and the message is raised with the next
frame the window draws. It has been checked against AT-SPI's `Announcement` signal on a private
bus; it has not been tried with Orca, NVDA or VoiceOver.

### Active descendant

A text field with a list under it keeps the keyboard, and Up and Down move a selection in the list.
A screen reader reads the field, not the row. `active_descendant` tells it which row is current
(unreleased, TRE-REQ-023):

```python
field.set(active_descendant=rows[2])   # the screen reader now reads this row as the current item
field.set(active_descendant=None)      # back to the field
```

It takes one node of the same window, or `None`, on any node, and reads back with `get`. **Keyboard focus
does not move**: `field.get("focused")` stays `True` and no `focus` or `unfocus` event fires. Give the row
its `role` (`"listitem"`) and `label` so there is something to read. A target that is hidden is left out
of the accessibility tree, and one that is destroyed reads back as `None`.

How a platform says it differs. AccessKit expresses the relation by moving the *focused state* from the
field to the row, so on Linux a screen reader sees AT-SPI `StateChanged(focused)` move to the row and back
(checked on a private bus); it does not send `ActiveDescendantChanged` for it. Use it with the polite
`announce` as a fallback where a platform does not deliver the relation. Not tried with Orca, NVDA or
VoiceOver.

## Testing

`window.simulate("a11y_action", node=..., action=..., value=...)` sends an
assistive request through the same path a screen reader's does, and every
property reads back with `get`:

```python
window.simulate("a11y_action", node=slider, action="set_value", value=0.75)
assert slider.get("value") == 0.75
```

An assistive technology's `scroll_into_view` request is testable the same
way: simulate it on a row, then read its scroll view's
`get("scroll_offset")`.
