//! 0.6.11 (P13): state layers -- the hover, pressed and focus overlays a toolkit declares once on
//! a node (`hover_layer`, `pressed_layer`, `focus_layer`, ...) and the engine draws and fades.
//!
//! The engine already knows which node is hovered, pressed and focused; a layer is only what it
//! paints there: a colour (its alpha is the layer's opacity) over the node's own fill and border,
//! under its children, inside its shape. One layer shows at a time (the UX spec's rule,
//! ADR-0009): pressed, else focus, else hover. The shown colour fades between targets over
//! `state_layer_ms` on `state_layer_easing`, and is simply there under reduced motion. Nothing is
//! a callback: every field is a colour, a flag, a number or a named easing, and a node without a
//! layer carries no `StateLayer` at all.

use std::time::{Duration, Instant};

use peniko::Color;

use crate::animation::{Animated, CompletionHandle};
use crate::easing::{Easing, STANDARD};
use crate::scrollbar::millis;

/// The fade, in milliseconds: the UX spec's Motion `short2`.
pub const DEFAULT_MS: f64 = 100.0;

/// Which of the three states hold for a node right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateHolds {
    pub hovered: bool,
    pub pressed: bool,
    /// Focus that shows a layer: the node has focus and either the focus is visible (keyboard or
    /// assistive technology) or the node asked for the layer on pointer focus too.
    pub focused: bool,
}

/// A node's state layers and the fade between them.
pub struct StateLayer {
    /// Drawn while the pointer is over the node or a descendant.
    pub hover: Option<Color>,
    /// Drawn while a primary press that started on the node or a descendant is held.
    pub pressed: Option<Color>,
    /// Drawn while the node has keyboard focus (see `focus_any_modality`).
    pub focus: Option<Color>,
    /// Also draw `focus` when focus came from a pointer press (a text input).
    pub focus_any_modality: bool,
    /// How long a change of layer takes. `0` is at once.
    pub ms: f64,
    /// The curve of that change.
    pub easing: Easing,
    /// What is drawn now: the colour of the layer in force, fading in or out. Alpha `0` is
    /// nothing; it keeps the rgb of the layer it fades from or to, so a fade does not pass
    /// through another hue.
    pub shown: Animated<Color>,
}

impl Default for StateLayer {
    fn default() -> Self {
        Self {
            hover: None,
            pressed: None,
            focus: None,
            focus_any_modality: false,
            ms: DEFAULT_MS,
            easing: STANDARD,
            shown: Animated::new(Color::TRANSPARENT),
        }
    }
}

impl StateLayer {
    /// The layer that applies when `holds`, by the precedence pressed, focus, hover: the first
    /// whose state holds and that has a colour.
    pub fn target(&self, holds: StateHolds) -> Option<Color> {
        let pick = |on: bool, layer: Option<Color>| if on { layer } else { None };
        pick(holds.pressed, self.pressed)
            .or(pick(holds.focused, self.focus))
            .or(pick(holds.hovered, self.hover))
    }

    /// Starts the fade to `target` (`None`: nothing). Instant when `reduced` or `ms` is `0`.
    /// Returns whether the shown colour changes (or is on its way to a new one).
    pub fn sync(&mut self, target: Option<Color>, now: Instant, reduced: bool) -> bool {
        let heading = *self.shown.target();
        let want = match target {
            Some(color) => color,
            None => with_alpha(heading, 0.0),
        };
        if want == heading {
            return false;
        }
        let duration = if reduced {
            Duration::ZERO
        } else {
            millis(self.ms)
        };
        if duration.is_zero() {
            self.shown.set_now(want);
            return true;
        }
        // Appearing from nothing starts from the new colour at no opacity: fading in is only
        // alpha, never a blend from whatever rgb the last fade ended on.
        if self.shown.current.components[3] <= 0.0 {
            self.shown.set_now(with_alpha(want, 0.0));
        }
        self.shown
            .animate_to(want, duration, self.easing.curve(), now);
        true
    }

    /// Advances the fade; whether it is still running.
    pub fn tick(&mut self, now: Instant, completed: &mut Vec<CompletionHandle>) -> bool {
        self.shown.tick(now, completed)
    }

    /// The colour to draw now, if any.
    pub fn visible(&self) -> Option<Color> {
        let color = self.shown.current;
        (color.components[3] > 0.0).then_some(color)
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    let [r, g, b, _] = color.components;
    Color::new([r, g, b, alpha])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(alpha: u8) -> Color {
        Color::from_rgba8(0, 0, 0, alpha)
    }

    fn layer() -> StateLayer {
        StateLayer {
            hover: Some(grey(20)),
            pressed: Some(grey(26)),
            focus: Some(grey(26)),
            ..Default::default()
        }
    }

    #[test]
    fn precedence_is_pressed_then_focus_then_hover() {
        let mut l = layer();
        l.focus = Some(grey(30));
        let all = StateHolds {
            hovered: true,
            pressed: true,
            focused: true,
        };
        assert_eq!(l.target(all), Some(grey(26)));
        assert_eq!(
            l.target(StateHolds {
                pressed: false,
                ..all
            }),
            Some(grey(30))
        );
        assert_eq!(
            l.target(StateHolds {
                hovered: true,
                ..Default::default()
            }),
            Some(grey(20))
        );
        assert_eq!(l.target(StateHolds::default()), None);
        // A state with no colour falls through to the next that has one.
        l.pressed = None;
        assert_eq!(l.target(all), Some(grey(30)));
    }

    #[test]
    fn fades_in_over_ms_and_out_again_keeping_the_hue() {
        let t0 = Instant::now();
        let mut l = layer();
        let mut done = Vec::new();
        assert!(l.sync(Some(grey(20)), t0, false));
        assert!(l.tick(t0, &mut done));
        assert!(l.shown.current.components[3] < 1e-6, "starts from nothing");
        assert!(l.tick(t0 + Duration::from_millis(50), &mut done));
        let mid = l.visible().expect("part way").components[3];
        assert!(mid > 0.0 && mid < grey(20).components[3], "{mid}");
        assert!(!l.tick(t0 + Duration::from_millis(100), &mut done));
        assert_eq!(l.visible(), Some(grey(20)));
        // Leaving fades back to alpha 0 on the same rgb.
        assert!(l.sync(None, t0 + Duration::from_millis(200), false));
        assert!(l.tick(t0 + Duration::from_millis(250), &mut done));
        let [r, g, b, a] = l.shown.current.components;
        assert_eq!([r, g, b], [0.0, 0.0, 0.0]);
        assert!(a > 0.0 && a < grey(20).components[3]);
        assert!(!l.tick(t0 + Duration::from_millis(300), &mut done));
        assert_eq!(l.visible(), None);
    }

    #[test]
    fn reduced_motion_and_zero_ms_are_instant() {
        let t0 = Instant::now();
        let mut l = layer();
        assert!(l.sync(Some(grey(20)), t0, true));
        assert_eq!(l.visible(), Some(grey(20)));
        assert!(l.shown.active.is_none());
        let mut l = layer();
        l.ms = 0.0;
        assert!(l.sync(Some(grey(20)), t0, false));
        assert_eq!(l.visible(), Some(grey(20)));
    }

    #[test]
    fn syncing_to_what_it_heads_for_does_nothing() {
        let t0 = Instant::now();
        let mut l = layer();
        assert!(l.sync(Some(grey(20)), t0, false));
        assert!(!l.sync(Some(grey(20)), t0, false));
        assert!(!layer().sync(None, t0, false));
    }
}
