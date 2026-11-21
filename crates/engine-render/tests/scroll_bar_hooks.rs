//! 0.6.11 (P1): the side, radius, track and arrows of a scrollbar on the pixels, and the partial
//! redraw of a scroll with them against a full draw (the block copy and the bar's damage).

use engine_core::{
    NodeId, NodeKind, PaintProperties, PropValue, ScrollOrientation, ScrollProp, ScrollViewState,
    Tree,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Position, Rect as TaffyRect, Size, Style, auto, length};

mod support;

const W: f32 = 260.0;
const H: f32 = 180.0;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The view: left, top, width, height.
const VIEW: (f32, f32, f32, f32) = (20.0, 20.0, 200.0, 120.0);
/// The content it scrolls over, and its tiles.
const CONTENT: (f32, f32) = (600.0, 420.0);
const TILE: f32 = 60.0;
const SURFACE: Color = Color::from_rgba8(25, 25, 30, 255);
const VIEW_FILL: Color = Color::from_rgba8(20, 20, 24, 255);

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn new() -> Self {
        let (device, queue) = pollster::block_on(support::device("two-axis scroll test device"));
        Self { device, queue }
    }
}

struct Window {
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    surface_view: wgpu::TextureView,
}

impl Window {
    fn new(gpu: &Gpu) -> Self {
        let surface = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stand-in swapchain image"),
            size: wgpu::Extent3d {
                width: W as u32,
                height: H as u32,
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
        let surface_view = surface.create_view(&wgpu::TextureViewDescriptor::default());
        let mut renderer = WindowRenderer::new(&gpu.device, FORMAT, W as u32, H as u32, true);
        renderer.set_scale(1.0);
        // The scenes are small: always use the shift, to test it.
        renderer.set_scroll_blit_min_nodes(0);
        Self {
            renderer,
            surface,
            surface_view,
        }
    }

    fn frame(&mut self, gpu: &Gpu, tree: &Tree, root: NodeId, partial: bool) -> Damage {
        let (width, height) = (W as u16, H as u16);
        let damage =
            self.renderer
                .prepare(tree, root, width, height, partial, &gpu.device, &gpu.queue);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            tree,
            root,
            width,
            height,
            &damage,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &self.surface,
            &self.surface_view,
        );
        gpu.queue.submit([encoder.finish()]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        damage
    }

    fn pixels(&self, gpu: &Gpu) -> Vec<u8> {
        support::read_texture(&gpu.device, &gpu.queue, &self.surface)
    }
}

fn placed(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style {
        position: Position::Absolute,
        inset: TaffyRect {
            left: length(x),
            top: length(y),
            right: auto(),
            bottom: auto(),
        },
        size: Size {
            width: length(w),
            height: length(h),
        },
        ..Default::default()
    }
}

fn layout(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(W),
            height: AvailableSpace::Definite(H),
        },
    );
}

fn shade(i: usize) -> Color {
    Color::from_rgba8(
        (40 + (i * 37) % 200) as u8,
        (60 + (i * 91) % 180) as u8,
        (90 + (i * 53) % 150) as u8,
        255,
    )
}

struct World {
    tree: Tree,
    root: NodeId,
    view: NodeId,
}

/// An opaque both-axes scroll view over a grid of coloured tiles, in a window of its own colour.
fn world(settings: &[(ScrollProp, PropValue)]) -> World {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, W, H),
        PaintProperties::new(SURFACE, 0.0, 1.0),
    );
    let mut state = ScrollViewState::new(false);
    state.set_orientation(ScrollOrientation::Both);
    for (prop, value) in settings {
        assert!(state.bar.settings.set(*prop, Some(*value)));
    }
    let (x, y, w, h) = VIEW;
    let view = tree.insert(
        NodeKind::ScrollView(state),
        placed(x, y, w, h),
        PaintProperties::new(VIEW_FILL, 0.0, 1.0),
    );
    tree.add_child(root, view);
    let content = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(CONTENT.0),
                height: length(CONTENT.1),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(view, content);
    let (cols, rows) = ((CONTENT.0 / TILE) as usize, (CONTENT.1 / TILE) as usize);
    for row in 0..rows {
        for col in 0..cols {
            let tile = tree.insert(
                NodeKind::Rect,
                placed(col as f32 * TILE, row as f32 * TILE, TILE - 2.0, TILE - 2.0),
                PaintProperties::new(shade(row * cols + col), 0.0, 1.0),
            );
            tree.add_child(content, tile);
        }
    }
    layout(&mut tree, root);
    World { tree, root, view }
}

fn state_of(world: &mut World) -> &mut ScrollViewState {
    let NodeKind::ScrollView(state) = &mut world.tree.get_mut(world.view).unwrap().kind else {
        unreachable!()
    };
    state
}

fn scroll_to(world: &mut World, x: f64, y: f64) {
    let state = state_of(world);
    state.scroll_x.current = x;
    state.scroll.current = y;
    layout(&mut world.tree, world.root);
}

fn rgba(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let stride = (W as u32 * 4).next_multiple_of(256);
    let at = (y * stride + x * 4) as usize;
    [data[at], data[at + 1], data[at + 2], data[at + 3]]
}

fn bytes(color: Color) -> [u8; 4] {
    color.to_rgba8().to_u8_array()
}

/// `over` at `alpha` on top of `under`, as the renderer blends (within rounding).
fn blended(over: Color, alpha: f32, under: [u8; 4]) -> [u8; 3] {
    let o = bytes(over);
    let mix =
        |c: usize| (f32::from(o[c]) * alpha + f32::from(under[c]) * (1.0 - alpha)).round() as u8;
    [mix(0), mix(1), mix(2)]
}

fn near(got: [u8; 4], want: [u8; 3], context: &str) {
    for c in 0..3 {
        assert!(
            got[c].abs_diff(want[c]) <= 2,
            "{context}: got {got:?}, want {want:?}"
        );
    }
}

/// What a fresh renderer draws for `tree` in full.
fn fresh(gpu: &Gpu, tree: &Tree, root: NodeId) -> Vec<u8> {
    let mut window = Window::new(gpu);
    window.frame(gpu, tree, root, false);
    window.pixels(gpu)
}

fn assert_same(got: &[u8], want: &[u8], context: &str) {
    if got == want {
        return;
    }
    let stride = (W as usize * 4).next_multiple_of(256);
    let (mut count, mut first) = (0, None);
    for (i, (a, b)) in got.chunks(4).zip(want.chunks(4)).enumerate() {
        if a != b {
            count += 1;
            first.get_or_insert((i % (stride / 4), i / (stride / 4), a.to_vec(), b.to_vec()));
        }
    }
    if count > 0 {
        panic!("{context}: {count} pixels differ; first {first:?}");
    }
}

/// The same scene with the bar invisible: what is under it.
/// The layout stays the one `settings` make (a bar on the near edge shifts the content).
fn bare(settings: &[(ScrollProp, PropValue)]) -> World {
    let mut quiet: Vec<_> = settings
        .iter()
        .filter(|(prop, _)| !matches!(prop, ScrollProp::TrackFill | ScrollProp::TrackOpacity))
        .copied()
        .collect();
    quiet.extend([
        (ScrollProp::Opacity, PropValue::Number(0.0)),
        (ScrollProp::OpacityHover, PropValue::Number(0.0)),
        (ScrollProp::OpacityDrag, PropValue::Number(0.0)),
    ]);
    world(&quiet)
}

fn choice(prop: ScrollProp, name: &str) -> (ScrollProp, PropValue) {
    let index = prop
        .choices()
        .unwrap()
        .iter()
        .position(|n| *n == name)
        .unwrap();
    (prop, PropValue::Choice(index as u8))
}

/// The light scheme's `on_surface_variant` (0.6.11 P4): what a bar with no `scrollbar_fill` paints.
const THUMB: Color = Color::from_rgba8(0x49, 0x45, 0x4F, 0xFF);

/// A frame of `world` and of the bare scene: `(with, under)`.
fn pair(gpu: &Gpu, settings: &[(ScrollProp, PropValue)]) -> (Vec<u8>, Vec<u8>) {
    let (with, under) = (world(settings), bare(settings));
    (
        fresh(gpu, &with.tree, with.root),
        fresh(gpu, &under.tree, under.root),
    )
}

/// View coordinates `(x, y)` in the window.
fn win(x: u32, y: u32) -> (u32, u32) {
    (VIEW.0 as u32 + x, VIEW.1 as u32 + y)
}

fn painted(with: &[u8], under: &[u8], view: (u32, u32), alpha: f32, context: &str) {
    let (x, y) = win(view.0, view.1);
    near(
        rgba(with, x, y),
        blended(THUMB, alpha, rgba(under, x, y)),
        context,
    );
}

fn bare_at(with: &[u8], under: &[u8], view: (u32, u32), context: &str) {
    let (x, y) = win(view.0, view.1);
    assert_eq!(rgba(with, x, y), rgba(under, x, y), "{context}");
}

#[test]
fn a_vertical_bar_on_the_left_and_a_horizontal_one_on_top_are_drawn_there() {
    let gpu = Gpu::new();
    // By default: right edge x 190..198, bottom edge y 110..118 of the 200 x 120 view.
    let (with, under) = pair(&gpu, &[]);
    painted(&with, &under, (194, 18), 0.7, "vertical at the right");
    bare_at(&with, &under, (6, 18), "nothing at the left");
    painted(&with, &under, (20, 114), 0.7, "horizontal at the bottom");
    bare_at(&with, &under, (20, 6), "nothing on top");
    let (with, under) = pair(
        &gpu,
        &[
            choice(ScrollProp::SideV, "start"),
            choice(ScrollProp::SideH, "start"),
        ],
    );
    painted(&with, &under, (6, 40), 0.7, "vertical at the left");
    bare_at(&with, &under, (194, 40), "none at the right");
    painted(&with, &under, (40, 6), 0.7, "horizontal on top");
    bare_at(&with, &under, (40, 114), "none at the bottom");
    // The corner at the top left is nobody's.
    bare_at(&with, &under, (6, 6), "the corner");
}

#[test]
fn the_radius_is_a_pill_by_default_and_a_property() {
    let gpu = Gpu::new();
    let (with, under) = pair(&gpu, &[]);
    // The thumb is x 190..198, y 2..34: a pill leaves its corner pixel alone.
    bare_at(&with, &under, (190, 2), "pill");
    painted(&with, &under, (194, 18), 0.7, "its middle");
    let (with, under) = pair(&gpu, &[(ScrollProp::Radius, PropValue::Number(0.0))]);
    painted(&with, &under, (190, 2), 0.7, "square");
    painted(&with, &under, (197, 33), 0.7, "square");
}

#[test]
fn a_track_is_painted_only_when_it_has_a_fill() {
    let gpu = Gpu::new();
    // Lane y 2..104 (120 less the margins and the 12 corner); the thumb is y 2..34.
    let (with, under) = pair(&gpu, &[]);
    bare_at(&with, &under, (194, 70), "track-less by default");
    let red = Color::from_rgba8(255, 0, 0, 255);
    let (with, _) = pair(&gpu, &[(ScrollProp::TrackFill, PropValue::Colour(red))]);
    let (x, y) = win(194, 70);
    near(rgba(&with, x, y), [255, 0, 0], "the track");
    let (with, under) = pair(
        &gpu,
        &[
            (ScrollProp::TrackFill, PropValue::Colour(red)),
            (ScrollProp::TrackOpacity, PropValue::Number(0.5)),
        ],
    );
    let (x, y) = win(194, 70);
    near(
        rgba(&with, x, y),
        blended(red, 0.5, rgba(&under, x, y)),
        "half a track",
    );
    // Not wider than the thumb.
    bare_at(&with, &under, (187, 70), "beside the track");
}

#[test]
fn arrows_are_triangles_at_the_ends_of_the_lane() {
    let gpu = Gpu::new();
    let (with, under) = pair(&gpu, &[choice(ScrollProp::Arrows, "both")]);
    // 120 - 4 - 12 - 16 = 88 of lane from y 10: the end arrow's square is y 98..106,
    // its triangle y 100..104 pointing down; the start arrow's y 2..10, pointing up.
    painted(&with, &under, (194, 101), 0.7, "the end arrow");
    painted(&with, &under, (194, 7), 0.7, "the start arrow");
    bare_at(&with, &under, (191, 103), "outside the triangle");
    bare_at(
        &with,
        &under,
        (194, 111),
        "past the end arrow, the corner's top",
    );
}

#[test]
fn scrolling_with_a_track_arrows_and_a_bar_on_the_left_matches_a_full_draw() {
    let gpu = Gpu::new();
    let mut world = world(&[
        choice(ScrollProp::SideV, "start"),
        choice(ScrollProp::Arrows, "both"),
        (
            ScrollProp::TrackFill,
            PropValue::Colour(Color::from_rgba8(200, 40, 40, 255)),
        ),
        (ScrollProp::TrackOpacity, PropValue::Number(0.4)),
    ]);
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &world.tree, world.root, true);
    for (step, &(x, y)) in [
        (0.0, 12.0),
        (9.0, 12.0),
        (9.0, 40.0),
        (31.0, 55.0),
        (120.0, 55.0),
        (118.0, 3.0),
        (400.0, 300.0),
        (0.0, 0.0),
    ]
    .iter()
    .enumerate()
    {
        scroll_to(&mut world, x, y);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, &world.tree, world.root),
            &format!("frame {step} (offset {x}, {y}, damage {damage:?})"),
        );
    }
}

#[test]
fn a_bar_that_fades_with_auto_hide_redraws_what_it_covered() {
    let gpu = Gpu::new();
    let mut world = world(&[choice(ScrollProp::Show, "auto_hide")]);
    scroll_to(&mut world, 40.0, 30.0);
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &world.tree, world.root, true);
    for (step, shown) in [0.75, 0.5, 0.0, 0.5, 1.0].into_iter().enumerate() {
        state_of(&mut world).bar.shown.current = shown;
        window.frame(&gpu, &world.tree, world.root, true);
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, &world.tree, world.root),
            &format!("step {step}: visibility {shown}"),
        );
    }
}
