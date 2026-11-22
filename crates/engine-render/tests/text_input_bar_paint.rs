//! 0.6.13: a multiline text input's own scrollbar, painted. Not drawn by default; the spec's thumb
//! (8 px, 2 px from the box's edge, at 70 percent opacity) once `scrollbar_show` is set, sized from
//! the text the renderer shaped (`TextRenderer::field_content_extent`), and moving with the
//! input's scroll offset, a partial redraw equal to a full draw (run with
//! `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check as well).

use engine_core::{
    NodeId, NodeKind, PaintProperties, PropValue, ScrollProp, Show, TextFieldState, Tree,
};
use engine_render::{Damage, TextRenderer, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size};

mod support;
use support::{BACKGROUND, Frame, WHITE, layout, placed, rgba, scene};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SIZE: u32 = 100;

/// An input 100 wide and 80 high at the origin of forty short lines, wrapped, the bar `show`n,
/// its text measured as the window would measure it.
fn input(show: Option<Show>, lines: usize) -> (Tree, NodeId, NodeId) {
    let (mut tree, root) = scene();
    let content = (0..lines)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut state = TextFieldState::new(content, "Roboto", 400.0, 14.0);
    state.multiline = true;
    state.wrap = Some(true);
    state.text_tint = engine_core::Animated::new(WHITE);
    // The caret is not wanted over the bar's pixels.
    state.caret_visible = false;
    state.cursor = 0;
    if let Some(show) = show {
        state
            .bar
            .settings
            .set(ScrollProp::Show, Some(PropValue::Choice(show.index())));
    }
    let id = tree.insert(
        NodeKind::TextField(Box::new(state)),
        placed(0.0, 0.0, 100.0, 80.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    layout(&mut tree, root);
    measure(&mut tree, id);
    (tree, root, id)
}

/// What `engine-py`'s `follow_fields` does for an input whose bar is on.
fn measure(tree: &mut Tree, id: NodeId) {
    let measured = {
        let NodeKind::TextField(state) = &tree.get(id).unwrap().kind else {
            unreachable!()
        };
        let layout = tree.layout(id);
        TextRenderer::new().field_content_extent(state, (layout.size.width, layout.size.height))
    };
    if let Some((extent, stamp)) = measured {
        tree.set_text_field_extent(id, extent, stamp);
    }
}

fn set(tree: &mut Tree, id: NodeId, edit: impl FnOnce(&mut TextFieldState)) {
    if let NodeKind::TextField(state) = &mut tree.get_mut(id).unwrap().kind {
        edit(state);
    }
}

fn over(color: Color, alpha: f32, under: [u8; 4]) -> [u8; 4] {
    let c = rgba(color);
    let mix = |a: u8, b: u8| (f32::from(a) * alpha + f32::from(b) * (1.0 - alpha)).round() as u8;
    [
        mix(c[0], under[0]),
        mix(c[1], under[1]),
        mix(c[2], under[2]),
        0xFF,
    ]
}

/// The box `(x0, y0, x1, y1)` of the pixels that are `want` (within 2), `x1` and `y1`
/// exclusive, or `None`.
fn bounds_of(frame: &Frame, want: [u8; 4]) -> Option<(u32, u32, u32, u32)> {
    let mut b = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            if support::close(frame.at(x, y), want, 2) {
                b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
            }
        }
    }
    (b.2 > 0).then_some(b)
}

#[test]
fn the_renderer_measures_the_shaped_text_and_measures_it_once() {
    let (mut tree, _, id) = input(Some(Show::Overflow), 40);
    let NodeKind::TextField(state) = &tree.get(id).unwrap().kind else {
        unreachable!()
    };
    // Forty lines are forty line-heights tall, whatever the font's own height is.
    let (width, height) = state.content_extent;
    let line = height / 40.0;
    assert!(line > 10.0 && line < 30.0, "a 14 px font's line: {line}");
    assert_eq!(width, 100.0, "a wrapped input is as wide as its box");
    // The same text, measured again, is the same answer and no new work.
    let mut renderer = TextRenderer::new();
    assert_eq!(renderer.field_content_extent(state, (100.0, 80.0)), None);
    // Shorter text measures shorter.
    set(&mut tree, id, |s| s.content = "a\nb".into());
    measure(&mut tree, id);
    let NodeKind::TextField(state) = &tree.get(id).unwrap().kind else {
        unreachable!()
    };
    assert!((state.content_extent.1 - 2.0 * line).abs() < 1.0);
}

#[test]
fn the_bar_is_not_drawn_by_default_and_is_the_spec_thumb_when_asked() {
    pollster::block_on(async {
        let (tree, root, _) = input(None, 40);
        let want = over(
            tree.scheme_colors().scrollbar_thumb(),
            0.70,
            rgba(BACKGROUND),
        );
        let frame = Frame::of(&tree, root).await;
        assert!(
            bounds_of(&frame, want).is_none(),
            "the default draws no bar"
        );

        let (tree, root, _) = input(Some(Show::Overflow), 40);
        let frame = Frame::of(&tree, root).await;
        let b = bounds_of(&frame, want).expect("a thumb at 70 percent opacity");
        assert_eq!(b.2 - b.0, 8, "8 px wide: {b:?}");
        assert_eq!(b.2, 98, "2 px from the box's edge: {b:?}");
        assert!(b.1 <= 3, "at the top, the input being unscrolled: {b:?}");
        assert!(
            b.3 - b.1 >= 30,
            "at least 32 px long (edge rows blend): {b:?}"
        );

        // Text that fits: no bar for `overflow`.
        let (tree, root, _) = input(Some(Show::Overflow), 2);
        let frame = Frame::of(&tree, root).await;
        assert!(bounds_of(&frame, want).is_none(), "nothing to scroll");
    });
}

struct Output {
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Output {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let surface = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text input bar test target"),
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

fn layout_to(tree: &mut Tree, root: NodeId) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(SIZE as f32),
            height: AvailableSpace::Definite(SIZE as f32),
        },
    );
}

#[test]
fn the_thumb_moves_with_the_scroll_and_repaints_like_a_full_draw() {
    let (device, queue) = pollster::block_on(support::device("text input bar device"));
    let (mut tree, root, id) = input(Some(Show::Overflow), 40);
    let mut output = Output::new(&device, &queue);
    output.frame(&device, &queue, &tree, root, true);
    let mut last = output.pixels(&device, &queue);
    let max = {
        let thumb = tree.scroll_thumbs(id).unwrap()[0].expect("a bar");
        thumb.max_scroll
    };
    for offset in [max / 2.0, max, 0.0] {
        set(&mut tree, id, |s| s.scroll_offset.current = offset);
        layout_to(&mut tree, root);
        let damage = output.frame(&device, &queue, &tree, root, true);
        assert_ne!(damage, Damage::None, "offset {offset} is damage");
        let partial = output.pixels(&device, &queue);
        assert_ne!(
            partial, last,
            "offset {offset} moves the text and the thumb"
        );
        let mut fresh = Output::new(&device, &queue);
        fresh.frame(&device, &queue, &tree, root, false);
        assert_eq!(
            partial,
            fresh.pixels(&device, &queue),
            "offset {offset}: the partial redraw equals a full draw"
        );
        last = partial;
    }
    // A setting of the bar repaints too: a wider thumb.
    set(&mut tree, id, |s| {
        s.bar
            .settings
            .set(ScrollProp::Width, Some(PropValue::Number(12.0)));
    });
    layout_to(&mut tree, root);
    let damage = output.frame(&device, &queue, &tree, root, true);
    assert_ne!(damage, Damage::None, "scrollbar_width is damage");
    let partial = output.pixels(&device, &queue);
    let mut fresh = Output::new(&device, &queue);
    fresh.frame(&device, &queue, &tree, root, false);
    assert_eq!(partial, fresh.pixels(&device, &queue), "scrollbar_width");
    assert_ne!(partial, last, "scrollbar_width changes the pixels");
}
