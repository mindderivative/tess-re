//! 0.6.11: what a scroll view or virtual list can be told about its scrollbar
//! and its scroll feel -- one table, `Prop`, that the node properties, the
//! resolved measures (`scrollbar.rs`), the Python side and the docs all read,
//! so a property cannot exist in one and not another.
//!
//! Every property is a number, a flag, a colour, a named choice or an easing,
//! never a callback, and `None` (unset) is the default: the Tesserae UX note
//! "Scrollbar and Scroll Container" (see `Defaults`).

use peniko::Color;

use crate::easing::Easing;
use crate::scrollbar::ScrollAxis;

/// The most a time property takes: an hour, in milliseconds.
pub const MAX_MILLIS: f64 = 3_600_000.0;

/// The most a speed property takes: a million pixels a second.
pub const MAX_SPEED: f64 = 1_000_000.0;

/// How a property's value is checked, and what its names are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A finite number from `min` to `max`.
    Number { min: f64, max: f64 },
    /// A colour.
    Colour,
    /// `true` or `false`.
    Flag,
    /// One of these names, by index.
    Choice(&'static [&'static str]),
    /// A preset name or a cubic bezier (`Easing`).
    Easing,
    /// Two finite numbers, each from `min` to `max`.
    Pair { min: f64, max: f64 },
}

/// A property's value: what a `Kind` checks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Colour(Color),
    Flag(bool),
    /// The index into the property's `Kind::Choice` names.
    Choice(u8),
    Easing(Easing),
    Pair(f64, f64),
}

impl Kind {
    /// Whether `value` is one this kind takes.
    pub fn accepts(self, value: &Value) -> bool {
        match (self, value) {
            (Kind::Number { min, max }, Value::Number(n)) => {
                n.is_finite() && (min..=max).contains(n)
            }
            (Kind::Colour, Value::Colour(_)) | (Kind::Flag, Value::Flag(_)) => true,
            (Kind::Choice(names), Value::Choice(i)) => usize::from(*i) < names.len(),
            (Kind::Easing, Value::Easing(_)) => true,
            (Kind::Pair { min, max }, Value::Pair(a, b)) => [a, b]
                .iter()
                .all(|n| n.is_finite() && (min..=max).contains(*n)),
            _ => false,
        }
    }
}

/// Where the bar of an axis sits, as a user names it. `Start` and `End` follow
/// the layout direction (`LAYOUT_DIRECTION`); the others are physical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarSide {
    End,
    Start,
    Left,
    Right,
    Top,
    Bottom,
}

/// The edge of the view a bar sits on: the left or top (`Near`) or the right or
/// bottom (`Far`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Near,
    Far,
}

/// The direction text and inline content run. There is no right-to-left layout
/// in tre today, so `LAYOUT_DIRECTION` is a constant; when a direction property
/// exists it replaces it and `BarSide::edge` is the only place that changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
}

/// The layout direction every view has until a direction property exists.
pub const LAYOUT_DIRECTION: Direction = Direction::LeftToRight;

/// The names `scrollbar_side_v` takes, in `BarSide` order of `VERTICAL_SIDES`.
pub const VERTICAL_SIDE_NAMES: [&str; 4] = ["end", "start", "left", "right"];
const VERTICAL_SIDES: [BarSide; 4] = [BarSide::End, BarSide::Start, BarSide::Left, BarSide::Right];
/// The names `scrollbar_side_h` takes.
pub const HORIZONTAL_SIDE_NAMES: [&str; 4] = ["end", "start", "bottom", "top"];
const HORIZONTAL_SIDES: [BarSide; 4] =
    [BarSide::End, BarSide::Start, BarSide::Bottom, BarSide::Top];

impl BarSide {
    /// The side named by index `index` of `axis`' name list
    /// (`VERTICAL_SIDE_NAMES`, `HORIZONTAL_SIDE_NAMES`).
    pub fn from_index(axis: ScrollAxis, index: u8) -> Option<Self> {
        let sides = match axis {
            ScrollAxis::Vertical => &VERTICAL_SIDES,
            ScrollAxis::Horizontal => &HORIZONTAL_SIDES,
        };
        sides.get(usize::from(index)).copied()
    }

    /// Which edge this names for the bar of `axis` (a vertical bar sits at the
    /// left or right edge, a horizontal one at the top or bottom): **the one
    /// place a side becomes an edge**. `End` is the right of a vertical bar
    /// (the left in a right-to-left layout) and the bottom of a horizontal
    /// one; `Start` the opposite.
    pub fn edge(self, axis: ScrollAxis, direction: Direction) -> Edge {
        match self {
            BarSide::Left | BarSide::Top => Edge::Near,
            BarSide::Right | BarSide::Bottom => Edge::Far,
            BarSide::Start | BarSide::End => {
                // Only the inline axis (a vertical bar's edge) flips with direction.
                let flipped = axis == ScrollAxis::Vertical && direction == Direction::RightToLeft;
                if (self == BarSide::End) != flipped {
                    Edge::Far
                } else {
                    Edge::Near
                }
            }
        }
    }
}

/// When the bar shows: `scrollbar_show`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// Whenever the axis overflows (the default).
    Overflow,
    /// Always, even without overflow (then a thumb as long as the lane).
    Always,
    /// When it overflows, fading out after `scrollbar_hide_delay_ms` of nothing
    /// happening to it, and back in on scroll, hover and drag.
    AutoHide,
    /// Never: the view still scrolls.
    Never,
}

pub const SHOW_NAMES: [&str; 4] = ["overflow", "always", "auto_hide", "never"];

impl Show {
    pub fn from_index(index: u8) -> Self {
        [Show::Overflow, Show::Always, Show::AutoHide, Show::Never][usize::from(index) % 4]
    }

    pub fn index(self) -> u8 {
        self as u8
    }
}

/// `scrollbar_arrows`: buttons at the ends of the lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrows {
    None,
    Both,
}

pub const ARROWS_NAMES: [&str; 2] = ["none", "both"];

impl Arrows {
    pub fn from_index(index: u8) -> Self {
        if index == 1 {
            Arrows::Both
        } else {
            Arrows::None
        }
    }

    pub fn index(self) -> u8 {
        self as u8
    }
}

/// `scrollbar_lane_click`: what a press on the lane beside the thumb does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneClick {
    /// Scroll one page toward the press, repeating while held.
    Page,
    /// Put the thumb's centre at the press and carry on dragging.
    Jump,
    /// Nothing: the press goes to what is under the lane.
    None,
}

pub const LANE_CLICK_NAMES: [&str; 3] = ["page", "jump", "none"];

impl LaneClick {
    pub fn from_index(index: u8) -> Self {
        [LaneClick::Page, LaneClick::Jump, LaneClick::None][usize::from(index) % 3]
    }

    pub fn index(self) -> u8 {
        self as u8
    }
}

const fn number(min: f64, max: f64) -> Kind {
    Kind::Number { min, max }
}

const NON_NEGATIVE: Kind = number(0.0, f64::MAX);
const FRACTION: Kind = number(0.0, 1.0);
const MILLIS: Kind = number(0.0, MAX_MILLIS);

macro_rules! props {
    ($($variant:ident, $name:literal, $kind:expr;)*) => {
        /// A scrollbar or scroll-feel property.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Prop {
            $($variant),*
        }

        /// How many properties there are.
        pub const COUNT: usize = [$($name),*].len();

        impl Prop {
            /// Every property, in table order.
            pub const ALL: [Prop; COUNT] = [$(Prop::$variant),*];

            /// The node property's name.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Prop::$variant => $name),*
                }
            }

            /// How its value is checked.
            pub const fn kind(self) -> Kind {
                match self {
                    $(Prop::$variant => $kind),*
                }
            }
        }
    };
}

props! {
    Width, "scrollbar_width", NON_NEGATIVE;
    HitWidth, "scrollbar_hit_width", NON_NEGATIVE;
    Margin, "scrollbar_margin", NON_NEGATIVE;
    MinLength, "scrollbar_min_length", NON_NEGATIVE;
    Gutter, "scrollbar_gutter", NON_NEGATIVE;
    Corner, "scrollbar_corner", NON_NEGATIVE;
    Radius, "scrollbar_radius", NON_NEGATIVE;
    Opacity, "scrollbar_opacity", FRACTION;
    OpacityHover, "scrollbar_opacity_hover", FRACTION;
    OpacityDrag, "scrollbar_opacity_drag", FRACTION;
    Fill, "scrollbar_fill", Kind::Colour;
    FillHover, "scrollbar_fill_hover", Kind::Colour;
    FillDrag, "scrollbar_fill_drag", Kind::Colour;
    TrackFill, "scrollbar_track_fill", Kind::Colour;
    TrackOpacity, "scrollbar_track_opacity", FRACTION;
    SideV, "scrollbar_side_v", Kind::Choice(&VERTICAL_SIDE_NAMES);
    SideH, "scrollbar_side_h", Kind::Choice(&HORIZONTAL_SIDE_NAMES);
    Show, "scrollbar_show", Kind::Choice(&SHOW_NAMES);
    HideDelay, "scrollbar_hide_delay_ms", MILLIS;
    FadeIn, "scrollbar_fade_in_ms", MILLIS;
    FadeOut, "scrollbar_fade_out_ms", MILLIS;
    HoverDelay, "scrollbar_hover_delay_ms", MILLIS;
    FadeEasing, "scrollbar_fade_easing", Kind::Easing;
    GutterReserve, "scrollbar_gutter_reserve", Kind::Flag;
    Arrows, "scrollbar_arrows", Kind::Choice(&ARROWS_NAMES);
    ArrowStep, "scrollbar_arrow_step", NON_NEGATIVE;
    LaneClick, "scrollbar_lane_click", Kind::Choice(&LANE_CLICK_NAMES);
    WheelNotch, "wheel_notch", NON_NEGATIVE;
    KeyLine, "key_line", NON_NEGATIVE;
    PageFraction, "page_fraction", number(0.01, 1.0);
    ScrollSmooth, "scroll_smooth_ms", MILLIS;
    ScrollEasing, "scroll_easing", Kind::Easing;
    PageRepeatDelay, "page_repeat_delay_ms", MILLIS;
    PageRepeat, "page_repeat_ms", number(1.0, MAX_MILLIS);
    SnapSettle, "scroll_snap_settle_ms", MILLIS;
    SnapDuration, "scroll_snap_duration_ms", MILLIS;
    SnapEasing, "scroll_snap_easing", Kind::Easing;
    FlingTau, "fling_tau_ms", number(1.0, MAX_MILLIS);
    FlingStop, "fling_stop_speed", number(0.1, MAX_SPEED);
    FlingMin, "fling_min_speed", number(0.0, MAX_SPEED);
    EdgeScroll, "edge_scroll", Kind::Flag;
    EdgeScrollBand, "edge_scroll_band", NON_NEGATIVE;
    EdgeScrollSpeed, "edge_scroll_speed", Kind::Pair { min: 0.0, max: MAX_SPEED };
}

impl Prop {
    /// The property called `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }

    /// Whether a terminal's history bar takes the property (0.6.12, UXREQ-006): the bar's
    /// measures, colours, fades, arrows and lane, and the page and its repeat. Not the gutter,
    /// the corner and the horizontal side (one bar, over the grid, no space reserved), nor the
    /// wheel, key, smooth-scroll, snap and fling ones (the session scrolls the history in lines).
    pub fn applies_to_terminal(self) -> bool {
        let bar = self.name().starts_with("scrollbar_")
            && !matches!(
                self,
                Prop::Gutter | Prop::GutterReserve | Prop::Corner | Prop::SideH
            );
        bar || matches!(
            self,
            Prop::PageFraction | Prop::PageRepeatDelay | Prop::PageRepeat
        )
    }

    /// Whether a multiline text input's own bar takes the property (0.6.13): the bar's measures,
    /// colours, fades, arrows and lane, the page and its repeat, and the wheel's notch. Not the
    /// gutter (one bar over the text, no space reserved; the padding is the input's own), nor
    /// the key, smooth-scroll, snap, fling and edge ones (the input's keys are the caret's, and
    /// its scroll is driven by the bar, the wheel and the caret, never eased or snapped).
    pub fn applies_to_text_input(self) -> bool {
        let bar = self.name().starts_with("scrollbar_")
            && !matches!(self, Prop::Gutter | Prop::GutterReserve);
        bar || matches!(
            self,
            Prop::WheelNotch | Prop::PageFraction | Prop::PageRepeatDelay | Prop::PageRepeat
        )
    }

    /// The choice names of the property, for a `Kind::Choice` one.
    pub fn choices(self) -> Option<&'static [&'static str]> {
        match self.kind() {
            Kind::Choice(names) => Some(names),
            _ => None,
        }
    }
}

/// What the user set: one slot per property, `None` for the default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overrides {
    slots: [Option<Value>; COUNT],
}

impl Default for Overrides {
    fn default() -> Self {
        Self {
            slots: [None; COUNT],
        }
    }
}

impl Overrides {
    /// What is set for `prop`.
    pub fn get(&self, prop: Prop) -> Option<Value> {
        self.slots[prop as usize]
    }

    /// Sets `prop` to `value`, or back to its default with `None`. A value its
    /// kind does not take is refused (`false`, nothing changed).
    pub fn set(&mut self, prop: Prop, value: Option<Value>) -> bool {
        if value.is_some_and(|v| !prop.kind().accepts(&v)) {
            return false;
        }
        self.slots[prop as usize] = value;
        true
    }

    /// `set` with a number; `None` unsets.
    pub fn set_number(&mut self, prop: Prop, value: Option<f64>) -> bool {
        self.set(prop, value.map(Value::Number))
    }

    pub fn number(&self, prop: Prop) -> Option<f64> {
        match self.get(prop) {
            Some(Value::Number(n)) => Some(n),
            _ => None,
        }
    }

    pub fn colour(&self, prop: Prop) -> Option<Color> {
        match self.get(prop) {
            Some(Value::Colour(c)) => Some(c),
            _ => None,
        }
    }

    pub fn flag(&self, prop: Prop) -> Option<bool> {
        match self.get(prop) {
            Some(Value::Flag(f)) => Some(f),
            _ => None,
        }
    }

    pub fn choice(&self, prop: Prop) -> Option<u8> {
        match self.get(prop) {
            Some(Value::Choice(i)) => Some(i),
            _ => None,
        }
    }

    pub fn pair(&self, prop: Prop) -> Option<(f64, f64)> {
        match self.get(prop) {
            Some(Value::Pair(a, b)) => Some((a, b)),
            _ => None,
        }
    }

    pub fn easing(&self, prop: Prop) -> Option<Easing> {
        match self.get(prop) {
            Some(Value::Easing(e)) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_property_has_a_unique_scrollbar_or_scroll_feel_name() {
        let mut names: Vec<&str> = Prop::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), COUNT);
        for prop in Prop::ALL {
            assert_eq!(Prop::from_name(prop.name()), Some(prop));
            assert!(
                prop.name().starts_with("scrollbar_")
                    || ["wheel_notch", "key_line", "page_fraction"].contains(&prop.name())
                    || prop.name().starts_with("scroll_")
                    || prop.name().starts_with("fling_")
                    || prop.name().starts_with("edge_scroll")
                    || prop.name().starts_with("page_"),
                "{}",
                prop.name()
            );
        }
    }

    #[test]
    fn a_value_its_kind_does_not_take_is_refused() {
        let mut set = Overrides::default();
        assert!(!set.set_number(Prop::Opacity, Some(1.5)));
        assert!(!set.set_number(Prop::Width, Some(-1.0)));
        assert!(!set.set_number(Prop::Width, Some(f64::NAN)));
        assert!(!set.set(Prop::Show, Some(Value::Number(1.0))));
        assert!(!set.set(Prop::Show, Some(Value::Choice(9))));
        assert!(set.set_number(Prop::Opacity, Some(0.5)));
        assert_eq!(set.number(Prop::Opacity), Some(0.5));
        assert!(set.set_number(Prop::Opacity, None));
        assert_eq!(set.get(Prop::Opacity), None);
    }

    #[test]
    fn a_side_resolves_to_an_edge_in_one_place_by_direction() {
        use BarSide::*;
        use ScrollAxis::*;
        let ltr = Direction::LeftToRight;
        let rtl = Direction::RightToLeft;
        // A vertical bar: end is the right edge, start the left, in ltr.
        assert_eq!(End.edge(Vertical, ltr), Edge::Far);
        assert_eq!(Start.edge(Vertical, ltr), Edge::Near);
        assert_eq!(End.edge(Vertical, rtl), Edge::Near);
        assert_eq!(Start.edge(Vertical, rtl), Edge::Far);
        // A horizontal bar: end is the bottom whatever the direction.
        assert_eq!(End.edge(Horizontal, ltr), Edge::Far);
        assert_eq!(End.edge(Horizontal, rtl), Edge::Far);
        assert_eq!(Start.edge(Horizontal, rtl), Edge::Near);
        // Physical names never flip.
        assert_eq!(Left.edge(Vertical, rtl), Edge::Near);
        assert_eq!(Right.edge(Vertical, rtl), Edge::Far);
        assert_eq!(Top.edge(Horizontal, ltr), Edge::Near);
        assert_eq!(Bottom.edge(Horizontal, ltr), Edge::Far);
    }

    #[test]
    fn side_names_map_to_sides_per_axis() {
        for (i, name) in VERTICAL_SIDE_NAMES.iter().enumerate() {
            let side = BarSide::from_index(ScrollAxis::Vertical, i as u8).unwrap();
            assert_eq!(
                format!("{side:?}").to_lowercase(),
                *name,
                "vertical side {i}"
            );
        }
        for (i, name) in HORIZONTAL_SIDE_NAMES.iter().enumerate() {
            let side = BarSide::from_index(ScrollAxis::Horizontal, i as u8).unwrap();
            assert_eq!(
                format!("{side:?}").to_lowercase(),
                *name,
                "horizontal side {i}"
            );
        }
    }
}
