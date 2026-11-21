//! 0.6.11 (P13): state layers -- the hover, pressed and focus overlays a toolkit declares once on
//! a node (`hover_layer`, `pressed_layer`, `focus_layer`, ...) and the engine draws and fades.
//!
//! The engine already knows which node is hovered, pressed and focused; a layer is only what it
//! paints there: a colour (its alpha is the layer's opacity) over the node's own fill and border,
//! under its children, inside its shape. One layer shows at a time (the UX spec's rule,
//! ADR-0009): pressed, else focus, else hover. The shown colour fades between targets over
//! `state_layer_ms` on `state_layer_easing` (each state may override both: `hover_layer_ms`,
//! `pressed_layer_ms`, `focus_layer_ms` and the three `_easing`s), and is simply there under
//! reduced motion. Nothing is a callback: every field is a colour, a flag, a number or a named easing, and a node without a
//! layer carries no `StateLayer` at all.

use std::time::{Duration, Instant};

use peniko::Color;

use crate::animation::{Animated, CompletionHandle};
use crate::easing::{Easing, STANDARD};
use crate::scrollbar::millis;

/// The fade, in milliseconds: the UX spec's Motion `short2`.
pub const DEFAULT_MS: f64 = 100.0;

/// One of the three layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerState {
    Hover,
    Pressed,
    Focus,
}

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
    /// How long a change of layer takes. `0` is at once. The shared value: a state's own
    /// `*_ms` replaces it for the fades that state decides (see [`StateLayer::timing`]).
    pub ms: f64,
    /// The curve of that change (the shared one, as with `ms`).
    pub easing: Easing,
    /// `hover_layer_ms`: `None` falls back to `ms`.
    pub hover_ms: Option<f64>,
    /// `pressed_layer_ms`: `None` falls back to `ms`.
    pub pressed_ms: Option<f64>,
    /// `focus_layer_ms`: `None` falls back to `ms`.
    pub focus_ms: Option<f64>,
    /// `hover_layer_easing`: `None` falls back to `easing`.
    pub hover_easing: Option<Easing>,
    /// `pressed_layer_easing`: `None` falls back to `easing`.
    pub pressed_easing: Option<Easing>,
    /// `focus_layer_easing`: `None` falls back to `easing`.
    pub focus_easing: Option<Easing>,
    /// The state whose colour `shown` is fading to, or from when it is fading out: the state
    /// last entered. Decides the timing of a fade out to nothing.
    pub active: Option<LayerState>,
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
            hover_ms: None,
            pressed_ms: None,
            focus_ms: None,
            hover_easing: None,
            pressed_easing: None,
            focus_easing: None,
            active: None,
            shown: Animated::new(Color::TRANSPARENT),
        }
    }
}

impl StateLayer {
    /// The layer that applies when `holds`, by the precedence pressed, focus, hover: the first
    /// whose state holds and that has a colour.
    pub fn target(&self, holds: StateHolds) -> Option<(LayerState, Color)> {
        let pick = |on: bool, state: LayerState, layer: Option<Color>| {
            layer.filter(|_| on).map(|color| (state, color))
        };
        pick(holds.pressed, LayerState::Pressed, self.pressed)
            .or(pick(holds.focused, LayerState::Focus, self.focus))
            .or(pick(holds.hovered, LayerState::Hover, self.hover))
    }

    /// The duration (ms) and easing of `state`'s fades: its own override, else the shared pair.
    pub fn timing(&self, state: LayerState) -> (f64, Easing) {
        let (ms, easing) = match state {
            LayerState::Hover => (self.hover_ms, self.hover_easing),
            LayerState::Pressed => (self.pressed_ms, self.pressed_easing),
            LayerState::Focus => (self.focus_ms, self.focus_easing),
        };
        (ms.unwrap_or(self.ms), easing.unwrap_or(self.easing))
    }

    /// Starts the fade to `target` (`None`: nothing). Which timing applies: the state being
    /// entered decides a fade in (and a change from one layer to another); the state being left
    /// decides a fade out to nothing. Instant when `reduced` or the duration is `0`. Returns
    /// whether the shown colour changes (or is on its way to a new one).
    pub fn sync(
        &mut self,
        target: Option<(LayerState, Color)>,
        now: Instant,
        reduced: bool,
    ) -> bool {
        let heading = *self.shown.target();
        let want = match target {
            Some((_, color)) => color,
            None => with_alpha(heading, 0.0),
        };
        let deciding = match target {
            Some((state, _)) => Some(state),
            None => self.active,
        };
        if let Some((state, _)) = target {
            self.active = Some(state);
        }
        if want == heading {
            return false;
        }
        let (ms, easing) = deciding.map_or((self.ms, self.easing), |s| self.timing(s));
        let duration = if reduced { Duration::ZERO } else { millis(ms) };
        if duration.is_zero() {
            self.shown.set_now(want);
            return true;
        }
        // Appearing from nothing starts from the new colour at no opacity: fading in is only
        // alpha, never a blend from whatever rgb the last fade ended on.
        if self.shown.current.components[3] <= 0.0 {
            self.shown.set_now(with_alpha(want, 0.0));
        }
        self.shown.animate_to(want, duration, easing.curve(), now);
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

    const EMPH: crate::Easing = crate::Easing::Bezier(0.3, 0.0, 0.8, 0.15);

    fn hover() -> Option<(LayerState, Color)> {
        Some((LayerState::Hover, grey(20)))
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
        assert_eq!(l.target(all), Some((LayerState::Pressed, grey(26))));
        assert_eq!(
            l.target(StateHolds {
                pressed: false,
                ..all
            }),
            Some((LayerState::Focus, grey(30)))
        );
        assert_eq!(
            l.target(StateHolds {
                hovered: true,
                ..Default::default()
            }),
            Some((LayerState::Hover, grey(20)))
        );
        assert_eq!(l.target(StateHolds::default()), None);
        // A state with no colour falls through to the next that has one.
        l.pressed = None;
        assert_eq!(l.target(all), Some((LayerState::Focus, grey(30))));
    }

    #[test]
    fn fades_in_over_ms_and_out_again_keeping_the_hue() {
        let t0 = Instant::now();
        let mut l = layer();
        let mut done = Vec::new();
        assert!(l.sync(hover(), t0, false));
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
        assert!(l.sync(hover(), t0, true));
        assert_eq!(l.visible(), Some(grey(20)));
        assert!(l.shown.active.is_none());
        let mut l = layer();
        l.ms = 0.0;
        assert!(l.sync(hover(), t0, false));
        assert_eq!(l.visible(), Some(grey(20)));
    }

    #[test]
    fn syncing_to_what_it_heads_for_does_nothing() {
        let t0 = Instant::now();
        let mut l = layer();
        assert!(l.sync(hover(), t0, false));
        assert!(!l.sync(hover(), t0, false));
        assert!(!layer().sync(None, t0, false));
    }

    /// Milliseconds of the fade a `sync` started: the first tick after which it has ended.
    fn fade_len(l: &mut StateLayer, target: Option<(LayerState, Color)>, t0: Instant) -> u64 {
        let mut done = Vec::new();
        assert!(l.sync(target, t0, false));
        (1..=1000)
            .find(|ms| !l.tick(t0 + Duration::from_millis(*ms), &mut done))
            .expect("the fade ends")
    }

    fn settled(l: &mut StateLayer, state: LayerState, t0: Instant) {
        l.sync(Some((state, grey(20))), t0, true);
    }

    #[test]
    fn each_state_decides_the_fade_in_and_the_fade_out_of_its_own_layer() {
        let t0 = Instant::now();
        for (state, own) in [
            (LayerState::Hover, 15.0),
            (LayerState::Pressed, 105.0),
            (LayerState::Focus, 150.0),
        ] {
            let mut l = layer();
            l.hover_ms = Some(15.0);
            l.pressed_ms = Some(105.0);
            l.focus_ms = Some(150.0);
            let target = Some((state, grey(20)));
            let len = fade_len(&mut l, target, t0);
            assert!((len as f64 - own).abs() <= 1.0, "{state:?} in: {len}");
            // Leaving to nothing takes the left state's timing.
            let len = fade_len(&mut l, None, t0 + Duration::from_secs(2)) as f64;
            assert!((len - own).abs() <= 1.0, "{state:?} out: {len}");
        }
    }

    #[test]
    fn a_change_between_layers_takes_the_entered_states_timing() {
        let t0 = Instant::now();
        let mut l = layer();
        l.hover_ms = Some(15.0);
        l.pressed_ms = Some(300.0);
        settled(&mut l, LayerState::Hover, t0);
        let len = fade_len(&mut l, Some((LayerState::Pressed, grey(26))), t0);
        assert!((299..=301).contains(&len), "hover to pressed: {len}");
        let len = fade_len(
            &mut l,
            Some((LayerState::Hover, grey(20))),
            t0 + Duration::from_secs(2),
        );
        assert!((14..=16).contains(&len), "pressed to hover: {len}");
    }

    #[test]
    fn a_state_without_an_override_uses_the_shared_ms_and_easing() {
        let mut l = layer();
        l.ms = 80.0;
        l.easing = crate::Easing::Linear;
        l.pressed_ms = Some(10.0);
        l.pressed_easing = Some(EMPH);
        assert_eq!(l.timing(LayerState::Hover), (80.0, crate::Easing::Linear));
        assert_eq!(l.timing(LayerState::Focus), (80.0, crate::Easing::Linear));
        assert_eq!(l.timing(LayerState::Pressed), (10.0, EMPH));
        let t0 = Instant::now();
        let len = fade_len(&mut l, hover(), t0);
        assert!((79..=81).contains(&len), "{len}");
    }

    #[test]
    fn a_zero_state_ms_is_instant_and_reduced_motion_overrides_every_state() {
        let t0 = Instant::now();
        let mut l = layer();
        l.hover_ms = Some(0.0);
        assert!(l.sync(hover(), t0, false));
        assert_eq!(l.visible(), Some(grey(20)), "hover_ms 0 is at once");
        let mut l = layer();
        l.pressed_ms = Some(500.0);
        assert!(l.sync(Some((LayerState::Pressed, grey(26))), t0, true));
        assert_eq!(l.visible(), Some(grey(26)));
        assert!(l.sync(None, t0, true));
        assert_eq!(l.visible(), None);
        assert!(l.shown.active.is_none());
    }
}
