//! 0.6.11 (UXREQ-001): the scrollbar of a scroll view and of a virtual list --
//! the geometry painting, hit-testing, dragging and damage all read, so they
//! cannot drift apart, and the one state both kinds share (`ScrollbarState`).
//!
//! The numbers of the Tesserae UX note "Scrollbar and Scroll Container" are
//! the defaults of a `both` view (`Defaults::Spec`); a `vertical` or
//! `horizontal` view and a virtual list keep the scrollbar they always had
//! (4 px thumb, 6 px of slop, the colour's own alpha) unless a property says
//! otherwise (`Defaults::Legacy`, `Defaults::List`). Every aspect is a property
//! (`Prop`); nothing here calls back into the app.

use peniko::Color;
use peniko::kurbo::{Point, Rect};
use std::time::{Duration, Instant};

use crate::animation::{Animated, CompletionHandle, MotionCurve};
use crate::easing::{self, Easing};
use crate::node::{
    SCROLLBAR_GRAB_SLOP, SCROLLBAR_MARGIN, SCROLLBAR_MIN_LENGTH, SCROLLBAR_THICKNESS,
    ScrollViewState, TerminalState, VirtualListState,
};
use crate::scrollbar_props::{
    Arrows, BarSide, Edge, LAYOUT_DIRECTION, LaneClick, Overrides, Prop, Show, Value,
};

/// How far one wheel notch scrolls, in pixels (`Lines` deltas; `Pixels`
/// deltas are taken as they come): `wheel_notch`.
pub const WHEEL_NOTCH: f64 = 40.0;
/// How far an arrow key, or a click on an arrow button, scrolls: `key_line`,
/// `scrollbar_arrow_step`.
pub const KEY_LINE: f64 = 40.0;
/// How much of the viewport a page (Page Up/Down, a click on the lane)
/// scrolls: the viewport less a tenth, so a line of context stays:
/// `page_fraction`.
pub const PAGE_FRACTION: f64 = 0.9;
/// A thumb changes opacity over this long (Motion `short2`):
/// `scrollbar_fade_in_ms`, `scrollbar_fade_out_ms`.
pub const FADE: Duration = Duration::from_millis(100);
/// A press on the lane pages again after this long, then every `PAGE_REPEAT`:
/// `page_repeat_delay_ms`, `page_repeat_ms`.
pub const PAGE_DELAY: Duration = Duration::from_millis(400);
pub const PAGE_REPEAT: Duration = Duration::from_millis(100);
/// 0.6.11: `scroll_snap_settle_ms` -- how long after its last input a snapping
/// view waits before it settles -- and `scroll_snap_duration_ms`, how long the
/// settling takes.
pub const SNAP_SETTLE: Duration = Duration::from_millis(120);
pub const SNAP_DURATION: Duration = Duration::from_millis(200);
/// 0.6.11: `fling_tau_ms` -- how quickly a flick slows: the time constant of
/// its exponential decay, in milliseconds.
pub const FLING_TAU_MS: f64 = 350.0;
/// `fling_stop_speed`: a flick coasts to a stop at this speed, pixels a second.
pub const FLING_STOP: f64 = 30.0;
/// `fling_min_speed`: a flick slower than this, pixels a second, does not coast.
pub const FLING_MIN: f64 = 150.0;
/// `scrollbar_hide_delay_ms`: how long `auto_hide` waits.
pub const HIDE_DELAY: Duration = Duration::from_millis(1000);

/// The spec's own numbers (`both` views).
pub const SPEC_THICKNESS: f64 = 8.0;
pub const SPEC_HIT: f64 = 12.0;
pub const SPEC_GUTTER: f64 = 12.0;
pub const SPEC_CORNER: f64 = 12.0;
pub const SPEC_OPACITY_REST: f64 = 0.70;
pub const SPEC_OPACITY_HOVER: f64 = 0.80;
pub const SPEC_OPACITY_DRAG: f64 = 1.0;
/// The thumb radius of a view that keeps its legacy scrollbar.
pub const LEGACY_RADIUS: f64 = 2.0;

/// One of a scroll view's two axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

impl ScrollAxis {
    pub const BOTH: [ScrollAxis; 2] = [ScrollAxis::Vertical, ScrollAxis::Horizontal];

    /// 0 for vertical, 1 for horizontal: the index into `[vertical, horizontal]` pairs.
    pub fn index(self) -> usize {
        match self {
            ScrollAxis::Vertical => 0,
            ScrollAxis::Horizontal => 1,
        }
    }

    pub fn other(self) -> ScrollAxis {
        match self {
            ScrollAxis::Vertical => ScrollAxis::Horizontal,
            ScrollAxis::Horizontal => ScrollAxis::Vertical,
        }
    }

    /// This axis' share of an `(x, y)` pair.
    pub fn pick(self, (x, y): (f64, f64)) -> f64 {
        match self {
            ScrollAxis::Vertical => y,
            ScrollAxis::Horizontal => x,
        }
    }
}

/// The axes a scroll view moves along: `orientation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollOrientation {
    Vertical,
    Horizontal,
    Both,
}

impl ScrollOrientation {
    pub const ALL: [(&'static str, ScrollOrientation); 3] = [
        ("vertical", ScrollOrientation::Vertical),
        ("horizontal", ScrollOrientation::Horizontal),
        ("both", ScrollOrientation::Both),
    ];
}

/// Which defaults a scroller's unset properties take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defaults {
    /// The Tesserae UX spec: a `both` scroll view.
    Spec,
    /// What a `vertical` or `horizontal` scroll view always had: a 4 px thumb
    /// with the colour's own alpha, a 6 px slop, no lane click, a page that is
    /// the whole viewport.
    Legacy,
    /// What a virtual list always had: the legacy bar, and a page of nine
    /// tenths of the viewport.
    List,
    /// 0.6.12 (UXREQ-006): a terminal's history bar. The spec's measures, as a `both` view
    /// has them, but off until `scrollbar_show` says otherwise (an opt-in).
    Terminal,
}

impl Defaults {
    /// Whether the measures are the Tesserae UX spec's (a `both` scroll view, a terminal).
    pub fn is_spec(self) -> bool {
        matches!(self, Defaults::Spec | Defaults::Terminal)
    }
}

/// The measures a scrollbar and the scroll feel are built from, resolved: what
/// the user set, else the defaults of the scroller (`Defaults`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarMetrics {
    /// Painted size across the axis.
    pub thickness: f64,
    /// Pointer-sensitive size across the axis, centred on the thumb.
    pub hit: f64,
    /// Between thumb and view edge, and thumb end and lane end.
    pub margin: f64,
    /// The shortest thumb.
    pub min_length: f64,
    /// Reserved at the edge of an axis that overflows (see `reserved_gutter`).
    pub gutter: f64,
    /// How much a lane stops short when the other axis has a bar too.
    pub corner: f64,
    /// The thumb's (and track's) corner radius.
    pub radius: f64,
    /// How far past the thumb's ends still grabs it.
    pub along_slop: f64,
    pub opacity_rest: f64,
    pub opacity_hover: f64,
    pub opacity_drag: f64,
    /// Multiplies the track fill's alpha.
    pub track_opacity: f64,
    /// The edge each bar sits on, by `ScrollAxis::index`.
    pub edges: [Edge; 2],
    pub show: Show,
    pub hide_delay_ms: f64,
    pub fade_in_ms: f64,
    pub fade_out_ms: f64,
    pub hover_delay_ms: f64,
    pub fade_easing: Easing,
    /// Whether the gutter is reserved (`true`) or the thumb overlays content.
    pub gutter_reserve: bool,
    pub arrows: Arrows,
    pub arrow_step: f64,
    pub lane_click: LaneClick,
    pub wheel_notch: f64,
    pub key_line: f64,
    pub page_fraction: f64,
    pub smooth_ms: f64,
    pub scroll_easing: Easing,
    pub page_delay_ms: f64,
    pub page_repeat_ms: f64,
    /// How long a snapping view waits after its last input before it settles.
    pub snap_settle_ms: f64,
    /// How long the settle takes (nothing under reduced motion).
    pub snap_duration_ms: f64,
    pub snap_easing: Easing,
    /// The time constant of a fling's decay, milliseconds.
    pub fling_tau_ms: f64,
    /// A fling ends at this speed, pixels a second.
    pub fling_stop_speed: f64,
    /// A flick slower than this does not coast, pixels a second.
    pub fling_min_speed: f64,
}

/// `ms` milliseconds as a duration, none for a number that is no time.
pub fn millis(ms: f64) -> Duration {
    if ms.is_finite() && ms > 0.0 {
        Duration::from_secs_f64(ms / 1000.0)
    } else {
        Duration::ZERO
    }
}

impl ScrollbarMetrics {
    /// What `overrides` and the defaults `d` make of every measure.
    pub fn resolve(o: &Overrides, d: Defaults) -> Self {
        let spec = d.is_spec();
        let n = |prop: Prop, default: f64| o.number(prop).unwrap_or(default);
        let thickness = n(
            Prop::Width,
            if spec {
                SPEC_THICKNESS
            } else {
                SCROLLBAR_THICKNESS
            },
        );
        let opacity_rest = n(Prop::Opacity, if spec { SPEC_OPACITY_REST } else { 1.0 });
        let edge = |axis: ScrollAxis, prop: Prop| {
            o.choice(prop)
                .and_then(|i| BarSide::from_index(axis, i))
                .unwrap_or(BarSide::End)
                .edge(axis, LAYOUT_DIRECTION)
        };
        Self {
            thickness,
            hit: n(
                Prop::HitWidth,
                if spec {
                    SPEC_HIT
                } else {
                    thickness + SCROLLBAR_GRAB_SLOP * 2.0
                },
            ),
            margin: n(Prop::Margin, SCROLLBAR_MARGIN),
            min_length: n(Prop::MinLength, SCROLLBAR_MIN_LENGTH),
            gutter: n(Prop::Gutter, if spec { SPEC_GUTTER } else { 0.0 }),
            corner: n(Prop::Corner, if spec { SPEC_CORNER } else { 0.0 }),
            radius: n(
                Prop::Radius,
                if spec { thickness / 2.0 } else { LEGACY_RADIUS },
            ),
            along_slop: if spec { 0.0 } else { SCROLLBAR_GRAB_SLOP },
            opacity_rest,
            opacity_hover: n(
                Prop::OpacityHover,
                if spec {
                    SPEC_OPACITY_HOVER
                } else {
                    opacity_rest
                },
            ),
            opacity_drag: n(
                Prop::OpacityDrag,
                if spec {
                    SPEC_OPACITY_DRAG
                } else {
                    opacity_rest
                },
            ),
            track_opacity: n(Prop::TrackOpacity, 1.0),
            edges: [
                edge(ScrollAxis::Vertical, Prop::SideV),
                edge(ScrollAxis::Horizontal, Prop::SideH),
            ],
            show: Show::from_index(o.choice(Prop::Show).unwrap_or(if d == Defaults::Terminal {
                Show::Never.index()
            } else {
                Show::Overflow.index()
            })),
            hide_delay_ms: n(Prop::HideDelay, HIDE_DELAY.as_secs_f64() * 1000.0),
            fade_in_ms: n(Prop::FadeIn, FADE.as_secs_f64() * 1000.0),
            fade_out_ms: n(Prop::FadeOut, FADE.as_secs_f64() * 1000.0),
            hover_delay_ms: n(Prop::HoverDelay, 0.0),
            fade_easing: o.easing(Prop::FadeEasing).unwrap_or(easing::STANDARD),
            gutter_reserve: o.flag(Prop::GutterReserve).unwrap_or(true),
            arrows: Arrows::from_index(o.choice(Prop::Arrows).unwrap_or(0)),
            arrow_step: n(Prop::ArrowStep, KEY_LINE),
            lane_click: LaneClick::from_index(o.choice(Prop::LaneClick).unwrap_or(if spec {
                LaneClick::Page.index()
            } else {
                LaneClick::None.index()
            })),
            wheel_notch: n(Prop::WheelNotch, WHEEL_NOTCH),
            key_line: n(Prop::KeyLine, KEY_LINE),
            page_fraction: n(
                Prop::PageFraction,
                if d == Defaults::Legacy {
                    1.0
                } else {
                    PAGE_FRACTION
                },
            ),
            smooth_ms: n(Prop::ScrollSmooth, 0.0),
            scroll_easing: o.easing(Prop::ScrollEasing).unwrap_or(easing::STANDARD),
            page_delay_ms: n(Prop::PageRepeatDelay, PAGE_DELAY.as_secs_f64() * 1000.0),
            page_repeat_ms: n(Prop::PageRepeat, PAGE_REPEAT.as_secs_f64() * 1000.0),
            snap_settle_ms: n(Prop::SnapSettle, SNAP_SETTLE.as_secs_f64() * 1000.0),
            snap_duration_ms: n(Prop::SnapDuration, SNAP_DURATION.as_secs_f64() * 1000.0),
            snap_easing: o.easing(Prop::SnapEasing).unwrap_or(easing::STANDARD),
            fling_tau_ms: n(Prop::FlingTau, FLING_TAU_MS),
            fling_stop_speed: n(Prop::FlingStop, FLING_STOP),
            fling_min_speed: n(Prop::FlingMin, FLING_MIN),
        }
    }

    /// What property `prop` reads: the value in force, for every property that
    /// is not a colour (colours and the side names are what the user set,
    /// `ScrollbarState::get`).
    pub fn get(&self, prop: Prop) -> Option<Value> {
        let number = |n: f64| Some(Value::Number(n));
        match prop {
            Prop::Width => number(self.thickness),
            Prop::HitWidth => number(self.hit),
            Prop::Margin => number(self.margin),
            Prop::MinLength => number(self.min_length),
            Prop::Gutter => number(self.gutter),
            Prop::Corner => number(self.corner),
            Prop::Radius => number(self.radius),
            Prop::Opacity => number(self.opacity_rest),
            Prop::OpacityHover => number(self.opacity_hover),
            Prop::OpacityDrag => number(self.opacity_drag),
            Prop::TrackOpacity => number(self.track_opacity),
            Prop::Show => Some(Value::Choice(self.show.index())),
            Prop::HideDelay => number(self.hide_delay_ms),
            Prop::FadeIn => number(self.fade_in_ms),
            Prop::FadeOut => number(self.fade_out_ms),
            Prop::HoverDelay => number(self.hover_delay_ms),
            Prop::FadeEasing => Some(Value::Easing(self.fade_easing)),
            Prop::GutterReserve => Some(Value::Flag(self.gutter_reserve)),
            Prop::Arrows => Some(Value::Choice(self.arrows.index())),
            Prop::ArrowStep => number(self.arrow_step),
            Prop::LaneClick => Some(Value::Choice(self.lane_click.index())),
            Prop::WheelNotch => number(self.wheel_notch),
            Prop::KeyLine => number(self.key_line),
            Prop::PageFraction => number(self.page_fraction),
            Prop::ScrollSmooth => number(self.smooth_ms),
            Prop::ScrollEasing => Some(Value::Easing(self.scroll_easing)),
            Prop::PageRepeatDelay => number(self.page_delay_ms),
            Prop::PageRepeat => number(self.page_repeat_ms),
            Prop::SnapSettle => number(self.snap_settle_ms),
            Prop::SnapDuration => number(self.snap_duration_ms),
            Prop::SnapEasing => Some(Value::Easing(self.snap_easing)),
            Prop::FlingTau => number(self.fling_tau_ms),
            Prop::FlingStop => number(self.fling_stop_speed),
            Prop::FlingMin => number(self.fling_min_speed),
            Prop::Fill
            | Prop::FillHover
            | Prop::FillDrag
            | Prop::TrackFill
            | Prop::SideV
            | Prop::SideH => None,
        }
    }

    /// The thumb's opacity at emphasis `level` (0 rest, 1 hover, 2 drag; in
    /// between while it fades).
    pub fn opacity(&self, level: f64) -> f64 {
        let level = level.clamp(0.0, 2.0);
        if level <= 1.0 {
            self.opacity_rest + (self.opacity_hover - self.opacity_rest) * level
        } else {
            self.opacity_hover + (self.opacity_drag - self.opacity_hover) * (level - 1.0)
        }
    }

    /// The edge the bar of `axis` sits on.
    pub fn edge(&self, axis: ScrollAxis) -> Edge {
        self.edges[axis.index()]
    }

    /// What the gutter reserves along the edge of an overflowing axis: nothing
    /// when `scrollbar_gutter_reserve` is off (the thumb overlays content) or
    /// no bar shows.
    pub fn reserved_gutter(&self) -> f64 {
        if self.gutter_reserve && self.show != Show::Never {
            self.gutter
        } else {
            0.0
        }
    }

    /// The length of one arrow button along the lane: none without arrows.
    pub fn arrow_len(&self) -> f64 {
        if self.arrows == Arrows::Both {
            self.thickness
        } else {
            0.0
        }
    }

    /// Whether `show` lets a bar show for the axis of a view of `viewport`
    /// over `content` along it: never for `never`, always for `always`, else
    /// when the content is longer.
    pub fn shows(&self, viewport: f64, content: f64) -> bool {
        match self.show {
            Show::Never => false,
            Show::Always => true,
            Show::Overflow | Show::AutoHide => content > viewport,
        }
    }

    /// How long a snap settle takes: nothing under reduced motion (the spec's
    /// AC-SCB-17).
    pub fn snap_duration(&self, reduced: bool) -> Duration {
        if reduced {
            Duration::ZERO
        } else {
            millis(self.snap_duration_ms)
        }
    }

    /// How long a smooth scroll takes: nothing under reduced motion.
    pub fn smooth(&self, reduced: bool) -> Duration {
        if reduced {
            Duration::ZERO
        } else {
            millis(self.smooth_ms)
        }
    }
}

/// An arrow button at one end of a lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    /// The painted square.
    pub rect: Rect,
    /// Where a press takes it: as long as the square, as wide as the hit strip.
    pub hit: Rect,
}

/// One thumb, with the regions around it, all in the view's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub axis: ScrollAxis,
    /// The painted thumb.
    pub thumb: Rect,
    /// Where a press grabs the thumb.
    pub hit: Rect,
    /// The strip along the axis a press pages in (the thumb is in it), between
    /// the arrows when there are any.
    pub lane: Rect,
    /// The painted track: the lane, as wide as the thumb.
    pub track_rect: Rect,
    /// The arrow buttons at the lane's ends, `[start, end]`, with
    /// `scrollbar_arrows`.
    pub arrows: Option<[Arrow; 2]>,
    /// The lane's length.
    pub track: f64,
    /// The thumb's length.
    pub length: f64,
    /// How far the content can scroll along this axis.
    pub max_scroll: f64,
}

impl Thumb {
    /// How far the thumb travels from one end of the lane to the other.
    pub fn travel(&self) -> f64 {
        self.track - self.length
    }

    /// The thumb's span along its axis.
    pub fn span(&self) -> (f64, f64) {
        match self.axis {
            ScrollAxis::Vertical => (self.thumb.y0, self.thumb.y1),
            ScrollAxis::Horizontal => (self.thumb.x0, self.thumb.x1),
        }
    }

    /// The lane's start along its axis.
    pub fn lane_start(&self) -> f64 {
        match self.axis {
            ScrollAxis::Vertical => self.lane.y0,
            ScrollAxis::Horizontal => self.lane.x0,
        }
    }

    /// Every region the bar paints: the track (when it has a fill, `track`),
    /// the thumb, the arrows.
    pub fn painted(&self, track: bool) -> impl Iterator<Item = Rect> {
        let arrows = self.arrows.into_iter().flatten().map(|a| a.rect);
        track
            .then_some(self.track_rect)
            .into_iter()
            .chain([self.thumb])
            .chain(arrows)
    }

    /// The arrow button a press at `point` (view coordinates) is on: 0 for the
    /// one at the start of the lane, 1 for the end.
    pub fn arrow_at(&self, point: Point) -> Option<usize> {
        let arrows = self.arrows?;
        arrows.iter().position(|a| holds(a.hit, point))
    }
}

/// `rect` holds `point`, edges included.
pub fn holds(rect: Rect, point: Point) -> bool {
    (rect.x0..=rect.x1).contains(&point.x) && (rect.y0..=rect.y1).contains(&point.y)
}

/// `(track, thumb length, along)` of a thumb for a `viewport` over `content`
/// at `offset`: the lane is the viewport less the margins, `reserve` (the
/// corner, at its `(start, end)`) and the arrows; the thumb is
/// `max(min_length, lane * viewport / content)`, never longer than the lane,
/// and sits along it by how far the content has scrolled.
pub fn axis_geometry(
    m: &ScrollbarMetrics,
    viewport: f64,
    content: f64,
    offset: f64,
    reserve: (f64, f64),
) -> (f64, f64, f64) {
    let arrow = m.arrow_len();
    let start = m.margin + reserve.0 + arrow;
    let track = viewport - start - m.margin - reserve.1 - arrow;
    if track <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let max_scroll = (content - viewport).max(0.0);
    let thumb = if content > 0.0 {
        (track * (viewport / content)).max(m.min_length)
    } else {
        track
    };
    let thumb = thumb.min(track);
    let progress = if max_scroll > 0.0 {
        (offset / max_scroll).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (track, thumb, start + (track - thumb) * progress)
}

impl ScrollbarMetrics {
    /// The thumbs, `[vertical, horizontal]`, of a view of `size` over `content`
    /// scrolled to `offsets` (all `(x, y)`): one for each of `axes` whose bar
    /// shows (`shows`), each lane stopping `corner` short of the end where the
    /// other axis has a bar too, so the two never meet. Each bar sits on its
    /// edge (`edges`).
    pub fn thumbs(
        &self,
        axes: &[ScrollAxis],
        size: (f64, f64),
        content: (f64, f64),
        offsets: (f64, f64),
    ) -> [Option<Thumb>; 2] {
        let mut shown = [false; 2];
        for &axis in axes {
            shown[axis.index()] = self.shows(axis.pick(size), axis.pick(content));
        }
        let mut bars = [None, None];
        for &axis in axes {
            if !shown[axis.index()] {
                continue;
            }
            let other = axis.other();
            // The lane stops short at the end where the other axis' bar is.
            let reserve = if shown[other.index()] {
                match self.edge(other) {
                    Edge::Far => (0.0, self.corner),
                    Edge::Near => (self.corner, 0.0),
                }
            } else {
                (0.0, 0.0)
            };
            let (viewport, extent, offset) =
                (axis.pick(size), axis.pick(content), axis.pick(offsets));
            let (track, length, along) = axis_geometry(self, viewport, extent, offset, reserve);
            if track <= 0.0 {
                continue;
            }
            let lane_start = self.margin + reserve.0 + self.arrow_len();
            // The size across the axis: the width for a vertical bar.
            let cross = other.pick(size);
            let (across_start, across_end) = match self.edge(axis) {
                Edge::Far => (cross - self.margin - self.thickness, cross - self.margin),
                Edge::Near => (self.margin, self.margin + self.thickness),
            };
            let centre = f64::midpoint(across_start, across_end);
            let (hit_a, hit_b) = (centre - self.hit / 2.0, centre + self.hit / 2.0);
            // A rectangle `from..to` along the axis and `a..b` across it.
            let rect = |from: f64, to: f64, a: f64, b: f64| match axis {
                ScrollAxis::Vertical => Rect::new(a, from, b, to),
                ScrollAxis::Horizontal => Rect::new(from, a, to, b),
            };
            let arrows = (self.arrows == Arrows::Both).then(|| {
                let len = self.arrow_len();
                let lane_end = lane_start + track;
                [(lane_start - len, lane_start), (lane_end, lane_end + len)].map(|(from, to)| {
                    Arrow {
                        rect: rect(from, to, across_start, across_end),
                        hit: rect(from, to, hit_a, hit_b),
                    }
                })
            });
            bars[axis.index()] = Some(Thumb {
                axis,
                thumb: rect(along, along + length, across_start, across_end),
                hit: rect(
                    along - self.along_slop,
                    along + length + self.along_slop,
                    hit_a,
                    hit_b,
                ),
                lane: rect(lane_start, lane_start + track, hit_a, hit_b),
                track_rect: rect(lane_start, lane_start + track, across_start, across_end),
                arrows,
                track,
                length,
                max_scroll: (extent - viewport).max(0.0),
            });
        }
        bars
    }
}

/// The state a scroll view and a virtual list share for their scrollbar: what
/// the user set, and how emphasised and how visible each bar is.
pub struct ScrollbarState {
    /// What the user set (`Prop`); the rest are defaults (`Defaults`).
    pub settings: Overrides,
    /// How emphasised each thumb is, `[vertical, horizontal]`: 0 at rest, 1
    /// with the pointer over its hit area, 2 while dragged. Animated over
    /// `scrollbar_fade_in_ms` / `scrollbar_fade_out_ms`; the thumb's opacity
    /// and colour are read off it.
    pub level: [Animated<f64>; 2],
    /// How visible the bars are, 0 to 1: 1 unless `scrollbar_show` is
    /// `auto_hide` and the bars have faded out.
    pub shown: Animated<f64>,
    /// Which thumb the pointer is over, and since when.
    pub hovered: [bool; 2],
    hover_since: [Option<Instant>; 2],
    /// `Some((pointer_coord, scroll_at_start))`, set the instant a thumb drag
    /// begins and cleared when it ends: a *relative*-delta anchor, so grabbing
    /// the thumb anywhere along its own length does not snap it so that point
    /// jumps under the pointer.
    pub drag_anchor: Option<(f64, f64)>,
    /// Which thumb the drag in progress holds.
    pub drag_axis: ScrollAxis,
    /// When `auto_hide` fades the bars out, if nothing happens to them first.
    hide_at: Option<Instant>,
    /// Whether the last tick found the bar hovered or dragged: the delay starts when that ends.
    was_emphasised: bool,
    /// The offsets the last tick saw: a change is a reason to show the bars.
    seen: Option<(f64, f64)>,
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollbarState {
    pub fn new() -> Self {
        Self {
            settings: Overrides::default(),
            level: [Animated::new(0.0), Animated::new(0.0)],
            shown: Animated::new(1.0),
            hovered: [false; 2],
            hover_since: [None; 2],
            drag_anchor: None,
            drag_axis: ScrollAxis::Vertical,
            hide_at: None,
            was_emphasised: false,
            seen: None,
        }
    }

    /// The measures in force for a scroller with defaults `d`.
    pub fn metrics(&self, d: Defaults) -> ScrollbarMetrics {
        ScrollbarMetrics::resolve(&self.settings, d)
    }

    /// What property `prop` reads: the value in force, except for a colour
    /// and a side, which read what was set (`None`: the default).
    pub fn get(&self, prop: Prop, d: Defaults) -> Option<Value> {
        match prop {
            Prop::Fill | Prop::FillHover | Prop::FillDrag | Prop::TrackFill => {
                self.settings.get(prop)
            }
            Prop::SideV | Prop::SideH => {
                Some(Value::Choice(self.settings.choice(prop).unwrap_or(0)))
            }
            _ => self.metrics(d).get(prop),
        }
    }

    /// The thumb's colour for `axis` now: `fill`, `fill_hover` or `fill_drag` by
    /// how emphasised it is (`default_fill` when none is set), at that level's
    /// opacity and how visible the bar is.
    pub fn thumb_color(
        &self,
        m: &ScrollbarMetrics,
        axis: ScrollAxis,
        default_fill: Color,
    ) -> Color {
        let level = self.level[axis.index()].current.clamp(0.0, 2.0);
        let rest = self.settings.colour(Prop::Fill).unwrap_or(default_fill);
        let hover = self.settings.colour(Prop::FillHover).unwrap_or(rest);
        let drag = self.settings.colour(Prop::FillDrag).unwrap_or(rest);
        let base = if level <= 1.0 {
            rest.lerp_rect(hover, level as f32)
        } else {
            hover.lerp_rect(drag, (level - 1.0) as f32)
        };
        let alpha = m.opacity(level) * self.shown.current;
        if alpha == 1.0 {
            base
        } else {
            base.multiply_alpha(alpha as f32)
        }
    }

    /// The track's colour now, `None` when no `scrollbar_track_fill` is set (a
    /// track-less bar, the default).
    pub fn track_color(&self, m: &ScrollbarMetrics) -> Option<Color> {
        let fill = self.settings.colour(Prop::TrackFill)?;
        let alpha = m.track_opacity * self.shown.current;
        (alpha > 0.0).then(|| fill.multiply_alpha(alpha as f32))
    }

    /// The pointer moved on or off the thumb of `axis`.
    pub fn set_hover(&mut self, axis: ScrollAxis, on: bool, now: Instant) {
        let i = axis.index();
        if on && !self.hovered[i] {
            self.hover_since[i] = Some(now);
        }
        if !on {
            self.hover_since[i] = None;
        }
        self.hovered[i] = on;
    }

    /// The emphasis `axis`' thumb should have: dragged 2, hovered 1 (once
    /// `scrollbar_hover_delay_ms` has passed), else rest 0; `true` as the second
    /// part while a hover is still waiting out its delay.
    fn target_level(&self, m: &ScrollbarMetrics, axis: ScrollAxis, now: Instant) -> (f64, bool) {
        if self.drag_anchor.is_some() && self.drag_axis == axis {
            return (2.0, false);
        }
        let i = axis.index();
        if !self.hovered[i] {
            return (0.0, false);
        }
        match self.hover_since[i] {
            Some(since) if now.saturating_duration_since(since) >= millis(m.hover_delay_ms) => {
                (1.0, false)
            }
            _ => (0.0, true),
        }
    }

    /// Starts fading each thumb to the emphasis it should have (`level`), by
    /// `scrollbar_fade_in_ms` going up and `scrollbar_fade_out_ms` coming down;
    /// instantly under `reduced` motion. Whether a hover is still waiting out
    /// `scrollbar_hover_delay_ms`, which keeps the scroller ticking until it
    /// has.
    pub fn sync_levels(&mut self, d: Defaults, now: Instant, reduced: bool) -> bool {
        let m = self.metrics(d);
        let mut busy = false;
        for axis in ScrollAxis::BOTH {
            let (target, waiting) = self.target_level(&m, axis, now);
            let level = &mut self.level[axis.index()];
            busy |= waiting;
            if (*level.target() - target).abs() < 1e-9 {
                continue;
            }
            let ms = if target > level.current {
                m.fade_in_ms
            } else {
                m.fade_out_ms
            };
            let duration = if reduced { Duration::ZERO } else { millis(ms) };
            if duration.is_zero() {
                level.set_now(target);
            } else {
                level.animate_to(target, duration, m.fade_easing.curve(), now);
            }
        }
        busy
    }

    /// Starts fading the bars to `target` visibility.
    fn fade_shown(&mut self, m: &ScrollbarMetrics, target: f64, now: Instant, reduced: bool) {
        let ms = if target > self.shown.current {
            m.fade_in_ms
        } else {
            m.fade_out_ms
        };
        let duration = if reduced { Duration::ZERO } else { millis(ms) };
        if duration.is_zero() {
            self.shown.set_now(target);
        } else {
            self.shown
                .animate_to(target, duration, m.fade_easing.curve(), now);
        }
    }

    /// Advances the bar to `now`: the emphasis fades, and `auto_hide`'s
    /// visibility -- shown by a change of `offsets`, a hover or a drag, hidden
    /// `scrollbar_hide_delay_ms` after the last of them. Whether anything is
    /// still moving or waiting, so the tree keeps ticking.
    pub fn tick(
        &mut self,
        d: Defaults,
        offsets: (f64, f64),
        now: Instant,
        reduced: bool,
        completed: &mut Vec<CompletionHandle>,
    ) -> bool {
        let m = self.metrics(d);
        let mut active = self.sync_levels(d, now, reduced);
        if m.show == Show::AutoHide {
            let moved = self.seen != Some(offsets);
            let emphasised = self.hovered.iter().any(|h| *h) || self.drag_anchor.is_some();
            if moved || emphasised {
                // While the pointer is on it nothing is waited for (the loop may sleep);
                // the delay starts when it leaves.
                self.hide_at = if emphasised {
                    None
                } else {
                    Some(now + millis(m.hide_delay_ms))
                };
                self.was_emphasised = emphasised;
                if *self.shown.target() < 1.0 {
                    self.fade_shown(&m, 1.0, now, reduced);
                }
            } else if self.was_emphasised {
                self.was_emphasised = false;
                self.hide_at = Some(now + millis(m.hide_delay_ms));
            } else if self.hide_at.is_some_and(|t| now >= t) {
                self.hide_at = None;
                self.fade_shown(&m, 0.0, now, reduced);
            }
            active |= self.hide_at.is_some();
        } else {
            self.hide_at = None;
            if *self.shown.target() < 1.0 {
                self.shown.set_now(1.0);
            }
        }
        self.seen = Some(offsets);
        let [vertical, horizontal] = &mut self.level;
        for animated in [vertical, horizontal, &mut self.shown] {
            active |= animated.tick(now, completed);
        }
        active
    }
}

/// A node that scrolls and carries a scrollbar: a scroll view or a virtual
/// list, so the bar's geometry, hit-testing, hover, drag and fade are one
/// implementation for both.
pub trait Scroller {
    fn bar(&self) -> &ScrollbarState;
    fn bar_mut(&mut self) -> &mut ScrollbarState;
    /// Which defaults its unset properties take.
    fn defaults(&self) -> Defaults;
    /// The axes it scrolls along, `vertical` first.
    fn scroll_axes(&self) -> &'static [ScrollAxis];
    /// `(x, y)`: how far its content is scrolled right and down.
    fn scroll_offsets(&self) -> (f64, f64);
    /// The animated offset along `axis` (only called for one of `scroll_axes`).
    fn offset_anim(&mut self, axis: ScrollAxis) -> &mut Animated<f64>;

    /// The measures in force.
    fn metrics(&self) -> ScrollbarMetrics {
        self.bar().metrics(self.defaults())
    }

    /// The thumbs, `[vertical, horizontal]`, for a view of `size` over `content`
    /// scrolled to `offsets`.
    fn thumbs(
        &self,
        size: (f64, f64),
        content: (f64, f64),
        offsets: (f64, f64),
    ) -> [Option<Thumb>; 2] {
        self.metrics()
            .thumbs(self.scroll_axes(), size, content, offsets)
    }
}

impl Scroller for ScrollViewState {
    fn bar(&self) -> &ScrollbarState {
        &self.bar
    }

    fn bar_mut(&mut self) -> &mut ScrollbarState {
        &mut self.bar
    }

    fn defaults(&self) -> Defaults {
        if self.both {
            Defaults::Spec
        } else {
            Defaults::Legacy
        }
    }

    fn scroll_axes(&self) -> &'static [ScrollAxis] {
        self.axes()
    }

    fn scroll_offsets(&self) -> (f64, f64) {
        self.offsets()
    }

    fn offset_anim(&mut self, axis: ScrollAxis) -> &mut Animated<f64> {
        self.animated(axis)
    }
}

impl Scroller for TerminalState {
    fn bar(&self) -> &ScrollbarState {
        &self.bar
    }

    fn bar_mut(&mut self) -> &mut ScrollbarState {
        &mut self.bar
    }

    fn defaults(&self) -> Defaults {
        Defaults::Terminal
    }

    fn scroll_axes(&self) -> &'static [ScrollAxis] {
        &[ScrollAxis::Vertical]
    }

    fn scroll_offsets(&self) -> (f64, f64) {
        (0.0, self.scroll.current)
    }

    fn offset_anim(&mut self, _axis: ScrollAxis) -> &mut Animated<f64> {
        &mut self.scroll
    }
}

impl Scroller for VirtualListState {
    fn bar(&self) -> &ScrollbarState {
        &self.bar
    }

    fn bar_mut(&mut self) -> &mut ScrollbarState {
        &mut self.bar
    }

    fn defaults(&self) -> Defaults {
        Defaults::List
    }

    fn scroll_axes(&self) -> &'static [ScrollAxis] {
        &[ScrollAxis::Vertical]
    }

    fn scroll_offsets(&self) -> (f64, f64) {
        (0.0, self.scroll_offset.current)
    }

    fn offset_anim(&mut self, _axis: ScrollAxis) -> &mut Animated<f64> {
        &mut self.scroll_offset
    }
}

impl ScrollViewState {
    /// The axes this view scrolls along, `vertical` first.
    pub fn axes(&self) -> &'static [ScrollAxis] {
        if self.both {
            &ScrollAxis::BOTH
        } else if self.horizontal {
            &[ScrollAxis::Horizontal]
        } else {
            &[ScrollAxis::Vertical]
        }
    }

    /// The axis `scroll_offset` is the offset of.
    pub fn main_axis(&self) -> ScrollAxis {
        if self.horizontal {
            ScrollAxis::Horizontal
        } else {
            ScrollAxis::Vertical
        }
    }

    pub fn orientation(&self) -> ScrollOrientation {
        if self.both {
            ScrollOrientation::Both
        } else if self.horizontal {
            ScrollOrientation::Horizontal
        } else {
            ScrollOrientation::Vertical
        }
    }

    pub fn set_orientation(&mut self, orientation: ScrollOrientation) {
        self.horizontal = orientation == ScrollOrientation::Horizontal;
        self.both = orientation == ScrollOrientation::Both;
    }

    /// Where the content is scrolled to along `axis`: 0 for an axis the view
    /// does not scroll along.
    pub fn offset(&self, axis: ScrollAxis) -> f64 {
        axis.pick(self.offsets())
    }

    /// `(x, y)` -- how far the content is scrolled right and down.
    pub fn offsets(&self) -> (f64, f64) {
        self.offsets_at(self.scroll.current, self.scroll_x.current)
    }

    /// `offsets` as they would be with the main offset at `main` and
    /// `scroll_x` at `x`.
    pub fn offsets_at(&self, main: f64, x: f64) -> (f64, f64) {
        if self.both {
            (x, main)
        } else if self.horizontal {
            (main, 0.0)
        } else {
            (0.0, main)
        }
    }

    /// The animated offset along `axis` -- for an axis the view does not
    /// scroll along, the always-zero `scroll_x`.
    pub fn animated(&mut self, axis: ScrollAxis) -> &mut Animated<f64> {
        if axis == self.main_axis() {
            &mut self.scroll
        } else {
            &mut self.scroll_x
        }
    }

    /// The thumb's opacity now, off its emphasis `level`.
    pub fn thumb_opacity(&self, axis: ScrollAxis) -> f64 {
        self.metrics().opacity(self.bar.level[axis.index()].current)
    }

    /// Which axes of this view overflow: those whose content is longer than the
    /// viewport, as measured. `size` is the view's, `content` its content's,
    /// both `(width, height)`.
    pub fn overflows(&self, size: (f64, f64), content: (f64, f64)) -> [bool; 2] {
        let mut out = [false; 2];
        for &axis in self.axes() {
            out[axis.index()] = axis.pick(content) > axis.pick(size);
        }
        out
    }

    /// The room the gutter takes at the near edge (left, top) of the view, `(x, y)`:
    /// what the other axis' bar reserves there when it sits on that edge
    /// (`scrollbar_side_v` `left`, `scrollbar_side_h` `top`) and its axis
    /// overflows. The content starts that far in. `size` and `content` are as
    /// for `overflows` (the content as laid out, without this inset).
    pub fn near_insets(&self, size: (f64, f64), content: (f64, f64)) -> (f64, f64) {
        let m = self.metrics();
        let gutter = m.reserved_gutter();
        if gutter <= 0.0 {
            return (0.0, 0.0);
        }
        let over = self.overflows(size, content);
        let inset = |axis: ScrollAxis| {
            if over[axis.index()] && m.edge(axis) == Edge::Near {
                gutter
            } else {
                0.0
            }
        };
        // The vertical bar's gutter is at the left, the horizontal's at the top.
        (inset(ScrollAxis::Vertical), inset(ScrollAxis::Horizontal))
    }
}

/// How an animation to `to` over `duration` is started on `offset`: at once for
/// no time. (Scroll offsets are driven directly and never ticked centrally
/// unless an animation runs on them.)
pub fn animate_offset(
    offset: &mut Animated<f64>,
    to: f64,
    duration: Duration,
    curve: MotionCurve,
    now: Instant,
) {
    if duration.is_zero() {
        offset.set_now(to);
    } else {
        offset.animate_to(to, duration, curve, now);
    }
}
