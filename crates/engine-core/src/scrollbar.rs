//! 0.6.11 (UXREQ-001): the scroll view's scrollbar -- the geometry painting,
//! hit-testing, dragging and damage all read, so they cannot drift apart.
//!
//! The numbers of the Tesserae UX note "Scrollbar and Scroll Container" are
//! the defaults of a `both` view; a `vertical` or `horizontal` view keeps the
//! scrollbar it always had (4 px thumb, 6 px of slop, the colour's own alpha)
//! unless a property says otherwise.

use peniko::kurbo::{Point, Rect};
use std::time::Duration;

use crate::animation::Animated;
use crate::node::{
    SCROLLBAR_GRAB_SLOP, SCROLLBAR_MARGIN, SCROLLBAR_MIN_LENGTH, SCROLLBAR_THICKNESS,
    ScrollViewState,
};

/// How far one wheel notch scrolls, in pixels (`Lines` deltas; `Pixels`
/// deltas are taken as they come).
pub const WHEEL_NOTCH: f64 = 40.0;
/// How much of the viewport a page (Page Up/Down, a click on the lane)
/// scrolls: the viewport less a tenth, so a line of context stays.
pub const PAGE_FRACTION: f64 = 0.9;
/// A thumb changes opacity over this long (Motion `short2`).
pub const FADE: Duration = Duration::from_millis(100);
/// A press on the lane pages again after this long, then every `PAGE_REPEAT`.
pub const PAGE_DELAY: Duration = Duration::from_millis(400);
pub const PAGE_REPEAT: Duration = Duration::from_millis(100);

/// The spec's own numbers (`both` views).
pub const SPEC_THICKNESS: f64 = 8.0;
pub const SPEC_HIT: f64 = 12.0;
pub const SPEC_GUTTER: f64 = 12.0;
pub const SPEC_CORNER: f64 = 12.0;
pub const SPEC_OPACITY_REST: f64 = 0.70;
pub const SPEC_OPACITY_HOVER: f64 = 0.80;
pub const SPEC_OPACITY_DRAG: f64 = 1.0;

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

/// What the user set of a scrollbar; `None` is the default of the view's
/// orientation (`ScrollViewState::metrics`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarOverrides {
    pub thickness: Option<f64>,
    pub hit: Option<f64>,
    pub margin: Option<f64>,
    pub min_length: Option<f64>,
    pub gutter: Option<f64>,
    pub corner: Option<f64>,
    pub opacity: Option<f64>,
    pub opacity_hover: Option<f64>,
    pub opacity_drag: Option<f64>,
}

/// The names of the node properties that set the scrollbar's measures, in the
/// order of `ScrollbarOverrides`' fields (`slot`, `ScrollbarMetrics::get`).
pub const SCROLLBAR_PROPS: [&str; 9] = [
    "scrollbar_width",
    "scrollbar_hit_width",
    "scrollbar_margin",
    "scrollbar_min_length",
    "scrollbar_gutter",
    "scrollbar_corner",
    "scrollbar_opacity",
    "scrollbar_opacity_hover",
    "scrollbar_opacity_drag",
];

impl ScrollbarOverrides {
    /// The setting that property `name` (one of `SCROLLBAR_PROPS`) sets.
    pub fn slot(&mut self, name: &str) -> Option<&mut Option<f64>> {
        Some(match name {
            "scrollbar_width" => &mut self.thickness,
            "scrollbar_hit_width" => &mut self.hit,
            "scrollbar_margin" => &mut self.margin,
            "scrollbar_min_length" => &mut self.min_length,
            "scrollbar_gutter" => &mut self.gutter,
            "scrollbar_corner" => &mut self.corner,
            "scrollbar_opacity" => &mut self.opacity,
            "scrollbar_opacity_hover" => &mut self.opacity_hover,
            "scrollbar_opacity_drag" => &mut self.opacity_drag,
            _ => return None,
        })
    }
}
/// The measures a scrollbar is built from, resolved.
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
    /// Reserved at the end edge of an axis that overflows.
    pub gutter: f64,
    /// How much a lane stops short when the other axis overflows too.
    pub corner: f64,
    /// How far past the thumb's ends still grabs it.
    pub along_slop: f64,
    pub opacity_rest: f64,
    pub opacity_hover: f64,
    pub opacity_drag: f64,
}

impl ScrollbarMetrics {
    /// What property `name` (one of `SCROLLBAR_PROPS`) reads: the measure in force.
    pub fn get(&self, name: &str) -> Option<f64> {
        Some(match name {
            "scrollbar_width" => self.thickness,
            "scrollbar_hit_width" => self.hit,
            "scrollbar_margin" => self.margin,
            "scrollbar_min_length" => self.min_length,
            "scrollbar_gutter" => self.gutter,
            "scrollbar_corner" => self.corner,
            "scrollbar_opacity" => self.opacity_rest,
            "scrollbar_opacity_hover" => self.opacity_hover,
            "scrollbar_opacity_drag" => self.opacity_drag,
            _ => return None,
        })
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
}

/// One thumb, with the regions around it, all in the view's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub axis: ScrollAxis,
    /// The painted thumb.
    pub thumb: Rect,
    /// Where a press grabs the thumb.
    pub hit: Rect,
    /// The strip along the axis a press pages in (the thumb is in it).
    pub lane: Rect,
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
}

/// `rect` holds `point`, edges included.
pub fn holds(rect: Rect, point: Point) -> bool {
    (rect.x0..=rect.x1).contains(&point.x) && (rect.y0..=rect.y1).contains(&point.y)
}

/// `(track, thumb length, along)` of a thumb for a `viewport` over `content`
/// at `offset`: the lane is the viewport less the margins and `reserve`; the
/// thumb is `max(min_length, lane * viewport / content)`, never longer than
/// the lane, and sits along it by how far the content has scrolled.
pub fn axis_geometry(
    m: &ScrollbarMetrics,
    viewport: f64,
    content: f64,
    offset: f64,
    reserve: f64,
) -> (f64, f64, f64) {
    let track = viewport - m.margin * 2.0 - reserve;
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
        offset / max_scroll
    } else {
        0.0
    };
    (track, thumb, m.margin + (track - thumb) * progress)
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

    /// The scrollbar's measures: what the user set, else the defaults of the
    /// view's orientation.
    pub fn metrics(&self) -> ScrollbarMetrics {
        let o = &self.scrollbar;
        let spec = self.both;
        let thickness = o.thickness.unwrap_or(if spec {
            SPEC_THICKNESS
        } else {
            SCROLLBAR_THICKNESS
        });
        let opacity_rest = o
            .opacity
            .unwrap_or(if spec { SPEC_OPACITY_REST } else { 1.0 });
        ScrollbarMetrics {
            thickness,
            hit: o.hit.unwrap_or(if spec {
                SPEC_HIT
            } else {
                thickness + SCROLLBAR_GRAB_SLOP * 2.0
            }),
            margin: o.margin.unwrap_or(SCROLLBAR_MARGIN),
            min_length: o.min_length.unwrap_or(SCROLLBAR_MIN_LENGTH),
            gutter: o.gutter.unwrap_or(if spec { SPEC_GUTTER } else { 0.0 }),
            corner: o.corner.unwrap_or(if spec { SPEC_CORNER } else { 0.0 }),
            along_slop: if spec { 0.0 } else { SCROLLBAR_GRAB_SLOP },
            opacity_rest,
            opacity_hover: o.opacity_hover.unwrap_or(if spec {
                SPEC_OPACITY_HOVER
            } else {
                opacity_rest
            }),
            opacity_drag: o.opacity_drag.unwrap_or(if spec {
                SPEC_OPACITY_DRAG
            } else {
                opacity_rest
            }),
        }
    }

    /// The thumb's opacity now, off its emphasis `level` (see
    /// `ScrollViewState::level`).
    pub fn thumb_opacity(&self, axis: ScrollAxis) -> f64 {
        self.metrics().opacity(self.level[axis.index()].current)
    }

    /// Which axes of this view show a thumb: those whose content is longer
    /// than the viewport, as measured. `size` is the view's, `content` its
    /// content's, both `(width, height)`.
    pub fn overflows(&self, size: (f64, f64), content: (f64, f64)) -> [bool; 2] {
        let mut out = [false; 2];
        for &axis in self.axes() {
            out[axis.index()] = axis.pick(content) > axis.pick(size);
        }
        out
    }

    /// The thumbs, `[vertical, horizontal]`, for a view of `size` over
    /// `content` scrolled to `offsets` (all `(x, y)`): one per axis that
    /// overflows, each lane stopping `corner` short of the end when the other
    /// axis has a thumb too, so the two never meet.
    pub fn thumbs(
        &self,
        size: (f64, f64),
        content: (f64, f64),
        offsets: (f64, f64),
    ) -> [Option<Thumb>; 2] {
        let m = self.metrics();
        let over = self.overflows(size, content);
        let mut bars = [None, None];
        for &axis in self.axes() {
            if !over[axis.index()] {
                continue;
            }
            let (viewport, extent, offset) =
                (axis.pick(size), axis.pick(content), axis.pick(offsets));
            let reserve = if over[axis.other().index()] {
                m.corner
            } else {
                0.0
            };
            let (track, length, along) = axis_geometry(&m, viewport, extent, offset, reserve);
            if track <= 0.0 {
                continue;
            }
            let across_end = match axis {
                ScrollAxis::Vertical => size.0,
                ScrollAxis::Horizontal => size.1,
            } - m.margin;
            let (across_start, centre) = (across_end - m.thickness, across_end - m.thickness / 2.0);
            let (hit_a, hit_b) = (centre - m.hit / 2.0, centre + m.hit / 2.0);
            let (thumb, hit, lane) = match axis {
                ScrollAxis::Vertical => (
                    Rect::new(across_start, along, across_end, along + length),
                    Rect::new(
                        hit_a,
                        along - m.along_slop,
                        hit_b,
                        along + length + m.along_slop,
                    ),
                    Rect::new(hit_a, m.margin, hit_b, m.margin + track),
                ),
                ScrollAxis::Horizontal => (
                    Rect::new(along, across_start, along + length, across_end),
                    Rect::new(
                        along - m.along_slop,
                        hit_a,
                        along + length + m.along_slop,
                        hit_b,
                    ),
                    Rect::new(m.margin, hit_a, m.margin + track, hit_b),
                ),
            };
            bars[axis.index()] = Some(Thumb {
                axis,
                thumb,
                hit,
                lane,
                track,
                length,
                max_scroll: (extent - viewport).max(0.0),
            });
        }
        bars
    }
}
