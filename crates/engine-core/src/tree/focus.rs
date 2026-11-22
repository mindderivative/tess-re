//! Hit-testing, hover, press, focus movement, activation, and pointer capture.

use super::*;
use crate::HitShape;

impl Tree {
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    /// 0.6.11 (UXREQ-008): the outline `id` draws now, if any: its `focus_outline_*` while it has
    /// keyboard focus that is visible, else its `outline_*`. A text input shows its focus
    /// outline for pointer focus too (the specification's `Focus Ring`: pointer users need to see
    /// where the caret is). The colour is resolved against the window's scheme.
    pub fn outline_of(&self, id: NodeId) -> Option<crate::Outline> {
        let node = self.nodes.get(id)?;
        let outlines = node.paint.outline.as_deref()?;
        let focus_shown = self.focused == Some(id)
            && (self.focus_visible || matches!(node.kind, NodeKind::TextField(_)));
        outlines.resolve(focus_shown, &self.scheme)
    }

    /// §11.10: pointer-to-node resolution, reverse paint order (topmost
    /// first -- the last child in `children`-list order paints on top,
    /// §6, so it's tested first here too; this is exactly what naturally
    /// reaches an appended overlay, §11.3, before ordinary background
    /// content, with zero special-casing).
    ///
    /// **Transform-aware since M5 Phase 2** (§11.9's own composed
    /// transform, landed M5 Phase 1): delegates to `hit_test_at`, which
    /// composes the same `parent * translate(layout.location) *
    /// own_transform` product `engine-render's paint walk` composes
    /// during paint -- if this formula and that one ever diverge,
    /// hit-testing and rendering will disagree about where a node is.
    /// (`absolute_position` composes the same product, via
    /// `composed_transform`.)
    ///
    /// **Since M5 Phase 3:** a `NodeKind::Canvas` with a `CustomHitTest`
    /// set (`Tree::set_canvas_content`) overrides the rect test below
    /// for that node only, per §11.10's own text; a `Canvas` with none
    /// (or any other `NodeKind`) keeps the ordinary rect test.
    pub fn hit_test(&self, root: NodeId, point: Point) -> Option<NodeId> {
        self.hit_test_at(root, point, Affine::IDENTITY)
            .map(|(id, _local_point)| id)
    }

    /// M18 Phase 1 (§8, §10, §11.9, §11.10): `hit_test`'s own sibling,
    /// additionally returning the click's own local-space point on the
    /// hit node -- needed by a caller (`TextRenderer::hit_test_position`)
    /// that has to turn a click into a byte offset within text painted
    /// at that node's own local origin, the same real transform-aware
    /// coordinate `hit_test_at` already computes internally for its own
    /// rect/custom-hit-test containment check but never exposed before
    /// this. Reuses `hit_test_at`'s own composition math verbatim, not a
    /// second way of computing it.
    pub fn hit_test_local(&self, root: NodeId, point: Point) -> Option<(NodeId, Point)> {
        self.hit_test_at(root, point, Affine::IDENTITY)
    }

    /// See `hit_test`'s own doc comment for the composition formula and
    /// why it has to match `draw_own`'s exactly. `parent_transform` is
    /// the caller's already-composed transform for `id`'s *parent*.
    pub(super) fn hit_test_at(
        &self,
        id: NodeId,
        point: Point,
        parent_transform: Affine,
    ) -> Option<(NodeId, Point)> {
        let node = self.nodes.get(id)?;
        if !node.visible {
            return None;
        }
        let layout = self.layout(id);
        let (sx, sy) = self.scroll_shift(id);
        let composed = parent_transform
            * Affine::translate((
                f64::from(layout.location.x) + sx,
                f64::from(layout.location.y) + sy,
            ))
            * node
                .paint
                .local_transform(f64::from(layout.size.width), f64::from(layout.size.height));

        for &child in self.children_in_paint_order(id).iter().rev() {
            if let Some(hit) = self.hit_test_at(child, point, composed) {
                return Some(hit);
            }
        }

        // Map the caller's canvas-space `point` into this node's local
        // space via the inverse of its composed transform -- both the
        // rect default and any `CustomHitTest` below test against this
        // same local point, exactly the local-space coordinates
        // `draw_own` draws into under the identical `composed`
        // transform (M5 Phase 1).
        let local_point = composed.inverse() * point;
        // 0.5.1 (#53): a canvas's painter coordinates start at its padding,
        // and a custom hit shape is in those coordinates.
        let painter_point = {
            let pad = layout.padding;
            local_point - peniko::kurbo::Vec2::new(f64::from(pad.left), f64::from(pad.top))
        };

        // M30 Phase 5 Step 1 (§5, §7): `Node::hit_testable`'s own real
        // opt-out, checked before the per-`NodeKind` match below --
        // `false` short-circuits straight to "no hit" here exactly the
        // way `NodeKind::Text` already does unconditionally,
        // generalized to any node a caller has explicitly opted out
        // (children were already checked above, so this only ever
        // affects whether *this* node itself claims the point).
        let hit = if node.hit_testable == Some(false) {
            false
        } else {
            match &node.kind {
                NodeKind::Canvas(state) => match &state.hit_test {
                    Some(CustomHitTest::Circle { cx, cy, radius }) => {
                        (painter_point - Point::new(*cx, *cy)).hypot() <= *radius
                    }
                    Some(CustomHitTest::Path { path, tolerance }) => {
                        path.segments()
                            .map(|seg| seg.nearest(painter_point, 0.1).distance_sq)
                            .fold(f64::INFINITY, f64::min)
                            .sqrt()
                            <= *tolerance
                    }
                    None => region::shape_contains(&node.paint, layout, local_point),
                },
                // 0.6.11 (P2): a text label claims a hit like any other node
                // (a tooltip on a label needs it). Before, a bare `Text` never
                // did, unless selectable or a link. Hover and enter/leave see it
                // now; a press still goes to the label's ancestor (`press_target`),
                // so a button's centred label does not take the button's clicks.
                NodeKind::Text(_) => rect_contains(layout, local_point),
                // 0.5.9 (#236): a path that opts in is hit by what it paints.
                NodeKind::Path(state) if state.hit_shape == HitShape::Painted => {
                    region::painted_contains(node, layout, local_point)
                }
                _ => region::shape_contains(&node.paint, layout, local_point),
            }
        };
        hit.then_some((id, local_point))
    }

    /// 0.6.11 (P2): the node a press on `hit` lands on. A bare `text` label is hit (so it is
    /// hovered and a tooltip on it works) but, as before, is not what a press or a click
    /// reaches: the press goes to the nearest ancestor that is not one, so a button's label
    /// is part of the button and a press on the label that ends on the button's padding is
    /// still one click. A label that is selectable, holds a link, or has `hit_testable` set
    /// to `True` is its own press target.
    pub(super) fn press_target(&self, hit: NodeId) -> NodeId {
        let mut id = hit;
        while let Some(node) = self.nodes.get(id) {
            let own = match &node.kind {
                NodeKind::Text(state) => {
                    node.hit_testable == Some(true)
                        || state.options.selectable
                        || state.options.spans.iter().any(|s| s.link.is_some())
                }
                _ => true,
            };
            if own {
                return id;
            }
            match node.parent {
                Some(parent) => id = parent,
                None => return id,
            }
        }
        hit
    }

    /// The concrete fulfillment of §7.3's own text: "hover needs no new
    /// dispatch mechanism -- it falls out of hit-testing, run every
    /// pointer-move... entirely inside `engine-core`." Hover draws
    /// nothing itself -- a framework styles it from `pointer_enter`/
    /// `pointer_leave` listeners.
    ///
    /// Returns the newly-hovered node (`None` if the pointer left every
    /// hit-testable node). A repeated call with the same result is a
    /// no-op.
    pub fn update_hover(&mut self, root: NodeId, point: Point) -> Option<NodeId> {
        let hit = self.hit_test_input(root, point);
        self.set_hovered(hit)
    }

    /// M94: `update_hover`'s own transition half, split out so
    /// `InputEvent::PointerLeft` can clear hover without a hit-test.
    /// Returns `hit`.
    /// The node the pointer was last found over.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    pub fn set_hovered(&mut self, hit: Option<NodeId>) -> Option<NodeId> {
        self.dirty = true;
        if hit == self.hovered {
            return hit;
        }
        let old = self.hovered;
        self.hovered = hit;
        // 0.6.11 (P13): the nodes the pointer left and entered (and their ancestors) take
        // or lose their hover layers.
        self.note_state_chain(old);
        self.note_state_chain(hit);
        hit
    }

    /// M38 Phase 5 (§5, §7): the single real chokepoint every real
    /// `self.pressed` mutation goes through.
    pub(super) fn set_pressed(&mut self, new: Option<(PointerButton, NodeId)>) {
        let old = self.pressed;
        if old.map(|(_, id)| id) == new.map(|(_, id)| id) {
            self.pressed = new;
            return;
        }
        self.pressed = new;
        // 0.6.11 (P13): the pressed node and its ancestors take or lose their pressed layers.
        self.note_state_chain(old.map(|(_, id)| id));
        self.note_state_chain(new.map(|(_, id)| id));
    }

    /// §10's own minimal keyboard focus model: Tab/Shift-Tab moves
    /// `focused` in tree order, wrapping at both ends, over the nodes in
    /// the Tab order (`AccessNodeData::in_tab_order`).
    ///
    /// M55: returns the real `(old, new)` transition, `Some` only on a
    /// genuine change -- `Tree::dispatch`'s own `KeyPressed`/`Key::Tab`
    /// arm reads it to produce a real `DispatchOutcome::FocusChanged`.
    pub fn move_focus(
        &mut self,
        root: NodeId,
        direction: FocusDirection,
    ) -> Option<(Option<NodeId>, Option<NodeId>)> {
        self.dirty = true;
        let mut order = Vec::new();
        let (scope, skip_layers) = self.focus_scope(root);
        self.collect_interactive(scope, skip_layers, &mut order);
        // M94: positive `tab_index` values first, ascending; then the
        // rest in tree order (the sort is stable).
        order.sort_by_key(|&id| match self.nodes[id].access.tab_index {
            index if index > 0 => (0, index),
            _ => (1, 0),
        });

        let old = self.focused;
        let new = if order.is_empty() {
            None
        } else {
            let next_index = match old.and_then(|f| order.iter().position(|&n| n == f)) {
                Some(idx) => match direction {
                    FocusDirection::Next => (idx + 1) % order.len(),
                    FocusDirection::Previous => (idx + order.len() - 1) % order.len(),
                },
                None => match direction {
                    FocusDirection::Next => 0,
                    FocusDirection::Previous => order.len() - 1,
                },
            };
            Some(order[next_index])
        };
        self.transition_focus(new)
    }

    /// M4 Phase 2 (§10): the direct-target counterpart to `move_focus`'s
    /// own tab-order computation -- what a platform-driven focus request
    /// actually needs ("a screen reader focusing a node directly"
    /// dispatches `accesskit::Action::Focus`, §10's own text), since
    /// jumping straight to a specific node is a different operation from
    /// "move to the next/previous interactive node in tree order."
    /// `node` need not itself have a non-empty `access.actions` --
    /// unlike `move_focus`'s own Tab-order, a platform accessibility
    /// client names the exact node it wants focused, the same way a
    /// mouse click names the exact node it hit regardless of that node's
    /// own `access.actions` (`Tree::activate`'s own doc comment makes
    /// the identical "don't invent a stricter rule for one path than the
    /// other" argument).
    /// M55 (§10, §16.2): the return, widened from `()`, is the real
    /// `(old, new)` transition -- `Some` only on a genuine change,
    /// `None` when `node` was already focused or doesn't exist (the
    /// same "no real fact to report" contract `HoverChanged` already
    /// has). Every real caller here and in `engine-py` (the one real
    /// non-`Tree::dispatch` mutation path: AccessKit's `Action::Focus`,
    /// `app.rs`) reads it to fire a real `FocusEnter`/`FocusExit` --
    /// `Tree::dispatch`'s own callers get theirs a different way, via
    /// `DispatchOutcome::FocusChanged`, since this method's own return
    /// only reaches a direct caller, not `dispatch`'s own outcome.
    pub fn set_focus_to(&mut self, node: NodeId) -> Option<(Option<NodeId>, Option<NodeId>)> {
        self.dirty = true;
        if !self.nodes.contains_key(node) {
            return None;
        }
        self.transition_focus(Some(node))
    }

    /// M94: `set_focus_to`'s counterpart that leaves nothing focused --
    /// how a simulated `blur` clears focus through the same transition
    /// (and `(old, new)` report) as every other focus change.
    pub fn clear_focus(&mut self) -> Option<(Option<NodeId>, Option<NodeId>)> {
        self.dirty = true;
        self.transition_focus(None)
    }

    /// Shared by `move_focus`/`set_focus_to`: animates the previously-
    /// focused node's `focus_ring` out and the newly-focused one's in,
    /// opt-in-only (Design Principle 6) exactly like `update_hover`
    /// animates `hover_opacity` -- factored out once a second real
    /// caller needed the identical transition logic, not duplicated.
    ///
    /// M55: returns the real `(old, new)` transition, `Some` only when
    /// `old != new` -- the one real chokepoint both `move_focus`/
    /// `set_focus_to` already shared, so this is also the one real
    /// place to compute it, mirroring `update_hover`'s own analogous
    /// role for `self.hovered`.
    pub(super) fn transition_focus(
        &mut self,
        new: Option<NodeId>,
    ) -> Option<(Option<NodeId>, Option<NodeId>)> {
        let old = self.focused;
        self.focused = new;
        if old == new {
            return None;
        }
        // 0.6.11 (P13): the nodes that lost and gained focus look at their focus layers again.
        self.note_state_node(old);
        self.note_state_node(new);
        // 0.4.2 M12 (issue #24): a newly focused node is scrolled into
        // view, however focus got there -- Tab, a click, `focus()`, or
        // assistive technology.
        if let Some(node) = new {
            self.scroll_into_view(node);
        }
        Some((old, new))
    }

    /// M94: `node` or its nearest ancestor that a framework marked
    /// `focusable`.
    pub(super) fn focusable_ancestor(&self, node: NodeId) -> Option<NodeId> {
        let mut current = Some(node);
        while let Some(id) = current {
            let n = self.nodes.get(id)?;
            if n.access.focusable == Some(true) {
                return Some(id);
            }
            current = n.parent;
        }
        None
    }

    /// Pre-order walk collecting every node whose `access.actions` is
    /// non-empty, in tree order -- `move_focus`'s own Tab-order.
    pub(super) fn collect_interactive(&self, id: NodeId, skip_layers: bool, out: &mut Vec<NodeId>) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        if !node.visible {
            return;
        }
        if node.access.in_tab_order() {
            out.push(id);
        }
        for &child in &node.children {
            if !(skip_layers && self.is_overlay(child)) {
                self.collect_interactive(child, skip_layers, out);
            }
        }
    }

    /// M4 Phase 2 (§10): the direct, non-`InputEvent` counterpart to a
    /// mouse click's own `Activated` outcome -- what `accesskit::
    /// Action::Click` from a platform accessibility client actually
    /// needs, since there's no press/release pair or on-screen point
    /// involved, just a named target node. Deliberately does **not**
    /// check `access.actions` first -- the real mouse-click path
    /// (`dispatch`'s `PointerPressed`/`PointerReleased` handling)
    /// doesn't gate on it either (hit-testing alone decides *which*
    /// node; whether anything is actually registered to react is the
    /// caller's own lookup, e.g. `engine-py`'s `click_handlers`) --
    /// keeping both activation paths symmetric.
    pub fn activate(&self, node: NodeId) -> DispatchOutcome {
        if self.nodes.contains_key(node) {
            DispatchOutcome::Activated(node)
        } else {
            DispatchOutcome::None
        }
    }

    /// M94: the node currently holding pointer capture, if any
    /// (`set_pointer_capture`).
    pub fn pointer_capture(&self) -> Option<NodeId> {
        self.pointer_capture
    }

    /// M94: routes every later pointer event to `node` until it is
    /// released -- by `None` here, by the pointer button's release
    /// (`engine-py`'s router releases it after delivering `pointer_up`),
    /// or by `node` leaving the tree. Only Python listener routing reads
    /// it; the engine's own built-in widget dispatch is unaffected.
    pub fn set_pointer_capture(&mut self, node: Option<NodeId>) {
        self.pointer_capture = node.filter(|&id| self.nodes.contains_key(id));
    }

    /// 0.5.0 M3 (issue #28): ends the current press without a click and
    /// releases any pointer capture -- for a press that started a window
    /// move or resize, whose release the platform may never deliver.
    /// Returns the node that was pressed.
    pub fn cancel_press(&mut self) -> Option<NodeId> {
        let pressed = self.pressed.map(|(_, id)| id);
        self.set_pressed(None);
        self.pointer_capture = None;
        pressed
    }

    /// 0.6.12 (UXREQ-007): whether a gesture the engine can cancel is held: a
    /// captured pointer, a scrollbar thumb drag, or one `set_gesture_hold` was
    /// told of (a dock drag).
    pub fn gesture_active(&self) -> bool {
        self.pointer_capture.is_some() || self.dragging.is_some() || self.gesture_hold
    }

    /// 0.6.12 (UXREQ-007): tells the tree a gesture it cannot see (a dock drag)
    /// starts or ends, which is what edge auto-scroll waits for.
    pub fn set_gesture_hold(&mut self, hold: bool) {
        self.gesture_hold = hold;
    }

    /// 0.6.12 (UXREQ-007): ends the gesture in progress without its release: the
    /// pointer capture and the press go, a held lane press stops, and a dragged
    /// scrollbar thumb puts its view back at the offset the drag started from.
    /// Returns the node that held the capture and the one that was pressed
    /// (what `pointer_cancel` goes to).
    pub fn cancel_gesture(&mut self, now: Instant) -> CancelledGesture {
        let holder = self.pointer_capture;
        if let Some(view) = self.dragging.take() {
            let mut start = None;
            if let Some(scroller) = self.nodes.get_mut(view).and_then(|n| n.kind.scroller_mut()) {
                let axis = scroller.bar().drag_axis;
                if let Some((_, scroll)) = scroller.bar_mut().drag_anchor.take() {
                    let offset = scroller.offset_anim(axis);
                    offset.stop();
                    offset.current = scroll;
                    start = Some(scroll);
                }
            }
            if start.is_some() {
                self.note_scroll_input(view);
            }
            self.refresh_bar(view, now);
        }
        self.page_repeat = None;
        self.edge_scroll_last = None;
        self.list_press_cancel();
        self.dirty = true;
        let pressed = self.cancel_press();
        CancelledGesture { holder, pressed }
    }

    /// 0.6.12 (UXREQ-007): where the pointer was when edge auto-scroll last moved
    /// a view, once (the caller re-reads hover and the dock target there).
    pub fn take_edge_scrolled(&mut self) -> Option<Point> {
        if std::mem::take(&mut self.edge_scrolled) {
            self.pointer_at
        } else {
            None
        }
    }
}

/// What `Tree::cancel_gesture` ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelledGesture {
    /// The node that held the pointer capture.
    pub holder: Option<NodeId>,
    /// The node the press was on.
    pub pressed: Option<NodeId>,
}
