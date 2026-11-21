//! 0.6.11 (P5): when a window's first frame is presented, and what its pixels are cleared to.
//!
//! `Window(first_present=...)` says when the first frame reaches the screen:
//!
//! * `"immediate"` (the default): as soon as it is drawn, as every window did before;
//! * `"on_ready"`: not until the app calls `window.ready()` -- the window is created and laid out
//!   but its first present is held, so the user never sees an unfinished first frame (or, on
//!   macOS and Windows, the OS's default background before the first frame);
//! * a number of milliseconds: `"on_ready"`, but at the latest that long after the window was
//!   created, whether or not `ready()` was called.
//!
//! `background` is the colour every frame is cleared to before the scene paints over it (the
//! scene's own fills win); `None` is the window scheme's `surface` role, so a dark app never
//! shows white. A see-through window (`transparent`) clears to nothing unless `background` is set.
//!
//! How the hold works: the frame closure (`app.rs`) asks [`FirstPresent::hold`] after layout
//! and before anything is drawn. A held frame draws and presents nothing; the loop sleeps until
//! `ready()` wakes it or, for a deadline, the window's alarm does. On Wayland a surface is mapped by
//! the first buffer it commits, so holding the present is all it takes. On X11, macOS and Windows
//! the OS window is also created hidden (`WindowOptions::start_hidden`) and shown right after
//! the first present (`finish`). Those three are unverified here (only Wayland is testable).
//!
//! 0.6.12: `LayerWindow` and `PopupWindow` take the same arguments. Their configure handshake is
//! settled by the layer thread (it acknowledges every configure before the app hears of it), and the
//! role-setting commit it makes carries no buffer, so the surface is not mapped until the first
//! present; a deadline counts from the first configure (when the app's side of the surface is
//! made, `FirstPresent::opened_now`). Settings made before then (`keyboard`, `input_region`,
//! `size`) are sent as usual (`LayerState.update_early`) and take effect when the surface maps.

use std::cell::Cell;
use std::time::{Duration, Instant};

use peniko::Color;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::node_props::parse_color;

/// The longest `first_present` deadline accepted (ten minutes): a typo for seconds is an error,
/// not a window that never shows.
const MAX_DEADLINE_MS: u64 = 600_000;

/// When the first frame is presented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Immediate,
    OnReady,
    /// On `ready()`, or this long after the window was created, whichever comes first.
    After(Duration),
}

impl Mode {
    /// `"immediate"`, `"on_ready"` or a whole number of milliseconds (more than zero).
    pub(crate) fn parse(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let bad = || {
            PyValueError::new_err(format!(
                "first_present must be 'immediate', 'on_ready' or a number of milliseconds \
                 (1 to {MAX_DEADLINE_MS}), got {}",
                value
                    .repr()
                    .map_or_else(|_| "?".to_string(), |r| r.to_string())
            ))
        };
        if let Ok(text) = value.extract::<String>() {
            return match text.as_str() {
                "immediate" => Ok(Self::Immediate),
                "on_ready" => Ok(Self::OnReady),
                _ => Err(bad()),
            };
        }
        // A bool is an int to Python; `first_present=True` is not a deadline.
        if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(bad());
        }
        match value.extract::<u64>() {
            Ok(ms) if (1..=MAX_DEADLINE_MS).contains(&ms) => {
                Ok(Self::After(Duration::from_millis(ms)))
            }
            _ => Err(bad()),
        }
    }

    pub(crate) fn to_py(self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(match self {
            Self::Immediate => "immediate".into_pyobject(py)?.into_any().unbind(),
            Self::OnReady => "on_ready".into_pyobject(py)?.into_any().unbind(),
            Self::After(wait) => (wait.as_millis() as u64)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
        })
    }
}

/// What a frame that would be the window's first present is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hold {
    /// Draw and present.
    No,
    /// Draw nothing; the app is not ready. A deadline is when to look again, if there is one.
    Until(Option<Instant>),
}

/// A window's first-present settings and state, shared with the frame loop.
pub(crate) struct FirstPresent {
    mode: Cell<Mode>,
    ready: Cell<bool>,
    /// Whether the first frame has been presented (always `true` for `Immediate` once it has).
    presented: Cell<bool>,
    /// When the OS window was created, for a deadline.
    opened: Cell<Option<Instant>>,
    /// `Window(background=...)`; `None` is the scheme's `surface`.
    background: Cell<Option<Color>>,
}

impl FirstPresent {
    pub(crate) fn new() -> Self {
        Self {
            mode: Cell::new(Mode::Immediate),
            ready: Cell::new(false),
            presented: Cell::new(false),
            opened: Cell::new(None),
            background: Cell::new(None),
        }
    }

    pub(crate) fn mode(&self) -> Mode {
        self.mode.get()
    }

    pub(crate) fn set_mode(&self, mode: Mode) {
        self.mode.set(mode);
    }

    pub(crate) fn background(&self) -> Option<Color> {
        self.background.get()
    }

    pub(crate) fn set_background(&self, color: Option<Color>) {
        self.background.set(color);
    }

    /// The constructor's `first_present=` and `background=` (both optional, `None` leaves the
    /// default), for `Window`, `LayerWindow` and `PopupWindow` alike.
    pub(crate) fn configure(
        &self,
        first_present: Option<&Bound<'_, PyAny>>,
        background: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        if let Some(mode) = first_present.filter(|v| !v.is_none()) {
            self.set_mode(Mode::parse(mode)?);
        }
        if let Some(color) = background {
            self.set_background(Self::parse_background(color)?);
        }
        Ok(())
    }

    /// `background=`: `None`, or an `(r, g, b, a)` tuple of 0-255 ints.
    pub(crate) fn parse_background(value: &Bound<'_, PyAny>) -> PyResult<Option<Color>> {
        if value.is_none() {
            return Ok(None);
        }
        parse_color(value, "background").map(Some)
    }

    /// Whether the window is created hidden (where the OS can).
    pub(crate) fn starts_hidden(&self) -> bool {
        self.mode.get() != Mode::Immediate
    }

    /// The OS window has just been made: a deadline counts from now, and the first present is
    /// still to come.
    pub(crate) fn opened_now(&self, now: Instant) {
        self.opened.set(Some(now));
        self.presented.set(false);
    }

    /// `window.ready()`; `true` when this changed anything (the first one).
    pub(crate) fn set_ready(&self) -> bool {
        !self.ready.replace(true)
    }

    pub(crate) fn presented(&self) -> bool {
        self.presented.get()
    }

    /// Whether the window is still waiting for its first present under a mode that holds it. Such a
    /// window is not occluded in any sense that matters: it has not been shown.
    pub(crate) fn awaiting(&self) -> bool {
        self.mode.get() != Mode::Immediate && !self.presented.get()
    }

    /// Whether this frame may present, at `now`.
    pub(crate) fn hold(&self, now: Instant) -> Hold {
        if self.presented.get() || self.ready.get() {
            return Hold::No;
        }
        match self.mode.get() {
            Mode::Immediate => Hold::No,
            Mode::OnReady => Hold::Until(None),
            Mode::After(wait) => {
                let opened = self.opened.get().unwrap_or_else(|| {
                    self.opened.set(Some(now));
                    now
                });
                let deadline = opened + wait;
                if now >= deadline {
                    Hold::No
                } else {
                    Hold::Until(Some(deadline))
                }
            }
        }
    }

    /// The first frame has been presented. `true` the first time, when a hidden window is to be
    /// shown.
    pub(crate) fn finish(&self) -> bool {
        !self.presented.replace(true)
    }

    /// The colour a frame is cleared to: the window's `background`, else the scheme's `surface`
    /// for a window that shows nothing behind it (a see-through one and a layer surface clear to
    /// nothing, so what is behind them shows).
    pub(crate) fn backdrop(&self, see_through: bool, surface: Color) -> Color {
        match self.background.get() {
            Some(color) => color,
            None if see_through => Color::TRANSPARENT,
            None => surface,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_never_holds() {
        let fp = FirstPresent::new();
        assert_eq!(fp.hold(Instant::now()), Hold::No);
        assert!(!fp.awaiting());
    }

    #[test]
    fn on_ready_holds_until_ready() {
        let fp = FirstPresent::new();
        fp.set_mode(Mode::OnReady);
        let now = Instant::now();
        assert_eq!(fp.hold(now), Hold::Until(None));
        assert!(fp.awaiting());
        assert!(fp.set_ready());
        assert!(!fp.set_ready());
        assert_eq!(fp.hold(now), Hold::No);
    }

    #[test]
    fn a_deadline_opens_the_hold_by_itself() {
        let fp = FirstPresent::new();
        fp.set_mode(Mode::After(Duration::from_millis(50)));
        let start = Instant::now();
        fp.opened_now(start);
        assert_eq!(
            fp.hold(start + Duration::from_millis(10)),
            Hold::Until(Some(start + Duration::from_millis(50)))
        );
        assert_eq!(fp.hold(start + Duration::from_millis(50)), Hold::No);
    }

    #[test]
    fn once_presented_nothing_holds() {
        let fp = FirstPresent::new();
        fp.set_mode(Mode::OnReady);
        assert!(fp.finish());
        assert!(!fp.finish());
        assert_eq!(fp.hold(Instant::now()), Hold::No);
        assert!(!fp.awaiting());
        // A window opened again holds again.
        fp.opened_now(Instant::now());
        assert_eq!(fp.hold(Instant::now()), Hold::Until(None));
    }

    #[test]
    fn the_backdrop_is_the_surface_unless_see_through_or_set() {
        let fp = FirstPresent::new();
        let surface = Color::from_rgba8(1, 2, 3, 255);
        assert_eq!(fp.backdrop(false, surface), surface);
        assert_eq!(fp.backdrop(true, surface), Color::TRANSPARENT);
        let own = Color::from_rgba8(9, 9, 9, 255);
        fp.set_background(Some(own));
        assert_eq!(fp.backdrop(true, surface), own);
        assert_eq!(fp.backdrop(false, surface), own);
    }
}
