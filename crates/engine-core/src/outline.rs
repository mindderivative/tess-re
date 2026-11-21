//! 0.6.11 (UXREQ-008): the outline, a ring drawn outside a node's border box.
//!
//! `stroke_color` and `stroke_width` draw inside the box, and a shadow with a spread
//! cannot leave a transparent gap, so neither makes the focus ring of the UX
//! specification (3 dp wide, 2 dp outside the control). The outline does: a
//! width, an offset from the border box and a colour, with an optional fill
//! for the gap. A node has two styles, [`Outlines::always`] and
//! [`Outlines::focus`]; the second replaces the first while the node has
//! keyboard focus that is visible (`Tree::outline_of`).
//!
//! Nothing here moves layout or hit testing, and nothing is a callback: every
//! number and colour is a node property.

use peniko::Color;

use crate::{SchemeColors, SchemeRole};

/// The role an outline with no colour of its own takes from its window's scheme:
/// the focus ring of the UX specification (`Focus Ring`, "Colour: role `secondary`").
pub const OUTLINE_ROLE: SchemeRole = SchemeRole::Secondary;

/// One outline, as its properties say it. A `width` of `0` (the default) is none.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OutlineStyle {
    /// The ring's thickness in the node's own pixels.
    pub width: f64,
    /// The gap between the border box and the ring's inner edge. Negative draws the ring inside.
    pub offset: f64,
    /// `None`: the window scheme's [`OUTLINE_ROLE`].
    pub color: Option<Color>,
    /// Fills the gap between the box and the ring when set; `None` leaves it transparent.
    pub gap_color: Option<Color>,
    /// The corner radius of the ring's inner edge; `None` follows the node's own corner
    /// radius plus `offset`, so the ring stays concentric with the box.
    pub radius: Option<f64>,
}

/// A node's two outline styles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outlines {
    /// Always shown (`outline_*`).
    pub always: OutlineStyle,
    /// Shown instead of `always` while the node has visible keyboard focus (`focus_outline_*`).
    pub focus: OutlineStyle,
}

impl Outlines {
    /// Whether neither style draws anything.
    pub fn is_none(&self) -> bool {
        self.always.width <= 0.0 && self.focus.width <= 0.0
    }

    /// How far past the border box the ring can reach, in either state: what a node's
    /// painted extent grows by. `0` when it stays inside the box.
    pub fn reach(&self) -> f64 {
        [self.always, self.focus]
            .iter()
            .filter(|s| s.width > 0.0 && s.width.is_finite())
            .map(|s| s.offset + s.width)
            .fold(0.0, f64::max)
    }

    /// The outline to draw now. `focus_shown` is whether the node has keyboard focus that is
    /// visible. The focus style, when it has a width, replaces the plain one whole.
    pub fn resolve(&self, focus_shown: bool, scheme: &SchemeColors) -> Option<Outline> {
        let style = if focus_shown && self.focus.width > 0.0 {
            &self.focus
        } else {
            &self.always
        };
        if !(style.width > 0.0 && style.width.is_finite() && style.offset.is_finite()) {
            return None;
        }
        Some(Outline {
            width: style.width,
            offset: style.offset,
            color: style.color.unwrap_or_else(|| scheme.get(OUTLINE_ROLE)),
            gap_color: style.gap_color,
            radius: style.radius,
        })
    }
}

/// An outline ready to draw: the style in force, its colour resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub width: f64,
    pub offset: f64,
    pub color: Color,
    pub gap_color: Option<Color>,
    pub radius: Option<f64>,
}

impl Outline {
    /// The radii `[top_left, top_right, bottom_right, bottom_left]` of the ring's inner edge
    /// for a box whose corners have `corners`: the box's own radius plus the offset (never
    /// below `0`), or the `radius` property for all four.
    pub fn inner_radii(&self, corners: [f64; 4]) -> [f64; 4] {
        match self.radius {
            Some(radius) => [radius.max(0.0); 4],
            None => corners.map(|r| (r + self.offset).max(0.0)),
        }
    }

    /// The radii of the ring's outer edge: the inner ones grown by `width`, so the two edges
    /// stay concentric. A ring whose inner edge has square corners stays square outside too.
    pub fn outer_radii(&self, corners: [f64; 4]) -> [f64; 4] {
        let inner = self.inner_radii(corners);
        std::array::from_fn(|i| {
            if inner[i] > 0.0 {
                inner[i] + self.width
            } else if self.radius.is_none() && corners[i] > 0.0 {
                // An inward ring past a rounded corner's own radius.
                (corners[i] + self.offset + self.width).max(0.0)
            } else {
                0.0
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scheme;

    fn scheme() -> SchemeColors {
        SchemeColors::baseline(Scheme::Light)
    }

    #[test]
    fn nothing_set_draws_nothing() {
        let none = Outlines::default();
        assert!(none.is_none());
        assert_eq!(none.reach(), 0.0);
        assert!(none.resolve(true, &scheme()).is_none());
    }

    #[test]
    fn default_colour_is_the_scheme_role() {
        let outlines = Outlines {
            always: OutlineStyle {
                width: 3.0,
                offset: 2.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let outline = outlines.resolve(false, &scheme()).unwrap();
        assert_eq!(outline.color, scheme().get(SchemeRole::Secondary));
    }

    #[test]
    fn reach_is_width_plus_offset_of_the_larger_style() {
        let outlines = Outlines {
            always: OutlineStyle {
                width: 1.0,
                offset: 0.0,
                ..Default::default()
            },
            focus: OutlineStyle {
                width: 3.0,
                offset: 2.0,
                ..Default::default()
            },
        };
        assert_eq!(outlines.reach(), 5.0);
        // An inward ring stays inside the box: nothing past it.
        let inward = Outlines {
            always: OutlineStyle {
                width: 3.0,
                offset: -3.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(inward.reach(), 0.0);
    }

    #[test]
    fn focus_style_replaces_the_plain_one_only_while_shown() {
        let outlines = Outlines {
            always: OutlineStyle {
                width: 1.0,
                ..Default::default()
            },
            focus: OutlineStyle {
                width: 3.0,
                offset: 2.0,
                ..Default::default()
            },
        };
        assert_eq!(outlines.resolve(false, &scheme()).unwrap().width, 1.0);
        let shown = outlines.resolve(true, &scheme()).unwrap();
        assert_eq!((shown.width, shown.offset), (3.0, 2.0));
        // A focus style with no width leaves the plain one in force.
        let plain_only = Outlines {
            focus: OutlineStyle::default(),
            ..outlines
        };
        assert_eq!(plain_only.resolve(true, &scheme()).unwrap().width, 1.0);
    }

    #[test]
    fn radius_follows_the_box_plus_the_offset() {
        let outline = Outlines {
            always: OutlineStyle {
                width: 3.0,
                offset: 2.0,
                ..Default::default()
            },
            ..Default::default()
        }
        .resolve(false, &scheme())
        .unwrap();
        assert_eq!(
            outline.inner_radii([8.0, 0.0, 4.0, 8.0]),
            [10.0, 2.0, 6.0, 10.0]
        );
        let inward = Outline {
            offset: -3.0,
            ..outline
        };
        assert_eq!(inward.inner_radii([2.0; 4]), [0.0; 4]);
        assert_eq!(
            outline.outer_radii([8.0, 0.0, 4.0, 8.0]),
            [13.0, 5.0, 9.0, 13.0]
        );
        assert_eq!(inward.outer_radii([2.0; 4]), [2.0; 4]);
        let fixed = Outline {
            radius: Some(5.0),
            ..outline
        };
        assert_eq!(fixed.inner_radii([8.0; 4]), [5.0; 4]);
    }
}
