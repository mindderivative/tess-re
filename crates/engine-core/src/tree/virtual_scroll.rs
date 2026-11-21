//! Virtual lists scrolled by key, by row index and by revealing a row: the
//! same keys and the same least-distance rule as a scroll view
//! ([UX spec "List", "Scrollbar and Scroll Container"]), kept apart from
//! `scroll.rs` which hooks into these.

use super::*;

/// Where `Tree::scroll_virtual_list_to_index` puts a row in the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAlign {
    /// The least scroll that shows the whole row; none if it is already
    /// shown. A row taller than the viewport goes to the viewport's start.
    Nearest,
    /// The row's top at the viewport's top (as far as the list can scroll).
    Start,
    /// The row's middle at the viewport's middle.
    Center,
    /// The row's bottom at the viewport's bottom.
    End,
}

impl ScrollAlign {
    /// `"nearest"`, `"start"`, `"center"` or `"end"`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "nearest" => Some(Self::Nearest),
            "start" => Some(Self::Start),
            "center" => Some(Self::Center),
            "end" => Some(Self::End),
            _ => None,
        }
    }
}

/// A page key moves by the viewport less this share of it, so the rows at
/// the old edge stay in view.
const PAGE_OVERLAP: f64 = 0.1;

impl Tree {
    /// A virtual list's current offset, viewport height and furthest offset;
    /// `None` for any other node.
    fn virtual_list_span(&self, list: NodeId) -> Option<(f64, f64, f64)> {
        let NodeKind::VirtualList(state) = &self.nodes.get(list)?.kind else {
            return None;
        };
        let viewport = f64::from(self.layout(list).size.height);
        let max = (state.total_extent() - viewport).max(0.0);
        Some((state.scroll_offset.current, viewport, max))
    }

    /// Sets virtual list `list`'s offset to `to` (clamped) at once, stopping
    /// any animation; whether it changed.
    fn set_virtual_list_offset(&mut self, list: NodeId, to: f64) -> bool {
        let Some((now, _, max)) = self.virtual_list_span(list) else {
            return false;
        };
        let to = to.clamp(0.0, max);
        if to == now {
            return false;
        }
        if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind {
            state.scroll_offset.stop();
            state.scroll_offset.current = to;
        }
        self.dirty = true;
        true
    }

    /// The index of the built row of `list` that is, or holds, the focused
    /// node. Such a row is kept built when it scrolls out of the window.
    pub(super) fn virtual_list_focused_row(&self, list: NodeId) -> Option<usize> {
        let NodeKind::VirtualList(state) = &self.nodes.get(list)?.kind else {
            return None;
        };
        let row = self
            .ancestors(self.focused?)
            .find(|&id| self.nodes[id].parent == Some(list))?;
        state
            .materialized
            .iter()
            .find_map(|(&idx, &id)| (id == row).then_some(idx))
            // A row past the end (the list shrank) is not kept: it has no offset.
            .filter(|&idx| idx < state.item_count)
    }

    /// `scroll_by_key` for a virtual list: `None` when `list` is not one.
    /// An arrow moves `KEY_SCROLL_LINE`, Page Up/Down the viewport less 10
    /// percent, Home/End to the ends; at once, not eased.
    pub(super) fn virtual_list_scroll_by_key(&mut self, list: NodeId, key: Key) -> Option<bool> {
        let (now, viewport, max) = self.virtual_list_span(list)?;
        let page = viewport * (1.0 - PAGE_OVERLAP);
        let to = match key {
            Key::ArrowUp => now - KEY_SCROLL_LINE,
            Key::ArrowDown => now + KEY_SCROLL_LINE,
            Key::PageUp => now - page,
            Key::PageDown => now + page,
            Key::Home => 0.0,
            Key::End => max,
            _ => return Some(false),
        };
        Some(self.set_virtual_list_offset(list, to))
    }

    /// Scrolls virtual list `list` by the least that shows the span
    /// `y0..y1` (window space), as `scroll_into_view` does for a scroll
    /// view; returns how far the offset moved (positive toward the end).
    pub(super) fn virtual_list_reveal(&mut self, list: NodeId, y0: f64, y1: f64) -> f64 {
        let Some((now, viewport, _)) = self.virtual_list_span(list) else {
            return 0.0;
        };
        let top = self.absolute_position(list).1;
        let delta = if y0 < top || y1 - y0 > viewport {
            y0 - top
        } else if y1 > top + viewport {
            y1 - (top + viewport)
        } else {
            return 0.0;
        };
        self.set_virtual_list_offset(list, now + delta);
        self.virtual_list_span(list)
            .map_or(0.0, |(after, _, _)| after - now)
    }

    /// Scrolls virtual list `list` so row `index` shows as `align` says,
    /// from the offsets the list knows -- `index * item_extent`, or the
    /// resolved `size_hint` offsets -- so a row that is not built is
    /// reached too. Needs a computed layout (the viewport height and, for a
    /// `size_hint` list, the offsets). Whether the offset changed; `None`
    /// when `list` is not a virtual list or `index` is out of range.
    pub fn scroll_virtual_list_to_index(
        &mut self,
        list: NodeId,
        index: usize,
        align: ScrollAlign,
    ) -> Option<bool> {
        let (now, viewport, _) = self.virtual_list_span(list)?;
        let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
            return None;
        };
        if index >= state.item_count {
            return None;
        }
        let (start, end) = (state.offset_of(index), state.offset_of(index + 1));
        let to = match align {
            ScrollAlign::Start => start,
            ScrollAlign::End => end - viewport,
            ScrollAlign::Center => (start + end - viewport) / 2.0,
            ScrollAlign::Nearest => {
                if start < now || end - start > viewport {
                    start
                } else if end > now + viewport {
                    end - viewport
                } else {
                    now
                }
            }
        };
        Some(self.set_virtual_list_offset(list, to))
    }

    /// `take_scroll_changes` for virtual lists: `(list, old, new)` for each
    /// whose offset differs from the one last reported.
    pub(super) fn take_virtual_list_scroll_changes(&mut self) -> Vec<ScrollChange> {
        if self.virtual_list_count == 0 {
            return Vec::new();
        }
        // A read-only pass first, as for scroll views: see `take_scroll_changes`.
        let moved: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, node)| {
                matches!(&node.kind, NodeKind::VirtualList(s) if s.scroll_offset.current != s.reported)
            })
            .map(|(id, _)| id)
            .collect();
        let mut changes = Vec::with_capacity(moved.len());
        for id in moved {
            if let NodeKind::VirtualList(state) = &mut self.nodes[id].kind {
                changes.push(ScrollChange {
                    view: id,
                    old: state.reported,
                    new: state.scroll_offset.current,
                    x: 0.0,
                    y: state.scroll_offset.current,
                });
                state.reported = state.scroll_offset.current;
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScrollViewState;
    use peniko::Color;

    fn paint() -> PaintProperties {
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0)
    }

    fn style(width: f32, height: f32) -> Style {
        Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            flex_shrink: 0.0,
            ..Default::default()
        }
    }

    fn lay_out(tree: &mut Tree, root: NodeId) {
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(400.0),
                height: AvailableSpace::Definite(400.0),
            },
        );
    }

    /// A 200x100 list of `count` rows, laid out.
    fn list_of(count: usize, extent: ItemExtent) -> (Tree, NodeId) {
        let mut tree = Tree::new();
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(count, extent)),
            style(200.0, 100.0),
            paint(),
        );
        lay_out(&mut tree, list);
        (tree, list)
    }

    /// 20 rows of 20 dp: 400 dp of content in 100 dp, so offsets 0..=300.
    fn fixed() -> (Tree, NodeId) {
        list_of(20, ItemExtent::Fixed(20.0))
    }

    /// Row heights 10, 30, 15, 25, 50, 50, 50, 50 (total 280).
    fn varied() -> (Tree, NodeId) {
        let (mut tree, list) = list_of(8, ItemExtent::Variable);
        let heights = [10.0, 30.0, 15.0, 25.0, 50.0, 50.0, 50.0, 50.0];
        let mut top = 0.0;
        let mut offsets = Vec::new();
        for (i, h) in heights.iter().enumerate() {
            offsets.push((i, top));
            top += h;
        }
        offsets.push((heights.len(), top));
        tree.set_virtual_list_resolved_offsets(list, offsets);
        (tree, list)
    }

    fn offset(tree: &Tree, list: NodeId) -> f64 {
        match &tree.get(list).unwrap().kind {
            NodeKind::VirtualList(s) => s.scroll_offset.current,
            _ => panic!("not a list"),
        }
    }

    fn row(tree: &mut Tree, list: NodeId, idx: usize) -> NodeId {
        tree.set_virtual_list_window(list, idx..idx + 1, |_| {
            (NodeKind::Rect, style(200.0, 20.0), paint())
        });
        lay_out(tree, list);
        match &tree.get(list).unwrap().kind {
            NodeKind::VirtualList(s) => s.materialized[&idx],
            _ => panic!("not a list"),
        }
    }

    #[test]
    fn keys_move_a_line_a_page_less_ten_percent_or_to_an_end() {
        let (mut tree, list) = fixed();
        for (key, expected) in [
            (Key::ArrowDown, 40.0),
            (Key::PageDown, 130.0), // 90 of a 100 viewport
            (Key::ArrowUp, 90.0),
            (Key::End, 300.0),
            (Key::PageUp, 210.0),
            (Key::Home, 0.0),
        ] {
            assert!(tree.scroll_by_key(list, key), "{key:?}");
            assert_eq!(offset(&tree, list), expected, "{key:?}");
        }
        assert!(!tree.scroll_by_key(list, Key::Home), "already at the top");
        assert!(!tree.scroll_by_key(list, Key::ArrowLeft), "not a list key");
    }

    #[test]
    fn a_page_is_the_same_for_size_hint_rows() {
        let (mut tree, list) = varied();
        // 280 of content in 100: furthest offset 180.
        assert!(tree.scroll_by_key(list, Key::PageDown));
        assert_eq!(offset(&tree, list), 90.0);
        assert!(tree.scroll_by_key(list, Key::PageDown));
        assert_eq!(offset(&tree, list), 180.0, "clamped to the furthest offset");
        assert!(tree.scroll_by_key(list, Key::ArrowUp));
        assert_eq!(offset(&tree, list), 140.0);
        assert!(tree.scroll_by_key(list, Key::Home));
        assert!(tree.scroll_by_key(list, Key::End));
        assert_eq!(offset(&tree, list), 180.0);
    }

    #[test]
    fn a_list_that_fits_takes_no_key() {
        let (mut tree, list) = list_of(3, ItemExtent::Fixed(20.0));
        assert_eq!(tree.scroll_view_for_key(list, Key::ArrowDown), None);
        assert!(!tree.scroll_by_key(list, Key::End));
    }

    #[test]
    fn keys_bubble_to_the_outer_scroller_when_the_list_cannot_move() {
        let mut tree = Tree::new();
        let outer = tree.insert(
            NodeKind::ScrollView(ScrollViewState::new(false)),
            style(200.0, 100.0),
            paint(),
        );
        let content = tree.insert(NodeKind::Container, style(200.0, 1000.0), paint());
        let list = tree.insert(
            NodeKind::VirtualList(VirtualListState::new(20, ItemExtent::Fixed(20.0))),
            style(200.0, 100.0),
            paint(),
        );
        tree.add_child(content, list);
        tree.add_child(outer, content);
        lay_out(&mut tree, outer);
        let rowid = row(&mut tree, list, 0);
        tree.scroll_scroll_view_by(outer, 200.0);

        assert_eq!(tree.scroll_view_for_key(rowid, Key::ArrowDown), Some(list));
        assert_eq!(
            tree.scroll_view_for_key(rowid, Key::ArrowUp),
            Some(outer),
            "list at its top"
        );
        assert_eq!(tree.scroll_view_for_key(rowid, Key::Home), Some(outer));
        assert_eq!(tree.scroll_view_for_key(rowid, Key::PageUp), Some(outer));
        tree.scroll_by_key(list, Key::End);
        assert_eq!(
            tree.scroll_view_for_key(rowid, Key::ArrowDown),
            Some(outer),
            "list at its end"
        );
        assert_eq!(tree.scroll_view_for_key(rowid, Key::End), Some(outer));
        assert_eq!(tree.scroll_view_for_key(rowid, Key::ArrowUp), Some(list));
        assert_eq!(
            tree.scroll_view_for_key(rowid, Key::ArrowRight),
            None,
            "no horizontal scroller"
        );
    }

    #[test]
    fn scroll_to_index_aligns_a_row_that_is_not_built() {
        let (mut tree, list) = fixed();
        let at = |tree: &mut Tree, i, a| tree.scroll_virtual_list_to_index(list, i, a);
        assert_eq!(at(&mut tree, 10, ScrollAlign::Start), Some(true));
        assert_eq!(offset(&tree, list), 200.0);
        assert_eq!(
            at(&mut tree, 10, ScrollAlign::Start),
            Some(false),
            "already there"
        );
        at(&mut tree, 10, ScrollAlign::Center);
        assert_eq!(offset(&tree, list), 160.0); // row 200..220, middle 210, minus 50
        at(&mut tree, 10, ScrollAlign::End);
        assert_eq!(offset(&tree, list), 120.0); // bottom 220 - 100
        at(&mut tree, 19, ScrollAlign::Start);
        assert_eq!(offset(&tree, list), 300.0, "clamped to the furthest offset");
        at(&mut tree, 0, ScrollAlign::Center);
        assert_eq!(offset(&tree, list), 0.0, "clamped to zero");
    }

    #[test]
    fn scroll_to_index_nearest_moves_the_least() {
        let (mut tree, list) = fixed();
        let at =
            |tree: &mut Tree, i| tree.scroll_virtual_list_to_index(list, i, ScrollAlign::Nearest);
        assert_eq!(at(&mut tree, 4), Some(false), "row 4 (80..100) is shown");
        assert_eq!(offset(&tree, list), 0.0);
        assert_eq!(
            at(&mut tree, 5),
            Some(true),
            "below: its bottom at the viewport's bottom"
        );
        assert_eq!(offset(&tree, list), 20.0);
        assert_eq!(at(&mut tree, 8), Some(true));
        assert_eq!(offset(&tree, list), 80.0);
        assert_eq!(
            at(&mut tree, 1),
            Some(true),
            "above: its top at the viewport's top"
        );
        assert_eq!(offset(&tree, list), 20.0);
        assert_eq!(at(&mut tree, 0), Some(true));
        assert_eq!(offset(&tree, list), 0.0);
    }

    #[test]
    fn scroll_to_index_uses_the_resolved_size_hint_offsets() {
        let (mut tree, list) = varied();
        // Row 4 spans 80..130.
        tree.scroll_virtual_list_to_index(list, 4, ScrollAlign::Start);
        assert_eq!(offset(&tree, list), 80.0);
        tree.scroll_virtual_list_to_index(list, 4, ScrollAlign::Center);
        assert_eq!(offset(&tree, list), 55.0);
        tree.scroll_virtual_list_to_index(list, 4, ScrollAlign::End);
        assert_eq!(offset(&tree, list), 30.0);
        tree.scroll_virtual_list_to_index(list, 1, ScrollAlign::Nearest);
        assert_eq!(offset(&tree, list), 10.0, "row 1 spans 10..40: its top");
    }

    #[test]
    fn scroll_to_index_rejects_what_is_not_a_row() {
        let (mut tree, list) = fixed();
        assert_eq!(
            tree.scroll_virtual_list_to_index(list, 20, ScrollAlign::Start),
            None
        );
        let plain = tree.insert(NodeKind::Container, style(10.0, 10.0), paint());
        assert_eq!(
            tree.scroll_virtual_list_to_index(plain, 0, ScrollAlign::Start),
            None
        );
    }

    #[test]
    fn revealing_a_built_row_scrolls_by_the_least_distance() {
        let (mut tree, list) = fixed();
        let below = row(&mut tree, list, 7); // 140..160, viewport 0..100
        assert!(tree.scroll_into_view(below));
        assert_eq!(
            offset(&tree, list),
            60.0,
            "its bottom at the viewport's bottom"
        );
        assert!(!tree.scroll_into_view(below), "now shown");
        let above = row(&mut tree, list, 1); // 20..40 under offset 60: above the top
        assert!(tree.scroll_into_view(above));
        assert_eq!(offset(&tree, list), 20.0, "its top at the viewport's top");
    }

    #[test]
    fn focus_moving_to_a_row_scrolls_the_list() {
        let (mut tree, list) = fixed();
        let below = row(&mut tree, list, 9);
        tree.set_focus_to(below);
        assert_eq!(offset(&tree, list), 100.0);
    }

    #[test]
    fn the_focused_row_stays_built_when_it_leaves_the_window() {
        let (mut tree, list) = fixed();
        let focused = row(&mut tree, list, 0);
        tree.set_focus_to(focused);
        tree.set_virtual_list_window(list, 0..1, |_| {
            (NodeKind::Rect, style(200.0, 20.0), paint())
        });
        let released = tree.virtual_list_release_outside(list, 10..15);
        assert!(released.is_empty());
        assert_eq!(
            tree.focused(),
            Some(focused),
            "focus stays, so keys keep working"
        );
        tree.set_focus_to(list);
        assert_eq!(
            tree.virtual_list_release_outside(list, 10..15),
            vec![focused]
        );
    }

    #[test]
    fn a_focused_row_past_the_end_of_a_shrunk_list_is_not_kept() {
        let (mut tree, list) = fixed();
        let focused = row(&mut tree, list, 15);
        tree.set_focus_to(focused);
        if let NodeKind::VirtualList(state) = &mut tree.get_mut(list).unwrap().kind {
            state.item_count = 10;
        }
        assert_eq!(
            tree.virtual_list_release_outside(list, 0..5),
            vec![focused],
            "row 15 no longer exists"
        );
    }

    #[test]
    fn scroll_changes_report_a_list_once_by_offset() {
        let (mut tree, list) = fixed();
        assert!(tree.take_scroll_changes().is_empty());
        tree.scroll_by_key(list, Key::ArrowDown);
        assert_eq!(
            tree.take_scroll_changes(),
            vec![ScrollChange {
                view: list,
                old: 0.0,
                new: 40.0,
                x: 0.0,
                y: 40.0
            }]
        );
        assert!(tree.take_scroll_changes().is_empty());
        tree.scroll_virtual_list_by(list, 10.0);
        assert_eq!(
            tree.take_scroll_changes(),
            vec![ScrollChange {
                view: list,
                old: 40.0,
                new: 50.0,
                x: 0.0,
                y: 50.0
            }]
        );
    }
}
