//! 0.6.11 (UXREQ-008): the outline on the pixels. A ring drawn outside the border box, `width`
//! thick and `offset` away, concentric with the box's corners, above the node's own paint and its
//! children, with a transparent gap unless `gap_color` fills it; the scheme's `secondary` when
//! the node names no colour; the focus style only while focus is visible; clipped by an ancestor's
//! `clip_children` and not by the node's own; and a partial redraw after a focus change equals a
//! full draw (run with `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check too).

use engine_core::{
    NodeId, NodeKind, OutlineStyle, Outlines, PaintProperties, Scheme, SchemeColors, SchemeRole,
    Tree,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use peniko::kurbo::Rect;

mod support;
use support::{BACKGROUND, Frame, GREEN, RED, WHITE, close, layout, placed, rgba, scene};

const BLUE: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF);

/// A 40 by 40 green box at (30, 30), corner radius `radius`, with `outlines`.
fn boxed(tree: &mut Tree, root: NodeId, radius: f64, outlines: Option<Outlines>) -> NodeId {
    let mut paint = PaintProperties::new(GREEN, radius, 1.0);
    paint.outline = outlines.map(Box::new);
    let id = tree.insert(NodeKind::Container, placed(30.0, 30.0, 40.0, 40.0), paint);
    tree.add_child(root, id);
    id
}

fn ring(width: f64, offset: f64) -> OutlineStyle {
    OutlineStyle {
        width,
        offset,
        color: Some(WHITE),
        ..Default::default()
    }
}

fn plain(style: OutlineStyle) -> Option<Outlines> {
    Some(Outlines {
        always: style,
        ..Default::default()
    })
}

fn exact(frame: &Frame, x: u32, y: u32, color: Color) -> bool {
    close(frame.at(x, y), rgba(color), 1)
}

#[test]
fn the_ring_sits_outside_the_box_with_a_transparent_gap() {
    pollster::block_on(async {
        // Width 3, offset 2, corner radius 8 (UXREQ-008, AC-FOC-2).
        let (mut tree, root) = scene();
        boxed(&mut tree, root, 8.0, plain(ring(3.0, 2.0)));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;

        // The box is 30..70: the gap is 28..30, the ring 25..28 on every side.
        for (gap, ring_px, outside, inside) in [
            ((29, 50), (26, 50), (24, 50), (31, 50)),
            ((70, 50), (72, 50), (75, 50), (68, 50)),
            ((50, 29), (50, 26), (50, 24), (50, 31)),
            ((50, 70), (50, 72), (50, 75), (50, 68)),
        ] {
            assert!(
                exact(&frame, gap.0, gap.1, BACKGROUND),
                "gap {gap:?} shows what is behind"
            );
            assert!(
                exact(&frame, ring_px.0, ring_px.1, WHITE),
                "ring {ring_px:?}"
            );
            assert!(
                exact(&frame, outside.0, outside.1, BACKGROUND),
                "past the ring {outside:?}"
            );
            assert!(exact(&frame, inside.0, inside.1, GREEN), "box {inside:?}");
        }
    });
}

#[test]
fn the_ring_is_concentric_with_the_rounded_corner() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        boxed(&mut tree, root, 8.0, plain(ring(3.0, 2.0)));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // Corner arcs have their centre at (38, 38): the box's radius is 8, the ring's inner edge
        // 10 and its outer edge 13.
        assert!(
            exact(&frame, 25, 25, BACKGROUND),
            "the outer corner is rounded, not square"
        );
        assert!(
            exact(&frame, 29, 29, WHITE),
            "on the diagonal, 12 from the centre: ring"
        );
        assert!(
            exact(&frame, 27, 27, BACKGROUND),
            "14.8 from the centre: past the ring"
        );
        assert!(
            exact(&frame, 31, 31, BACKGROUND),
            "9.2 from the centre: the gap, outside the box"
        );
        assert!(exact(&frame, 34, 34, GREEN), "inside the box");
    });
}

#[test]
fn a_gap_color_fills_the_gap_and_only_the_gap() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let style = OutlineStyle {
            gap_color: Some(BLUE),
            ..ring(3.0, 2.0)
        };
        boxed(&mut tree, root, 8.0, plain(style));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            exact(&frame, 28, 50, BLUE) && exact(&frame, 29, 50, BLUE),
            "the gap is filled"
        );
        assert!(exact(&frame, 71, 50, BLUE) && exact(&frame, 50, 28, BLUE));
        assert!(exact(&frame, 26, 50, WHITE), "the ring is untouched");
        assert!(exact(&frame, 24, 50, BACKGROUND), "nothing past the ring");
        assert!(exact(&frame, 35, 50, GREEN), "nothing inside the box");
        assert!(
            exact(&frame, 31, 31, BLUE),
            "the gap follows the rounded corner too"
        );
    });
}

#[test]
fn the_radius_property_sets_the_inner_edge() {
    pollster::block_on(async {
        // A square box with a ring whose inner edge has radius 10: its corner is rounded.
        let (mut tree, root) = scene();
        let style = OutlineStyle {
            radius: Some(10.0),
            ..ring(3.0, 0.0)
        };
        boxed(&mut tree, root, 0.0, plain(style));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            exact(&frame, 27, 27, BACKGROUND),
            "outer corner (radius 13) is round"
        );
        assert!(
            exact(&frame, 28, 50, WHITE),
            "the ring touches the box at its sides"
        );
    });
}

#[test]
fn an_inward_ring_stays_inside_the_box() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        boxed(&mut tree, root, 0.0, plain(ring(3.0, -3.0)));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // The ring spans 3 px inward from the edge at 30: pixels 30 to 32.
        assert!(exact(&frame, 30, 50, WHITE), "inside the edge");
        assert!(exact(&frame, 32, 50, WHITE));
        assert!(
            exact(&frame, 34, 50, GREEN),
            "the box's own fill past the ring"
        );
        assert!(exact(&frame, 29, 50, BACKGROUND), "nothing outside the box");
    });
}

#[test]
fn width_zero_and_no_outline_paint_the_same_as_before() {
    pollster::block_on(async {
        let (mut bare, root) = scene();
        boxed(&mut bare, root, 8.0, None);
        layout(&mut bare, root);
        let reference = Frame::of(&bare, root).await;

        let (mut zero, root) = scene();
        boxed(&mut zero, root, 8.0, plain(ring(0.0, 2.0)));
        layout(&mut zero, root);
        let frame = Frame::of(&zero, root).await;
        assert_eq!(frame.data, reference.data);
    });
}

#[test]
fn with_no_colour_the_ring_is_the_scheme_role() {
    pollster::block_on(async {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let (mut tree, root) = scene();
            let style = OutlineStyle {
                color: None,
                ..ring(3.0, 2.0)
            };
            boxed(&mut tree, root, 8.0, plain(style));
            tree.set_scheme_colors(SchemeColors::baseline(scheme));
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert!(
                exact(&frame, 26, 50, SchemeRole::Secondary.baseline(scheme)),
                "{scheme:?}: the ring is `secondary`"
            );
        }
    });
}

#[test]
fn the_ring_is_above_the_nodes_children_and_outside_its_own_clip() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let id = boxed(&mut tree, root, 0.0, plain(ring(3.0, 2.0)));
        tree.get_mut(id).unwrap().paint.clip_children = true;
        // A child hanging out of the box over the ring's place, clipped by the box itself.
        let child = tree.insert(
            NodeKind::Rect,
            placed(-10.0, 10.0, 20.0, 20.0),
            PaintProperties::new(RED, 0.0, 1.0),
        );
        tree.add_child(id, child);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            exact(&frame, 26, 50, WHITE),
            "the ring survives the box's own clip"
        );
        assert!(
            exact(&frame, 33, 50, RED),
            "the child is clipped to the box"
        );
        assert!(
            exact(&frame, 29, 50, BACKGROUND),
            "and shows nothing in the gap"
        );
    });
}

#[test]
fn an_ancestors_clip_cuts_the_ring() {
    pollster::block_on(async {
        // The clipping parent spans 28..100, so the ring's left part (25..28) is cut.
        let (mut tree, root) = scene();
        let mut paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0);
        paint.clip_children = true;
        let clip = tree.insert(NodeKind::Container, placed(28.0, 0.0, 72.0, 100.0), paint);
        tree.add_child(root, clip);
        let mut paint = PaintProperties::new(GREEN, 0.0, 1.0);
        paint.outline = plain(ring(3.0, 2.0)).map(Box::new);
        let inner = tree.insert(NodeKind::Container, placed(2.0, 30.0, 40.0, 40.0), paint);
        tree.add_child(clip, inner);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(exact(&frame, 26, 50, BACKGROUND), "cut by the ancestor");
        assert!(
            exact(&frame, 50, 26, WHITE),
            "the rest of the ring is whole"
        );
    });
}

// --- the focus style, and partial redraw ---

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SIZE: u32 = 100;

struct Output {
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Output {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("outline test target"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = surface.create_view(&wgpu::TextureViewDescriptor::default());
        let mut renderer = WindowRenderer::new(device, queue, FORMAT, SIZE, SIZE, true);
        renderer.set_scale(1.0);
        Self {
            renderer,
            surface,
            view,
        }
    }

    fn frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &Tree,
        root: NodeId,
        partial: bool,
    ) -> Damage {
        let size = SIZE as u16;
        let damage = self
            .renderer
            .prepare(tree, root, size, size, partial, device, queue);
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            tree,
            root,
            size,
            size,
            &damage,
            device,
            queue,
            &mut encoder,
            &self.surface,
            &self.view,
        );
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        damage
    }

    fn pixels(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        support::read_texture(device, queue, &self.surface)
    }
}

fn pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let bytes_per_row = (SIZE * 4).next_multiple_of(256);
    let start = (y * bytes_per_row + x * 4) as usize;
    [
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ]
}

#[test]
fn the_focus_ring_follows_focus_visibility_and_partial_redraw_matches_a_full_draw() {
    let (device, queue) = pollster::block_on(support::device("outline focus test device"));
    let (mut tree, root) = scene();
    let focus = Outlines {
        focus: ring(3.0, 2.0),
        ..Default::default()
    };
    let id = boxed(&mut tree, root, 8.0, Some(focus));
    layout(&mut tree, root);

    let mut output = Output::new(&device, &queue);
    output.frame(&device, &queue, &tree, root, true);
    let unfocused = output.pixels(&device, &queue);
    assert_eq!(
        pixel(&unfocused, 26, 50),
        rgba(BACKGROUND),
        "no focus, no ring"
    );

    // Keyboard focus: the ring appears, and the damage reaches the whole ring.
    tree.set_focus_to(id);
    let damage = output.frame(&device, &queue, &tree, root, true);
    let Damage::Rects(rects) = &damage else {
        panic!("a focus change damages a rect, got {damage:?}");
    };
    let ring_box = Rect::new(25.0, 25.0, 75.0, 75.0);
    assert!(
        rects.iter().any(|r| r.contains_rect(ring_box)),
        "the damage {rects:?} covers the ring {ring_box:?}"
    );
    let shown = output.pixels(&device, &queue);
    assert_eq!(
        pixel(&shown, 26, 50),
        rgba(WHITE),
        "keyboard focus shows the ring"
    );
    let mut fresh = Output::new(&device, &queue);
    fresh.frame(&device, &queue, &tree, root, false);
    assert_eq!(shown, fresh.pixels(&device, &queue), "partial equals full");

    // A pointer press moved focus here: the ring is hidden again.
    tree.set_focus_visible(false);
    output.frame(&device, &queue, &tree, root, true);
    assert_eq!(
        output.pixels(&device, &queue),
        unfocused,
        "pointer focus has no ring"
    );

    // The next key turns it on.
    tree.set_focus_visible(true);
    output.frame(&device, &queue, &tree, root, true);
    assert_eq!(output.pixels(&device, &queue), shown);

    // Nothing changed: no damage.
    assert_eq!(
        output.frame(&device, &queue, &tree, root, true),
        Damage::None
    );

    // Losing focus clears the ring.
    tree.clear_focus();
    output.frame(&device, &queue, &tree, root, true);
    assert_eq!(output.pixels(&device, &queue), unfocused);
}
