//! 0.5.6 (#162, #165): what the focused text input needs from the window's
//! loop each frame -- the caret's blink, and where the IME should put its
//! candidate window.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use engine_core::{NodeKind, Tree};

use crate::timers::SharedAlarm;

/// A text input's or a terminal's blink: what `update_blink` reads and writes.
struct Blink<'a> {
    epoch: &'a mut Option<Instant>,
    on: &'a mut bool,
}

/// The blink of `node`, if it is a text input or a terminal.
fn blink_of(node: &mut engine_core::Node) -> Option<Blink<'_>> {
    match &mut node.kind {
        NodeKind::TextField(state) => Some(Blink {
            epoch: &mut state.caret_epoch,
            on: &mut state.caret_on,
        }),
        NodeKind::Terminal(state) => Some(Blink {
            epoch: &mut state.caret_epoch,
            on: &mut state.caret_on,
        }),
        _ => None,
    }
}

/// Moves the focused field's caret through its blink at `now` and wakes the
/// loop for the next change, so an idle field wakes at each edge and not every
/// frame. A field whose blink is off keeps a solid caret. Touches the tree only
/// when the phase changes, so a frame with nothing new is not a redraw.
/// 0.6.12 (P8): a focused terminal's cursor blinks the same way, but never under
/// reduced motion (the UX note "Terminal": steady with reduced motion).
pub(crate) fn update_blink(tree: &Rc<RefCell<Tree>>, now: Instant, alarm: &SharedAlarm) {
    let mut tree = tree.borrow_mut();
    let Some(field) = tree.focused() else {
        return;
    };
    let reduced = tree.reduced_motion();
    let Some(node) = tree.get(field) else {
        return;
    };
    let steady = reduced && matches!(node.kind, NodeKind::Terminal(_));
    // Read first, and touch the tree only when something changes: `get_mut` marks it dirty.
    let (period, epoch, on) = match &node.kind {
        NodeKind::TextField(state) => (state.caret_blink_ms, state.caret_epoch, state.caret_on),
        NodeKind::Terminal(state) => (state.caret_blink_ms, state.caret_epoch, state.caret_on),
        _ => return,
    };
    if period == 0 || steady {
        // Blinking is off (or the motion reduced) while the caret may be in its off phase.
        if !on && let Some(blink) = tree.get_mut(field).and_then(blink_of) {
            *blink.on = true;
        }
        return;
    }
    let epoch = match epoch {
        Some(epoch) => epoch,
        None => {
            if let Some(blink) = tree.get_mut(field).and_then(blink_of) {
                *blink.epoch = Some(now);
            }
            now
        }
    };
    let length = Duration::from_millis(u64::from(period));
    let phase = now.saturating_duration_since(epoch).as_millis() / u128::from(period);
    let should_be_on = phase.is_multiple_of(2);
    if should_be_on != on
        && let Some(blink) = tree.get_mut(field).and_then(blink_of)
    {
        *blink.on = should_be_on;
    }
    drop(tree);
    if let Some(alarm) = alarm.borrow().as_ref() {
        let next = epoch + length * u32::try_from(phase + 1).unwrap_or(u32::MAX);
        alarm.arm(next);
    }
}

/// What the OS needs to know about the focused text input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImeState {
    /// The caret's box in window coordinates `(x, y, width, height)`, where the
    /// IME puts its candidate window.
    pub(crate) area: (f64, f64, f64, f64),
    /// A password field: the IME should not learn or suggest from it.
    pub(crate) password: bool,
}

/// What `ime_state` last worked out from: shaping a field is the costly part,
/// so it is redone only when something that places the caret changed.
pub(crate) type ImeKey = (engine_core::NodeId, usize, usize, usize, [u64; 5]);

/// The focused text input's `ImeState`, or `None` when no text input has focus.
/// `cache` remembers the last answer and what it was worked out from.
pub(crate) fn ime_state(
    tree: &Rc<RefCell<Tree>>,
    cache: &mut Option<(ImeKey, Option<ImeState>)>,
) -> Option<ImeState> {
    let (field, password, key) = {
        let tree = tree.borrow();
        let Some(field) = tree.focused() else {
            *cache = None;
            return None;
        };
        let Some(NodeKind::TextField(state)) = tree.get(field).map(|n| &n.kind) else {
            *cache = None;
            return None;
        };
        let size = tree.layout(field).size;
        let (x, y) = tree.absolute_position(field);
        let key: ImeKey = (
            field,
            state.cursor,
            state.content.len(),
            state.preedit.as_ref().map_or(0, String::len),
            [
                x.to_bits(),
                y.to_bits(),
                state.horizontal_scroll_offset.current.to_bits()
                    ^ state.scroll_offset.current.to_bits().rotate_left(17),
                u64::from(size.width.to_bits()) << 32 | u64::from(size.height.to_bits()),
                u64::from(state.font_size.to_bits()) << 32
                    | u64::from(state.caret_width.to_bits()) ^ u64::from(state.obscured),
            ],
        );
        (field, state.obscured, key)
    };
    if let Some((last, answer)) = cache
        && *last == key
    {
        return *answer;
    }
    let answer = crate::text_interaction::field_caret_rect(tree, field).map(|rect| {
        let (x, y) = tree.borrow().absolute_position(field);
        ImeState {
            area: (
                x + rect.x0,
                y + rect.y0,
                rect.width().max(1.0),
                rect.height(),
            ),
            password,
        }
    });
    *cache = Some((key, answer));
    answer
}
