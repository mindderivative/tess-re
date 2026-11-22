//! Row selection of a virtual list: which rows are chosen, the anchor a range
//! starts from, which rows cannot be chosen, and the rules that change them
//! (UX spec "Selection", AC-SLT-1 to AC-SLT-6).
//!
//! The set is a sorted list of disjoint index ranges, never a flag per row, so a
//! range or select-all over a hundred thousand rows costs the number of ranges, and
//! no row has to be built for it. A row's key is its index in the list.
//!
//! This module is pure state: it knows the item count it is told and nothing of
//! nodes, focus or input. `tree/list_selection.rs` feeds it clicks and keys.

use std::ops::Range;

/// A set of `usize` kept as sorted, disjoint, non-adjacent, non-empty ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RangeSet {
    ranges: Vec<Range<usize>>,
}

impl RangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// The set of every index in `items`, each once, however they are ordered.
    pub fn from_indices(items: impl IntoIterator<Item = usize>) -> Self {
        let mut sorted: Vec<usize> = items.into_iter().collect();
        sorted.sort_unstable();
        sorted.dedup();
        let mut set = Self::new();
        for i in sorted {
            match set.ranges.last_mut() {
                Some(last) if last.end == i => last.end += 1,
                _ => set.ranges.push(i..i + 1),
            }
        }
        set
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// How many indices the set holds.
    pub fn len(&self) -> usize {
        self.ranges.iter().map(ExactSizeIterator::len).sum()
    }

    /// The ranges, ascending.
    pub fn ranges(&self) -> &[Range<usize>] {
        &self.ranges
    }

    /// Every index, ascending.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.ranges.iter().flat_map(Clone::clone)
    }

    pub fn first(&self) -> Option<usize> {
        self.ranges.first().map(|r| r.start)
    }

    pub fn contains(&self, i: usize) -> bool {
        let at = self.ranges.partition_point(|r| r.end <= i);
        self.ranges.get(at).is_some_and(|r| r.start <= i)
    }

    pub fn clear(&mut self) {
        self.ranges.clear();
    }

    /// Adds every index of `range`.
    pub fn insert(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        // The ranges that overlap or touch `range` are merged with it.
        let lo = self.ranges.partition_point(|r| r.end < range.start);
        let hi = self.ranges.partition_point(|r| r.start <= range.end);
        let (mut start, mut end) = (range.start, range.end);
        if lo < hi {
            start = start.min(self.ranges[lo].start);
            end = end.max(self.ranges[hi - 1].end);
        }
        self.ranges.splice(lo..hi, std::iter::once(start..end));
    }

    /// Adds every index of `range` that is not in `except`.
    pub fn insert_except(&mut self, range: Range<usize>, except: &RangeSet) {
        let mut at = range.start;
        let first = except.ranges.partition_point(|r| r.end <= range.start);
        for gap in except.ranges[first..]
            .iter()
            .take_while(|r| r.start < range.end)
        {
            if gap.start > at {
                self.insert(at..gap.start);
            }
            at = at.max(gap.end);
        }
        if at < range.end {
            self.insert(at..range.end);
        }
    }

    /// Takes out every index of `range`.
    pub fn remove(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let lo = self.ranges.partition_point(|r| r.end <= range.start);
        let hi = self.ranges.partition_point(|r| r.start < range.end);
        if lo >= hi {
            return;
        }
        let mut kept = Vec::with_capacity(2);
        if self.ranges[lo].start < range.start {
            kept.push(self.ranges[lo].start..range.start);
        }
        if self.ranges[hi - 1].end > range.end {
            kept.push(range.end..self.ranges[hi - 1].end);
        }
        self.ranges.splice(lo..hi, kept);
    }

    /// Takes out every index of `other`.
    pub fn subtract(&mut self, other: &RangeSet) {
        for range in &other.ranges {
            self.remove(range.clone());
        }
    }

    /// Adds `i` if it is not in the set and takes it out if it is; whether it is in now.
    pub fn toggle(&mut self, i: usize) -> bool {
        if self.contains(i) {
            self.remove(i..i + 1);
            false
        } else {
            self.insert(i..i + 1);
            true
        }
    }

    /// Takes out every index from `len` on.
    pub fn clip(&mut self, len: usize) {
        self.remove(len..usize::MAX);
    }

    /// The smallest index from `i` on that is not in the set.
    fn next_free(&self, i: usize) -> usize {
        let at = self.ranges.partition_point(|r| r.end <= i);
        match self.ranges.get(at) {
            Some(r) if r.start <= i => r.end,
            _ => i,
        }
    }

    /// The largest index up to `i` that is not in the set, if any.
    fn prev_free(&self, i: usize) -> Option<usize> {
        let at = self.ranges.partition_point(|r| r.end <= i);
        match self.ranges.get(at) {
            Some(r) if r.start <= i => r.start.checked_sub(1),
            _ => Some(i),
        }
    }
}

/// How a list's rows are chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// Nothing is chosen by the engine: no keys, clicks or set (the default).
    #[default]
    None,
    /// At most one row.
    Single,
    /// Any set of rows.
    Multiple,
}

impl SelectionMode {
    pub const ALL: [(&'static str, SelectionMode); 3] = [
        ("none", Self::None),
        ("single", Self::Single),
        ("multiple", Self::Multiple),
    ];
}

/// What made the selection change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionSource {
    Pointer,
    Keyboard,
    /// A property: `selection`, `selection_mode`, `unselectable_keys`, `item_count`.
    Api,
}

impl SelectionSource {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Keyboard => "keyboard",
            Self::Api => "api",
        }
    }
}

/// A virtual list's selection.
#[derive(Clone, Debug, Default)]
pub struct SelectionState {
    pub mode: SelectionMode,
    selected: RangeSet,
    unselectable: RangeSet,
    anchor: Option<usize>,
    /// `select_all_key`: whether Ctrl+A selects every selectable row.
    pub select_all_key: bool,
    /// `select_on_focus`: whether moving focus by Up, Down, Home, End, Page Up or
    /// Page Down also chooses the row focus lands on.
    pub select_on_focus: bool,
    /// The row the keyboard is on while focus is not in a row (the list itself holds it).
    pub active: Option<usize>,
    /// The row focus should go to when it is built (`Tree::focus_list_row`).
    pub pending_focus: Option<usize>,
    /// A change not yet reported as `selection_change`, and what made the last of it.
    pending: Option<SelectionSource>,
}

impl SelectionState {
    pub fn selected(&self) -> &RangeSet {
        &self.selected
    }

    pub fn unselectable(&self) -> &RangeSet {
        &self.unselectable
    }

    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    pub fn is_selected(&self, i: usize) -> bool {
        self.selected.contains(i)
    }

    /// Whether row `i` of `count` can be chosen.
    pub fn is_selectable(&self, i: usize, count: usize) -> bool {
        i < count && !self.unselectable.contains(i)
    }

    /// The change since the last call, if there is one, and what made it.
    pub fn take_pending(&mut self) -> Option<SelectionSource> {
        self.pending.take()
    }

    /// Whether a change is waiting to be reported.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn replace(&mut self, next: RangeSet, source: SelectionSource) {
        if next != self.selected {
            self.selected = next;
            self.pending = Some(source);
        }
    }

    /// Chooses row `i` and nothing else; it is the anchor.
    pub fn select_only(&mut self, i: usize, count: usize, source: SelectionSource) {
        if !self.is_selectable(i, count) {
            return;
        }
        let mut next = RangeSet::new();
        next.insert(i..i + 1);
        self.replace(next, source);
        self.anchor = Some(i);
    }

    /// Flips row `i`. It becomes the anchor, unless it was flipped off and `keep_anchor_off` is set.
    pub fn toggle(
        &mut self,
        i: usize,
        count: usize,
        keep_anchor_off: bool,
        source: SelectionSource,
    ) {
        if !self.is_selectable(i, count) {
            return;
        }
        let mut next = self.selected.clone();
        let on = next.toggle(i);
        self.replace(next, source);
        if on || !keep_anchor_off {
            self.anchor = Some(i);
        }
    }

    /// `row` is the anchor if there is none.
    pub fn ensure_anchor(&mut self, row: usize) {
        self.anchor.get_or_insert(row);
    }

    /// Chooses every selectable row from the anchor to `to` -- replacing the set, or
    /// adding to it when `add`. The anchor stays; with none, `to` becomes it.
    pub fn select_range(&mut self, to: usize, count: usize, add: bool, source: SelectionSource) {
        if to >= count {
            return;
        }
        let from = self.anchor.unwrap_or(to);
        let (lo, hi) = (from.min(to), from.max(to).min(count - 1));
        let mut next = if add {
            self.selected.clone()
        } else {
            RangeSet::new()
        };
        next.insert_except(lo..hi + 1, &self.unselectable);
        self.replace(next, source);
        self.anchor.get_or_insert(to);
    }

    /// Chooses every selectable row. The anchor is where it was.
    pub fn select_all(&mut self, count: usize, source: SelectionSource) {
        let mut next = RangeSet::new();
        next.insert_except(0..count, &self.unselectable);
        self.replace(next, source);
    }

    /// Chooses no row. The anchor is where it was.
    pub fn clear(&mut self, source: SelectionSource) {
        self.replace(RangeSet::new(), source);
    }

    /// `selection`: the set becomes exactly `set`, which the caller has checked.
    pub fn set_selected(&mut self, set: RangeSet, source: SelectionSource) {
        let anchor = self.anchor;
        self.replace(set, source);
        // The anchor stays while it is still chosen, else it is the first row chosen.
        self.anchor = anchor
            .filter(|&a| self.selected.contains(a))
            .or_else(|| self.selected.first());
    }

    /// `unselectable_keys`: those rows can no longer be chosen, and leave the set.
    pub fn set_unselectable(&mut self, set: RangeSet, source: SelectionSource) {
        self.unselectable = set;
        let mut next = self.selected.clone();
        next.subtract(&self.unselectable);
        self.replace(next, source);
        if self.anchor.is_some_and(|a| self.unselectable.contains(a)) {
            self.anchor = None;
        }
    }

    /// `selection_mode`: `None` clears the set, `Single` keeps one row (the anchor if chosen,
    /// else the first).
    pub fn set_mode(&mut self, mode: SelectionMode, source: SelectionSource) {
        self.mode = mode;
        match mode {
            SelectionMode::None => {
                self.clear(source);
                self.anchor = None;
                self.active = None;
                self.pending_focus = None;
            }
            SelectionMode::Single if self.selected.len() > 1 => {
                let keep = self
                    .anchor
                    .filter(|&a| self.selected.contains(a))
                    .or_else(|| self.selected.first());
                if let Some(keep) = keep {
                    let mut next = RangeSet::new();
                    next.insert(keep..keep + 1);
                    self.replace(next, source);
                    self.anchor = Some(keep);
                }
            }
            _ => {}
        }
    }

    /// The list has `count` rows now: nothing past them stays chosen, anchored or active.
    pub fn clip(&mut self, count: usize, source: SelectionSource) {
        let mut next = self.selected.clone();
        next.clip(count);
        self.replace(next, source);
        self.unselectable.clip(count);
        self.anchor = self.anchor.filter(|&a| a < count);
        self.active = self.active.filter(|&a| a < count);
        self.pending_focus = self.pending_focus.filter(|&a| a < count);
    }

    /// The first selectable row after `from`.
    pub fn next_selectable(&self, from: usize, count: usize) -> Option<usize> {
        let next = self.unselectable.next_free(from.checked_add(1)?);
        (next < count).then_some(next)
    }

    /// The last selectable row before `from`.
    pub fn prev_selectable(&self, from: usize) -> Option<usize> {
        self.unselectable.prev_free(from.checked_sub(1)?)
    }

    pub fn first_selectable(&self, count: usize) -> Option<usize> {
        let first = self.unselectable.next_free(0);
        (first < count).then_some(first)
    }

    pub fn last_selectable(&self, count: usize) -> Option<usize> {
        self.unselectable.prev_free(count.checked_sub(1)?)
    }

    /// The selectable row nearest `i` going `forward` (or back) -- `i` itself if it is one;
    /// if none lies that way, the nearest the other way.
    pub fn nearest_selectable(&self, i: usize, count: usize, forward: bool) -> Option<usize> {
        let i = i.min(count.checked_sub(1)?);
        if self.is_selectable(i, count) {
            return Some(i);
        }
        let ahead = if forward {
            self.next_selectable(i, count)
        } else {
            self.prev_selectable(i)
        };
        ahead.or_else(|| {
            if forward {
                self.prev_selectable(i)
            } else {
                self.next_selectable(i, count)
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)] // the expected ranges are written as ranges
mod tests {
    use super::*;

    const KEY: SelectionSource = SelectionSource::Keyboard;

    fn set(indices: &[usize]) -> RangeSet {
        RangeSet::from_indices(indices.iter().copied())
    }

    fn chosen(state: &SelectionState) -> Vec<usize> {
        state.selected().iter().collect()
    }

    #[test]
    fn a_range_set_merges_what_overlaps_or_touches() {
        let mut s = RangeSet::new();
        s.insert(5..8);
        s.insert(0..2);
        assert_eq!(s.ranges(), [0..2, 5..8]);
        s.insert(2..5);
        assert_eq!(s.ranges(), [0..8], "touching ranges join");
        s.insert(20..30);
        s.insert(4..25);
        assert_eq!(s.ranges(), [0..30]);
        s.insert(3..3);
        assert_eq!(s.ranges(), [0..30], "an empty range adds nothing");
        assert_eq!(s.len(), 30);
    }

    #[test]
    fn a_range_set_removes_the_middle_of_a_range() {
        let mut s = set(&[0, 1, 2, 3, 4, 5, 6, 7, 9]);
        s.remove(3..5);
        assert_eq!(s.ranges(), [0..3, 5..8, 9..10]);
        s.remove(0..100);
        assert!(s.is_empty());
    }

    #[test]
    fn a_range_set_answers_membership_and_toggles() {
        let mut s = set(&[1, 2, 3, 7]);
        assert!(s.contains(2) && s.contains(7));
        assert!(!s.contains(0) && !s.contains(4) && !s.contains(8));
        assert!(s.toggle(4));
        assert_eq!(s.ranges(), [1..5, 7..8]);
        assert!(!s.toggle(2));
        assert_eq!(s.ranges(), [1..2, 3..5, 7..8]);
        s.clip(4);
        assert_eq!(s.ranges(), [1..2, 3..4]);
    }

    #[test]
    fn insert_except_leaves_the_excluded_rows_out() {
        let except = set(&[2, 3, 7]);
        let mut s = RangeSet::new();
        s.insert_except(0..10, &except);
        assert_eq!(s.ranges(), [0..2, 4..7, 8..10]);
        let mut inside = RangeSet::new();
        inside.insert_except(2..4, &except);
        assert!(inside.is_empty());
    }

    #[test]
    fn from_indices_ignores_order_and_repeats() {
        assert_eq!(set(&[5, 1, 2, 5, 3, 9]).ranges(), [1..4, 5..6, 9..10]);
    }

    #[test]
    fn a_click_chooses_only_that_row_and_anchors_it() {
        let mut s = SelectionState::default();
        s.select_only(1, 5, SelectionSource::Pointer);
        s.select_only(3, 5, SelectionSource::Pointer);
        assert_eq!(chosen(&s), [3]);
        assert_eq!(s.anchor(), Some(3));
        assert_eq!(s.take_pending(), Some(SelectionSource::Pointer));
        assert_eq!(s.take_pending(), None);
    }

    /// AC-SLT-1 on items A to E (0 to 4).
    #[test]
    fn the_click_sequence_of_ac_slt_1() {
        let mut s = SelectionState::default();
        let p = SelectionSource::Pointer;
        s.select_only(1, 5, p); // click B
        assert_eq!((chosen(&s), s.anchor()), (vec![1], Some(1)));
        s.select_range(3, 5, false, p); // shift+click D
        assert_eq!((chosen(&s), s.anchor()), (vec![1, 2, 3], Some(1)));
        s.toggle(4, 5, false, p); // ctrl+click E
        assert_eq!((chosen(&s), s.anchor()), (vec![1, 2, 3, 4], Some(4)));
        s.select_range(2, 5, false, p); // shift+click C: E..C replaces
        assert_eq!((chosen(&s), s.anchor()), (vec![2, 3, 4], Some(4)));
    }

    #[test]
    fn ctrl_shift_adds_the_range_to_the_set() {
        let mut s = SelectionState::default();
        s.select_only(0, 10, KEY);
        s.toggle(5, 10, false, KEY);
        s.select_range(7, 10, true, KEY);
        assert_eq!(chosen(&s), [0, 5, 6, 7]);
    }

    #[test]
    fn a_toggle_off_moves_the_anchor_unless_told_to_keep_it() {
        let mut s = SelectionState::default();
        s.select_only(1, 5, KEY);
        s.toggle(1, 5, false, KEY);
        assert_eq!(s.anchor(), Some(1));
        s.select_only(2, 5, KEY);
        s.toggle(2, 5, true, KEY);
        assert_eq!(s.anchor(), Some(2), "toggled off, anchor kept");
        s.toggle(4, 5, true, KEY);
        assert_eq!(s.anchor(), Some(4), "toggled on, anchor moves");
    }

    #[test]
    fn a_range_skips_unselectable_rows_and_they_cannot_be_chosen() {
        let mut s = SelectionState::default();
        s.set_unselectable(set(&[2]), SelectionSource::Api);
        s.select_only(0, 5, KEY);
        s.select_range(4, 5, false, KEY);
        assert_eq!(chosen(&s), [0, 1, 3, 4]);
        s.select_only(2, 5, KEY);
        s.toggle(2, 5, false, KEY);
        assert_eq!(chosen(&s), [0, 1, 3, 4], "unchanged");
        s.select_all(5, KEY);
        assert_eq!(chosen(&s), [0, 1, 3, 4]);
    }

    #[test]
    fn making_a_chosen_row_unselectable_unchooses_it() {
        let mut s = SelectionState::default();
        s.select_all(4, KEY);
        s.take_pending();
        s.set_unselectable(set(&[1, 3]), SelectionSource::Api);
        assert_eq!(chosen(&s), [0, 2]);
        assert_eq!(s.take_pending(), Some(SelectionSource::Api));
    }

    #[test]
    fn a_hundred_thousand_rows_are_one_range() {
        let mut s = SelectionState::default();
        s.select_only(0, 100_000, KEY);
        s.select_range(99_999, 100_000, false, KEY);
        assert_eq!(s.selected().len(), 100_000);
        assert_eq!(s.selected().ranges(), [0..100_000]);
        s.select_all(100_000, KEY);
        assert_eq!(s.selected().ranges().len(), 1);
    }

    #[test]
    fn a_change_is_pending_only_when_the_set_changes() {
        let mut s = SelectionState::default();
        s.select_only(1, 5, KEY);
        s.take_pending();
        s.select_only(1, 5, KEY);
        assert!(!s.has_pending(), "the same set again");
        s.select_only(9, 5, KEY);
        assert!(!s.has_pending(), "past the end");
        s.clear(SelectionSource::Api);
        assert_eq!(s.take_pending(), Some(SelectionSource::Api));
    }

    #[test]
    fn navigation_skips_unselectable_rows() {
        let mut s = SelectionState::default();
        s.set_unselectable(set(&[0, 3, 4, 9]), SelectionSource::Api);
        let count = 10;
        assert_eq!(s.first_selectable(count), Some(1));
        assert_eq!(s.last_selectable(count), Some(8));
        assert_eq!(s.next_selectable(2, count), Some(5));
        assert_eq!(s.next_selectable(8, count), None);
        assert_eq!(s.prev_selectable(5), Some(2));
        assert_eq!(s.prev_selectable(1), None);
        assert_eq!(s.nearest_selectable(3, count, true), Some(5));
        assert_eq!(s.nearest_selectable(3, count, false), Some(2));
        assert_eq!(
            s.nearest_selectable(9, count, true),
            Some(8),
            "none ahead: back"
        );
        assert_eq!(s.nearest_selectable(7, count, true), Some(7));
        assert_eq!(s.first_selectable(0), None);
    }

    #[test]
    fn a_list_of_only_unselectable_rows_has_nowhere_to_go() {
        let mut s = SelectionState::default();
        s.set_unselectable(set(&[0, 1, 2]), SelectionSource::Api);
        assert_eq!(s.first_selectable(3), None);
        assert_eq!(s.last_selectable(3), None);
        assert_eq!(s.nearest_selectable(1, 3, true), None);
    }

    #[test]
    fn a_set_keeps_the_anchor_while_it_is_chosen() {
        let mut s = SelectionState::default();
        s.select_only(2, 10, KEY);
        s.set_selected(set(&[2, 3]), SelectionSource::Api);
        assert_eq!(s.anchor(), Some(2));
        s.set_selected(set(&[5, 6]), SelectionSource::Api);
        assert_eq!(s.anchor(), Some(5), "the first row chosen");
        s.set_selected(RangeSet::new(), SelectionSource::Api);
        assert_eq!(s.anchor(), None);
    }

    #[test]
    fn single_mode_keeps_one_row_and_none_keeps_nothing() {
        let mut s = SelectionState::default();
        s.select_all(6, KEY);
        s.set_mode(SelectionMode::Single, SelectionSource::Api);
        assert_eq!(chosen(&s), [0]);
        s.set_mode(SelectionMode::None, SelectionSource::Api);
        assert!(chosen(&s).is_empty());
        assert_eq!(s.anchor(), None);
    }

    #[test]
    fn a_shorter_list_drops_what_is_past_its_end() {
        let mut s = SelectionState::default();
        s.select_all(10, KEY);
        s.active = Some(8);
        s.clip(4, SelectionSource::Api);
        assert_eq!(chosen(&s), [0, 1, 2, 3]);
        assert_eq!(s.active, None);
    }
}
