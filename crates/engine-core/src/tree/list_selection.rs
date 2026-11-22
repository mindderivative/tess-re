//! Choosing the rows of a virtual list by pointer and keys (UX spec "Selection",
//! AC-SLT-1 to AC-SLT-6 and 13): the rules of `selection.rs` fed with clicks and keys,
//! and the focus that follows. A list whose `selection_mode` is `none` (the default)
//! is never touched by any of it.

use super::*;
use crate::selection::{RangeSet, SelectionMode, SelectionSource, SelectionState};

/// A press on a chosen row of a multiple selection list, which chooses that row alone
/// at its release -- unless the pointer got `drag_start_px` away first (the app's drag).
#[derive(Clone, Copy, Debug)]
pub(super) struct ListPress {
    list: NodeId,
    row: usize,
    origin: Point,
    dragged: bool,
}

/// What `Tree::list_selection_key` did with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListKey {
    /// The key was the list's: it must not also scroll the list, and a Space must not
    /// also click the row.
    pub consumed: bool,
    /// Focus moved from the first node to the second.
    pub focus: Option<(Option<NodeId>, Option<NodeId>)>,
}

/// A list's chosen set changed (`Tree::take_selection_changes`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionChange {
    pub list: NodeId,
    pub selected: RangeSet,
    pub anchor: Option<usize>,
    pub source: SelectionSource,
}

const NOT_MINE: ListKey = ListKey {
    consumed: false,
    focus: None,
};

impl Tree {
    /// `list`'s selection, if it is a virtual list.
    pub fn virtual_list_selection(&self, list: NodeId) -> Option<&SelectionState> {
        match &self.nodes.get(list)?.kind {
            NodeKind::VirtualList(state) => Some(&state.selection),
            _ => None,
        }
    }

    /// The nearest list around `node` (`node` too) whose rows the engine chooses.
    fn selecting_list_of(&self, node: NodeId) -> Option<NodeId> {
        self.ancestors(node).find(|&id| {
            matches!(&self.nodes[id].kind,
                NodeKind::VirtualList(state) if state.selection.mode != SelectionMode::None)
        })
    }

    /// The list, and the index of its row, that `node` is or is inside, when that list
    /// chooses rows.
    fn selecting_row(&self, node: NodeId) -> Option<(NodeId, usize)> {
        self.ancestors(node).find_map(|id| {
            let list = self.nodes[id].parent?;
            let NodeKind::VirtualList(state) = &self.nodes[list].kind else {
                return None;
            };
            if state.selection.mode == SelectionMode::None {
                return None;
            }
            let (&row, _) = state.materialized.iter().find(|(_, built)| **built == id)?;
            Some((list, row))
        })
    }

    fn selection_mut(&mut self, list: NodeId) -> Option<(&mut SelectionState, usize)> {
        match &mut self.nodes.get_mut(list)?.kind {
            NodeKind::VirtualList(state) => Some((&mut state.selection, state.item_count)),
            _ => None,
        }
    }

    /// A primary press at `position` on `hit`: chooses rows as the click rules say
    /// (AC-SLT-1). Shift and Ctrl (or Cmd) are the window's held keys.
    pub(super) fn list_press(&mut self, hit: Option<NodeId>, position: Point) {
        self.list_press = None;
        let Some(hit) = hit else { return };
        let held = self.held;
        let toggle = held.ctrl || held.meta;
        let Some((list, row)) = self.selecting_row(hit) else {
            // The empty part of a list: clears a multiple selection (UX spec "Selection", rule 5).
            let multiple = matches!(&self.nodes[hit].kind,
                NodeKind::VirtualList(s) if s.selection.mode == SelectionMode::Multiple);
            if multiple
                && !held.shift
                && !toggle
                && let Some((sel, _)) = self.selection_mut(hit)
            {
                sel.clear(SelectionSource::Pointer);
            }
            return;
        };
        let Some((sel, count)) = self.selection_mut(list) else {
            return;
        };
        sel.active = Some(row);
        sel.pending_focus = None;
        if !sel.is_selectable(row, count) {
            return;
        }
        let pointer = SelectionSource::Pointer;
        match (sel.mode, held.shift, toggle) {
            (SelectionMode::Multiple, true, add) => sel.select_range(row, count, add, pointer),
            (SelectionMode::Multiple, false, true) => sel.toggle(row, count, false, pointer),
            // A row of several, pressed: it may be the start of a drag of all of them, so
            // the set stays until the release shows that it was a click.
            (SelectionMode::Multiple, false, false)
                if sel.is_selected(row) && sel.selected().len() > 1 =>
            {
                self.list_press = Some(ListPress {
                    list,
                    row,
                    origin: position,
                    dragged: false,
                });
            }
            _ => sel.select_only(row, count, pointer),
        }
    }

    /// The pointer moved: a press that has gone `drag_start_px` from where it landed is a drag.
    /// With the default 0 no move is.
    pub(super) fn list_press_moved(&mut self, position: Point) {
        let start = self.drag_start_px;
        if let Some(press) = &mut self.list_press
            && start > 0.0
            && (position - press.origin).hypot() >= start
        {
            press.dragged = true;
        }
    }

    /// The primary button went up over `hit`: a press on a chosen row that was not a drag
    /// chooses that row alone (AC-SLT-13).
    pub(super) fn list_release(&mut self, hit: Option<NodeId>) {
        let Some(press) = self.list_press.take() else {
            return;
        };
        let same_row = hit.and_then(|h| self.selecting_row(h)) == Some((press.list, press.row));
        if same_row
            && !press.dragged
            && let Some((sel, count)) = self.selection_mut(press.list)
        {
            sel.select_only(press.row, count, SelectionSource::Pointer);
        }
    }

    /// Drops a press waiting for its release (the gesture was cancelled).
    pub(super) fn list_press_cancel(&mut self) {
        self.list_press = None;
    }

    /// `drag_start_px` of the window, which the window sets before each event.
    pub fn set_drag_start_px(&mut self, px: f64) {
        self.drag_start_px = px;
    }

    /// Moves focus into the rows `focus_list_row` was waiting for and that are built now (call it
    /// after layout), as long as focus is still in their list or nowhere; the focus transitions made.
    pub fn resolve_list_focus(&mut self) -> Vec<(Option<NodeId>, Option<NodeId>)> {
        if self.virtual_list_count == 0 {
            return Vec::new();
        }
        let waiting: Vec<(NodeId, Option<NodeId>)> = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.kind {
                NodeKind::VirtualList(s) => {
                    let row = s.selection.pending_focus?;
                    Some((id, s.materialized.get(&row).copied()))
                }
                _ => None,
            })
            .collect();
        let mut moves = Vec::new();
        for (list, built) in waiting {
            let in_list = self
                .focused
                .is_none_or(|f| self.ancestors(f).any(|id| id == list));
            let Some(built) = built.filter(|_| in_list) else {
                // Not built yet, and focus is still its to take; or focus has gone elsewhere.
                if !in_list && let Some((sel, _)) = self.selection_mut(list) {
                    sel.pending_focus = None;
                }
                continue;
            };
            if let Some((sel, _)) = self.selection_mut(list) {
                sel.pending_focus = None;
            }
            moves.extend(self.focus_into(built));
        }
        moves
    }

    /// The lists whose chosen set changed since the last call, each with the whole set.
    pub fn take_selection_changes(&mut self) -> Vec<SelectionChange> {
        if self.virtual_list_count == 0 {
            return Vec::new();
        }
        // A read-only pass first, as for scrolling: see `take_scroll_changes`.
        let changed: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, node)| {
                matches!(&node.kind, NodeKind::VirtualList(s) if s.selection.has_pending())
            })
            .map(|(id, _)| id)
            .collect();
        let mut changes = Vec::with_capacity(changed.len());
        for list in changed {
            if let NodeKind::VirtualList(state) = &mut self.nodes[list].kind
                && let Some(source) = state.selection.take_pending()
            {
                changes.push(SelectionChange {
                    list,
                    selected: state.selection.selected().clone(),
                    anchor: state.selection.anchor(),
                    source,
                });
            }
        }
        changes
    }

    /// Ctrl+A: chooses every selectable row of the multiple selection list focus is in, if it
    /// has `select_all_key`. Whether it did (AC-SLT-4).
    pub fn select_all_in_focused_list(&mut self) -> bool {
        let Some(list) = self.focused.and_then(|f| self.selecting_list_of(f)) else {
            return false;
        };
        let Some((sel, count)) = self.selection_mut(list) else {
            return false;
        };
        if sel.mode != SelectionMode::Multiple || !sel.select_all_key {
            return false;
        }
        sel.select_all(count, SelectionSource::Keyboard);
        true
    }

    /// The row the keyboard is on in `list`: the focused row, else the one last moved to.
    fn list_cursor(&self, list: NodeId) -> Option<usize> {
        self.virtual_list_focused_row(list)
            .or_else(|| self.virtual_list_selection(list)?.active)
    }

    /// The key `key` (with the held modifiers) pressed while focus is in a list that chooses
    /// rows (AC-SLT-2, 3, 5): Up, Down, Home, End, Page Up and Page Down move focus to a
    /// selectable row, extending the range from the anchor with Shift, moving focus alone
    /// with Ctrl; Space toggles (Shift: range) in a multiple list and chooses in a single one;
    /// Enter chooses in a single one (and still activates). Escape is nobody's here.
    pub fn list_selection_key(&mut self, key: Key, shift: bool) -> ListKey {
        let Some(focused) = self.focused else {
            return NOT_MINE;
        };
        let Some(list) = self.selecting_list_of(focused) else {
            return NOT_MINE;
        };
        let held = self.held;
        if held.alt || held.meta {
            return NOT_MINE;
        }
        let ctrl = held.ctrl;
        let cursor = self.list_cursor(list);
        let keyboard = SelectionSource::Keyboard;
        let Some((sel, count)) = self.selection_mut(list) else {
            return NOT_MINE;
        };
        let (mode, select_on_focus) = (sel.mode, sel.select_on_focus);
        match key {
            Key::Space => {
                let Some(row) = cursor else { return NOT_MINE };
                match mode {
                    SelectionMode::Multiple if shift => {
                        sel.ensure_anchor(row);
                        sel.select_range(row, count, ctrl, keyboard);
                    }
                    SelectionMode::Multiple => sel.toggle(row, count, true, keyboard),
                    _ => sel.select_only(row, count, keyboard),
                }
                ListKey {
                    consumed: true,
                    focus: None,
                }
            }
            Key::Enter => {
                if mode == SelectionMode::Single
                    && let Some(row) = cursor
                {
                    sel.select_only(row, count, keyboard);
                }
                NOT_MINE
            }
            Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown => {
                let Some(target) = self.list_key_target(list, key, cursor) else {
                    return ListKey {
                        consumed: true,
                        focus: None,
                    };
                };
                let Some((sel, count)) = self.selection_mut(list) else {
                    return NOT_MINE;
                };
                if mode == SelectionMode::Multiple && shift {
                    sel.ensure_anchor(cursor.unwrap_or(target));
                    sel.select_range(target, count, ctrl, keyboard);
                } else if !ctrl && !shift && select_on_focus {
                    sel.select_only(target, count, keyboard);
                }
                ListKey {
                    consumed: true,
                    focus: self.focus_list_row(list, target),
                }
            }
            _ => NOT_MINE,
        }
    }

    /// The row a movement key takes the keyboard to from `cursor`: always a selectable row.
    fn list_key_target(&self, list: NodeId, key: Key, cursor: Option<usize>) -> Option<usize> {
        let NodeKind::VirtualList(state) = &self.nodes.get(list)?.kind else {
            return None;
        };
        let (sel, count) = (&state.selection, state.item_count);
        let Some(from) = cursor else {
            // Nowhere yet: going down or to the start begins at the first row, going up or to the
            // end at the last.
            return match key {
                Key::ArrowUp | Key::End | Key::PageUp => sel.last_selectable(count),
                _ => sel.first_selectable(count),
            };
        };
        let page = f64::from(self.layout(list).size.height) * state.metrics().page_fraction;
        // A `size_hint` list that has not been laid out knows no offsets: a page is a row.
        let measured = state.total_extent() > 0.0;
        match key {
            Key::Home => sel.first_selectable(count),
            Key::End => sel.last_selectable(count),
            Key::ArrowDown => sel.next_selectable(from, count).or(Some(from)),
            Key::ArrowUp => sel.prev_selectable(from).or(Some(from)),
            Key::PageDown => {
                let to = if measured {
                    state.index_at(state.offset_of(from) + page).max(from + 1)
                } else {
                    from + 1
                };
                sel.nearest_selectable(to, count, true)
            }
            Key::PageUp => {
                let to = if measured {
                    state.index_at((state.offset_of(from) - page).max(0.0))
                } else {
                    0
                }
                .min(from.saturating_sub(1));
                sel.nearest_selectable(to, count, false)
            }
            _ => None,
        }
    }

    /// Takes the keyboard to row `row` of `list`: shows it, and moves focus into it. A row that is
    /// not built yet gets focus when it is (`virtual_list_adopt`). Focus's transition, if it moved.
    pub(super) fn focus_list_row(
        &mut self,
        list: NodeId,
        row: usize,
    ) -> Option<(Option<NodeId>, Option<NodeId>)> {
        let built = match &mut self.nodes.get_mut(list)?.kind {
            NodeKind::VirtualList(state) => {
                state.selection.active = Some(row);
                state.selection.pending_focus = None;
                state.materialized.get(&row).copied()
            }
            _ => return None,
        };
        self.scroll_virtual_list_to_index(list, row, ScrollAlign::Nearest);
        match built {
            Some(built) => self.focus_into(built),
            None => {
                if let Some((sel, _)) = self.selection_mut(list) {
                    sel.pending_focus = Some(row);
                }
                None
            }
        }
    }

    /// Focus to the first node of `row` that takes it.
    pub(super) fn focus_into(&mut self, row: NodeId) -> Option<(Option<NodeId>, Option<NodeId>)> {
        let mut stops = Vec::new();
        self.collect_interactive(row, false, &mut stops);
        self.set_focus_to(*stops.first()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;
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

    /// A 200x100 list of `count` focusable 20 dp rows in `mode`, with rows 0..=5 built.
    fn make_list(count: usize, mode: SelectionMode) -> (Tree, NodeId) {
        let mut tree = Tree::new();
        let mut state = VirtualListState::new(count, ItemExtent::Fixed(20.0));
        state.selection.mode = mode;
        let list = tree.insert(NodeKind::VirtualList(state), style(200.0, 100.0), paint());
        lay_out(&mut tree, list);
        build(&mut tree, list, 0..6);
        (tree, list)
    }

    /// What the host does at layout: builds the rows in `window` (focusable boxes), keeping
    /// the ones built.
    fn build(tree: &mut Tree, list: NodeId, window: std::ops::Range<usize>) {
        for idx in window {
            let built = matches!(&tree.get(list).unwrap().kind,
                NodeKind::VirtualList(s) if s.materialized.contains_key(&idx));
            if built {
                continue;
            }
            let row = tree.insert(NodeKind::Rect, style(200.0, 20.0), paint());
            tree.get_mut(row).unwrap().access.focusable = Some(true);
            assert!(tree.virtual_list_adopt(list, idx, row));
        }
        lay_out(tree, list);
    }

    fn row(tree: &Tree, list: NodeId, idx: usize) -> NodeId {
        match &tree.get(list).unwrap().kind {
            NodeKind::VirtualList(s) => s.materialized[&idx],
            _ => panic!("not a list"),
        }
    }

    fn held(tree: &mut Tree, shift: bool, ctrl: bool) {
        tree.set_held_modifiers(Modifiers {
            shift,
            ctrl,
            ..Modifiers::default()
        });
    }

    fn press_row(tree: &mut Tree, list: NodeId, idx: usize, shift: bool, ctrl: bool) {
        held(tree, shift, ctrl);
        let hit = row(tree, list, idx);
        tree.list_press(Some(hit), Point::new(10.0, 10.0));
        tree.list_release(Some(hit));
    }

    fn chosen(tree: &Tree, list: NodeId) -> Vec<usize> {
        tree.virtual_list_selection(list)
            .unwrap()
            .selected()
            .iter()
            .collect()
    }

    fn anchor(tree: &Tree, list: NodeId) -> Option<usize> {
        tree.virtual_list_selection(list).unwrap().anchor()
    }

    fn key(tree: &mut Tree, key: Key, shift: bool, ctrl: bool) -> ListKey {
        held(tree, shift, ctrl);
        tree.list_selection_key(key, shift)
    }

    fn focus(tree: &mut Tree, list: NodeId, idx: usize) {
        let row = row(tree, list, idx);
        tree.set_focus_to(row);
    }

    #[test]
    fn a_list_with_mode_none_is_left_alone() {
        let (mut tree, list) = make_list(10, SelectionMode::None);
        press_row(&mut tree, list, 1, false, false);
        focus(&mut tree, list, 1);
        assert_eq!(key(&mut tree, Key::ArrowDown, false, false), NOT_MINE);
        assert_eq!(key(&mut tree, Key::Space, false, false), NOT_MINE);
        assert!(!tree.select_all_in_focused_list());
        assert!(chosen(&tree, list).is_empty());
        assert!(tree.take_selection_changes().is_empty());
    }

    /// AC-SLT-1: on items A to E (0 to 4), click B, Shift+click D, Ctrl+click E, Shift+click C.
    #[test]
    fn clicks_choose_ranges_and_toggles() {
        let (mut tree, list) = make_list(5, SelectionMode::Multiple);
        for (idx, shift, ctrl, set, anchor_at) in [
            (1, false, false, vec![1], 1),
            (3, true, false, vec![1, 2, 3], 1),
            (4, false, true, vec![1, 2, 3, 4], 4),
            (2, true, false, vec![2, 3, 4], 4),
        ] {
            press_row(&mut tree, list, idx, shift, ctrl);
            assert_eq!(chosen(&tree, list), set);
            assert_eq!(anchor(&tree, list), Some(anchor_at));
        }
        let changes = tree.take_selection_changes();
        assert_eq!(
            changes.len(),
            1,
            "one report for what has changed since the last call"
        );
        assert_eq!(changes[0].source, SelectionSource::Pointer);
        assert!(tree.take_selection_changes().is_empty());
    }

    #[test]
    fn single_mode_chooses_one_whatever_the_modifiers() {
        let (mut tree, list) = make_list(5, SelectionMode::Single);
        press_row(&mut tree, list, 1, false, false);
        press_row(&mut tree, list, 3, true, false);
        assert_eq!(chosen(&tree, list), [3]);
        press_row(&mut tree, list, 4, false, true);
        assert_eq!(chosen(&tree, list), [4]);
    }

    #[test]
    fn ctrl_shift_click_adds_the_range() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        press_row(&mut tree, list, 0, false, false);
        press_row(&mut tree, list, 4, false, true);
        press_row(&mut tree, list, 2, true, true);
        assert_eq!(chosen(&tree, list), [0, 2, 3, 4]);
        assert_eq!(anchor(&tree, list), Some(4));
    }

    #[test]
    fn a_click_in_the_empty_part_clears_a_multiple_selection() {
        let (mut tree, list) = make_list(3, SelectionMode::Multiple);
        press_row(&mut tree, list, 1, false, false);
        held(&mut tree, false, false);
        tree.list_press(Some(list), Point::new(10.0, 90.0));
        assert!(chosen(&tree, list).is_empty());
        // With Ctrl or Shift held it is not a click on nothing: the set stays.
        press_row(&mut tree, list, 1, false, false);
        held(&mut tree, false, true);
        tree.list_press(Some(list), Point::new(10.0, 90.0));
        assert_eq!(chosen(&tree, list), [1]);
    }

    /// AC-SLT-13: a press on a chosen row of several, released without moving, chooses it alone;
    /// one that goes `drag_start_px` leaves the set for the app's drag.
    #[test]
    fn a_press_on_a_chosen_row_waits_for_its_release() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        press_row(&mut tree, list, 1, false, false);
        press_row(&mut tree, list, 3, true, false);
        assert_eq!(chosen(&tree, list), [1, 2, 3]);
        tree.take_selection_changes();
        tree.set_drag_start_px(4.0);

        held(&mut tree, false, false);
        let hit = row(&tree, list, 2);
        tree.list_press(Some(hit), Point::new(10.0, 50.0));
        assert_eq!(chosen(&tree, list), [1, 2, 3], "not on the press");
        tree.list_press_moved(Point::new(12.0, 51.0));
        tree.list_release(Some(hit));
        assert_eq!(chosen(&tree, list), [2], "2.2 px is a click");

        press_row(&mut tree, list, 1, false, false);
        press_row(&mut tree, list, 3, true, false);
        held(&mut tree, false, false);
        tree.list_press(Some(hit), Point::new(10.0, 50.0));
        tree.list_press_moved(Point::new(14.0, 50.0));
        tree.list_press_moved(Point::new(10.0, 50.0));
        tree.list_release(Some(hit));
        assert_eq!(
            chosen(&tree, list),
            [1, 2, 3],
            "4 px started a drag, and stays one"
        );
    }

    #[test]
    fn with_no_drag_start_a_press_and_release_is_always_a_click() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        press_row(&mut tree, list, 0, false, false);
        press_row(&mut tree, list, 3, true, false);
        let hit = row(&tree, list, 1);
        held(&mut tree, false, false);
        tree.list_press(Some(hit), Point::new(10.0, 30.0));
        tree.list_press_moved(Point::new(150.0, 30.0));
        tree.list_release(Some(hit));
        assert_eq!(chosen(&tree, list), [1]);
    }

    #[test]
    fn a_release_on_another_row_or_a_cancel_leaves_the_set() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        press_row(&mut tree, list, 0, false, false);
        press_row(&mut tree, list, 3, true, false);
        let hit = row(&tree, list, 1);
        held(&mut tree, false, false);
        tree.list_press(Some(hit), Point::new(10.0, 30.0));
        tree.list_release(Some(row(&tree, list, 2)));
        assert_eq!(chosen(&tree, list), [0, 1, 2, 3]);
        tree.list_press(Some(hit), Point::new(10.0, 30.0));
        tree.list_press_cancel();
        tree.list_release(Some(hit));
        assert_eq!(chosen(&tree, list), [0, 1, 2, 3]);
    }

    /// AC-SLT-2: focus B (chosen), Shift+Down twice -> B..D, Shift+Up once -> B..C; the anchor stays.
    #[test]
    fn shift_arrows_extend_from_the_anchor() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        press_row(&mut tree, list, 1, false, false);
        focus(&mut tree, list, 1);
        for (k, set) in [
            (Key::ArrowDown, vec![1, 2]),
            (Key::ArrowDown, vec![1, 2, 3]),
            (Key::ArrowUp, vec![1, 2]),
        ] {
            assert!(key(&mut tree, k, true, false).consumed);
            assert_eq!(chosen(&tree, list), set);
            assert_eq!(anchor(&tree, list), Some(1));
        }
        assert_eq!(tree.focused(), Some(row(&tree, list, 2)));
        // A range with no anchor starts at the focused row.
        let (mut tree, list) = list_of_six();
        focus(&mut tree, list, 2);
        key(&mut tree, Key::ArrowDown, true, false);
        assert_eq!(chosen(&tree, list), [2, 3]);
    }

    fn list_of_six() -> (Tree, NodeId) {
        make_list(6, SelectionMode::Multiple)
    }

    /// AC-SLT-3: Ctrl+Down moves focus alone; Ctrl+Space toggles the focused row; Space toggles in
    /// a multiple list and chooses in a single one.
    #[test]
    fn ctrl_arrows_move_focus_alone_and_space_toggles() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        press_row(&mut tree, list, 0, false, false);
        focus(&mut tree, list, 0);
        key(&mut tree, Key::ArrowDown, false, true);
        key(&mut tree, Key::ArrowDown, false, true);
        assert_eq!(tree.focused(), Some(row(&tree, list, 2)));
        assert_eq!(chosen(&tree, list), [0]);
        assert!(key(&mut tree, Key::Space, false, true).consumed);
        assert_eq!(chosen(&tree, list), [0, 2]);
        key(&mut tree, Key::Space, false, false);
        assert_eq!(chosen(&tree, list), [0], "toggled off");

        let (mut tree, list) = make_list(6, SelectionMode::Single);
        focus(&mut tree, list, 1);
        key(&mut tree, Key::Space, false, false);
        key(&mut tree, Key::ArrowDown, false, false);
        key(&mut tree, Key::Space, false, false);
        assert_eq!(chosen(&tree, list), [2]);
    }

    #[test]
    fn plain_arrows_move_focus_and_choose_only_with_select_on_focus() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        focus(&mut tree, list, 0);
        key(&mut tree, Key::ArrowDown, false, false);
        assert_eq!(tree.focused(), Some(row(&tree, list, 1)));
        assert!(chosen(&tree, list).is_empty());
        if let NodeKind::VirtualList(s) = &mut tree.get_mut(list).unwrap().kind {
            s.selection.select_on_focus = true;
        }
        key(&mut tree, Key::ArrowDown, false, false);
        assert_eq!(chosen(&tree, list), [2]);
        key(&mut tree, Key::ArrowDown, false, true);
        assert_eq!(
            chosen(&tree, list),
            [2],
            "Ctrl moves focus only, in every case"
        );
    }

    #[test]
    fn enter_chooses_in_a_single_list_and_is_still_an_activation() {
        let (mut tree, list) = make_list(6, SelectionMode::Single);
        focus(&mut tree, list, 3);
        assert_eq!(key(&mut tree, Key::Enter, false, false), NOT_MINE);
        assert_eq!(chosen(&tree, list), [3]);
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        focus(&mut tree, list, 3);
        key(&mut tree, Key::Enter, false, false);
        assert!(chosen(&tree, list).is_empty());
    }

    /// AC-SLT-5: Up, Down, Home, End skip the unselectable rows; a range passes them.
    #[test]
    fn focus_never_lands_on_an_unselectable_row() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        if let NodeKind::VirtualList(s) = &mut tree.get_mut(list).unwrap().kind {
            s.selection
                .set_unselectable(RangeSet::from_indices([0, 2, 3]), SelectionSource::Api);
        }
        focus(&mut tree, list, 1);
        key(&mut tree, Key::ArrowDown, false, false);
        assert_eq!(tree.focused(), Some(row(&tree, list, 4)));
        key(&mut tree, Key::ArrowUp, false, false);
        key(&mut tree, Key::ArrowUp, false, false);
        assert_eq!(
            tree.focused(),
            Some(row(&tree, list, 1)),
            "0 is skipped, and there is nothing before 1"
        );
        key(&mut tree, Key::Home, false, false);
        assert_eq!(tree.focused(), Some(row(&tree, list, 1)));
        key(&mut tree, Key::End, false, false);
        assert_eq!(tree.focused(), Some(row(&tree, list, 5)));
        press_row(&mut tree, list, 1, false, false);
        press_row(&mut tree, list, 5, true, false);
        assert_eq!(chosen(&tree, list), [1, 4, 5]);
        press_row(&mut tree, list, 2, false, false);
        assert_eq!(chosen(&tree, list), [1, 4, 5], "a click on one is nothing");
    }

    #[test]
    fn page_keys_move_a_page_of_rows() {
        let (mut tree, list) = make_list(100, SelectionMode::Multiple);
        focus(&mut tree, list, 0);
        key(&mut tree, Key::PageDown, false, false);
        // 100 dp viewport less 10 percent from row 0's top: row 4, which is built (rows 0..6).
        assert_eq!(tree.focused(), Some(row(&tree, list, 4)));
        key(&mut tree, Key::PageUp, false, false);
        assert_eq!(tree.focused(), Some(row(&tree, list, 0)));
    }

    #[test]
    fn a_row_that_is_not_built_gets_focus_when_it_is() {
        let (mut tree, list) = make_list(100, SelectionMode::Multiple);
        focus(&mut tree, list, 0);
        let result = key(&mut tree, Key::End, false, false);
        assert!(
            result.consumed && result.focus.is_none(),
            "row 99 is not built"
        );
        assert_eq!(tree.focused(), Some(row(&tree, list, 0)));
        let visible = tree.virtual_list_visible(list);
        assert_eq!(visible, 95..100, "the list scrolled to the end");
        build(&mut tree, list, visible);
        assert_eq!(tree.resolve_list_focus().len(), 1);
        assert_eq!(tree.focused(), Some(row(&tree, list, 99)));
        assert_eq!(
            tree.virtual_list_selection(list).unwrap().pending_focus,
            None
        );
        assert!(tree.resolve_list_focus().is_empty());
    }

    /// AC-SLT-6 at the model: Shift+End from row 0 of 100000 chooses all of them, and the only rows
    /// built are the window and the one that holds focus.
    #[test]
    fn shift_end_over_a_hundred_thousand_rows_builds_no_row_for_it() {
        let (mut tree, list) = make_list(100_000, SelectionMode::Multiple);
        press_row(&mut tree, list, 0, false, false);
        focus(&mut tree, list, 0);
        key(&mut tree, Key::End, true, false);
        assert_eq!(
            tree.virtual_list_selection(list).unwrap().selected().len(),
            100_000
        );
        let visible = tree.virtual_list_visible(list);
        build(&mut tree, list, visible);
        tree.resolve_list_focus();
        tree.virtual_list_release_outside(list, tree.virtual_list_visible(list));
        let built = match &tree.get(list).unwrap().kind {
            NodeKind::VirtualList(s) => s.materialized.len(),
            _ => unreachable!(),
        };
        assert!(built <= 5, "{built} rows built, 5 are shown");
    }

    /// AC-SLT-4: Ctrl+A chooses every selectable row, only when the list asks for it.
    #[test]
    fn select_all_is_opt_in_and_skips_unselectable_rows() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        focus(&mut tree, list, 1);
        assert!(!tree.select_all_in_focused_list(), "select_all_key is off");
        if let NodeKind::VirtualList(s) = &mut tree.get_mut(list).unwrap().kind {
            s.selection.select_all_key = true;
            s.selection
                .set_unselectable(RangeSet::from_indices([2]), SelectionSource::Api);
        }
        assert!(tree.select_all_in_focused_list());
        assert_eq!(chosen(&tree, list), [0, 1, 3, 4, 5]);
        tree.clear_focus();
        assert!(
            !tree.select_all_in_focused_list(),
            "focus is not in the list"
        );
    }

    #[test]
    fn a_single_list_has_no_select_all() {
        let (mut tree, list) = make_list(6, SelectionMode::Single);
        if let NodeKind::VirtualList(s) = &mut tree.get_mut(list).unwrap().kind {
            s.selection.select_all_key = true;
        }
        focus(&mut tree, list, 1);
        assert!(!tree.select_all_in_focused_list());
    }

    #[test]
    fn keys_and_escape_with_alt_or_meta_or_nothing_focused_are_not_the_lists() {
        let (mut tree, list) = make_list(6, SelectionMode::Multiple);
        assert_eq!(
            key(&mut tree, Key::ArrowDown, false, false),
            NOT_MINE,
            "nothing focused"
        );
        focus(&mut tree, list, 1);
        tree.set_held_modifiers(Modifiers {
            alt: true,
            ..Modifiers::default()
        });
        assert_eq!(tree.list_selection_key(Key::ArrowDown, false), NOT_MINE);
        assert_eq!(key(&mut tree, Key::Escape, false, false), NOT_MINE);
        assert_eq!(key(&mut tree, Key::Tab, false, false), NOT_MINE);
    }
}
