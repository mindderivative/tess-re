//! 0.6.11: the named easings of the engine, in one place: a name from this
//! table or a cubic bezier `(x1, y1, x2, y2)` is how anything that takes an
//! easing is given one: `Node.animate` and the other animation calls
//! (`engine-py`'s `parse_easing`), the scrollbar's fade, the smooth scroll and
//! the scroll snap (`scrollbar.rs`). The table is presets of
//! `MotionCurve::Linear` and `MotionCurve::Bezier`; nothing here is a callback.

use crate::animation::MotionCurve;

/// An easing a property names: a preset or a cubic bezier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    /// A CSS `cubic-bezier(x1, y1, x2, y2)`; `x1` and `x2` lie in `0..=1`.
    Bezier(f64, f64, f64, f64),
}

/// The Tesserae Motion `easing-standard`.
pub const STANDARD: Easing = Easing::Bezier(0.2, 0.0, 0.0, 1.0);

/// The presets, by the name a property takes. The first seven are the Tesserae
/// Motion table (`Tesserae UX/20 Foundations and Tokens/Motion.md`, "Easing"),
/// with `emphasized` in the single-segment form the note resolves it to
/// (identical to `standard`); the rest are the CSS keywords.
pub const PRESETS: [(&str, Easing); 11] = [
    ("linear", Easing::Linear),
    ("standard", STANDARD),
    ("standard_decelerate", Easing::Bezier(0.0, 0.0, 0.0, 1.0)),
    ("standard_accelerate", Easing::Bezier(0.3, 0.0, 1.0, 1.0)),
    ("emphasized", Easing::Bezier(0.2, 0.0, 0.0, 1.0)),
    ("emphasized_decelerate", Easing::Bezier(0.05, 0.7, 0.1, 1.0)),
    ("emphasized_accelerate", Easing::Bezier(0.3, 0.0, 0.8, 0.15)),
    ("ease", Easing::Bezier(0.25, 0.1, 0.25, 1.0)),
    ("ease_in", Easing::Bezier(0.42, 0.0, 1.0, 1.0)),
    ("ease_out", Easing::Bezier(0.0, 0.0, 0.58, 1.0)),
    ("ease_in_out", Easing::Bezier(0.42, 0.0, 0.58, 1.0)),
];

impl Easing {
    /// The preset names, comma separated, for an error message.
    pub fn names() -> String {
        PRESETS
            .iter()
            .map(|(n, _)| *n)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The preset called `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        PRESETS.iter().find(|(n, _)| *n == name).map(|(_, e)| *e)
    }

    /// A bezier with these control points, if they are a valid curve: finite,
    /// with `x1` and `x2` in `0..=1` (the precondition of CSS and of
    /// `MotionCurve::Bezier`).
    pub fn bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> Option<Self> {
        let valid = [x1, y1, x2, y2].iter().all(|v| v.is_finite())
            && (0.0..=1.0).contains(&x1)
            && (0.0..=1.0).contains(&x2);
        valid.then_some(Self::Bezier(x1, y1, x2, y2))
    }

    /// The preset name of this easing, if it is one.
    pub fn name(self) -> Option<&'static str> {
        PRESETS.iter().find(|(_, e)| *e == self).map(|(n, _)| *n)
    }

    /// The animation curve.
    pub fn curve(self) -> MotionCurve {
        match self {
            Self::Linear => MotionCurve::Linear,
            Self::Bezier(a, b, c, d) => MotionCurve::Bezier(a, b, c, d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_round_trip_by_name_and_standard_is_the_motion_curve() {
        for (name, easing) in PRESETS {
            assert_eq!(Easing::from_name(name), Some(easing));
            // `emphasized` has the values of `standard` (the Motion note), so
            // it reads back under the first name of its curve.
            let back = easing.name().expect("a preset has a name");
            assert_eq!(Easing::from_name(back), Some(easing));
            if name != "emphasized" {
                assert_eq!(back, name);
            }
        }
        assert_eq!(
            Easing::from_name("standard").map(Easing::curve),
            Some(MotionCurve::Bezier(0.2, 0.0, 0.0, 1.0))
        );
        assert_eq!(Easing::from_name("nope"), None);
    }

    #[test]
    fn a_bezier_needs_x_in_the_unit_range() {
        assert_eq!(
            Easing::bezier(0.2, 0.0, 0.0, 1.0),
            Some(Easing::Bezier(0.2, 0.0, 0.0, 1.0))
        );
        assert_eq!(Easing::bezier(1.2, 0.0, 0.0, 1.0), None);
        assert_eq!(Easing::bezier(0.2, 0.0, -0.1, 1.0), None);
        assert_eq!(Easing::bezier(f64::NAN, 0.0, 0.0, 1.0), None);
        assert_eq!(
            Easing::bezier(0.2, -3.0, 0.0, 4.0).and_then(Easing::name),
            None
        );
    }

    #[test]
    fn the_presets_are_the_motion_table_at_a_quarter_half_and_three_quarters() {
        // The y column of Motion.md, "Easing" (AC-MOT-3: within 0.005).
        let table = [
            ("linear", [0.250, 0.500, 0.750]),
            ("standard", [0.607, 0.878, 0.975]),
            ("standard_decelerate", [0.691, 0.890, 0.976]),
            ("standard_accelerate", [0.129, 0.372, 0.668]),
            ("emphasized", [0.607, 0.878, 0.975]),
            ("emphasized_decelerate", [0.832, 0.950, 0.991]),
            ("emphasized_accelerate", [0.035, 0.154, 0.406]),
        ];
        for (name, ys) in table {
            let curve = Easing::from_name(name).expect(name).curve();
            for (x, want) in [0.25, 0.5, 0.75].into_iter().zip(ys) {
                let got = curve.ease(x);
                assert!((got - want).abs() < 0.005, "{name} at {x}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn the_names_list_every_preset() {
        let names = Easing::names();
        for (name, _) in PRESETS {
            assert!(names.contains(name), "{name} in {names}");
        }
    }
}
