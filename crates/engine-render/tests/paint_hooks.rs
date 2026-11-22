//! 0.6.12 (P9): a path's stroke style (caps, joins, miter limit, dashes), an image's sampling,
//! and `shadows` on every node kind. Same headless render-to-texture-then-readback harness as
//! `m95_paint.rs` (run with `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check too).

use engine_core::{
    CanvasState, ImageSampling, ImageState, NodeId, NodeKind, PaintProperties, PathData, PathState,
    Shadow, Shadows, TerminalState, Tree,
};
use peniko::kurbo::{Cap, Join};

mod support;
use support::*;

/// A 100x100 scene with one path node over all of it, stroked white `width` wide.
fn stroked(data: &str, width: f64, edit: impl FnOnce(&mut PathState)) -> (Tree, NodeId) {
    let (mut tree, root) = scene();
    let mut state = PathState::new(PathData::from_svg(data).unwrap());
    edit(&mut state);
    let path = tree.insert(
        NodeKind::Path(state),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(CLEAR, 0.0, 1.0),
    );
    tree.add_child(root, path);
    let paint = &mut tree.get_mut(path).unwrap().paint;
    paint.border_color.current = WHITE;
    paint.border_width.current = width;
    layout(&mut tree, root);
    (tree, root)
}

const LINE: &str = "M20,50 L80,50";
/// A right angle with its corner at (20, 20).
const CORNER: &str = "M20,80 L20,20 L80,20";

fn is_white(pixel: [u8; 4]) -> bool {
    close(pixel, rgba(WHITE), 2)
}

fn is_background(pixel: [u8; 4]) -> bool {
    close(pixel, rgba(BACKGROUND), 2)
}

#[test]
fn the_default_stroke_is_round_capped_and_round_joined() {
    pollster::block_on(async {
        let (tree, root) = stroked(LINE, 20.0, |_| {});
        let frame = Frame::of(&tree, root).await;
        assert!(
            is_white(frame.at(12, 50)),
            "a round cap reaches past the end"
        );
        assert!(
            is_background(frame.at(11, 41)),
            "...but not into the square's corner"
        );
        let (tree, root) = stroked(CORNER, 20.0, |_| {});
        let frame = Frame::of(&tree, root).await;
        assert!(is_background(frame.at(11, 11)), "a round join has no tip");
        assert!(is_white(frame.at(14, 14)), "inside the round join");
    });
}

#[test]
fn a_butt_cap_stops_at_the_end_and_a_square_cap_reaches_the_corners() {
    pollster::block_on(async {
        let (tree, root) = stroked(LINE, 20.0, |s| s.stroke_style.cap = Cap::Butt);
        let frame = Frame::of(&tree, root).await;
        assert!(
            is_background(frame.at(17, 50)),
            "butt: nothing past the end"
        );
        assert!(is_white(frame.at(22, 50)), "butt: the line itself");
        let (tree, root) = stroked(LINE, 20.0, |s| s.stroke_style.cap = Cap::Square);
        let frame = Frame::of(&tree, root).await;
        assert!(
            is_white(frame.at(12, 50)),
            "square: half a width past the end"
        );
        assert!(is_white(frame.at(11, 41)), "square: the corner is filled");
        assert!(is_background(frame.at(8, 50)), "square: no further");
    });
}

#[test]
fn a_miter_join_has_a_tip_a_bevel_cuts_and_the_limit_bevels_a_sharp_one() {
    pollster::block_on(async {
        let (tree, root) = stroked(CORNER, 20.0, |s| {
            s.stroke_style.join = Join::Miter;
            s.stroke_style.cap = Cap::Butt;
        });
        let frame = Frame::of(&tree, root).await;
        assert!(is_white(frame.at(11, 11)), "miter: the tip");
        let (tree, root) = stroked(CORNER, 20.0, |s| {
            s.stroke_style.join = Join::Bevel;
            s.stroke_style.cap = Cap::Butt;
        });
        let frame = Frame::of(&tree, root).await;
        assert!(is_background(frame.at(11, 11)), "bevel: the tip is cut");
        assert!(is_white(frame.at(16, 16)), "bevel: the inside is filled");
        // A right angle's miter is 1.41 half-widths long: a limit below that bevels it.
        let (tree, root) = stroked(CORNER, 20.0, |s| {
            s.stroke_style.join = Join::Miter;
            s.stroke_style.miter_limit = 1.2;
            s.stroke_style.cap = Cap::Butt;
        });
        let frame = Frame::of(&tree, root).await;
        assert!(is_background(frame.at(11, 11)), "a low miter limit bevels");
    });
}

#[test]
fn a_dash_pattern_alternates_and_its_offset_shifts_it() {
    pollster::block_on(async {
        let flat = |s: &mut PathState| {
            s.stroke_style.cap = Cap::Butt;
            s.stroke_style.dash = vec![20.0, 20.0];
        };
        let (tree, root) = stroked("M0,50 L100,50", 10.0, flat);
        let frame = Frame::of(&tree, root).await;
        assert!(is_white(frame.at(10, 50)), "first dash");
        assert!(is_background(frame.at(30, 50)), "first gap");
        assert!(is_white(frame.at(50, 50)), "second dash");
        assert!(is_background(frame.at(70, 50)), "second gap");
        let (tree, root) = stroked("M0,50 L100,50", 10.0, |s| {
            flat(s);
            s.stroke_style.dash_offset = 20.0;
        });
        let frame = Frame::of(&tree, root).await;
        assert!(
            is_background(frame.at(10, 50)),
            "offset: the gap comes first"
        );
        assert!(is_white(frame.at(30, 50)), "offset: then a dash");
        // An odd pattern repeats to an even one: [10] is 10 on, 10 off.
        let (tree, root) = stroked("M0,50 L100,50", 10.0, |s| {
            s.stroke_style.cap = Cap::Butt;
            s.stroke_style.dash = vec![10.0];
        });
        let frame = Frame::of(&tree, root).await;
        assert!(is_white(frame.at(5, 50)));
        assert!(is_background(frame.at(15, 50)));
        assert!(is_white(frame.at(25, 50)));
    });
}

/// A 2x1 image, red on the left and green on the right.
fn red_green() -> peniko::ImageData {
    peniko::ImageData {
        data: peniko::Blob::from(vec![255u8, 0, 0, 255, 0, 255, 0, 255]),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 2,
        height: 1,
    }
}

fn sampled(sampling: ImageSampling) -> (Tree, NodeId) {
    let (mut tree, root) = scene();
    let mut state = ImageState::new(red_green());
    state.sampling = sampling;
    let image = tree.insert(
        NodeKind::Image(state),
        placed(0.0, 0.0, 40.0, 40.0),
        PaintProperties::new(CLEAR, 0.0, 1.0),
    );
    tree.add_child(root, image);
    layout(&mut tree, root);
    (tree, root)
}

#[test]
fn nearest_sampling_keeps_hard_edges_and_linear_blends() {
    pollster::block_on(async {
        let (tree, root) = sampled(ImageSampling::Nearest);
        let frame = Frame::of(&tree, root).await;
        assert!(
            close(frame.at(18, 20), rgba(RED), 2),
            "{:?}",
            frame.at(18, 20)
        );
        assert!(
            close(frame.at(22, 20), rgba(GREEN), 2),
            "{:?}",
            frame.at(22, 20)
        );
        let (tree, root) = sampled(ImageSampling::Linear);
        let frame = Frame::of(&tree, root).await;
        let mix = frame.at(20, 20);
        assert!(
            mix[0] > 40 && mix[1] > 40,
            "linear blends the two across the seam: {mix:?}"
        );
        let (tree, root) = sampled(ImageSampling::Cubic);
        let frame = Frame::of(&tree, root).await;
        let mix = frame.at(20, 20);
        assert!(
            mix[0] > 40 && mix[1] > 40,
            "cubic blends across the seam too: {mix:?}"
        );
    });
}

/// A node of `kind` placed at (10, 30), 30x30, with a hard red shadow shifted 50 to the right.
/// The node itself paints nothing but `paint`'s, so the shadow is what lands at (75, 45).
fn shadowed(kind: NodeKind, mut paint: PaintProperties) -> (Tree, NodeId) {
    let (mut tree, root) = scene();
    paint.shadows.current = Shadows(vec![Shadow {
        color: RED,
        offset_x: 50.0,
        offset_y: 0.0,
        blur: 0.0,
        spread: 0.0,
    }]);
    let node = tree.insert(kind, placed(10.0, 30.0, 30.0, 30.0), paint);
    tree.add_child(root, node);
    layout(&mut tree, root);
    (tree, root)
}

fn clear() -> PaintProperties {
    PaintProperties::new(CLEAR, 0.0, 1.0)
}

async fn assert_shadow_lands(name: &str, tree: &Tree, root: NodeId) {
    let frame = Frame::of(tree, root).await;
    assert!(
        close(frame.at(75, 45), rgba(RED), 2),
        "{name}: the shadow's centre is {:?}",
        frame.at(75, 45)
    );
    assert!(
        is_background(frame.at(75, 70)),
        "{name}: below the shadow is clear"
    );
}

#[test]
fn shadows_cast_by_every_node_kind() {
    pollster::block_on(async {
        // A circle: a box with a corner radius of half its side.
        let mut circle = PaintProperties::new(WHITE, 15.0, 1.0);
        circle.border_width.current = 0.0;
        let (tree, root) = shadowed(NodeKind::Rect, circle);
        assert_shadow_lands("circle", &tree, root).await;
        let frame = Frame::of(&tree, root).await;
        assert!(
            is_background(frame.at(61, 31)),
            "the circle's shadow is round: its box's corner is clear"
        );
        assert!(
            close(frame.at(75, 31), rgba(RED), 2) || !is_background(frame.at(75, 31)),
            "the circle's shadow reaches its top"
        );

        // A ring: the same with a border and no fill; the shadow is the whole disc's, as a box's.
        let mut ring = PaintProperties::new(CLEAR, 15.0, 1.0);
        ring.border_color.current = WHITE;
        ring.border_width.current = 4.0;
        let (tree, root) = shadowed(NodeKind::Rect, ring);
        assert_shadow_lands("ring", &tree, root).await;

        let mut state = ImageState::new(red_green());
        state.sampling = ImageSampling::Nearest;
        let (tree, root) = shadowed(NodeKind::Image(state), clear());
        assert_shadow_lands("image", &tree, root).await;

        let (tree, root) = shadowed(NodeKind::Canvas(CanvasState::new()), clear());
        assert_shadow_lands("canvas", &tree, root).await;

        let state = TerminalState::new(4, 2, "Hack Nerd Font Mono", 12.0);
        let (tree, root) = shadowed(NodeKind::Terminal(Box::new(state)), clear());
        assert_shadow_lands("terminal", &tree, root).await;

        let (tree, root) = shadowed(NodeKind::Container, clear());
        assert_shadow_lands("container", &tree, root).await;
    });
}
