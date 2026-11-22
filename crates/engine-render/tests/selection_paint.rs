//! 0.6.12 (P8): how a selection is painted. A static `text` takes `selection_fill` as a text
//! input does (the text colour at 30% without it), `selection_foreground` recolours the glyphs
//! the selection covers and no others, `selection_radius` rounds the highlight's corners (square
//! by default), a text input's `caret_width` is the caret's width in pixels, and changing any of
//! them at run time repaints what a full draw of the new state shows (run with
//! `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check as well).

use engine_core::{
    Animated, NodeId, NodeKind, PaintProperties, TextAlign, TextFieldState, TextState, Tree,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size};

mod support;
use support::{BACKGROUND, Frame, GREEN, RED, WHITE, layout, placed, rgba, scene};

const BACKGROUND_G: u8 = 0x11;
const BLUE: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF);

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

/// A selectable two-letter text in red, `selection` over its bytes.
fn text(tree: &mut Tree, root: NodeId, selection: (usize, usize)) -> NodeId {
    let options = engine_core::TextOptions {
        selectable: true,
        selection: Some(selection),
        ..Default::default()
    };
    let id = tree.insert(
        NodeKind::Text(TextState {
            content: "II".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 700.0,
            font_size: 40.0,
            align: TextAlign::Start,
            line_height: None,
            options,
        }),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(RED, 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn set_text(tree: &mut Tree, id: NodeId, edit: impl FnOnce(&mut engine_core::TextOptions)) {
    if let NodeKind::Text(state) = &mut tree.get_mut(id).unwrap().kind {
        edit(&mut state.options);
    }
}

/// A two-letter text input in red, the first letter selected.
fn field(tree: &mut Tree, root: NodeId) -> NodeId {
    let mut state = TextFieldState::new("II", "Roboto", 700.0, 40.0);
    state.text_tint = Animated::new(RED);
    state.selection_anchor = Some(0);
    state.cursor = 1;
    let id = tree.insert(
        NodeKind::TextField(Box::new(state)),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn set_field(tree: &mut Tree, id: NodeId, edit: impl FnOnce(&mut TextFieldState)) {
    if let NodeKind::TextField(state) = &mut tree.get_mut(id).unwrap().kind {
        edit(state);
    }
}

/// The columns, from `x0` to `x1`, where any pixel of rows `0..100` is `want` (within 2).
fn columns_of(frame: &Frame, want: Color, x0: u32, x1: u32) -> Vec<u32> {
    let want = rgba(want);
    (x0..x1)
        .filter(|&x| (0..100).any(|y| support::close(frame.at(x, y), want, 2)))
        .collect()
}

/// The box `(x0, y0, x1, y1)` of the pixels that are `want` (within 2), `x1` and `y1` exclusive.
fn bounds_of(frame: &Frame, want: [u8; 4]) -> (u32, u32, u32, u32) {
    let mut b = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..100 {
        for x in 0..100 {
            if support::close(frame.at(x, y), want, 2) {
                b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
            }
        }
    }
    assert!(b.2 > 0, "no pixel is {want:?}");
    b
}

/// The x where the highlight's right edge is: the last column whose pixel row 2 is `fill`.
fn highlight_right(frame: &Frame, fill: [u8; 4]) -> u32 {
    (0..100)
        .rev()
        .find(|&x| support::close(frame.at(x, 2), fill, 2))
        .expect("the highlight is on row 2")
}

#[test]
fn a_static_texts_selection_defaults_to_its_colour_at_30_percent() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        text(&mut tree, root, (0, 2));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        let want = over(RED, 0.3, rgba(BACKGROUND));
        assert!(
            frame.any_in(0, 0, 100, 100, |p| support::close(p, want, 2)),
            "the highlight is the text colour at 30 percent, wanted {want:?}"
        );
    });
}

#[test]
fn a_static_texts_selection_fill_paints_the_highlight() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let id = text(&mut tree, root, (0, 2));
        set_text(&mut tree, id, |o| o.selection_fill = Some(GREEN));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // Square by default: the highlight's own corner pixel is the fill.
        assert_eq!(frame.at(0, 0), rgba(GREEN), "an opaque fill, square corner");
        assert!(
            !frame.any_in(0, 0, 100, 100, |p| support::close(
                p,
                over(RED, 0.3, rgba(BACKGROUND)),
                2
            )),
            "and not the default colour"
        );
    });
}

#[test]
fn selection_foreground_recolours_only_the_selected_glyphs() {
    pollster::block_on(async {
        // The first of two letters is selected.
        let (mut tree, root) = scene();
        let id = text(&mut tree, root, (0, 1));
        set_text(&mut tree, id, |o| o.selection_fill = Some(BLUE));
        layout(&mut tree, root);
        let plain = Frame::of(&tree, root).await;
        assert!(
            !columns_of(&plain, RED, 0, 100).is_empty(),
            "without selection_foreground the glyphs keep their colour"
        );
        assert!(columns_of(&plain, WHITE, 0, 100).is_empty());
        let edge = highlight_right(&plain, rgba(BLUE));

        set_text(&mut tree, id, |o| o.selection_foreground = Some(WHITE));
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        let white = columns_of(&frame, WHITE, 0, 100);
        let red = columns_of(&frame, RED, 0, 100);
        assert!(!white.is_empty(), "the selected glyph is white");
        assert!(!red.is_empty(), "the other glyph keeps its own colour");
        assert!(
            white.iter().all(|&x| x <= edge + 1),
            "white only inside the highlight (edge {edge}, white {white:?})"
        );
        assert!(
            red.iter().all(|&x| x + 1 >= edge),
            "red only outside it (edge {edge}, red {red:?})"
        );
    });
}

#[test]
fn selection_radius_rounds_the_highlight() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let id = text(&mut tree, root, (0, 2));
        set_text(&mut tree, id, |o| {
            o.selection_fill = Some(GREEN);
            o.selection_radius = 12.0;
        });
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(
            frame.at(0, 0),
            rgba(BACKGROUND),
            "a rounded highlight leaves its corner bare"
        );
        // Its middle is filled all the same (above the glyph stems, left of the first).
        assert_eq!(frame.at(1, 20), rgba(GREEN));
        set_text(&mut tree, id, |o| o.selection_radius = 0.0);
        layout(&mut tree, root);
        let square = Frame::of(&tree, root).await;
        assert_eq!(square.at(0, 0), rgba(GREEN), "radius 0 is square");
    });
}

#[test]
fn a_text_inputs_selection_foreground_and_radius_paint() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let id = field(&mut tree, root);
        set_field(&mut tree, id, |s| s.selection_fill = Some(BLUE));
        layout(&mut tree, root);
        let plain = Frame::of(&tree, root).await;
        // A single-line input centres its text, so the highlight's own top is not the box's.
        let (x0, y0, x1, _) = bounds_of(&plain, rgba(BLUE));
        assert_eq!(plain.at(x0, y0), rgba(BLUE), "square by default");
        assert!(columns_of(&plain, WHITE, 0, 100).is_empty());
        let edge = x1 - 1;

        set_field(&mut tree, id, |s| {
            s.selection_foreground = Some(WHITE);
            s.selection_radius = 12.0;
        });
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert_eq!(frame.at(x0, y0), rgba(BACKGROUND), "rounded corner");
        let white = columns_of(&frame, WHITE, 0, 100);
        let red = columns_of(&frame, RED, 0, 100);
        assert!(!white.is_empty() && !red.is_empty());
        assert!(
            white.iter().all(|&x| x <= edge + 1),
            "white inside: {white:?}"
        );
        assert!(red.iter().all(|&x| x + 1 >= edge), "red outside: {red:?}");
    });
}

#[test]
fn a_text_inputs_caret_is_caret_width_pixels_wide() {
    pollster::block_on(async {
        for width in [1.5_f32, 4.0, 9.0] {
            let (mut tree, root) = scene();
            let mut state = TextFieldState::new("", "Roboto", 400.0, 40.0);
            state.text_tint = Animated::new(GREEN);
            state.caret_width = width;
            let id = tree.insert(
                NodeKind::TextField(Box::new(state)),
                placed(0.0, 0.0, 100.0, 100.0),
                PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
            );
            tree.add_child(root, id);
            tree.set_focus_to(id);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            // The caret is the one vertical bar of the text colour: its box is `width` wide.
            let (x0, y0, x1, y1) = bounds_of(&frame, rgba(GREEN));
            let mid = (y0 + y1) / 2;
            // Anti-aliased edges carry part of the colour: count the coverage along a row.
            let coverage: f32 = (x0.saturating_sub(1)..x1 + 1)
                .map(|x| {
                    let p = frame.at(x, mid);
                    f32::from(p[1].saturating_sub(BACKGROUND_G)) / f32::from(0xFF - BACKGROUND_G)
                })
                .sum();
            assert!(
                (coverage - width).abs() <= 0.6,
                "caret_width {width}: the caret covers {coverage} pixels of a row"
            );
            assert!(y1 - y0 > 20, "and it is a tall bar");
        }
    });
}

// --- A change at run time repaints, partial redraw included ---

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
            label: Some("selection paint test target"),
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
fn changing_how_a_selection_paints_repaints_like_a_full_draw() {
    let (device, queue) = pollster::block_on(support::device("selection paint test device"));
    let (mut tree, root) = scene();
    let t = text(&mut tree, root, (0, 1));
    let f = field(&mut tree, root);
    // Side by side, not on top of each other.
    set_field(&mut tree, f, |s| s.font_size = 30.0);
    tree.get_mut(f).unwrap();
    tree.set_layout_style(f, placed(0.0, 55.0, 100.0, 45.0));
    tree.set_layout_style(t, placed(0.0, 0.0, 100.0, 50.0));
    layout_to(&mut tree, root);

    let mut output = Output::new(&device, &queue);
    output.frame(&device, &queue, &tree, root, true);
    let before = output.pixels(&device, &queue);

    type Edit = Box<dyn Fn(&mut Tree)>;
    let edits: [(&str, Edit); 3] = [
        (
            "selection_fill",
            Box::new(move |tree| {
                set_text(tree, t, |o| o.selection_fill = Some(GREEN));
                set_field(tree, f, |s| s.selection_fill = Some(GREEN));
            }),
        ),
        (
            "selection_foreground",
            Box::new(move |tree| {
                set_text(tree, t, |o| o.selection_foreground = Some(WHITE));
                set_field(tree, f, |s| s.selection_foreground = Some(WHITE));
            }),
        ),
        (
            "selection_radius",
            Box::new(move |tree| {
                set_text(tree, t, |o| o.selection_radius = 10.0);
                set_field(tree, f, |s| s.selection_radius = 10.0);
            }),
        ),
    ];
    let mut last = before;
    for (name, edit) in edits {
        edit(&mut tree);
        layout_to(&mut tree, root);
        let damage = output.frame(&device, &queue, &tree, root, true);
        assert_ne!(damage, Damage::None, "{name} is damage");
        let partial = output.pixels(&device, &queue);
        assert_ne!(partial, last, "{name} changes the pixels");
        let mut fresh = Output::new(&device, &queue);
        fresh.frame(&device, &queue, &tree, root, false);
        assert_eq!(
            partial,
            fresh.pixels(&device, &queue),
            "{name}: the partial redraw equals a full draw"
        );
        last = partial;
    }
}
