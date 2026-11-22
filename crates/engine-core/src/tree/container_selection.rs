//! Choosing the children of a box, container or scroll view (UX spec "Selection", AC-SLT-1 to
//! AC-SLT-5): the rules of `list_selection.rs` and `selection.rs`, with the container's children
//! as the items and their indices as the keys. The index is all the engine remembers of a child:
//! a child moved or removed leaves the choice where it was, by index. A scroll view has one child,
//! its content, so the items of a scroll view are the children of that content.
//!
//! A child that is not `item_selectable`, or not `visible`, cannot be chosen. The set of such
//! indices is read from the children again before each press, key and report
//! (`sync_container_selection`), so a child's flag needs no hook of its own. A container whose
//! `selection_mode` is `none` (the default) is never touched by any of it.

use super::*;
use crate::selection::{RangeSet, SelectionMode, SelectionSource, SelectionState};
use std::collections::HashSet;

impl Tree {
    /// Whether a node of `kind` can have its children chosen: a box, a container, a scroll view.
    pub fn holds_items(kind: &NodeKind) -> bool {
        matches!(
            kind,
            NodeKind::Rect | NodeKind::Container | NodeKind::ScrollView(_)
        )
    }

    /// `id`'s selection, if it is a box, container or scroll view that has one.
    pub fn container_selection(&self, id: NodeId) -> Option<&SelectionState> {
        let node = self.nodes.get(id)?;
        node.selection
            .as_deref()
            .filter(|_| Self::holds_items(&node.kind))
    }

    /// Whether `id` is a container that chooses its children.
    pub(super) fn selects_children(&self, id: NodeId) -> bool {
        self.container_selection(id)
            .is_some_and(|s| s.mode != SelectionMode::None)
    }

    /// The node whose children are the items `host` chooses: `host` itself, or, for a scroll view,
    /// the one child that holds its content.
    pub fn item_parent(&self, host: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(host)?;
        match node.kind {
            NodeKind::ScrollView(_) => node.children.first().copied(),
            _ => Some(host),
        }
    }

    /// The items `host` chooses, in order: their indices are the keys.
    pub fn container_items(&self, host: NodeId) -> &[NodeId] {
        self.item_parent(host)
            .and_then(|parent| self.nodes.get(parent))
            .map_or(&[], |parent| &parent.children)
    }

    /// The container that chooses the children of `parent`, if one does: `parent` itself, or the
    /// scroll view whose content `parent` is.
    pub(super) fn item_host(&self, parent: NodeId) -> Option<NodeId> {
        let node = self.nodes.get(parent)?;
        if !matches!(node.kind, NodeKind::ScrollView(_)) && self.selects_children(parent) {
            return Some(parent);
        }
        let view = node.parent?;
        (self.item_parent(view) == Some(parent)
            && matches!(self.nodes[view].kind, NodeKind::ScrollView(_))
            && self.selects_children(view))
        .then_some(view)
    }

    /// Keeps `selection_hosts` right about `id`, after a property of it changed or its change
    /// was reported: a container is in it while its mode is not `none` or a change of it waits.
    pub fn refresh_selection_host(&mut self, id: NodeId) {
        let hosts = self
            .container_selection(id)
            .is_some_and(|s| s.mode != SelectionMode::None || s.has_pending());
        if hosts {
            self.selection_hosts.insert(id);
        } else {
            self.selection_hosts.remove(&id);
        }
    }

    /// Whether child `child` of a container cannot be chosen.
    fn child_unselectable(&self, child: NodeId) -> bool {
        self.nodes
            .get(child)
            .is_none_or(|c| !c.item_selectable || !c.visible)
    }

    /// Reads which children of `id` can be chosen, and how many there are, again: nothing past
    /// the last child stays chosen, and a child that can no longer be chosen leaves the set.
    pub(super) fn sync_container_selection(&mut self, id: NodeId) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        if !node
            .selection
            .as_deref()
            .is_some_and(|s| s.mode != SelectionMode::None)
        {
            return;
        }
        let items = self.container_items(id);
        let unselectable = RangeSet::from_indices(
            items
                .iter()
                .enumerate()
                .filter(|&(_, &child)| self.child_unselectable(child))
                .map(|(i, _)| i),
        );
        let count = items.len();
        // Only a change reaches the node mutably: that marks it to be drawn again.
        let current = node.selection.as_deref();
        if current.is_some_and(|s| s.fits(count) && *s.unselectable() == unselectable) {
            return;
        }
        if let Some(sel) = self.nodes[id].selection.as_deref_mut() {
            let api = SelectionSource::Api;
            sel.clip(count, api);
            if *sel.unselectable() != unselectable {
                sel.set_unselectable(unselectable, api);
            }
        }
    }

    /// The index of `child` among the items of `host`, when `host` chooses its children.
    pub(super) fn container_item_index(&self, host: NodeId, child: NodeId) -> Option<usize> {
        if !self.selects_children(host) {
            return None;
        }
        self.container_items(host).iter().position(|&c| c == child)
    }

    /// The item of `host` that focus is in or on.
    pub(super) fn container_focused_item(&self, host: NodeId) -> Option<usize> {
        let parent = self.item_parent(host)?;
        let item = self
            .ancestors(self.focused?)
            .find(|&id| self.nodes[id].parent == Some(parent))?;
        self.container_items(host).iter().position(|&c| c == item)
    }

    /// The child a Page Up or Page Down from child `from` goes to, before unselectable children
    /// are passed over: about a page of the container's height away by the children's tops --
    /// at least one child away, and never the container's own end.
    pub(super) fn container_page_target(&self, host: NodeId, down: bool, from: usize) -> usize {
        let items = self.container_items(host);
        if from >= items.len() {
            return from;
        }
        let fraction = self.nodes[host]
            .kind
            .scroller()
            .map_or(1.0, |s| s.metrics().page_fraction);
        let page = f64::from(self.layout(host).size.height) * fraction;
        let top = |i: usize| f64::from(self.layout(items[i]).location.y);
        let last = items.len().saturating_sub(1);
        if down {
            let to = top(from) + page;
            let reached = (from + 1..=last).take_while(|&i| top(i) <= to).last();
            reached.unwrap_or((from + 1).min(last))
        } else {
            let to = top(from) - page;
            let reached = (0..from).rev().take_while(|&i| top(i) >= to).last();
            reached.unwrap_or(from.saturating_sub(1))
        }
    }

    /// Takes the keyboard to child `index` of `host`: shows it, and moves focus into it if it
    /// takes any. Focus's transition, if it moved.
    pub(super) fn focus_container_item(
        &mut self,
        host: NodeId,
        index: usize,
    ) -> Option<(Option<NodeId>, Option<NodeId>)> {
        let child = *self.container_items(host).get(index)?;
        if let Some(sel) = self.nodes[host].selection.as_deref_mut() {
            sel.active = Some(index);
        }
        self.scroll_into_view(child);
        self.focus_into(child)
    }

    /// Whether `node` is, or is inside, an item that cannot be chosen, of a list or container
    /// that chooses its items: focus does not go there (AC-SLT-5).
    pub(super) fn in_unselectable_item(&self, node: NodeId) -> bool {
        self.ancestors(node).any(|id| {
            let Some(parent) = self.nodes[id].parent else {
                return false;
            };
            match &self.nodes[parent].kind {
                NodeKind::VirtualList(state) if state.selection.mode != SelectionMode::None => {
                    state
                        .materialized
                        .iter()
                        .find(|(_, built)| **built == id)
                        .is_some_and(|(&row, _)| state.selection.unselectable().contains(row))
                }
                _ => self.item_host(parent).is_some() && self.child_unselectable(id),
            }
        })
    }

    /// Every built item, of every list and container that chooses its items, that cannot be
    /// chosen: the nodes Tab passes over. Empty for a tree with no selection at all.
    pub(super) fn unselectable_item_nodes(&self) -> HashSet<NodeId> {
        let mut blocked = HashSet::new();
        if self.virtual_list_count > 0 {
            for (_, node) in self.nodes.iter() {
                if let NodeKind::VirtualList(state) = &node.kind
                    && state.selection.mode != SelectionMode::None
                {
                    blocked.extend(
                        state
                            .materialized
                            .iter()
                            .filter(|(row, _)| state.selection.unselectable().contains(**row))
                            .map(|(_, &built)| built),
                    );
                }
            }
        }
        for &host in &self.selection_hosts {
            if self.selects_children(host) {
                blocked.extend(
                    self.container_items(host)
                        .iter()
                        .copied()
                        .filter(|&child| self.child_unselectable(child)),
                );
            }
        }
        blocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;
    use crate::selection::DEFAULT_ANNOUNCE_MS;
    use peniko::Color;
    use std::time::Duration;

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
            flex_direction: taffy::FlexDirection::Column,
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

    /// A 200x100 box of `count` focusable 20 dp children in `mode`; the box is the root.
    fn make_box(count: usize, mode: SelectionMode) -> (Tree, NodeId, Vec<NodeId>) {
        let mut tree = Tree::new();
        let host = tree.insert(NodeKind::Rect, style(200.0, 100.0), paint());
        let children: Vec<NodeId> = (0..count)
            .map(|_| {
                let child = tree.insert(NodeKind::Rect, style(200.0, 20.0), paint());
                tree.get_mut(child).unwrap().access.focusable = Some(true);
                tree.add_child(host, child);
                child
            })
            .collect();
        let mut state = SelectionState::default();
        state.mode = mode;
        tree.get_mut(host).unwrap().selection = Some(Box::new(state));
        tree.refresh_selection_host(host);
        lay_out(&mut tree, host);
        (tree, host, children)
    }

    fn held(tree: &mut Tree, shift: bool, ctrl: bool) {
        tree.set_held_modifiers(Modifiers {
            shift,
            ctrl,
            ..Modifiers::default()
        });
    }

    fn press(tree: &mut Tree, child: NodeId, shift: bool, ctrl: bool) {
        held(tree, shift, ctrl);
        tree.list_press(Some(child), Point::new(10.0, 10.0));
        tree.list_release(Some(child));
    }

    fn chosen(tree: &Tree, host: NodeId) -> Vec<usize> {
        tree.container_selection(host)
            .unwrap()
            .selected()
            .iter()
            .collect()
    }

    fn key(tree: &mut Tree, key: Key, shift: bool, ctrl: bool) -> ListKey {
        held(tree, shift, ctrl);
        tree.list_selection_key(key, shift)
    }

    fn unselectable(tree: &mut Tree, child: NodeId) {
        tree.get_mut(child).unwrap().item_selectable = false;
    }

    #[test]
    fn a_container_with_mode_none_is_left_alone() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::None);
        press(&mut tree, kids[1], false, false);
        tree.set_focus_to(kids[1]);
        assert!(!key(&mut tree, Key::ArrowDown, false, false).consumed);
        assert!(!key(&mut tree, Key::Space, false, false).consumed);
        assert!(!tree.select_all_in_focused_list());
        assert!(chosen(&tree, host).is_empty());
        assert!(tree.take_selection_changes().is_empty());
        assert!(tree.selection_hosts.is_empty());
    }

    /// AC-SLT-1 on the children A to E of a box: click B, Shift+click D, Ctrl+click E,
    /// Shift+click C.
    #[test]
    fn clicks_choose_ranges_and_toggles() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::Multiple);
        for (idx, shift, ctrl, set) in [
            (1, false, false, vec![1]),
            (3, true, false, vec![1, 2, 3]),
            (4, false, true, vec![1, 2, 3, 4]),
            (2, true, false, vec![2, 3, 4]),
        ] {
            press(&mut tree, kids[idx], shift, ctrl);
            assert_eq!(chosen(&tree, host), set);
        }
        assert_eq!(tree.container_selection(host).unwrap().anchor(), Some(4));
        let changes = tree.take_selection_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].list, host);
        assert_eq!(changes[0].source, SelectionSource::Pointer);
        assert!(tree.take_selection_changes().is_empty());
    }

    /// AC-SLT-5: a child that is not `item_selectable` is skipped by a range, by select all and
    /// by the arrow keys, and a click on it chooses nothing and moves nothing.
    #[test]
    fn an_unselectable_child_is_passed_over() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::Multiple);
        unselectable(&mut tree, kids[2]);
        press(&mut tree, kids[0], false, false);
        press(&mut tree, kids[4], true, false);
        assert_eq!(chosen(&tree, host), [0, 1, 3, 4]);
        press(&mut tree, kids[2], false, false);
        assert_eq!(
            chosen(&tree, host),
            [0, 1, 3, 4],
            "a click on it chooses nothing"
        );
        tree.set_focus_to(kids[1]);
        let moved = key(&mut tree, Key::ArrowDown, false, false);
        assert!(moved.consumed);
        assert_eq!(tree.focused(), Some(kids[3]), "Down passes over it");
        key(&mut tree, Key::ArrowUp, false, false);
        assert_eq!(tree.focused(), Some(kids[1]));
        assert!(!tree.container_selection(host).unwrap().is_selectable(2, 5));
    }

    #[test]
    fn select_all_skips_unselectable_children_and_a_hidden_one() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::Multiple);
        tree.get_mut(host)
            .unwrap()
            .selection
            .as_deref_mut()
            .unwrap()
            .select_all_key = true;
        unselectable(&mut tree, kids[0]);
        tree.get_mut(kids[3]).unwrap().visible = false;
        tree.set_focus_to(kids[1]);
        assert!(tree.select_all_in_focused_list());
        assert_eq!(chosen(&tree, host), [1, 2, 4]);
    }

    /// A chosen child that turns unselectable (or goes away) leaves the set, and that is reported.
    #[test]
    fn a_chosen_child_that_cannot_be_chosen_any_more_leaves_the_set() {
        let (mut tree, host, kids) = make_box(4, SelectionMode::Multiple);
        press(&mut tree, kids[1], false, false);
        press(&mut tree, kids[2], true, false);
        assert_eq!(tree.take_selection_changes().len(), 1);
        unselectable(&mut tree, kids[2]);
        let changes = tree.take_selection_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].source, SelectionSource::Api);
        assert_eq!(chosen(&tree, host), [1]);
        // The last child goes: nothing past the new end stays chosen.
        press(&mut tree, kids[3], false, false);
        tree.take_selection_changes();
        tree.remove(kids[3]);
        assert_eq!(tree.take_selection_changes().len(), 1);
        assert!(chosen(&tree, host).is_empty());
    }

    #[test]
    fn shift_arrows_range_from_the_anchor_and_ctrl_arrows_only_move() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::Multiple);
        press(&mut tree, kids[1], false, false);
        tree.set_focus_to(kids[1]);
        key(&mut tree, Key::ArrowDown, true, false);
        key(&mut tree, Key::ArrowDown, true, false);
        assert_eq!(chosen(&tree, host), [1, 2, 3]);
        key(&mut tree, Key::ArrowDown, false, true);
        assert_eq!(tree.focused(), Some(kids[4]));
        assert_eq!(chosen(&tree, host), [1, 2, 3], "Ctrl+Down moves focus only");
        key(&mut tree, Key::Home, true, false);
        assert_eq!(
            chosen(&tree, host),
            [0, 1],
            "the range runs from the anchor, 1, to Home"
        );
        key(&mut tree, Key::Space, false, true);
        assert!(tree.focused() == Some(kids[0]));
    }

    #[test]
    fn page_keys_move_about_a_box_height() {
        let (mut tree, _host, kids) = make_box(30, SelectionMode::Multiple);
        // The box is 100 high and its children 20: a page is about four or five of them.
        tree.set_focus_to(kids[0]);
        key(&mut tree, Key::PageDown, false, false);
        let down = kids
            .iter()
            .position(|&k| Some(k) == tree.focused())
            .unwrap();
        assert!((3..=5).contains(&down), "page down went to {down}");
        key(&mut tree, Key::PageUp, false, false);
        assert_eq!(tree.focused(), Some(kids[0]));
    }

    #[test]
    fn a_right_click_chooses_an_unchosen_child_alone_and_keeps_the_set_of_a_chosen_one() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::Multiple);
        press(&mut tree, kids[0], false, false);
        press(&mut tree, kids[2], true, false);
        assert_eq!(chosen(&tree, host), [0, 1, 2]);
        tree.list_secondary_press(Some(kids[1]));
        assert_eq!(
            chosen(&tree, host),
            [0, 1, 2],
            "on a chosen child the set stays"
        );
        tree.list_secondary_press(Some(kids[4]));
        assert_eq!(
            chosen(&tree, host),
            [4],
            "on an unchosen one it is chosen alone"
        );
        unselectable(&mut tree, kids[3]);
        tree.list_secondary_press(Some(kids[3]));
        assert_eq!(
            chosen(&tree, host),
            [4],
            "an unselectable one is left alone"
        );
    }

    #[test]
    fn tab_passes_over_unselectable_children() {
        let (mut tree, host, kids) = make_box(4, SelectionMode::Multiple);
        unselectable(&mut tree, kids[1]);
        tree.set_focus_to(kids[0]);
        tree.move_focus(host, FocusDirection::Next);
        assert_eq!(tree.focused(), Some(kids[2]));
        tree.move_focus(host, FocusDirection::Previous);
        assert_eq!(tree.focused(), Some(kids[0]));
        // With the mode off, Tab goes to every child as before.
        tree.get_mut(host)
            .unwrap()
            .selection
            .as_deref_mut()
            .unwrap()
            .set_mode(SelectionMode::None, SelectionSource::Api);
        tree.refresh_selection_host(host);
        tree.move_focus(host, FocusDirection::Next);
        assert_eq!(tree.focused(), Some(kids[1]));
    }

    // ---- accessibility (AC-SLT-8) ----

    fn access_node(update: &accesskit::TreeUpdate, id: NodeId) -> &accesskit::Node {
        let want = crate::to_access_id(id);
        &update.nodes.iter().find(|(n, _)| *n == want).unwrap().1
    }

    #[test]
    fn the_container_is_multiselectable_and_its_children_are_selected_as_the_set_says() {
        let (mut tree, host, kids) = make_box(3, SelectionMode::Multiple);
        press(&mut tree, kids[0], false, false);
        press(&mut tree, kids[2], false, true);
        // An explicit `selected` on a child is overridden while the container chooses.
        tree.get_mut(kids[1]).unwrap().access.selected = Some(true);
        let update = tree.build_access_update(host);
        assert!(access_node(&update, host).is_multiselectable());
        let selected: Vec<_> = kids
            .iter()
            .map(|&k| access_node(&update, k).is_selected())
            .collect();
        assert_eq!(selected, [Some(true), Some(false), Some(true)]);
        // A single container is not multiselectable and has at most one selected.
        let (mut tree, host, kids) = make_box(3, SelectionMode::Single);
        press(&mut tree, kids[1], false, false);
        press(&mut tree, kids[2], false, false);
        let update = tree.build_access_update(host);
        assert!(!access_node(&update, host).is_multiselectable());
        let selected: Vec<_> = kids
            .iter()
            .map(|&k| access_node(&update, k).is_selected())
            .collect();
        assert_eq!(selected, [Some(false), Some(false), Some(true)]);
    }

    #[test]
    fn a_container_that_does_not_choose_leaves_selected_to_the_app() {
        let (mut tree, host, kids) = make_box(2, SelectionMode::None);
        tree.get_mut(kids[1]).unwrap().access.selected = Some(true);
        let update = tree.build_access_update(host);
        assert!(!access_node(&update, host).is_multiselectable());
        assert_eq!(access_node(&update, kids[0]).is_selected(), None);
        assert_eq!(access_node(&update, kids[1]).is_selected(), Some(true));
    }

    // ---- announcement (AC-SLT-9) ----

    fn heard(tree: &Tree, root: NodeId) -> Vec<String> {
        tree.build_access_update(root)
            .nodes
            .iter()
            .filter(|(id, _)| id.0 & (1 << 62) != 0)
            .map(|(_, node)| node.label().unwrap_or_default().to_string())
            .collect()
    }

    #[test]
    fn ten_changes_within_the_delay_are_announced_once() {
        let (mut tree, host, kids) = make_box(5, SelectionMode::Multiple);
        tree.get_mut(host)
            .unwrap()
            .selection
            .as_deref_mut()
            .unwrap()
            .set_announce(Some("{count} selected".to_string()));
        let start = Instant::now();
        for i in 0..10 {
            let now = start + Duration::from_millis(i * 20);
            tree.clock = now;
            press(&mut tree, kids[(i % 5) as usize], false, true);
            tree.take_selection_changes();
            tree.tick_all(now);
            assert!(
                !tree.has_pending_announcements(),
                "nothing before the burst is over"
            );
        }
        let last = start + Duration::from_millis(9 * 20);
        let (active, _) =
            tree.tick_all(last + Duration::from_millis(u64::from(DEFAULT_ANNOUNCE_MS) - 1));
        assert!(active, "the window keeps drawing until it is made");
        assert!(!tree.has_pending_announcements());
        let (active, _) =
            tree.tick_all(last + Duration::from_millis(u64::from(DEFAULT_ANNOUNCE_MS)));
        assert!(!active);
        let chosen_now = chosen(&tree, host).len();
        assert_eq!(heard(&tree, host), [format!("{chosen_now} selected")]);
        // Once: nothing more is waiting.
        tree.tick_all(last + Duration::from_secs(5));
        assert!(!tree.has_pending_announcements());
    }

    #[test]
    fn without_a_template_nothing_is_announced() {
        let (mut tree, host, kids) = make_box(3, SelectionMode::Multiple);
        press(&mut tree, kids[0], false, false);
        tree.take_selection_changes();
        let (active, _) = tree.tick_all(Instant::now() + Duration::from_secs(5));
        assert!(!active);
        assert!(heard(&tree, host).is_empty());
    }

    /// A scroll view has one child, its content: the items are the content's children.
    #[test]
    fn the_items_of_a_scroll_view_are_the_children_of_its_content() {
        let mut tree = Tree::new();
        let view = tree.insert(
            NodeKind::ScrollView(crate::ScrollViewState::new(false)),
            style(200.0, 60.0),
            paint(),
        );
        let content = tree.insert(NodeKind::Rect, style(200.0, 200.0), paint());
        tree.add_child(view, content);
        let kids: Vec<NodeId> = (0..5)
            .map(|_| {
                let kid = tree.insert(NodeKind::Rect, style(200.0, 20.0), paint());
                tree.get_mut(kid).unwrap().access.focusable = Some(true);
                tree.add_child(content, kid);
                kid
            })
            .collect();
        let mut state = SelectionState::default();
        state.mode = SelectionMode::Multiple;
        state.select_all_key = true;
        tree.get_mut(view).unwrap().selection = Some(Box::new(state));
        tree.refresh_selection_host(view);
        lay_out(&mut tree, view);
        assert_eq!(tree.container_items(view), kids);
        press(&mut tree, kids[1], false, false);
        press(&mut tree, kids[3], true, false);
        assert_eq!(chosen(&tree, view), [1, 2, 3]);
        tree.set_focus_to(kids[3]);
        key(&mut tree, Key::ArrowDown, false, false);
        assert_eq!(tree.focused(), Some(kids[4]));
        assert!(tree.select_all_in_focused_list());
        assert_eq!(chosen(&tree, view), [0, 1, 2, 3, 4]);
        // The empty part of the content clears.
        held(&mut tree, false, false);
        tree.list_press(Some(content), Point::new(10.0, 10.0));
        assert!(chosen(&tree, view).is_empty());
        press(&mut tree, kids[2], false, false);
        let update = tree.build_access_update(view);
        assert!(access_node(&update, view).is_multiselectable());
        let selected: Vec<_> = kids
            .iter()
            .map(|&k| access_node(&update, k).is_selected())
            .collect();
        assert_eq!(
            selected,
            [
                Some(false),
                Some(false),
                Some(true),
                Some(false),
                Some(false)
            ]
        );
        // The content's own children are never told anything about the scroll view's content.
        assert_eq!(access_node(&update, content).is_selected(), None);
    }
}
