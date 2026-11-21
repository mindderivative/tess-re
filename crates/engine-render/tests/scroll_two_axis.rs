//! 0.6.11 (UXREQ-001): a scroll view of `orientation="both"` -- its two thumbs on
//! the pixels, and the partial redraw of its scrolling and hover fades against a
//! full draw (the block copy and the thumbs' damage).

use engine_core::{
    NodeId, NodeKind, PaintProperties, ScrollAxis, ScrollOrientation, ScrollViewState, Tree,
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
fn world() -> World {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        placed(0.0, 0.0, W, H),
        PaintProperties::new(SURFACE, 0.0, 1.0),
    );
    let mut state = ScrollViewState::new(false);
    state.set_orientation(ScrollOrientation::Both);
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

/// The same scene with the thumbs invisible: what is under them.
fn bare_world() -> World {
    let mut world = world();
    let state = state_of(&mut world);
    state.scrollbar.opacity = Some(0.0);
    state.scrollbar.opacity_hover = Some(0.0);
    state.scrollbar.opacity_drag = Some(0.0);
    world
}

#[test]
fn the_two_thumbs_are_drawn_at_rest_opacity_and_no_wider_than_8() {
    let gpu = Gpu::new();
    let world = world();
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &world.tree, world.root, false);
    let pixels = window.pixels(&gpu);
    let bare = bare_world();
    let under = fresh(&gpu, &bare.tree, bare.root);
    let (vx, vy, vw, vh) = VIEW;
    let role = Color::from_rgba8(0xCA, 0xC4, 0xD0, 0xFF);
    // Vertical thumb: x 190..198, y 2..34 in the view; its centre.
    let (cx, cy) = ((vx + vw - 6.0) as u32, (vy + 18.0) as u32);
    near(
        rgba(&pixels, cx, cy),
        blended(role, 0.60, rgba(&under, cx, cy)),
        "vertical thumb at rest",
    );
    // Horizontal thumb: y 90..98, x 2..48 in the view.
    let (hx, hy) = ((vx + 20.0) as u32, (vy + vh - 6.0) as u32);
    near(
        rgba(&pixels, hx, hy),
        blended(role, 0.60, rgba(&under, hx, hy)),
        "horizontal thumb at rest",
    );
    // The thumb is 8 wide (x 190..198): 5 px left of its centre is the content, untouched.
    assert_eq!(
        rgba(&pixels, cx - 5, cy),
        rgba(&under, cx - 5, cy),
        "the thumb is no wider than 8"
    );
}

#[test]
fn scrolling_both_axes_matches_a_full_draw_frame_by_frame() {
    let gpu = Gpu::new();
    let mut world = world();
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &world.tree, world.root, true);
    let mut moved_as_a_block = false;
    let steps = [
        (0.0, 12.0),
        (9.0, 12.0),
        (9.0, 40.0),
        (31.0, 55.0),
        (120.0, 55.0),
        (118.0, 3.0),
        (400.0, 300.0),
        (0.0, 0.0),
    ];
    let view_area = f64::from(VIEW.2 * VIEW.3);
    for (step, &(x, y)) in steps.iter().enumerate() {
        scroll_to(&mut world, x, y);
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, &world.tree, world.root),
            &format!("frame {step} (offset {x}, {y}, damage {damage:?})"),
        );
        if let Damage::Rects(rects) = &damage {
            let area: f64 = rects.iter().map(peniko::kurbo::Rect::area).sum();
            moved_as_a_block |= area < view_area;
        }
    }
    assert!(
        moved_as_a_block,
        "a both-axes view's scroll is moved as a block when it can be"
    );
}

#[test]
fn a_thumb_fading_in_and_out_redraws_what_it_covers() {
    let gpu = Gpu::new();
    let mut world = world();
    scroll_to(&mut world, 40.0, 30.0);
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &world.tree, world.root, true);
    // The emphasis of each thumb through a fade: only that pixels change, and all of them are redrawn.
    for (step, level) in [0.25, 0.5, 1.0, 1.5, 2.0, 1.0, 0.0].into_iter().enumerate() {
        state_of(&mut world).level[ScrollAxis::Vertical.index()].current = level;
        state_of(&mut world).level[ScrollAxis::Horizontal.index()].current = 2.0 - level;
        let damage = window.frame(&gpu, &world.tree, world.root, true);
        assert!(
            !matches!(damage, Damage::None),
            "step {step}: a thumb's opacity is pixels"
        );
        assert_same(
            &window.pixels(&gpu),
            &fresh(&gpu, &world.tree, world.root),
            &format!("fade step {step} (level {level})"),
        );
    }
}

#[test]
fn the_corner_and_the_ends_of_both_lanes_are_never_painted_over() {
    let gpu = Gpu::new();
    let mut world = world();
    // Scrolled to both far ends: the thumbs are at their lowest and rightmost.
    scroll_to(&mut world, 10_000.0, 10_000.0);
    let mut window = Window::new(&gpu);
    window.frame(&gpu, &world.tree, world.root, false);
    let pixels = window.pixels(&gpu);
    let mut bare = bare_world();
    scroll_to(&mut bare, 10_000.0, 10_000.0);
    let without = fresh(&gpu, &bare.tree, bare.root);
    let (vx, vy, vw, vh) = VIEW;
    // The 12 x 12 corner (inside the view's right and bottom edges) is exactly the content.
    for dy in 0..12 {
        for dx in 0..12 {
            let (x, y) = ((vx + vw) as u32 - 12 + dx, (vy + vh) as u32 - 12 + dy);
            assert_eq!(
                rgba(&pixels, x, y),
                rgba(&without, x, y),
                "corner pixel ({x}, {y}) has a thumb on it"
            );
        }
    }
}
