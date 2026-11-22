//! 0.6.13: a multiline text input's own scrollbar. The input scrolls its text by its own two
//! offsets (`scroll_offset`, `horizontal_scroll_offset`, which the caret-follow and the paint
//! read); the bar is the scroll view's (`scrollbar.rs`: thumb, track, arrows, lane click, page,
//! fades, hit area) laid over them, so a thumb dragged, a lane or an arrow pressed writes those
//! same offsets, and the caret-follow and the wheel move the thumb. The input keeps its keys: the
//! caret's, not the bar's. The wheel moves it only while the bar is on.
//!
//! Off until `scrollbar_show` says otherwise (`TextFieldState::bar_on`), so an input that sets
//! nothing looks and behaves as it did. The thumb is sized from how big the shaped text is, which
//! only the window's text renderer can say (`engine-core` has no font): it measures while the bar
//! is on and hands the answer to `Tree::set_text_field_extent`.

use super::*;
use crate::animation::Animated;
use crate::scrollbar::{Defaults, ScrollAxis, ScrollbarState, Scroller, Thumb};
use crate::scrollbar_props::{Prop, Show};

impl TextFieldState {
    /// Whether the input's bar is on: a multiline input that has `scrollbar_show` set to
    /// something but `never`. (A single-line input takes and keeps the properties, and shows no
    /// bar; one that is not multiline yet has none until it is.)
    pub fn bar_on(&self) -> bool {
        self.multiline
            && self
                .bar
                .settings
                .choice(Prop::Show)
                .is_some_and(|show| show != Show::Never.index())
    }
}

impl Scroller for TextFieldState {
    fn bar(&self) -> &ScrollbarState {
        &self.bar
    }

    fn bar_mut(&mut self) -> &mut ScrollbarState {
        &mut self.bar
    }

    fn defaults(&self) -> Defaults {
        Defaults::Field
    }

    /// The vertical axis of a multiline input; its horizontal too when its lines are not
    /// wrapped (`wrap` other than `"word"`), as they then run on sideways. None for a
    /// single-line input.
    fn scroll_axes(&self) -> &'static [ScrollAxis] {
        if !self.multiline {
            &[]
        } else if self.wraps() {
            &[ScrollAxis::Vertical]
        } else {
            &ScrollAxis::BOTH
        }
    }

    fn scroll_offsets(&self) -> (f64, f64) {
        (
            self.horizontal_scroll_offset.current,
            self.scroll_offset.current,
        )
    }

    fn offset_anim(&mut self, axis: ScrollAxis) -> &mut Animated<f64> {
        match axis {
            ScrollAxis::Vertical => &mut self.scroll_offset,
            ScrollAxis::Horizontal => &mut self.horizontal_scroll_offset,
        }
    }

    /// No thumbs while the bar is off, without resolving its measures (every input asks).
    fn thumbs(
        &self,
        size: (f64, f64),
        content: (f64, f64),
        offsets: (f64, f64),
    ) -> [Option<Thumb>; 2] {
        if !self.bar_on() {
            return [None, None];
        }
        self.metrics()
            .thumbs(self.scroll_axes(), size, content, offsets)
    }
}

impl Tree {
    /// The text of input `field` as a scroller's content: the shaped text (`content_extent`) and
    /// the padding around it, so the content less the box is how far the text scrolls
    /// (the box being the node's, its content box inside the padding). A wrapped input's lines
    /// are as wide as the box.
    pub(super) fn field_content_box(
        &self,
        field: NodeId,
        state: &TextFieldState,
        size: (f64, f64),
    ) -> (f64, f64) {
        let pad = self.layout(field).padding;
        let (horizontal, vertical) = (
            f64::from(pad.left + pad.right),
            f64::from(pad.top + pad.bottom),
        );
        let width = if state.wraps() {
            size.0
        } else {
            state.content_extent.0 + horizontal
        };
        (width, state.content_extent.1 + vertical)
    }

    /// The text inputs whose scrollbar is on: the ones the window's text renderer measures
    /// (`set_text_field_extent`). Empty without any text input in the tree.
    pub fn bar_fields(&self) -> Vec<NodeId> {
        if self.text_field_count == 0 {
            return Vec::new();
        }
        self.nodes
            .iter()
            .filter(|(_, node)| matches!(&node.kind, NodeKind::TextField(state) if state.bar_on()))
            .map(|(id, _)| id)
            .collect()
    }

    /// Sets how big the shaped text of input `id` is, `(width, height)` in pixels, and the stamp
    /// of what it was measured from (`TextFieldState::extent_stamp`), touching the node only when
    /// something changed, so a settled input costs no frame. A text that got shorter pulls the
    /// scroll back to the end of what is left.
    pub fn set_text_field_extent(&mut self, id: NodeId, extent: (f64, f64), stamp: u64) {
        let Some(NodeKind::TextField(state)) = self.nodes.get(id).map(|n| &n.kind) else {
            return;
        };
        if state.content_extent == extent && state.extent_stamp == stamp {
            return;
        }
        let moved = state.content_extent != extent;
        let Some(NodeKind::TextField(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind) else {
            return;
        };
        state.content_extent = extent;
        state.extent_stamp = stamp;
        if !moved {
            return;
        }
        self.dirty = true;
        for axis in ScrollAxis::BOTH {
            let Some(max) = self.max_scroll_axis(id, axis) else {
                continue;
            };
            let Some(NodeKind::TextField(state)) = self.nodes.get_mut(id).map(|n| &mut n.kind)
            else {
                return;
            };
            let offset = state.offset_anim(axis);
            if offset.current > max {
                offset.current = max;
            }
        }
    }
}
