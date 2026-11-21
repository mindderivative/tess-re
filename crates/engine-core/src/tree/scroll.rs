//! Scrolling: scroll views and virtual lists, their windows of rows, and their scrollbar-thumb drags.

use super::*;
use crate::scrollbar::{
    FADE as SCROLLBAR_FADE, PAGE_DELAY, PAGE_FRACTION, PAGE_REPEAT, ScrollAxis, Thumb, holds,
};
use std::time::Duration;

/// 0.4.2 M12: how far an arrow key scrolls a scroll view, in pixels.
pub const KEY_SCROLL_LINE: f64 = 40.0;

/// 0.5.4 (#136): how a flick at `speed` (pixels a second, signed along the
/// scroller's axis, positive toward the end) coasts: velocity decays
/// exponentially with time constant `FLING_TAU` until it falls below
/// `FLING_STOP`. Returns `(distance, duration, k)` for an animation from `at`
/// along `0..=max`, as `MotionCurve::Decay(k)` over `duration`; cut short, with
/// the same curve, if it would run past an end. `None` for a flick too slow to
/// coast.
pub(crate) fn fling_plan(speed: f64, at: f64, max: f64) -> Option<(f64, Duration, f64)> {
    let v0 = speed.abs();
    if !v0.is_finite() || v0 < FLING_MIN {
        return None;
    }
    // Speed is v0 * e^(-t/tau); it ends when that reaches FLING_STOP.
    let remaining = FLING_STOP / v0;
    let full_time = FLING_TAU * (1.0 / remaining).ln();
    let full_distance = v0 * FLING_TAU * (1.0 - remaining);
    let room = if speed > 0.0 { max - at } else { at };
    if room <= 0.0 {
        return None;
    }
    let (time, distance) = if full_distance <= room {
        (full_time, full_distance)
    } else {
        // Reaches the end first: the time at which the coast covers `room`.
        let fraction = room / full_distance;
        let t = -FLING_TAU * (1.0 - fraction * (1.0 - remaining)).ln();
        (t, room)
    };
    let k = time / FLING_TAU;
    Some((distance * speed.signum(), Duration::from_secs_f64(time), k))
}

/// 0.5.6 (#166): how long after its last input a snapping scroll view waits
/// before it settles, and how long the settling takes.
const SNAP_SETTLE: Duration = Duration::from_millis(120);
const SNAP_DURATION: Duration = Duration::from_millis(200);

/// How quickly a flick slows: the time constant of its decay, in seconds.
const FLING_TAU: f64 = 0.35;
/// A flick coasts to a stop at this speed, pixels a second.
const FLING_STOP: f64 = 30.0;
/// A flick slower than this does not coast at all.
const FLING_MIN: f64 = 150.0;

impl Tree {
    /// 0.5.4 (#136): lets the scroller under a lifted finger coast. `from` is the
    /// node the touch was on; `velocity` the finger's speed in pixels a second.
    /// Finds the nearest scroll view or virtual list that can move that way, as
    /// the wheel does, and animates its offset on a decay curve. `true` if one
    /// took it.
    pub fn fling_scroll(
        &mut self,
        from: NodeId,
        velocity: peniko::kurbo::Vec2,
        now: Instant,
    ) -> bool {
        let mut current = Some(from);
        while let Some(id) = current {
            let Some(node) = self.nodes.get(id) else {
                return false;
            };
            // The finger moves with the content: dragging up scrolls toward the end.
            match &node.kind {
                NodeKind::ScrollView(state) => {
                    // Each axis the view scrolls along coasts on its own (a `both`
                    // view takes the finger's two components at once).
                    let mut started = false;
                    let mut too_slow = false;
                    for &axis in state.axes() {
                        let along = -axis.pick((velocity.x, velocity.y));
                        if along == 0.0 || !self.can_scroll_axis(id, axis, along) {
                            continue;
                        }
                        let max = self.max_scroll_axis(id, axis).unwrap_or(0.0);
                        let NodeKind::ScrollView(state) = &mut self.nodes[id].kind else {
                            unreachable!("checked above")
                        };
                        let at = state.offset(axis);
                        match fling_plan(along, at, max) {
                            Some((distance, duration, k)) => {
                                state.animated(axis).animate_to(
                                    at + distance,
                                    duration,
                                    crate::MotionCurve::Decay(k),
                                    now,
                                );
                                started = true;
                            }
                            None => too_slow = true,
                        }
                    }
                    if started {
                        self.note_scroll_input(id);
                        self.dirty = true;
                        return true;
                    }
                    // Too slow to coast: nothing more to try outward.
                    if too_slow {
                        return false;
                    }
                }
                NodeKind::VirtualList(state) => {
                    let along = -velocity.y;
                    if along != 0.0 && self.can_scroll(id, along) {
                        let viewport = f64::from(self.layout(id).size.height);
                        let at = state.scroll_offset.current;
                        let max = (state.total_extent() - viewport).max(0.0);
                        if let Some((distance, duration, k)) = fling_plan(along, at, max) {
                            if let NodeKind::VirtualList(state) = &mut self.nodes[id].kind {
                                state.scroll_offset.animate_to(
                                    at + distance,
                                    duration,
                                    crate::MotionCurve::Decay(k),
                                    now,
                                );
                            }
                            self.dirty = true;
                            return true;
                        }
                        // Too slow to coast: nothing more to try outward.
                        return false;
                    }
                }
                _ => {}
            }
            current = self.nodes[id].parent;
        }
        false
    }

    /// 0.5.4 (#136): stops a fling (or any scroll animation) in the scrollers
    /// around `from`, leaving them where they are: a finger landing on moving
    /// content catches it.
    pub fn stop_scroll_animation(&mut self, from: NodeId) {
        let mut current = Some(from);
        while let Some(id) = current {
            let Some(node) = self.nodes.get_mut(id) else {
                return;
            };
            match &mut node.kind {
                NodeKind::ScrollView(state) => {
                    state.scroll.stop();
                    state.scroll_x.stop();
                }
                NodeKind::VirtualList(state) => state.scroll_offset.stop(),
                _ => {}
            }
            current = node.parent;
        }
    }

    /// M36 Phase 1 (§5, §7, §11.7): moves `id`'s own real `ScrollView`
    /// scroll position by `delta` real pixels along its own configured
    /// axis, clamped to `[0.0, max_scroll]` -- the identical real
    /// clamp-on-write shape `scroll_virtual_list_by` already
    /// establishes, except `max_scroll` here comes from the one real
    /// child's own measured content size (`sync_scroll_view_layouts`'s
    /// own doc comment), not an item-count formula. A positive `delta`
    /// increases the offset (content moves toward its own end), the
    /// identical real convention `scroll_virtual_list_by` already
    /// states. Panics if `id` isn't a real `NodeKind::ScrollView` in
    /// this `Tree`, the same "internal bug, not a runtime condition"
    /// contract every other direct scroll method here already uses.
    pub fn scroll_scroll_view_by(&mut self, id: NodeId, delta: f64) {
        self.dirty = true;
        let node = self
            .nodes
            .get(id)
            .expect("scroll_scroll_view_by: NodeId not found in this Tree");
        let NodeKind::ScrollView(state) = &node.kind else {
            panic!("scroll_scroll_view_by: {id:?} is not a NodeKind::ScrollView");
        };
        let axis = state.main_axis();
        self.scroll_axis_by(id, axis, delta);
    }

    /// 0.6.11: `scroll_scroll_view_by` along one axis -- a `both` view has two.
    /// An axis the view does not scroll along has no range, so nothing moves.
    pub fn scroll_axis_by(&mut self, id: NodeId, axis: ScrollAxis, delta: f64) {
        self.dirty = true;
        let node = self
            .nodes
            .get(id)
            .expect("scroll_axis_by: NodeId not found in this Tree");
        if !matches!(node.kind, NodeKind::ScrollView(_)) {
            panic!("scroll_axis_by: {id:?} is not a NodeKind::ScrollView");
        }
        let Some(max_scroll) = self.max_scroll_axis(id, axis) else {
            return;
        };
        let NodeKind::ScrollView(state) = &mut self.nodes[id].kind else {
            unreachable!("checked above")
        };
        // A hand on the content stops a fling in flight.
        let offset = state.animated(axis);
        offset.stop();
        offset.current = (offset.current + delta).clamp(0.0, max_scroll);
        self.note_scroll_input(id);
    }

    /// 0.5.6 (#166): the user scrolled `id`: if it snaps, it settles on a snap
    /// point once the input stops (`settle_snaps`, each tick).
    pub(super) fn note_scroll_input(&mut self, id: NodeId) {
        let snaps = matches!(
            self.nodes.get(id).map(|n| &n.kind),
            Some(NodeKind::ScrollView(state)) if state.snap.is_some()
        );
        if !snaps {
            return;
        }
        match self.snap_pending.iter_mut().find(|(view, _)| *view == id) {
            Some(entry) => entry.1 = None,
            None => self.snap_pending.push((id, None)),
        }
    }

    /// 0.5.6 (#166): where `view` should settle: the offset, in `[0, max]`, that
    /// lines up the item nearest its current one -- each item (a child of the
    /// scroll content) by its own `snap_align`, or the view's. `None` if there
    /// is nothing to snap to or it is already there.
    fn snap_target(&self, view: NodeId) -> Option<f64> {
        let node = self.nodes.get(view)?;
        let NodeKind::ScrollView(state) = &node.kind else {
            return None;
        };
        let align = state.snap?;
        let content = *node.children.first()?;
        let viewport_size = self.layout(view).size;
        let viewport = f64::from(if state.horizontal {
            viewport_size.width
        } else {
            viewport_size.height
        });
        let max = self.max_scroll(view)?;
        let offset = state.scroll.current;
        let mut best: Option<f64> = None;
        for &item in &self.nodes.get(content)?.children {
            let Some(item_node) = self.nodes.get(item) else {
                continue;
            };
            if !item_node.visible {
                continue;
            }
            let layout = self.layout(item);
            let (start, size) = if state.horizontal {
                (f64::from(layout.location.x), f64::from(layout.size.width))
            } else {
                (f64::from(layout.location.y), f64::from(layout.size.height))
            };
            let target = match item_node.snap_align.unwrap_or(align) {
                crate::SnapAlign::Start => start,
                crate::SnapAlign::Center => start + size / 2.0 - viewport / 2.0,
                crate::SnapAlign::End => start + size - viewport,
            }
            .clamp(0.0, max);
            if best.is_none_or(|b| (target - offset).abs() < (b - offset).abs()) {
                best = Some(target);
            }
        }
        best.filter(|target| (target - offset).abs() > 0.5)
    }

    /// 0.5.6 (#166): settles the scroll views the user has stopped scrolling.
    /// A view settles `SNAP_SETTLE` after its last input, once no animation
    /// (a fling) or thumb drag is moving it. Returns whether an animation
    /// began, so the loop keeps ticking.
    pub(super) fn settle_snaps(&mut self, now: Instant) -> bool {
        let pending = std::mem::take(&mut self.snap_pending);
        let mut keep = Vec::new();
        let mut started = false;
        for (id, since) in pending {
            let Some(NodeKind::ScrollView(state)) = self.nodes.get(id).map(|n| &n.kind) else {
                continue;
            };
            if state.snap.is_none() {
                continue;
            }
            if self.dragging == Some(id) || state.scroll.active.is_some() {
                keep.push((id, None));
                continue;
            }
            match since {
                None => keep.push((id, Some(now))),
                Some(t) if now.saturating_duration_since(t) < SNAP_SETTLE => {
                    keep.push((id, since));
                }
                Some(_) => {
                    if let Some(target) = self.snap_target(id)
                        && let Some(NodeKind::ScrollView(state)) =
                            self.nodes.get_mut(id).map(|n| &mut n.kind)
                    {
                        state.scroll.animate_to(
                            target,
                            SNAP_DURATION,
                            crate::MotionCurve::Bezier(0.2, 0.0, 0.0, 1.0),
                            now,
                        );
                        started = true;
                    }
                }
            }
        }
        self.snap_pending = keep;
        started
    }

    /// 0.6.11: a scroll view's own size and its content's, `(width, height)`
    /// each, by the last layout. `None` for a node that is not a scroll view or
    /// has no content yet.
    pub(super) fn scroll_view_boxes(&self, view: NodeId) -> Option<((f64, f64), (f64, f64))> {
        let node = self.nodes.get(view)?;
        if !matches!(node.kind, NodeKind::ScrollView(_)) {
            return None;
        }
        let &child = node.children.first()?;
        let (size, content) = (self.layout(view).size, self.layout(child).size);
        Some((
            (f64::from(size.width), f64::from(size.height)),
            (f64::from(content.width), f64::from(content.height)),
        ))
    }

    /// 0.6.11: the scrollbar thumbs `[vertical, horizontal]` of scroll view
    /// `view`, in its own coordinates and by its live offsets: one for each
    /// axis that overflows. The geometry painting, hit-testing, dragging and
    /// damage all read (`ScrollViewState::thumbs`). `None` for a node that is
    /// not a scroll view or has no content yet.
    pub fn scroll_thumbs(&self, view: NodeId) -> Option<[Option<Thumb>; 2]> {
        let (size, content) = self.scroll_view_boxes(view)?;
        let NodeKind::ScrollView(state) = &self.nodes[view].kind else {
            return None;
        };
        Some(state.thumbs(size, content, state.offsets()))
    }

    /// `point` (window coordinates) in `view`'s own.
    fn local_to_view(&self, view: NodeId, point: Point) -> Point {
        let (ox, oy) = self.absolute_position(view);
        Point::new(point.x - ox, point.y - oy)
    }

    /// M38 Phase 6 (§5, §7, §11.7): which of `view`'s scrollbar thumbs a real
    /// press at `point` (absolute canvas coordinates, the same space
    /// `PointerPressed`'s own `position` already arrives in) grabs --
    /// ported from pyCopper's own real `ScrollViewElement.grabs_thumb`
    /// (`widgets/scroll.py`): the thumb's hit area, which is wider than the
    /// thumb (a bare 4 px target is unusable with a mouse). `None` for a
    /// `ScrollView` with nothing to scroll (no thumb painted at all) or no
    /// children yet.
    pub(super) fn grabs_scroll_view_thumb(&self, view: NodeId, point: Point) -> Option<ScrollAxis> {
        let thumbs = self.scroll_thumbs(view)?;
        let local = self.local_to_view(view, point);
        thumbs
            .iter()
            .flatten()
            .find(|t| holds(t.hit, local))
            .map(|t| t.axis)
    }

    /// 0.6.11: a press at `point` on the lane of a `both` view's thumb, off the
    /// thumb itself, scrolls one page (`PAGE_FRACTION` of the viewport) toward
    /// it, and goes on doing so while held: after `PAGE_DELAY`, then every
    /// `PAGE_REPEAT` (`repeat_pages`), until the thumb reaches the press.
    /// Whether the press was on a lane (and is used up).
    pub(super) fn press_scroll_lane(&mut self, view: NodeId, point: Point, now: Instant) -> bool {
        let Some(NodeKind::ScrollView(state)) = self.nodes.get(view).map(|n| &n.kind) else {
            return false;
        };
        if !state.both {
            return false;
        }
        let (Some(thumbs), Some((size, _))) =
            (self.scroll_thumbs(view), self.scroll_view_boxes(view))
        else {
            return false;
        };
        let local = self.local_to_view(view, point);
        let Some(thumb) = thumbs.iter().flatten().find(|t| holds(t.lane, local)) else {
            return false;
        };
        let axis = thumb.axis;
        let (start, _) = thumb.span();
        let dir = if axis.pick((local.x, local.y)) < start {
            -1.0
        } else {
            1.0
        };
        self.scroll_axis_by(view, axis, dir * axis.pick(size) * PAGE_FRACTION);
        self.page_repeat = Some(PageRepeat {
            view,
            axis,
            press: axis.pick((point.x, point.y)),
            dir,
            next: now + PAGE_DELAY,
        });
        true
    }

    /// 0.6.11: the pages a held lane press (`press_scroll_lane`) is owed by
    /// `now`. It stops when the thumb has reached the press (or gone past it),
    /// or the view can go no further. Whether a press is still repeating,
    /// which keeps the loop ticking.
    pub(super) fn repeat_pages(&mut self, now: Instant) -> bool {
        let Some(mut repeat) = self.page_repeat else {
            return false;
        };
        // A held press owes a page every `PAGE_REPEAT`; a stalled loop catches up.
        for _ in 0..64 {
            if now < repeat.next {
                self.page_repeat = Some(repeat);
                return true;
            }
            let (Some(thumbs), Some((size, _))) = (
                self.scroll_thumbs(repeat.view),
                self.scroll_view_boxes(repeat.view),
            ) else {
                break;
            };
            let Some(thumb) = thumbs[repeat.axis.index()] else {
                break;
            };
            let (start, end) = thumb.span();
            let origin = repeat.axis.pick(self.absolute_position(repeat.view));
            // The thumb has the press under it, or has gone past it.
            let toward = if repeat.press < origin + start {
                -1.0
            } else if repeat.press > origin + end {
                1.0
            } else {
                0.0
            };
            if toward != repeat.dir || !self.can_scroll_axis(repeat.view, repeat.axis, repeat.dir) {
                break;
            }
            self.scroll_axis_by(
                repeat.view,
                repeat.axis,
                repeat.dir * repeat.axis.pick(size) * PAGE_FRACTION,
            );
            repeat.next += PAGE_REPEAT;
        }
        self.page_repeat = None;
        false
    }

    /// 0.6.11: the thumb of a `both` view the pointer is over -- its hit area,
    /// the nearest view first -- walking up from `from`, the node under it.
    fn scrollbar_under(&self, from: Option<NodeId>, point: Point) -> Option<(NodeId, ScrollAxis)> {
        let mut current = from;
        while let Some(id) = current {
            if matches!(&self.nodes[id].kind, NodeKind::ScrollView(state) if state.both)
                && let Some(axis) = self.grabs_scroll_view_thumb(id, point)
            {
                return Some((id, axis));
            }
            current = self.nodes[id].parent;
        }
        None
    }

    /// 0.6.11: the pointer moved over `hit` at `point`: the thumb under it
    /// takes the hover opacity, the one it left goes back to rest.
    pub(super) fn update_scrollbar_hover(
        &mut self,
        hit: Option<NodeId>,
        point: Point,
        now: Instant,
    ) {
        if self.scroll_view_count == 0 && self.scrollbar_hover.is_none() {
            return;
        }
        let under = self.scrollbar_under(hit, point);
        if under == self.scrollbar_hover {
            return;
        }
        let before = std::mem::replace(&mut self.scrollbar_hover, under);
        for (view, axis) in before.into_iter().chain(under) {
            self.fade_thumb(view, axis, now);
        }
    }

    /// 0.6.11: starts fading a `both` view's thumb to the emphasis it should have:
    /// dragged 2, hovered 1, else rest 0 (`ScrollViewState::level`), over
    /// `SCROLLBAR_FADE`.
    pub(super) fn fade_thumb(&mut self, view: NodeId, axis: ScrollAxis, now: Instant) {
        let Some(NodeKind::ScrollView(state)) = self.nodes.get(view).map(|n| &n.kind) else {
            return;
        };
        if !state.both {
            return;
        }
        let target = if self.dragging == Some(view)
            && state.thumb_drag_anchor.is_some()
            && state.thumb_drag_axis == axis
        {
            2.0
        } else if self.scrollbar_hover == Some((view, axis)) {
            1.0
        } else {
            0.0
        };
        if (*state.level[axis.index()].target() - target).abs() < 1e-9 {
            return;
        }
        if let NodeKind::ScrollView(state) = &mut self.nodes[view].kind {
            state.level[axis.index()].animate_to(
                target,
                SCROLLBAR_FADE,
                crate::MotionCurve::Bezier(0.2, 0.0, 0.0, 1.0),
                now,
            );
        }
    }

    /// M38 Phase 6 (§5, §7, §11.7): live-follows-the-cursor thumb drag,
    /// `update_drag`'s own real `ScrollView` arm -- ported directly
    /// from pyCopper's own real `ScrollViewElement.on_pointer_move`
    /// (`widgets/scroll.py`): thumb travel (`track - thumb`) maps to
    /// scroll travel (`max_scroll`) 1:1 by ratio, so the content keeps
    /// pace with the pointer instead of running ahead of or behind it.
    /// A relative-delta computation from `state.thumb_drag_anchor` (set
    /// by `PointerPressed`'s own dispatch arm below), not an absolute
    /// pointer-to-scroll mapping -- preserves wherever along the
    /// thumb's own length the real press actually grabbed it, the
    /// identical real UX pyCopper's own design already chose. It follows the
    /// pointer wherever it goes, outside the hit area too. A true
    /// no-op if the drag anchor is missing (defensive: `update_drag`'s
    /// own caller already guarantees `self.dragging == Some(view)`
    /// only after a real successful grab set it) or the real track has
    /// no room to travel.
    pub(super) fn update_scroll_view_thumb_drag(
        &mut self,
        view: NodeId,
        point: Point,
        _now: Instant,
    ) {
        let Some(thumbs) = self.scroll_thumbs(view) else {
            return;
        };
        let NodeKind::ScrollView(state) = &self.nodes[view].kind else {
            return;
        };
        let Some((anchor_coord, anchor_scroll)) = state.thumb_drag_anchor else {
            return;
        };
        let axis = state.thumb_drag_axis;
        let Some(thumb) = thumbs[axis.index()] else {
            return;
        };
        let travel = thumb.travel();
        if travel <= 0.0 {
            return;
        }
        let moved = axis.pick((point.x, point.y)) - anchor_coord;
        let target =
            (anchor_scroll + moved * (thumb.max_scroll / travel)).clamp(0.0, thumb.max_scroll);
        let NodeKind::ScrollView(state) = &mut self.nodes[view].kind else {
            unreachable!("checked above")
        };
        // A real, direct write, not `animate_to` -- the identical
        // "driven directly, never eased" precedent `ScrollViewState.
        // scroll`'s own doc comment already establishes for every
        // other real scroll mutation (`scroll_scroll_view_by`).
        state.animated(axis).current = target;
        self.note_scroll_input(view);
    }

    /// M47 (§5, §7, §11.7): a real `VirtualList`'s own live viewport
    /// extent along its (always vertical) scroll axis -- mirrors
    /// `scroll_view_extents`'s own viewport half, but `VirtualList` has
    /// no single measured child to read a "content extent" from (it's
    /// windowed materialization, `Fixed` or `Variable`), so content
    /// extent comes from `VirtualListState::total_extent()` directly at
    /// each real call site instead of being returned from here. `None`
    /// if `id` isn't a real `NodeKind::VirtualList` in this `Tree`.
    pub(super) fn virtual_list_viewport_extent(&self, id: NodeId) -> Option<f64> {
        let node = self.nodes.get(id)?;
        if !matches!(node.kind, NodeKind::VirtualList(_)) {
            return None;
        }
        Some(f64::from(self.layout(id).size.height))
    }

    /// M47 (§5, §7, §11.7): whether a real press at `point` grabs
    /// `list`'s own real scrollbar thumb -- the identical real grab-
    /// tolerance technique `grabs_scroll_view_thumb` (M38 Phase 6)
    /// already established, narrowed to `VirtualList`'s own vertical-
    /// only axis. `false` when there's nothing to scroll (`total_
    /// extent() <= viewport`, the same real condition `engine-render`'s
    /// own thumb-paint arm uses) or the list has no real viewport yet.
    pub(super) fn grabs_virtual_list_thumb(&self, list: NodeId, point: Point) -> bool {
        let Some(viewport) = self.virtual_list_viewport_extent(list) else {
            return false;
        };
        let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
            return false;
        };
        if state.total_extent() <= viewport {
            return false;
        }
        let (track, thumb, along) = state.thumb_geometry(viewport);
        if track <= 0.0 {
            return false;
        }
        let (ox, oy) = self.absolute_position(list);
        let size = self.layout(list).size;
        let slop = SCROLLBAR_GRAB_SLOP;
        let tx = ox + f64::from(size.width) - SCROLLBAR_THICKNESS - SCROLLBAR_MARGIN;
        let ty = oy + along;
        (tx - slop..=tx + SCROLLBAR_THICKNESS + slop).contains(&point.x)
            && (ty - slop..=ty + thumb + slop).contains(&point.y)
    }

    /// M47 (§5, §7, §11.7): live-follows-the-cursor thumb drag for a
    /// `VirtualList` -- the identical real ratio-mapped technique
    /// `update_scroll_view_thumb_drag` (M38 Phase 6) already
    /// established, narrowed to the vertical-only axis, writing through
    /// the same clamp `Tree::scroll_virtual_list_by` already uses so
    /// wheel-scroll and thumb-drag can never disagree about the real
    /// clamp bounds. A true no-op if the drag anchor is missing or the
    /// real track has no room to travel.
    pub(super) fn update_virtual_list_thumb_drag(&mut self, list: NodeId, point: Point) {
        let Some(viewport) = self.virtual_list_viewport_extent(list) else {
            return;
        };
        let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
            return;
        };
        let max_scroll = (state.total_extent() - viewport).max(0.0);
        let Some((anchor_y, anchor_scroll)) = state.thumb_drag_anchor else {
            return;
        };
        let (track, thumb, _along) = state.thumb_geometry(viewport);
        let travel = track - thumb;
        if travel <= 0.0 {
            return;
        }
        let moved = point.y - anchor_y;
        let target = (anchor_scroll + moved * (max_scroll / travel)).clamp(0.0, max_scroll);
        let NodeKind::VirtualList(state) = &mut self.nodes[list].kind else {
            unreachable!("checked above")
        };
        // A real, direct write, not `animate_to` -- the identical
        // "driven directly, never eased" precedent `VirtualListState.
        // scroll_offset`'s own doc comment already establishes.
        state.scroll_offset.current = target;
    }

    /// Moves the drag in progress along with the pointer: a scrollbar
    /// thumb's, the only drags the engine still owns (M99 removed the
    /// splitter, slider, carousel, and time-picker-dial drags with their
    /// kinds). `PointerPressed` only sets `self.dragging` after a real
    /// thumb grab (`grabs_scroll_view_thumb`/`grabs_virtual_list_thumb`).
    pub(super) fn update_drag(&mut self, point: Point, now: Instant) {
        let Some(dragging) = self.dragging else {
            return;
        };
        match &self.nodes[dragging].kind {
            NodeKind::ScrollView(_) => self.update_scroll_view_thumb_drag(dragging, point, now),
            NodeKind::VirtualList(_) => self.update_virtual_list_thumb_drag(dragging, point),
            _ => {}
        }
    }

    /// §14 step 15 (§11.7): materializes/recycles a `NodeKind::
    /// VirtualList`'s real children to match exactly `visible` (the
    /// caller's own already-overscanned logical-index range) -- the
    /// concrete mechanism behind §11.7's own claim that a 100,000-row
    /// list never needs 100,000 real `Node`s.
    ///
    /// An index newly entering `visible` is built by calling
    /// `materialize(index)` for its `(NodeKind, Style, PaintProperties)`,
    /// then inserted and absolutely positioned by this method itself --
    /// `top: state.offset_of(idx)` (M12 Phase 1: `idx * item_extent` for
    /// `Fixed`, a real resolved cumulative offset for `Variable`),
    /// vertical-list only (the common list/data-grid case; a horizontal
    /// virtual list would need the same treatment along the other axis,
    /// not built since nothing here needs it yet). `Position::Absolute` here resolves against `list`
    /// itself, `list` being the item's own direct parent -- verified
    /// directly in `taffy`'s own `compute/flexbox.rs` at step 15 Stage A
    /// (no separate "positioned ancestor" walk needed, matching
    /// `open_overlay`'s own precedent). An index leaving `visible` is
    /// dropped via `Tree::remove` -- its real generational `NodeId`
    /// invalidation (`SlotMap`'s own behavior, §5) is what makes a stray
    /// reference to a scrolled-away item fail safely rather than
    /// silently reading whatever a reused slot now holds; nothing extra
    /// is needed for "recycling" beyond this ordinary remove+insert, per
    /// §11.7's own text.
    ///
    /// Panics if `list` isn't a `NodeKind::VirtualList` -- an internal
    /// bookkeeping bug, not a runtime condition.
    pub fn set_virtual_list_window(
        &mut self,
        list: NodeId,
        visible: std::ops::Range<usize>,
        mut materialize: impl FnMut(usize) -> (NodeKind, Style, PaintProperties),
    ) {
        self.dirty = true;
        match &self
            .nodes
            .get(list)
            .expect("set_virtual_list_window: NodeId not found in this Tree")
            .kind
        {
            NodeKind::VirtualList(_) => {}
            _ => panic!("set_virtual_list_window: {list:?} is not a NodeKind::VirtualList"),
        };

        let currently_materialized: Vec<(usize, NodeId)> = match &self.nodes[list].kind {
            NodeKind::VirtualList(state) => {
                state.materialized.iter().map(|(&i, &id)| (i, id)).collect()
            }
            _ => unreachable!("checked at the top of this function"),
        };

        for (idx, id) in &currently_materialized {
            if !visible.contains(idx) {
                self.remove(*id);
            }
        }
        if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
            state.materialized.retain(|idx, _| visible.contains(idx));
        }

        let already_materialized: std::collections::BTreeSet<usize> = match &self.nodes[list].kind {
            NodeKind::VirtualList(state) => state.materialized.keys().copied().collect(),
            _ => unreachable!("checked at the top of this function"),
        };

        for idx in visible {
            if already_materialized.contains(&idx) {
                continue;
            }
            let (kind, mut style, paint) = materialize(idx);
            let top = match &self.nodes[list].kind {
                NodeKind::VirtualList(state) => state.offset_of(idx),
                _ => unreachable!("checked at the top of this function"),
            };
            style.position = Position::Absolute;
            style.inset = TaffyRect {
                left: length(0.0),
                top: length(top as f32),
                right: auto(),
                bottom: auto(),
            };
            let id = self.insert(kind, style, paint);
            self.add_child(list, id);
            if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
                state.materialized.insert(idx, id);
            }
        }
    }

    /// M8 Phase 3 (§11.7): moves `id`'s own real `scroll_offset` by
    /// `delta_y` real pixels, clamped to `[0.0, max_offset]` --
    /// `max_offset` is `id`'s own real content extent (`VirtualList
    /// State::total_extent`, M12 Phase 1: `item_extent * item_count`
    /// for `Fixed`, the real resolved sum for `Variable`) minus its own
    /// real, computed viewport height (`Tree::layout`), floored at
    /// `0.0` (a list whose content is shorter than its own viewport
    /// can't scroll at all, correctly).
    /// A positive `delta_y` increases the offset (content moves up,
    /// later items come into view) -- this crate's own chosen, stated
    /// convention (`PLAN.md`), not one `winit`'s own docs pin down.
    /// Exposed as its own real method `dispatch` reuses internally --
    /// panics if `id` isn't a real `NodeKind::VirtualList` in this
    /// `Tree` (an internal bug, not a runtime condition).
    pub fn scroll_virtual_list_by(&mut self, id: NodeId, delta_y: f64) {
        self.dirty = true;
        let node = self
            .nodes
            .get(id)
            .expect("scroll_virtual_list_by: NodeId not found in this Tree");
        let NodeKind::VirtualList(state) = &node.kind else {
            panic!("scroll_virtual_list_by: {id:?} is not a NodeKind::VirtualList");
        };
        let content_extent = state.total_extent();
        let viewport_height = f64::from(self.layout(id).size.height);
        let max_offset = (content_extent - viewport_height).max(0.0);

        let NodeKind::VirtualList(state) = &mut self.nodes[id].kind else {
            unreachable!("checked above")
        };
        state.scroll_offset.stop();
        state.scroll_offset.current =
            (state.scroll_offset.current + delta_y).clamp(0.0, max_offset);
    }

    /// M96: the rows of `list` its viewport shows -- every row whose extent
    /// meets `[scroll, scroll + height)` -- by binary search over the
    /// rows' offsets, so a long list costs `log n`. Needs a computed
    /// layout.
    pub fn virtual_list_visible(&self, list: NodeId) -> std::ops::Range<usize> {
        let Some(NodeKind::VirtualList(state)) = self.nodes.get(list).map(|n| &n.kind) else {
            return 0..0;
        };
        let top = state.scroll_offset.current;
        let bottom = top + f64::from(self.layout(list).size.height);
        // The first row whose `edge_of` passes `past`, for a list whose
        // offsets only grow.
        let first = |edge_of: &dyn Fn(usize) -> f64, past: &dyn Fn(f64) -> bool| {
            let (mut lo, mut hi) = (0, state.item_count);
            while lo < hi {
                let mid = (lo + hi) / 2;
                if past(edge_of(mid)) {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            lo
        };
        // Visible rows end after `top` and start before `bottom`.
        let start = first(&|idx| state.offset_of(idx + 1), &|end| end > top);
        let end = first(&|idx| state.offset_of(idx), &|begin| begin >= bottom);
        start..end.max(start)
    }

    /// M96: detaches every materialized row of `list` outside `keep`, and
    /// returns them -- still alive, for the caller to free or keep.
    pub fn virtual_list_release_outside(
        &mut self,
        list: NodeId,
        keep: std::ops::Range<usize>,
    ) -> Vec<NodeId> {
        // The row holding keyboard focus stays built, so its keys keep working.
        let pinned = self.virtual_list_focused_row(list);
        let released: Vec<NodeId> = match self.nodes.get(list).map(|n| &n.kind) {
            Some(NodeKind::VirtualList(state)) => state
                .materialized
                .iter()
                .filter(|(idx, _)| !keep.contains(idx) && Some(**idx) != pinned)
                .map(|(_, &id)| id)
                .collect(),
            _ => return Vec::new(),
        };
        if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
            state
                .materialized
                .retain(|idx, _| keep.contains(idx) || Some(*idx) == pinned);
        }
        for &row in &released {
            if self.nodes.get(row).and_then(|n| n.parent) == Some(list) {
                self.detach(list, row);
            }
        }
        released
    }

    /// M96: attaches `row` -- a node the caller built -- as row `index` of
    /// `list`, the list's full width and the row's own extent tall; layout
    /// places it at its offset (`sync_virtual_list_layouts`). Returns
    /// `false`, changing nothing, if `row` is `list` or an ancestor.
    pub fn virtual_list_adopt(&mut self, list: NodeId, index: usize, row: NodeId) -> bool {
        let extent = match self.nodes.get(list).map(|n| &n.kind) {
            Some(NodeKind::VirtualList(state)) => {
                state.offset_of(index + 1) - state.offset_of(index)
            }
            _ => return false,
        };
        if !self.try_add_child(list, row) {
            return false;
        }
        let mut style = self.nodes[row].layout_style.clone();
        style.size = Size {
            width: taffy::prelude::percent(1.0),
            height: length(extent as f32),
        };
        self.set_layout_style(row, style);
        if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
            state.materialized.insert(index, row);
        }
        true
    }

    /// M12 Phase 1 (§11.7): the real way a caller supplies resolved
    /// cumulative offsets for a `Variable`-extent list -- `engine-core`
    /// itself never computes a cumulative sum from raw per-item heights
    /// (no size-hint callback lives here, §4); it only stores what it's
    /// given, mirroring `materialized`'s own "resolve once, cache"
    /// shape. `offsets`'s own key `item_count` (one past the last real
    /// item) is where the real total content extent belongs -- see
    /// `VirtualListState::offset_of`/`total_extent`'s own doc comments.
    /// Panics if `list` isn't a real `NodeKind::VirtualList` in this
    /// `Tree`, the same "internal bug, not a runtime condition"
    /// contract `scroll_virtual_list_by` already uses.
    pub fn set_virtual_list_resolved_offsets(
        &mut self,
        list: NodeId,
        offsets: impl IntoIterator<Item = (usize, f64)>,
    ) {
        self.dirty = true;
        let NodeKind::VirtualList(state) = &mut self
            .nodes
            .get_mut(list)
            .expect("set_virtual_list_resolved_offsets: NodeId not found in this Tree")
            .kind
        else {
            panic!("set_virtual_list_resolved_offsets: {list:?} is not a NodeKind::VirtualList");
        };
        state.resolved_offsets.extend(offsets);
    }

    /// The axis `key` scrolls `state`'s content along, if it scrolls it at all:
    /// Up/Down and Page Up/Down for a vertical view, Left/Right for a
    /// horizontal one, Home/End for either (0.6.11: a `both` view takes Up,
    /// Down, Home and End on the vertical axis, Left and Right on the
    /// horizontal, and Page Up/Down on the vertical or, with Shift, the
    /// horizontal -- one-axis views take Shift as no different).
    fn key_axis(state: &ScrollViewState, key: Key, shift: bool) -> Option<ScrollAxis> {
        if state.both {
            return match key {
                Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => Some(ScrollAxis::Vertical),
                Key::ArrowLeft | Key::ArrowRight => Some(ScrollAxis::Horizontal),
                Key::PageUp | Key::PageDown if shift => Some(ScrollAxis::Horizontal),
                Key::PageUp | Key::PageDown => Some(ScrollAxis::Vertical),
                _ => None,
            };
        }
        let wants = match key {
            Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => ScrollAxis::Vertical,
            Key::ArrowLeft | Key::ArrowRight => ScrollAxis::Horizontal,
            Key::Home | Key::End => state.main_axis(),
            _ => return None,
        };
        (wants == state.main_axis()).then_some(wants)
    }

    /// 0.4.2 M12 (issue #24): the scroll view `key` scrolls from `from` --
    /// `from` itself or its nearest ancestor that's a scroll view along the
    /// key's axis: Up/Down and Page Up/Down for a vertical one, Left/Right
    /// for a horizontal one, Home/End for either. So arrows in a carousel
    /// inside a page still move the page up and down.
    ///
    /// 0.4.4 M21: and that can move the key's way (`can_scroll`) -- Down,
    /// Right, Page Down, and End toward the end, the rest toward the start
    /// -- so a key at an inner view's end moves the view outside it, as
    /// the wheel does and as in a browser. `None` when nothing can.
    /// 0.6.11: a `both` view has both axes (`key_axis`); whether Shift is held
    /// is the tree's (`set_held_modifiers`).
    pub fn scroll_view_for_key(&self, from: NodeId, key: Key) -> Option<NodeId> {
        let toward = match key {
            Key::ArrowUp | Key::PageUp | Key::ArrowLeft | Key::Home => -1.0,
            Key::ArrowDown | Key::PageDown | Key::ArrowRight | Key::End => 1.0,
            _ => return None,
        };
        self.ancestors(from).find(|&id| match &self.nodes[id].kind {
            NodeKind::ScrollView(state) => Self::key_axis(state, key, self.held.shift)
                .is_some_and(|axis| self.can_scroll_axis(id, axis, toward)),
            // A virtual list is vertical: every key but Left/Right.
            NodeKind::VirtualList(_) => {
                !matches!(key, Key::ArrowLeft | Key::ArrowRight) && self.can_scroll(id, toward)
            }
            _ => false,
        })
    }

    /// 0.4.2 M12: scrolls `view` as `key` says -- an arrow by
    /// `KEY_SCROLL_LINE`, Page Up/Down by the viewport (0.6.11: a `both` view's
    /// by the viewport less a tenth), Home/End to the ends -- at once, not
    /// eased. Whether the offset changed.
    pub fn scroll_by_key(&mut self, view: NodeId, key: Key) -> bool {
        if let Some(moved) = self.virtual_list_scroll_by_key(view, key) {
            return moved;
        }
        let Some((size, content)) = self.scroll_view_boxes(view) else {
            return false;
        };
        let NodeKind::ScrollView(state) = &mut self.nodes[view].kind else {
            return false;
        };
        let Some(axis) = Self::key_axis(state, key, self.held.shift) else {
            return false;
        };
        let viewport = axis.pick(size);
        let max_scroll = (axis.pick(content) - viewport).max(0.0);
        let page = if state.both {
            viewport * PAGE_FRACTION
        } else {
            viewport
        };
        let offset = state.animated(axis);
        let now = offset.current;
        let to = match key {
            Key::ArrowUp | Key::ArrowLeft => now - KEY_SCROLL_LINE,
            Key::ArrowDown | Key::ArrowRight => now + KEY_SCROLL_LINE,
            Key::PageUp => now - page,
            Key::PageDown => now + page,
            Key::Home => 0.0,
            Key::End => max_scroll,
            _ => return false,
        }
        .clamp(0.0, max_scroll);
        if to == now {
            return false;
        }
        offset.current = to;
        self.dirty = true;
        true
    }

    /// 0.4.2 M12 (issue #24): scrolls every scroll view around `id` just
    /// enough to show it -- innermost first, each by the least that brings
    /// the node's box inside its viewport along its axis (to the box's start
    /// when the box is longer than the viewport), at once. Positions come
    /// from the last layout, which callers keep current (`simulate` and
    /// every frame lay out first). Whether anything scrolled. A `both` view
    /// does that on each of its axes.
    pub fn scroll_into_view(&mut self, id: NodeId) -> bool {
        if !self.nodes.contains_key(id) {
            return false;
        }
        let (x, y) = self.absolute_position(id);
        let size = self.layout(id).size;
        let (mut x0, mut y0) = (x, y);
        let (mut x1, mut y1) = (x + f64::from(size.width), y + f64::from(size.height));
        let views: Vec<NodeId> = self
            .ancestors(id)
            .skip(1)
            .filter(|&a| {
                matches!(
                    self.nodes[a].kind,
                    NodeKind::ScrollView(_) | NodeKind::VirtualList(_)
                )
            })
            .collect();
        let mut moved = false;
        for view in views {
            if matches!(self.nodes[view].kind, NodeKind::VirtualList(_)) {
                let shift = self.virtual_list_reveal(view, y0, y1);
                if shift != 0.0 {
                    moved = true;
                    y0 -= shift;
                    y1 -= shift;
                }
                continue;
            }
            let Some((view_size, content)) = self.scroll_view_boxes(view) else {
                continue;
            };
            let NodeKind::ScrollView(state) = &self.nodes[view].kind else {
                continue;
            };
            let axes = state.axes();
            let origin = self.absolute_position(view);
            for &axis in axes {
                let (viewport, content) = (axis.pick(view_size), axis.pick(content));
                let (start, end) = match axis {
                    ScrollAxis::Horizontal => (x0, x1),
                    ScrollAxis::Vertical => (y0, y1),
                };
                let v_start = axis.pick(origin);
                let v_end = v_start + viewport;
                let delta = if start < v_start || end - start > viewport {
                    start - v_start
                } else if end > v_end {
                    end - v_end
                } else {
                    continue;
                };
                let max_scroll = (content - viewport).max(0.0);
                let NodeKind::ScrollView(state) = &mut self.nodes[view].kind else {
                    continue;
                };
                let offset = state.animated(axis);
                let before = offset.current;
                let after = (before + delta).clamp(0.0, max_scroll);
                offset.current = after;
                // The node moves with the content, which the next view out
                // sees.
                let shift = after - before;
                if shift != 0.0 {
                    moved = true;
                    match axis {
                        ScrollAxis::Horizontal => {
                            x0 -= shift;
                            x1 -= shift;
                        }
                        ScrollAxis::Vertical => {
                            y0 -= shift;
                            y1 -= shift;
                        }
                    }
                }
            }
        }
        if moved {
            self.dirty = true;
        }
        moved
    }

    /// 0.4.3 M15: how far scroll view `view` can scroll along its axis --
    /// its content (first child) past its viewport, `0.0` if it fits --
    /// by the last layout; `None` for a node that isn't a scroll view or has
    /// no content yet. The one range every clamp uses: layout's own, and
    /// `animate`'s target. A `both` view's is its vertical (main) axis'.
    pub fn max_scroll(&self, view: NodeId) -> Option<f64> {
        let node = self.nodes.get(view)?;
        // 0.5.6 (#166): a virtual list's furthest offset: its rows' extent less its height.
        if let NodeKind::VirtualList(state) = &node.kind {
            let viewport = f64::from(self.layout(view).size.height);
            return Some((state.total_extent() - viewport).max(0.0));
        }
        let NodeKind::ScrollView(state) = &node.kind else {
            return None;
        };
        self.max_scroll_axis(view, state.main_axis())
    }

    /// 0.6.11: `max_scroll` along `axis`. An axis the view does not scroll
    /// along has a range of nothing.
    pub fn max_scroll_axis(&self, view: NodeId, axis: ScrollAxis) -> Option<f64> {
        let node = self.nodes.get(view)?;
        let NodeKind::ScrollView(state) = &node.kind else {
            return None;
        };
        let &child = node.children.first()?;
        if !state.axes().contains(&axis) {
            return Some(0.0);
        }
        let (outer, inner) = (self.layout(view).size, self.layout(child).size);
        let (content, viewport) = match axis {
            ScrollAxis::Horizontal => (inner.width, outer.width),
            ScrollAxis::Vertical => (inner.height, outer.height),
        };
        Some((f64::from(content) - f64::from(viewport)).max(0.0))
    }

    /// 0.4.4 M21: whether scroll view or virtual list `id` can move by
    /// `delta` along its own axis -- positive toward the end -- at all:
    /// something to scroll, and not already at that end. What scroll
    /// chaining asks of each view on the way out: one that can't passes the
    /// wheel, or the key, to the next. `false` for any other node, or a
    /// `delta` of zero. A `both` view's own axis is its vertical.
    pub fn can_scroll(&self, id: NodeId, delta: f64) -> bool {
        let (current, max) = match self.nodes.get(id).map(|n| &n.kind) {
            Some(NodeKind::ScrollView(state)) => {
                return self.can_scroll_axis(id, state.main_axis(), delta);
            }
            Some(NodeKind::VirtualList(state)) => {
                let viewport = f64::from(self.layout(id).size.height);
                (
                    state.scroll_offset.current,
                    (state.total_extent() - viewport).max(0.0),
                )
            }
            _ => return false,
        };
        (delta > 0.0 && current < max) || (delta < 0.0 && current > 0.0)
    }

    /// 0.6.11: `can_scroll` for scroll view `id` along `axis`.
    pub fn can_scroll_axis(&self, id: NodeId, axis: ScrollAxis, delta: f64) -> bool {
        let Some(NodeKind::ScrollView(state)) = self.nodes.get(id).map(|n| &n.kind) else {
            return false;
        };
        let Some(max) = self.max_scroll_axis(id, axis) else {
            return false;
        };
        let current = state.offset(axis);
        (delta > 0.0 && current < max) || (delta < 0.0 && current > 0.0)
    }

    /// 0.4.2 M12 (issue #24): every scroll view whose offset has changed
    /// since this was last asked, by comparison, so no cause is missed: the
    /// wheel, keys, `scroll_into_view`, focus, `set`, a thumb drag, or an
    /// animation. 0.6.11: on either axis of a `both` view.
    pub fn take_scroll_changes(&mut self) -> Vec<ScrollChange> {
        let mut changes = Vec::new();
        if self.scroll_view_count == 0 {
            return self.take_virtual_list_scroll_changes();
        }
        // Found with a read-only pass: iterating `&mut self.nodes` counts every
        // node as touched, which sent the damage tracker down its full walk on
        // every frame of any tree that has a scroll view.
        let moved: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, node)| {
                matches!(&node.kind, NodeKind::ScrollView(s)
                    if s.scroll.current != s.reported || s.scroll_x.current != s.reported_x)
            })
            .map(|(id, _)| id)
            .collect();
        for id in moved {
            if let NodeKind::ScrollView(state) = &mut self.nodes[id].kind {
                let (x, y) = state.offsets();
                changes.push(ScrollChange {
                    view: id,
                    old: state.reported,
                    new: state.scroll.current,
                    x,
                    y,
                });
                state.reported = state.scroll.current;
                state.reported_x = state.scroll_x.current;
            }
        }
        changes.extend(self.take_virtual_list_scroll_changes());
        changes
    }
}

/// 0.6.11: a scroll view's offset changed (`Tree::take_scroll_changes`): the
/// main-axis offset before (`old`) and after (`new`), and where the content is
/// now, `x` pixels scrolled right and `y` down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollChange {
    pub view: NodeId,
    pub old: f64,
    pub new: f64,
    pub x: f64,
    pub y: f64,
}

/// 0.6.11: a press holding the lane of a scroll view's thumb, which pages again
/// (`Tree::repeat_pages`): `press` is where along `axis` (window coordinates),
/// `dir` the way the page goes, `next` when the next page is owed.
#[derive(Clone, Copy)]
pub(super) struct PageRepeat {
    view: NodeId,
    axis: ScrollAxis,
    press: f64,
    dir: f64,
    next: Instant,
}
