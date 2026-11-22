//! 0.6.11 (P13): a node's state layer on the pixels. The overlay is the layer's colour at its own
//! alpha over the node's fill and border, inside its rounded corners, under its children; a node
//! with no layer showing paints exactly as before; and a change of the shown colour (a fade step)
//! is damage that a partial redraw paints the same as a full one (run with
//! `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check as well).

use engine_core::{Animated, NodeId, NodeKind, PaintProperties, Tree};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;

mod support;
use support::{BACKGROUND, Frame, RED, close, layout, placed, rgba, scene};

const FILL: Color = Color::from_rgba8(200, 200, 200, 255);
/// Black at 8% (the hover layer of the UX spec): alpha 20 of 255.
const HOVER: Color = Color::from_rgba8(0, 0, 0, 20);

/// A 60x60 box at (20, 20), with a 4px white border and a 20px radius if asked, and the layer
/// showing `shown` (as a fade would leave it).
fn button(tree: &mut Tree, root: NodeId, shown: Option<Color>, radius: f64) -> NodeId {
    let mut paint = PaintProperties::new(FILL, radius, 1.0);
    paint.border_color = Animated::new(Color::from_rgba8(255, 255, 255, 255));
    paint.border_width = Animated::new(4.0);
    let id = tree.insert(NodeKind::Rect, placed(20.0, 20.0, 60.0, 60.0), paint);
    tree.add_child(root, id);
    if let Some(shown) = shown {
        show(tree, id, shown);
    }
    id
}

fn show(tree: &mut Tree, id: NodeId, shown: Color) {
    tree.edit_state_layer(id, |l| l.shown.set_now(shown));
}

/// What `under` becomes with the hover layer over it: black at alpha 20.
fn dimmed(under: u8) -> u8 {
    (f32::from(under) * (1.0 - 20.0 / 255.0)).round() as u8
}

#[test]
fn the_layer_paints_over_the_fill_and_the_border_inside_the_box() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        button(&mut tree, root, Some(HOVER), 0.0);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        let d = dimmed(200);
        assert!(
            close(frame.at(50, 50), [d, d, d, 255], 2),
            "{:?}",
            frame.at(50, 50)
        );
        // The border is drawn first, so the layer is over it too.
        let b = dimmed(255);
        assert!(
            close(frame.at(21, 50), [b, b, b, 255], 2),
            "{:?}",
            frame.at(21, 50)
        );
        // Outside the box, nothing.
        assert_eq!(frame.at(10, 10), rgba(BACKGROUND));
        assert_eq!(frame.at(90, 50), rgba(BACKGROUND));
    });
}

#[test]
fn the_layer_follows_the_rounded_corner() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        button(&mut tree, root, Some(HOVER), 20.0);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // The box's very corner is clipped away: still the window's background.
        assert_eq!(
            frame.at(21, 21),
            rgba(BACKGROUND),
            "no layer in the cut corner"
        );
        let d = dimmed(200);
        assert!(close(frame.at(50, 50), [d, d, d, 255], 2));
    });
}

#[test]
fn a_child_paints_over_the_layer() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let id = button(&mut tree, root, Some(HOVER), 0.0);
        let child = tree.insert(
            NodeKind::Rect,
            placed(10.0, 10.0, 20.0, 20.0),
            PaintProperties::new(RED, 0.0, 1.0),
        );
        tree.add_child(id, child);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // (35, 35) is inside the child (box origin 20 + 10 + 5): exactly the child's colour.
        assert_eq!(frame.at(35, 35), rgba(RED), "the layer is under the child");
        // Beside the child, the layer.
        let d = dimmed(200);
        assert!(close(frame.at(70, 70), [d, d, d, 255], 2));
    });
}

#[test]
fn no_layer_showing_paints_as_before() {
    pollster::block_on(async {
        let (mut plain, root) = scene();
        button(&mut plain, root, None, 20.0);
        layout(&mut plain, root);
        let before = Frame::of(&plain, root).await.data;

        // A node with a layer declared but faded out (alpha 0), and one never shown, are alike.
        let (mut declared, root) = scene();
        let id = button(&mut declared, root, None, 20.0);
        declared.edit_state_layer(id, |l| {
            l.hover = Some(HOVER);
            l.shown.set_now(Color::from_rgba8(0, 0, 0, 0));
        });
        layout(&mut declared, root);
        assert_eq!(Frame::of(&declared, root).await.data, before);
    });
}

// --- A change of the shown colour is damage, and a partial redraw equals a full draw ---

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
            label: Some("state layer test target"),
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

#[test]
fn each_step_of_a_fade_is_damage_and_a_partial_redraw_equals_a_full_draw() {
    let (device, queue) = pollster::block_on(support::device("state layer damage test device"));
    let (mut tree, root) = scene();
    let id = button(&mut tree, root, None, 20.0);
    tree.edit_state_layer(id, |l| l.hover = Some(HOVER));
    layout(&mut tree, root);

    let mut output = Output::new(&device, &queue);
    output.frame(&device, &queue, &tree, root, true);
    let rest = output.pixels(&device, &queue);

    // Nothing changed: no damage.
    assert_eq!(
        output.frame(&device, &queue, &tree, root, true),
        Damage::None
    );

    // The layer part way in, then in.
    for (alpha, step) in [(7u8, "part way"), (20, "in")] {
        show(&mut tree, id, Color::from_rgba8(0, 0, 0, alpha));
        let damage = output.frame(&device, &queue, &tree, root, true);
        assert_ne!(damage, Damage::None, "{step}: the shown colour changed");
        let partial = output.pixels(&device, &queue);
        assert_ne!(partial, rest, "{step}");
        let mut fresh = Output::new(&device, &queue);
        fresh.frame(&device, &queue, &tree, root, false);
        assert_eq!(
            partial,
            fresh.pixels(&device, &queue),
            "{step}: same as a full draw"
        );
    }

    // And out again: back to the frame before the layer.
    show(&mut tree, id, Color::from_rgba8(0, 0, 0, 0));
    assert_ne!(
        output.frame(&device, &queue, &tree, root, true),
        Damage::None
    );
    assert_eq!(output.pixels(&device, &queue), rest);
}
