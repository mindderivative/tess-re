//! 0.6.11: the window's colour scheme on the pixels. A text, a text input's text, placeholder,
//! caret and selection, a scroll view's thumb and a terminal's foreground, cursor and selection
//! paint in the roles of the scheme's baseline (light or dark) when the node sets no colour; a
//! colour the node sets wins; a per-role override changes the role; and switching the scheme at
//! run time repaints exactly what a full draw of the new scheme shows (run with
//! `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check as well).

use engine_core::{
    CellColor, NodeId, NodeKind, PaintProperties, Scheme, SchemeColors, SchemeRole,
    ScrollViewState, TerminalCell, TerminalState, TextAlign, TextFieldState, TextState, Tree,
};
use engine_render::{Damage, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

mod support;
use support::{BACKGROUND, Frame, GREEN, RED, layout, placed, rgba, scene};

fn baseline(scheme: Scheme) -> SchemeColors {
    SchemeColors::baseline(scheme)
}

fn with_scheme(tree: &mut Tree, scheme: Scheme) {
    tree.set_scheme_colors(baseline(scheme));
}

/// A big letter, so its stem has pixels of the glyph's exact colour.
fn text(tree: &mut Tree, root: NodeId, paint: PaintProperties) -> NodeId {
    let id = tree.insert(
        NodeKind::Text(TextState {
            content: "I".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 700.0,
            font_size: 90.0,
            align: TextAlign::Start,
            line_height: None,
            options: Default::default(),
        }),
        placed(0.0, 0.0, 100.0, 100.0),
        paint,
    );
    tree.add_child(root, id);
    id
}

fn scheme_text() -> PaintProperties {
    let mut paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
    paint.scheme_fill = true;
    paint
}

/// Whether any pixel of the frame is `color`, exactly.
fn has(frame: &Frame, color: Color) -> bool {
    let want = rgba(color);
    frame.any_in(0, 0, 100, 100, |p| p == want)
}

#[test]
fn a_text_with_no_fill_paints_on_surface_of_the_scheme() {
    pollster::block_on(async {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let (mut tree, root) = scene();
            text(&mut tree, root, scheme_text());
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert!(
                has(&frame, baseline(scheme).text()),
                "{scheme:?}: the glyph is on_surface"
            );
            let other = baseline(if scheme == Scheme::Light {
                Scheme::Dark
            } else {
                Scheme::Light
            });
            assert!(
                !has(&frame, other.text()),
                "{scheme:?}: and not the other's"
            );
        }
    });
}

#[test]
fn the_light_text_default_is_on_surface_not_black() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        text(&mut tree, root, scheme_text());
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(has(&frame, Color::from_rgba8(0x1D, 0x1B, 0x20, 0xFF)));
        assert!(!has(&frame, Color::from_rgba8(0, 0, 0, 255)));
    });
}

#[test]
fn a_text_with_its_own_fill_ignores_the_scheme() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        text(&mut tree, root, PaintProperties::new(RED, 0.0, 1.0));
        with_scheme(&mut tree, Scheme::Dark);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(has(&frame, RED), "an explicit fill wins in the dark scheme");
        assert!(!has(&frame, baseline(Scheme::Dark).text()));
    });
}

#[test]
fn an_override_of_a_role_changes_what_paints_in_it() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        text(&mut tree, root, scheme_text());
        let mut colors = baseline(Scheme::Dark);
        colors.set(SchemeRole::OnSurface, GREEN);
        tree.set_scheme_colors(colors);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(
            has(&frame, GREEN),
            "the overridden on_surface paints the text"
        );
    });
}

fn field(tree: &mut Tree, root: NodeId, mut state: TextFieldState) -> NodeId {
    state.font_size = 70.0;
    state.font_weight = 700.0;
    let id = tree.insert(
        NodeKind::TextField(Box::new(state)),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn scheme_field(content: &str) -> TextFieldState {
    let mut state = TextFieldState::new(content, "Roboto", 700.0, 70.0);
    state.tint_scheme = true;
    state
}

#[test]
fn a_text_inputs_text_placeholder_and_caret_paint_in_their_roles() {
    pollster::block_on(async {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colors = baseline(scheme);

            let (mut tree, root) = scene();
            field(&mut tree, root, scheme_field("I"));
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert!(has(&frame, colors.text()), "{scheme:?}: text is on_surface");

            let (mut tree, root) = scene();
            let mut state = scheme_field("");
            state.placeholder = "I".to_string();
            field(&mut tree, root, state);
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert!(
                has(&frame, colors.placeholder()),
                "{scheme:?}: the placeholder is on_surface_variant"
            );
            assert!(
                !has(&frame, colors.text()),
                "{scheme:?}: not the text colour"
            );

            let (mut tree, root) = scene();
            let mut state = scheme_field("");
            state.caret_width = 12.0;
            let id = field(&mut tree, root, state);
            tree.set_focus_to(id);
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert!(
                has(&frame, colors.caret()),
                "{scheme:?}: the caret is primary"
            );
        }
    });
}

#[test]
fn a_text_inputs_selection_paints_primary_at_30_percent() {
    pollster::block_on(async {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let (mut tree, root) = scene();
            let mut state = scheme_field("III");
            state.selection_anchor = Some(0);
            let id = field(&mut tree, root, state);
            if let NodeKind::TextField(state) = &mut tree.get_mut(id).unwrap().kind {
                state.cursor = 3;
            }
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let want = over(baseline(scheme).caret(), 0.3, rgba(BACKGROUND));
            assert!(
                frame.any_in(0, 0, 100, 100, |p| support::close(p, want, 2)),
                "{scheme:?}: the highlight is primary at 30 percent, wanted {want:?}"
            );
        }
    });
}

#[test]
fn a_text_input_with_a_colour_of_its_own_keeps_the_tint_derived_defaults() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = TextFieldState::new("", "Roboto", 700.0, 70.0);
        state.text_tint = engine_core::Animated::new(RED);
        state.placeholder = "I".to_string();
        field(&mut tree, root, state);
        with_scheme(&mut tree, Scheme::Dark);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        // The placeholder is the tint at 60% over the background, whatever the scheme.
        let (bg, want) = (rgba(BACKGROUND), rgba(RED));
        assert!(
            frame.any_in(0, 0, 100, 100, |p| {
                p[0] > bg[0] + 100 && p[0] < want[0] && p[1] <= 0x40 && p[2] <= 0x40
            }),
            "a red-tinted field's placeholder is red at 60%, not a scheme role"
        );
        assert!(!has(&frame, baseline(Scheme::Dark).placeholder()));
    });
}

fn tall_view(tree: &mut Tree, root: NodeId) {
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, view);
    let content = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(100.0),
                height: length(400.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(view, content);
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

#[test]
fn a_scroll_views_thumb_paints_on_surface_variant_of_the_scheme() {
    pollster::block_on(async {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let (mut tree, root) = scene();
            tall_view(&mut tree, root);
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            // The one-axis thumb: the role at 0x8C of 255 over what is behind it.
            let want = over(
                baseline(scheme).scrollbar_thumb(),
                140.0 / 255.0,
                rgba(BACKGROUND),
            );
            let found = frame.any_in(90, 0, 100, 40, |p| support::close(p, want, 2));
            assert!(found, "{scheme:?}: a thumb of {want:?}");
        }
    });
}

#[test]
fn a_scroll_views_own_fill_beats_the_scheme() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        tall_view(&mut tree, root);
        let view = tree.get(root).unwrap().children[0];
        if let NodeKind::ScrollView(state) = &mut tree.get_mut(view).unwrap().kind {
            state.bar.settings.set(
                engine_core::ScrollProp::Fill,
                Some(engine_core::PropValue::Colour(RED)),
            );
        }
        with_scheme(&mut tree, Scheme::Dark);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(has(&frame, RED));
    });
}

fn terminal(tree: &mut Tree, root: NodeId, mut state: TerminalState) -> NodeId {
    state.font_size = 40.0;
    let id = tree.insert(
        NodeKind::Terminal(Box::new(state)),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn full_blocks() -> TerminalState {
    let mut state = TerminalState::new(2, 1, "Hack Nerd Font Mono", 40.0);
    for cell in state.cells.iter_mut() {
        *cell = TerminalCell {
            ch: '\u{2588}',
            fg: CellColor::Default,
            ..TerminalCell::blank()
        };
    }
    state
}

#[test]
fn a_terminals_foreground_cursor_and_selection_paint_in_their_roles() {
    pollster::block_on(async {
        for scheme in [Scheme::Light, Scheme::Dark] {
            let colors = baseline(scheme);
            let (mut tree, root) = scene();
            terminal(&mut tree, root, full_blocks());
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert!(
                has(&frame, colors.text()),
                "{scheme:?}: foreground is on_surface"
            );

            // Blank cells under a selection show its highlight.
            let (mut tree, root) = scene();
            let mut state = TerminalState::new(2, 1, "Hack Nerd Font Mono", 40.0);
            state.selection_start = Some((0, 0));
            state.selection_end = Some((0, 2));
            terminal(&mut tree, root, state);
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let want = over(colors.caret(), 0.3, rgba(BACKGROUND));
            assert!(
                frame.any_in(0, 0, 100, 100, |p| support::close(p, want, 2)),
                "{scheme:?}: selection is primary at 30 percent, wanted {want:?}"
            );

            // The cursor is primary at half alpha over what is behind it.
            let (mut tree, root) = scene();
            let id = terminal(
                &mut tree,
                root,
                TerminalState::new(2, 1, "Hack Nerd Font Mono", 40.0),
            );
            tree.set_focus_to(id);
            with_scheme(&mut tree, scheme);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let want = over(colors.caret(), 0.5, rgba(BACKGROUND));
            assert!(
                frame.any_in(0, 0, 100, 100, |p| support::close(p, want, 2)),
                "{scheme:?}: the cursor is primary at 50%, wanted {want:?}"
            );
        }
    });
}

#[test]
fn a_terminals_own_foreground_beats_the_scheme() {
    pollster::block_on(async {
        let (mut tree, root) = scene();
        let mut state = full_blocks();
        state.palette.foreground = Some(RED);
        terminal(&mut tree, root, state);
        with_scheme(&mut tree, Scheme::Dark);
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        assert!(has(&frame, RED));
        assert!(!has(&frame, baseline(Scheme::Dark).text()));
    });
}

// --- A scheme switch at run time repaints, partial redraw included ---

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SIZE: u32 = 100;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

struct Output {
    renderer: WindowRenderer,
    surface: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Output {
    fn new(gpu: &Gpu) -> Self {
        let surface = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scheme test target"),
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
        let mut renderer = WindowRenderer::new(&gpu.device, FORMAT, SIZE, SIZE, true);
        renderer.set_scale(1.0);
        Self {
            renderer,
            surface,
            view,
        }
    }

    fn frame(&mut self, gpu: &Gpu, tree: &Tree, root: NodeId, partial: bool) -> Damage {
        let size = SIZE as u16;
        let damage =
            self.renderer
                .prepare(tree, root, size, size, partial, &gpu.device, &gpu.queue);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.renderer.draw(
            tree,
            root,
            size,
            size,
            &damage,
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &self.surface,
            &self.view,
        );
        gpu.queue.submit([encoder.finish()]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        damage
    }

    fn pixels(&self, gpu: &Gpu) -> Vec<u8> {
        support::read_texture(&gpu.device, &gpu.queue, &self.surface)
    }
}

#[test]
fn switching_the_scheme_repaints_every_node_that_follows_it() {
    let (device, queue) = pollster::block_on(support::device("scheme switch test device"));
    let gpu = Gpu { device, queue };

    // A text, a text input, a scroll view and a terminal, none with a colour of its own.
    let (mut tree, root) = scene();
    let t = tree.insert(
        NodeKind::Text(TextState {
            content: "I".to_string(),
            font_family: "Roboto".to_string(),
            font_weight: 700.0,
            font_size: 40.0,
            align: TextAlign::Start,
            line_height: None,
            options: Default::default(),
        }),
        placed(0.0, 0.0, 30.0, 50.0),
        scheme_text(),
    );
    tree.add_child(root, t);
    let mut state = scheme_field("I");
    state.font_size = 40.0;
    let f = tree.insert(
        NodeKind::TextField(Box::new(state)),
        placed(30.0, 0.0, 30.0, 50.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, f);
    let view = tree.insert(
        NodeKind::ScrollView(ScrollViewState::new(false)),
        placed(60.0, 0.0, 40.0, 50.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, view);
    let content = tree.insert(
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(40.0),
                height: length(300.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(view, content);
    let mut cells = full_blocks();
    cells.font_size = 30.0;
    let term = tree.insert(
        NodeKind::Terminal(Box::new(cells)),
        placed(0.0, 50.0, 100.0, 50.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, term);
    layout_to(&mut tree, root);

    let mut output = Output::new(&gpu);
    output.frame(&gpu, &tree, root, true);
    let light = output.pixels(&gpu);

    // The same tree in the dark, drawn by a renderer that kept the light frame...
    with_scheme(&mut tree, Scheme::Dark);
    let damage = output.frame(&gpu, &tree, root, true);
    assert_ne!(damage, Damage::None, "a new scheme is damage");
    let switched = output.pixels(&gpu);
    assert_ne!(switched, light, "the dark frame differs from the light one");

    // ...is what a fresh renderer draws from scratch.
    let mut fresh = Output::new(&gpu);
    fresh.frame(&gpu, &tree, root, false);
    assert_eq!(
        switched,
        fresh.pixels(&gpu),
        "the partial redraw after the switch equals a full draw"
    );

    // And back to the light scheme.
    with_scheme(&mut tree, Scheme::Light);
    output.frame(&gpu, &tree, root, true);
    assert_eq!(output.pixels(&gpu), light, "back to light, as it was");

    // A frame with nothing changed is no damage.
    assert_eq!(output.frame(&gpu, &tree, root, true), Damage::None);
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
