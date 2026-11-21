//! 0.6.11 (UXREQ-008): paints a node's outline, the ring outside its border box.
//!
//! The ring is the area between two rounded rects, an inner edge `offset` past the box and an
//! outer edge `width` past that, filled with the even-odd rule so the hole stays transparent. A
//! `gap_color` fills the gap between the box and the inner edge. Drawn after the node's own
//! paint and its children (in `Painter::leave`), so it is above both, and outside the node's own
//! clip: it is still inside the clip layers of the node's ancestors, which cut it where they cut
//! everything else.

use engine_core::Outline;
use peniko::kurbo::{Affine, BezPath, Rect, RoundedRect, RoundedRectRadii, Shape};
use vello_gpu::Scene;

/// How far the gap fill reaches under the ring, so the ring's antialiased inner edge lies on
/// the gap colour and not on what is behind it.
const GAP_UNDERLAP: f64 = 0.5;

/// A rounded rect of `rect` with `radii` (top-left, top-right, bottom-right, bottom-left), each
/// radius kept within what the rect can hold. `None` for a rect with no area.
fn rounded(rect: Rect, radii: [f64; 4]) -> Option<BezPath> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let most = rect.width().min(rect.height()) / 2.0;
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0.0, most));
    Some(RoundedRect::from_rect(rect, RoundedRectRadii::new(tl, tr, br, bl)).to_path(0.1))
}

/// `radii` each grown by `by`, never below `0`.
fn grown(radii: [f64; 4], by: f64) -> [f64; 4] {
    radii.map(|r| if r > 0.0 { r + by } else { 0.0 })
}

/// Paints `outline` around a `w` by `h` box under `composed`. `corners` are the box's own
/// corner radii.
pub(crate) fn paint(
    outline: &Outline,
    w: f64,
    h: f64,
    corners: [f64; 4],
    composed: Affine,
    scene: &mut Scene,
) {
    let box_rect = Rect::new(0.0, 0.0, w, h);
    let inner_rect = box_rect.inflate(outline.offset, outline.offset);
    let outer_rect = box_rect.inflate(
        outline.offset + outline.width,
        outline.offset + outline.width,
    );
    let inner_radii = outline.inner_radii(corners);
    let Some(outer) = rounded(outer_rect, outline.outer_radii(corners)) else {
        return;
    };
    scene.set_transform(composed);
    scene.set_fill_rule(peniko::Fill::EvenOdd);

    // The gap, under the ring: from the box out to the ring's inner edge, grown a little under
    // the ring. Only outward rings have one.
    if let Some(gap_color) = outline.gap_color
        && outline.offset > 0.0
    {
        let under = GAP_UNDERLAP.min(outline.width / 2.0);
        let mut path = rounded(inner_rect.inflate(under, under), grown(inner_radii, under))
            .unwrap_or_default();
        if let Some(hole) = rounded(box_rect, corners) {
            path.extend(hole);
        }
        scene.set_paint(gap_color);
        scene.fill_path(&path);
    }

    let mut ring = outer;
    if let Some(hole) = rounded(inner_rect, inner_radii) {
        ring.extend(hole);
    }
    scene.set_paint(outline.color);
    scene.fill_path(&ring);
    scene.set_fill_rule(peniko::Fill::NonZero);
}
