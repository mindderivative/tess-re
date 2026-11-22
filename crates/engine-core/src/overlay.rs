//! §11.3's overlay mechanism (§14 step 13): menus, tooltips, and
//! dialogs are all the same primitive -- tree-resident, not a parallel
//! structure. M96's layers (`window.show_layer`) are built on it.
//! `OverlayMeta` is the *only* new bookkeeping this needs -- everything else
//! (positioning, paint order, hit-testing, the focus model,
//! `build_access_update()`) reuses machinery that already exists,
//! per §11.3's own text, which `Tree::open_overlay`/`close_overlay`
//! (in `tree/tests.rs`) exist to prove for real, not merely assert.

use crate::node::NodeId;

/// Matches §11.3's own struct sketch exactly. `dismiss_on_outside_click`/
/// `dismiss_on_escape` are real data acted on by `Tree::dispatch`'s own
/// real `PointerPressed`/`KeyPressed` arms (M10 Phase 1) -- real
/// pointer/keyboard `InputEvent` dispatch and hit-testing (§11.10) have
/// existed since M4/M5, respectively; this field waited on `Tree::
/// dismiss_overlays_outside`/`dismiss_escapable_overlays` actually
/// being written, not on the dispatch mechanism itself.
/// M96: the side of its anchor a layer prefers (`OverlayMeta.placement`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    #[default]
    Below,
    Above,
    /// Before the anchor on the horizontal axis (left, in left-to-right).
    Start,
    /// After the anchor on the horizontal axis.
    End,
    /// 0.5.7 (#179): the node's centre on the anchor's top-start corner, for
    /// a badge or a marker. This and the three below, and `Center`, name a
    /// *point* of the anchor and never flip or shift to fit.
    TopStart,
    TopEnd,
    BottomStart,
    BottomEnd,
    /// The node's centre on the anchor's centre.
    Center,
}

impl Placement {
    pub fn opposite(self) -> Self {
        match self {
            Self::Below => Self::Above,
            Self::Above => Self::Below,
            Self::Start => Self::End,
            Self::End => Self::Start,
            // A point has no other side.
            point => point,
        }
    }

    /// 0.5.7 (#179): whether this names a point of the anchor (a corner or
    /// its centre) rather than a side of it.
    pub fn is_point(self) -> bool {
        matches!(
            self,
            Self::TopStart | Self::TopEnd | Self::BottomStart | Self::BottomEnd | Self::Center
        )
    }

    /// 0.5.7 (#179): where a node `w` x `h` goes against `anchor`, as its
    /// top-left corner, before any offset. The four sides put the node outside
    /// the anchor, aligned to the anchor's start edge; a point puts the node's
    /// centre on that point.
    pub fn origin(self, anchor: AnchorBox, w: f64, h: f64) -> (f64, f64) {
        self.origin_aligned(anchor, w, h, Align::Start)
    }

    /// 0.6.12 (P10): [`origin`](Self::origin) with the node aligned to the anchor's `align` edge
    /// (or its middle) on the axis along the anchor's side; a point ignores it.
    pub fn origin_aligned(self, anchor: AnchorBox, w: f64, h: f64, align: Align) -> (f64, f64) {
        let AnchorBox { x, y, w: aw, h: ah } = anchor;
        let centred = |cx: f64, cy: f64| (cx - w / 2.0, cy - h / 2.0);
        match self {
            Self::Below => (align.along(x, aw, w), y + ah),
            Self::Above => (align.along(x, aw, w), y - h),
            Self::Start => (x - w, align.along(y, ah, h)),
            Self::End => (x + aw, align.along(y, ah, h)),
            Self::TopStart => centred(x, y),
            Self::TopEnd => centred(x + aw, y),
            Self::BottomStart => centred(x, y + ah),
            Self::BottomEnd => centred(x + aw, y + ah),
            Self::Center => centred(x + aw / 2.0, y + ah / 2.0),
        }
    }
}

/// 0.6.12 (P10): which edge of the anchor a layer placed on one of its sides lines up with, on
/// the axis along that side: `Start` (left or top, the default and what layers always did),
/// `Center` or `End` (right or bottom).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

impl Align {
    /// Where a node of `extent` begins along an axis, against an anchor spanning
    /// `[start, start + anchor_extent]`.
    fn along(self, start: f64, anchor_extent: f64, extent: f64) -> f64 {
        match self {
            Self::Start => start,
            Self::Center => start + (anchor_extent - extent) / 2.0,
            Self::End => start + anchor_extent - extent,
        }
    }
}

/// 0.6.12 (P10): a distance for each side of a window, in layout pixels (`viewport_margin`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Edges {
    /// The same distance on every side.
    pub fn all(distance: f64) -> Self {
        Self {
            left: distance,
            top: distance,
            right: distance,
            bottom: distance,
        }
    }
}

/// 0.6.12 (P10): the window-wide policy a layer follows when it sets none of its own
/// (`Window.layer_viewport_margin`, `Window.layer_flip_threshold`). The defaults are what layers
/// always did: no margin, no slack.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayerDefaults {
    pub viewport_margin: Edges,
    pub flip_threshold: f64,
}

/// 0.6.12 (P10): everything [`place`] needs to put a layer against its anchor.
#[derive(Clone, Copy, Debug)]
pub struct PlaceInput {
    /// The window's size.
    pub window: (f64, f64),
    /// The anchor's box in window space.
    pub target: AnchorBox,
    /// The layer's size.
    pub size: (f64, f64),
    /// The anchor offset, already in window space.
    pub offset: (f64, f64),
    pub preferred: Placement,
    pub align: Align,
    /// The layer may go to the opposite side when the preferred one lacks room.
    pub flip: bool,
    /// The layer is moved to keep inside the window (less the margin).
    pub shift: bool,
    /// The distance a flipped or shifted layer keeps from each window edge.
    pub margin: Edges,
    /// The extra free space the preferred side must have beyond the layer's own size before the
    /// layer stays on it.
    pub flip_threshold: f64,
}

/// 0.6.12 (P10): where a layer goes, as its top-left corner in window space, and the side it ended
/// on. The room on a side is measured to the window edge less the viewport margin on that side; the
/// layer flips when the preferred side's room is below the layer's size plus the threshold and the
/// opposite side has more; after that it is moved along both axes to keep the margin. A point
/// placement never flips or shifts.
pub fn place(input: &PlaceInput) -> (f64, f64, Placement) {
    let PlaceInput {
        window,
        target,
        size: (w, h),
        offset: (dx, dy),
        preferred,
        align,
        flip,
        shift,
        margin,
        flip_threshold,
    } = *input;
    let room = |side: Placement| match side {
        Placement::Below => window.1 - margin.bottom - (target.y + target.h),
        Placement::Above => target.y - margin.top,
        Placement::Start => target.x - margin.left,
        Placement::End => window.0 - margin.right - (target.x + target.w),
        _ => f64::INFINITY,
    };
    let needed = |side: Placement| match side {
        Placement::Below | Placement::Above => h,
        _ => w,
    };
    let side_placed = !preferred.is_point();
    let side = if flip
        && side_placed
        && room(preferred) < needed(preferred) + flip_threshold
        && room(preferred.opposite()) > room(preferred)
    {
        preferred.opposite()
    } else {
        preferred
    };
    let (x, y) = side.origin_aligned(target, w, h, align);
    let (x, y) = (x + dx, y + dy);
    if shift && side_placed {
        let fit = |at: f64, extent: f64, limit: f64, low: f64, high: f64| {
            at.min(limit - extent - high).max(low)
        };
        (
            fit(x, w, window.0, margin.left, margin.right),
            fit(y, h, window.1, margin.top, margin.bottom),
            side,
        )
    } else {
        (x, y, side)
    }
}

/// 0.5.7 (#179): an anchor's box in window space (after its transform), the
/// rectangle a node is placed against.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnchorBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One open overlay: a legacy one (`Tree::open_overlay`), which closes
/// itself on dismissal, or an M96 layer (`Tree::show_layer`), which reports
/// dismissal instead. `Tree` keeps them in stacking order, newest on top.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OverlayMeta {
    /// Positions the overlay's root relative to this node's computed
    /// bounds. Always set for a legacy overlay; a layer without one sits
    /// at its own `x`/`y`.
    pub anchor: Option<NodeId>,
    pub dismiss_on_outside_click: bool,
    pub dismiss_on_escape: bool,
    /// M30 Phase 4 Step 1 (§11.3): a real modal dialog must block
    /// interaction with everything behind it -- a press outside a modal
    /// overlay never reaches the background, dismissing or not. `false`
    /// is a true no-op: an outside press falls through to normal
    /// hit-testing.
    pub modal: bool,
    /// M96: a layer that reports an outside press or Escape as a
    /// `dismiss` (`Tree::take_dismissals`) rather than closing itself --
    /// what closing means is the framework's call.
    pub dismissible: bool,
    /// M96: the side of `anchor` a layer prefers; placed at every layout,
    /// flipped or shifted to fit the window. `None` for legacy overlays,
    /// placed once when opened.
    pub placement: Option<Placement>,
    /// M96: the side the last layout actually placed the layer on.
    pub placed: Option<Placement>,
    /// 0.5.7 (#179): moves the point of the anchor the layer attaches to, in
    /// the anchor's own coordinates (a scaled anchor scales it).
    pub offset: (f64, f64),
    /// 0.5.7 (#179): `true` never flips the layer to the other side. A placement that
    /// names a point of the anchor never does either way. `false`, the default, is what
    /// layers always did. (0.6.12, P10: flipping and shifting are separate; `show_layer(flip=False)`
    /// still turns off both unless `shift` says otherwise.)
    pub no_flip: bool,
    /// 0.6.12 (P10): `true` never moves the layer to keep it inside the window.
    pub no_shift: bool,
    /// 0.6.12 (P10): which edge of the anchor the layer lines up with.
    pub align: Align,
    /// 0.6.12 (P10): the distance the layer keeps from each window edge when it flips or shifts;
    /// `None` follows `Tree::layer_defaults`.
    pub viewport_margin: Option<Edges>,
    /// 0.6.12 (P10): the extra free space the preferred side needs before the layer stays on it;
    /// `None` follows `Tree::layer_defaults`.
    pub flip_threshold: Option<f64>,
    /// M96: the node that held focus when the layer opened, for
    /// `hide_layer` to hand focus back to.
    pub restore_focus: Option<NodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 200 x 100 window with a 20 x 10 anchor at `(ax, ay)` and a 50 x 40 layer.
    fn input(ax: f64, ay: f64, preferred: Placement) -> PlaceInput {
        PlaceInput {
            window: (200.0, 100.0),
            target: AnchorBox {
                x: ax,
                y: ay,
                w: 20.0,
                h: 10.0,
            },
            size: (50.0, 40.0),
            offset: (0.0, 0.0),
            preferred,
            align: Align::Start,
            flip: true,
            shift: true,
            margin: Edges::default(),
            flip_threshold: 0.0,
        }
    }

    #[test]
    fn the_defaults_are_the_old_rule() {
        // Room below 100 - 85 - 10 = 5 < 40, above 85 > 5: flips, then shifts inside.
        let (x, y, side) = place(&input(190.0, 85.0, Placement::Below));
        assert_eq!((x, y, side), (150.0, 45.0, Placement::Above));
        // Plenty of room below: stays, no shift.
        let (x, y, side) = place(&input(10.0, 10.0, Placement::Below));
        assert_eq!((x, y, side), (10.0, 20.0, Placement::Below));
    }

    #[test]
    fn the_viewport_margin_shifts_a_layer_off_every_edge() {
        let mut i = input(190.0, 10.0, Placement::Below);
        i.margin = Edges::all(8.0);
        let (x, y, side) = place(&i);
        // 200 - 50 - 8 on the right; below the anchor at 20 is inside 100 - 40 - 8 = 52.
        assert_eq!((x, y, side), (142.0, 20.0, Placement::Below));
        // The top and left margin hold a layer pushed out the other way.
        let mut i = input(0.0, 0.0, Placement::Above);
        i.margin = Edges {
            left: 3.0,
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
        };
        let (x, y, side) = place(&i);
        // No room above (0 - 6 < 0) and more below: flips; x stays at the left margin.
        assert_eq!((x, y, side), (3.0, 10.0, Placement::Below));
    }

    #[test]
    fn the_margin_counts_in_the_room_that_decides_a_flip() {
        // Room below 100 - 40 - 10 = 50 >= 40: stays without a margin...
        let i = input(10.0, 40.0, Placement::Below);
        assert_eq!(place(&i).2, Placement::Below);
        // ...but a 12 px bottom margin leaves 38 < 40 and the anchor has 40 above: flips.
        let mut i = input(10.0, 40.0, Placement::Below);
        i.margin.bottom = 12.0;
        assert_eq!(place(&i), (10.0, 0.0, Placement::Above));
    }

    #[test]
    fn a_flip_threshold_asks_for_free_space_beyond_the_layer() {
        // Room below 40 against 40 needed: fine with no slack, short with 15 (50 above).
        let mut i = input(10.0, 50.0, Placement::Below);
        assert_eq!(place(&i).2, Placement::Below);
        i.flip_threshold = 15.0;
        assert_eq!(place(&i).2, Placement::Above);
        // The opposite side must also have more: with less above, it stays.
        let mut i = input(10.0, 20.0, Placement::Below);
        i.window = (200.0, 60.0);
        i.flip_threshold = 15.0;
        // below 60 - 30 = 30 < 55, above 20 < 30: stays.
        assert_eq!(place(&i).2, Placement::Below);
    }

    #[test]
    fn flip_and_shift_switch_off_on_their_own() {
        let mut i = input(190.0, 85.0, Placement::Below);
        i.flip = false;
        // Not flipped, but still shifted inside the window.
        assert_eq!(place(&i), (150.0, 60.0, Placement::Below));
        let mut i = input(190.0, 85.0, Placement::Below);
        i.shift = false;
        assert_eq!(place(&i), (190.0, 45.0, Placement::Above));
        i.flip = false;
        assert_eq!(place(&i), (190.0, 95.0, Placement::Below));
    }

    #[test]
    fn a_point_never_flips_or_shifts_whatever_the_policy() {
        let mut i = input(0.0, 0.0, Placement::TopStart);
        i.margin = Edges::all(20.0);
        i.flip_threshold = 500.0;
        // Centred on the anchor's corner, outside the window if it lands there.
        assert_eq!(place(&i), (-25.0, -20.0, Placement::TopStart));
    }

    #[test]
    fn alignment_lines_the_layer_up_with_the_anchor_edge() {
        let at = |align| {
            let mut i = input(100.0, 20.0, Placement::Below);
            i.align = align;
            place(&i)
        };
        assert_eq!(at(Align::Start).0, 100.0);
        assert_eq!(at(Align::Center).0, 85.0);
        assert_eq!(at(Align::End).0, 70.0);
        // On a horizontal side the vertical axis aligns.
        let mut i = input(100.0, 20.0, Placement::End);
        i.align = Align::End;
        // End edges line up at 20 + 10 - 40 = -10, then the shift keeps it in the window.
        assert_eq!((place(&i).0, place(&i).1), (120.0, 0.0));
    }

    #[test]
    fn the_offset_is_added_before_the_shift() {
        let mut i = input(10.0, 10.0, Placement::Below);
        i.offset = (-30.0, 0.0);
        i.margin = Edges::all(4.0);
        assert_eq!(place(&i), (4.0, 20.0, Placement::Below));
    }
}
