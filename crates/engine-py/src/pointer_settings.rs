//! 0.6.12 (P6): the pointer thresholds a window keeps beside the touch recogniser's
//! (`GestureConfig`): how quick and how near presses must be to make a multi-click, and how far a
//! pressed pointer must travel before it is a drag.
//!
//! * `multi_click_ms` (500) and `multi_click_px` (4): presses within that time and distance of the
//!   one before count 2 and 3 in text (a double click selects the word, a triple click the line).
//!   `multi_click_px` is also how far apart a title-bar double click's presses may be.
//! * `drag_start_px` (0, off): while the primary button is held, `pointer_move` listeners hear
//!   nothing until the pointer has moved this far from where it was pressed, and everything after.
//!   A press that never gets that far is a click, whatever the hand's wobble.
//!
//! Defaults are the values these were before they were settable, so an unset window behaves as it
//! always did; the Tesserae UX spec asks for 4 px on `drag_start_px`, which a design system sets.

use std::cell::Cell;
use std::time::Duration;

use engine_core::{InputEvent, PointerButton};
use peniko::kurbo::Point;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(crate) const MULTI_CLICK_MS: f64 = 500.0;
pub(crate) const MULTI_CLICK_PX: f64 = 4.0;
pub(crate) const DRAG_START_PX: f64 = 0.0;

/// A window's pointer thresholds, and the press the drag start distance is measured from.
pub(crate) struct PointerSettings {
    multi_click_ms: Cell<f64>,
    multi_click_px: Cell<f64>,
    drag_start_px: Cell<f64>,
    /// The primary press being held: where it landed, and whether the pointer has since gone
    /// `drag_start_px` away from there.
    press: Cell<Option<(Point, bool)>>,
}

impl Default for PointerSettings {
    fn default() -> Self {
        Self {
            multi_click_ms: Cell::new(MULTI_CLICK_MS),
            multi_click_px: Cell::new(MULTI_CLICK_PX),
            drag_start_px: Cell::new(DRAG_START_PX),
            press: Cell::new(None),
        }
    }
}

impl PointerSettings {
    pub(crate) fn multi_click_ms(&self) -> f64 {
        self.multi_click_ms.get()
    }

    pub(crate) fn set_multi_click_ms(&self, ms: f64) {
        self.multi_click_ms.set(ms);
    }

    pub(crate) fn multi_click_px(&self) -> f64 {
        self.multi_click_px.get()
    }

    pub(crate) fn set_multi_click_px(&self, px: f64) {
        self.multi_click_px.set(px);
    }

    pub(crate) fn drag_start_px(&self) -> f64 {
        self.drag_start_px.get()
    }

    pub(crate) fn set_drag_start_px(&self, px: f64) {
        self.drag_start_px.set(px);
    }

    /// The multi-click time as a `Duration`.
    pub(crate) fn multi_click(&self) -> Duration {
        Duration::from_secs_f64(self.multi_click_ms.get() / 1000.0)
    }

    /// Follows the primary press through `event` and says whether it is a pointer move that
    /// `pointer_move` listeners should not hear yet: the press is held and the pointer has not
    /// gone `drag_start_px` from where it landed. Always `false` while the distance is 0.
    pub(crate) fn holds_move(&self, event: &InputEvent) -> bool {
        match *event {
            InputEvent::PointerPressed {
                position,
                button: PointerButton::Primary,
            } => self.press.set(Some((position, false))),
            InputEvent::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => self.press.set(None),
            InputEvent::PointerMoved { position } => {
                let start = self.drag_start_px.get();
                if let Some((origin, false)) = self.press.get()
                    && start > 0.0
                {
                    if (position - origin).hypot() >= start {
                        self.press.set(Some((origin, true)));
                    } else {
                        return true;
                    }
                }
            }
            _ => {}
        }
        false
    }
}

/// The longest time any of these may be, in milliseconds: an hour (and short of what
/// `Duration` can hold, which a number from Python could otherwise overflow).
const MAX_MS: f64 = 3_600_000.0;

/// One `Window.set` threshold, checked and not yet stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Threshold {
    LongPressMs(f64),
    TapMs(f64),
    DoubleTapMs(f64),
    DoubleTapPx(f64),
    SwipeSpeed(f64),
    SwipePx(f64),
    RotateSlop(f64),
    MultiClickMs(f64),
    MultiClickPx(f64),
    DragStartPx(f64),
}

/// The window properties of this module, for `Window.get`'s error message.
pub(crate) const NAMES: &str = "gesture_long_press_ms, gesture_tap_ms, gesture_double_tap_ms, \
    gesture_double_tap_px, gesture_swipe_speed, gesture_swipe_px, gesture_rotate_slop, \
    multi_click_ms, multi_click_px, drag_start_px";

/// A number in `min..=max`, or a `ValueError` naming the property and what it measures.
fn number(
    name: &str,
    value: &Bound<'_, PyAny>,
    what: &str,
    min: f64,
    max: f64,
    min_open: bool,
) -> PyResult<f64> {
    value
        .extract::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v <= max && if min_open { *v > min } else { *v >= min })
        .ok_or_else(|| {
            let bound = if min_open { "more than" } else { "at least" };
            PyValueError::new_err(format!(
                "window property `{name}` must be {what}, {bound} {min} and at most {max}"
            ))
        })
}

impl Threshold {
    /// `Some` for one of this module's property names, with the value checked.
    pub(crate) fn parse(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Option<Self>> {
        const PX: f64 = 1.0e6;
        let ms = "a number of milliseconds";
        let px = "a number of pixels";
        Ok(Some(match name {
            "gesture_long_press_ms" => {
                Self::LongPressMs(number(name, value, ms, 0.0, MAX_MS, true)?)
            }
            "gesture_tap_ms" => Self::TapMs(number(name, value, ms, 0.0, MAX_MS, true)?),
            "gesture_double_tap_ms" => {
                Self::DoubleTapMs(number(name, value, ms, 0.0, MAX_MS, false)?)
            }
            "gesture_double_tap_px" => Self::DoubleTapPx(number(name, value, px, 0.0, PX, false)?),
            "gesture_swipe_speed" => Self::SwipeSpeed(number(
                name,
                value,
                "a speed in pixels a second",
                0.0,
                PX,
                false,
            )?),
            "gesture_swipe_px" => Self::SwipePx(number(name, value, px, 0.0, PX, false)?),
            "gesture_rotate_slop" => Self::RotateSlop(number(
                name,
                value,
                "an angle in radians",
                0.0,
                std::f64::consts::PI,
                false,
            )?),
            "multi_click_ms" => Self::MultiClickMs(number(name, value, ms, 0.0, MAX_MS, false)?),
            "multi_click_px" => Self::MultiClickPx(number(name, value, px, 0.0, PX, false)?),
            "drag_start_px" => Self::DragStartPx(number(name, value, px, 0.0, PX, false)?),
            _ => return Ok(None),
        }))
    }
}

fn millis(ms: f64) -> Duration {
    Duration::from_secs_f64(ms / 1000.0)
}

impl crate::window::WindowHandles {
    /// 0.6.12 (P6): stores checked thresholds.
    pub(crate) fn apply_thresholds(&self, changes: &[Threshold]) {
        if changes.is_empty() {
            return;
        }
        let mut gestures = self.touch.borrow().gesture_config();
        for change in changes {
            match *change {
                Threshold::LongPressMs(v) => gestures.long_press = millis(v),
                Threshold::TapMs(v) => gestures.tap_max = millis(v),
                Threshold::DoubleTapMs(v) => gestures.double_tap_gap = millis(v),
                Threshold::DoubleTapPx(v) => gestures.double_tap_distance = v,
                Threshold::SwipeSpeed(v) => gestures.swipe_velocity = v,
                Threshold::SwipePx(v) => gestures.swipe_distance = v,
                Threshold::RotateSlop(v) => gestures.rotate_slop = v,
                Threshold::MultiClickMs(v) => self.pointer.set_multi_click_ms(v),
                Threshold::MultiClickPx(v) => self.pointer.set_multi_click_px(v),
                Threshold::DragStartPx(v) => self.pointer.set_drag_start_px(v),
            }
        }
        self.touch.borrow_mut().set_gesture_config(gestures);
    }

    /// 0.6.12 (P6): the current value of one of the thresholds, `None` for any other name.
    pub(crate) fn threshold(&self, name: &str) -> Option<f64> {
        let g = self.touch.borrow().gesture_config();
        Some(match name {
            "gesture_long_press_ms" => g.long_press.as_secs_f64() * 1000.0,
            "gesture_tap_ms" => g.tap_max.as_secs_f64() * 1000.0,
            "gesture_double_tap_ms" => g.double_tap_gap.as_secs_f64() * 1000.0,
            "gesture_double_tap_px" => g.double_tap_distance,
            "gesture_swipe_speed" => g.swipe_velocity,
            "gesture_swipe_px" => g.swipe_distance,
            "gesture_rotate_slop" => g.rotate_slop,
            "multi_click_ms" => self.pointer.multi_click_ms(),
            "multi_click_px" => self.pointer.multi_click_px(),
            "drag_start_px" => self.pointer.drag_start_px(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(x: f64, y: f64) -> InputEvent {
        InputEvent::PointerPressed {
            position: Point::new(x, y),
            button: PointerButton::Primary,
        }
    }

    fn moved(x: f64, y: f64) -> InputEvent {
        InputEvent::PointerMoved {
            position: Point::new(x, y),
        }
    }

    fn release(x: f64, y: f64) -> InputEvent {
        InputEvent::PointerReleased {
            position: Point::new(x, y),
            button: PointerButton::Primary,
        }
    }

    #[test]
    fn the_defaults_are_what_they_were_before_they_were_settable() {
        let s = PointerSettings::default();
        assert_eq!(s.multi_click(), Duration::from_millis(500));
        assert_eq!(s.multi_click_px(), 4.0);
        assert_eq!(s.drag_start_px(), 0.0);
    }

    #[test]
    fn with_no_start_distance_every_move_is_heard() {
        let s = PointerSettings::default();
        assert!(!s.holds_move(&press(10.0, 10.0)));
        assert!(!s.holds_move(&moved(10.5, 10.0)));
    }

    #[test]
    fn a_held_press_is_silent_until_the_pointer_leaves_the_start_distance() {
        let s = PointerSettings::default();
        s.set_drag_start_px(4.0);
        // No press: moves are heard.
        assert!(!s.holds_move(&moved(0.0, 0.0)));
        s.holds_move(&press(10.0, 10.0));
        assert!(s.holds_move(&moved(12.0, 11.0)), "2.2 px: still a click");
        assert!(s.holds_move(&moved(10.0, 13.9)));
        assert!(!s.holds_move(&moved(14.0, 10.0)), "4 px: the drag starts");
        // And once started it stays started, even back at the press.
        assert!(!s.holds_move(&moved(10.0, 10.0)));
        s.holds_move(&release(10.0, 10.0));
        assert!(!s.holds_move(&moved(10.0, 10.0)));
        // A new press starts afresh.
        s.holds_move(&press(50.0, 50.0));
        assert!(s.holds_move(&moved(51.0, 50.0)));
    }

    #[test]
    fn only_the_primary_button_counts() {
        let s = PointerSettings::default();
        s.set_drag_start_px(4.0);
        s.holds_move(&InputEvent::PointerPressed {
            position: Point::new(0.0, 0.0),
            button: PointerButton::Secondary,
        });
        assert!(!s.holds_move(&moved(1.0, 0.0)));
    }
}
