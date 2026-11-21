//! 0.6.12: a path node's `shadows` follow the path as it is filled -- holes included -- instead of
//! its bounding box. Same headless render-to-texture-then-readback harness as `m95_paint.rs`.

use engine_core::{NodeId, NodeKind, PaintProperties, PathData, PathState, Shadow, Shadows, Tree};
use peniko::Color;

mod support;
use support::*;

/// An 80x80 ring (outer clockwise, inner counter-clockwise) placed at (10, 10): the band is 16
/// wide, so the hole is x and y in 26..74 on the target.
const RING: &str = "M0,0 L80,0 L80,80 L0,80 Z M16,16 L16,64 L64,64 L64,16 Z";

fn ring(shadows: Vec<Shadow>) -> (Tree, NodeId) {
    let (mut tree, root) = scene();
    let path = tree.insert(
        NodeKind::Path(PathState::new(PathData::from_svg(RING).unwrap())),
        placed(10.0, 10.0, 80.0, 80.0),
        PaintProperties::new(WHITE, 0.0, 1.0),
    );
    tree.add_child(root, path);
    tree.get_mut(path).unwrap().paint.shadows.current = Shadows(shadows);
    layout(&mut tree, root);
    (tree, root)
}

fn red_shadow(offset_x: f64, offset_y: f64, blur: f64, spread: f64) -> Shadow {
    Shadow {
        color: RED,
        offset_x,
        offset_y,
        blur,
        spread,
    }
}

#[test]
fn the_ring_itself_fills_its_band_and_leaves_its_hole() {
    pollster::block_on(async {
        let (tree, root) = ring(Vec::new());
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(10, 50), rgba(WHITE), "the band's first column");
        assert_eq!(frame.at(25, 50), rgba(WHITE), "the band's last column");
        assert_eq!(
            frame.at(26, 50),
            rgba(BACKGROUND),
            "the hole's first column"
        );
        assert_eq!(frame.at(50, 50), rgba(BACKGROUND), "the hole");
        assert_eq!(frame.at(74, 50), rgba(WHITE), "the right band");
        assert_eq!(frame.at(9, 50), rgba(BACKGROUND), "outside");
    });
}

#[test]
fn a_hard_shadow_is_the_ring_with_its_hole() {
    pollster::block_on(async {
        // Offset (30, 0): the band's shadow lands inside the hole and past the right side.
        let (tree, root) = ring(vec![red_shadow(30.0, 0.0, 0.0, 0.0)]);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(
            frame.at(50, 50),
            rgba(RED),
            "the shifted left band, in the hole"
        );
        assert_eq!(
            frame.at(35, 50),
            rgba(BACKGROUND),
            "the hole before the shifted band"
        );
        assert_eq!(
            frame.at(65, 50),
            rgba(BACKGROUND),
            "the shifted band's own hole"
        );
        assert_eq!(
            frame.at(95, 15),
            rgba(RED),
            "the shifted top band, past the node"
        );
        assert_eq!(frame.at(50, 5), rgba(BACKGROUND), "above the shadow");
        assert_eq!(
            frame.at(74, 50),
            rgba(WHITE),
            "the node paints over its shadow"
        );
    });
}

#[test]
fn a_blurred_shadow_leaves_the_hole_far_from_the_band_clear() {
    pollster::block_on(async {
        // Blur 8 is a standard deviation of 4; the hole is 48 wide, so its centre is 24 from the band.
        let (tree, root) = ring(vec![red_shadow(0.0, 0.0, 8.0, 0.0)]);
        let frame = Frame::of(&tree, root).await;
        let background = rgba(BACKGROUND);
        assert!(
            close(frame.at(50, 50), background, 2),
            "the centre of the hole keeps the window background, not a shadow: {:?}",
            frame.at(50, 50)
        );
        // Beside the band the shadow shows, fading with distance.
        let near = frame.at(28, 50)[0];
        let mid = frame.at(34, 50)[0];
        let far = frame.at(42, 50)[0];
        assert!(
            near > 60 && near > mid && mid > far,
            "{near} > {mid} > {far}"
        );
        // Outside the ring it falls off as a box shadow's edge does.
        let edge = frame.at(8, 50)[0];
        let beyond = frame.at(3, 50)[0];
        assert!(
            edge > 60 && edge > beyond,
            "outside the ring: {edge} > {beyond}"
        );
        assert!(frame.at(50, 2)[0] < 0x40, "past the blur");
    });
}

#[test]
fn a_positive_spread_grows_the_band_into_the_hole_and_outward() {
    pollster::block_on(async {
        let (tree, root) = ring(vec![red_shadow(0.0, 0.0, 0.0, 4.0)]);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(7, 50), rgba(RED), "grown past the outer edge");
        assert_eq!(frame.at(4, 50), rgba(BACKGROUND), "not past the spread");
        assert_eq!(frame.at(28, 50), rgba(RED), "grown into the hole");
        assert_eq!(
            frame.at(32, 50),
            rgba(BACKGROUND),
            "the hole keeps its middle"
        );
        assert_eq!(frame.at(50, 50), rgba(BACKGROUND));
    });
}

#[test]
fn a_negative_spread_narrows_the_band() {
    pollster::block_on(async {
        // The band's shadow, shifted 30 right and narrowed 4 each side: x 44..52 of the 40..56 band.
        let (tree, root) = ring(vec![red_shadow(30.0, 0.0, 0.0, -4.0)]);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(48, 50), rgba(RED), "the narrowed band");
        assert_eq!(frame.at(42, 50), rgba(BACKGROUND), "cut from the left side");
        assert_eq!(
            frame.at(54, 50),
            rgba(BACKGROUND),
            "cut from the right side"
        );
        // The cut stays in the shadow: the window background is intact elsewhere.
        assert_eq!(frame.at(2, 2), rgba(BACKGROUND));
    });
}

#[test]
fn a_stroke_only_path_casts_the_shadow_of_its_stroke() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let path = tree.insert(
            NodeKind::Path(PathState::new(PathData::from_svg("M10,20 L90,20").unwrap())),
            placed(0.0, 0.0, 100.0, 100.0),
            PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
        );
        tree.add_child(root, path);
        let node = tree.get_mut(path).unwrap();
        node.paint.border_color.current = WHITE;
        node.paint.border_width.current = 10.0;
        node.paint.shadows.current = Shadows(vec![red_shadow(0.0, 40.0, 0.0, 0.0)]);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(50, 20), rgba(WHITE), "the stroke");
        assert_eq!(frame.at(50, 60), rgba(RED), "its shadow, 40 below");
        assert_eq!(
            frame.at(50, 40),
            rgba(BACKGROUND),
            "not the path's whole box"
        );
        assert_eq!(frame.at(50, 90), rgba(BACKGROUND), "nor a box shadow");
    });
}
