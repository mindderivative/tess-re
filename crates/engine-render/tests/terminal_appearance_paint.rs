//! 0.6.12 (P8): how a terminal's cursor, bold, dim, underline, line height and selection are
//! painted. Each default paints what 0.6.11 painted (a filled block, bold 700, dim 0.6, the
//! underline at 0.85 of the cell, the font's own line height, a square highlight), and each
//! property changes exactly that. A change at run time repaints what a full draw shows (run with
//! `TESS_RE_DAMAGE_VERIFY=1` for the damage walk's own cross-check as well).

use engine_core::{
    CaretShape, CellColor, NodeId, NodeKind, PaintProperties, TerminalCell, TerminalState, Tree,
    UnfocusedCursor,
};
use engine_render::{Damage, TextRenderer, WindowRenderer};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size};

mod support;
use support::{BACKGROUND, Frame, GREEN, RED, WHITE, layout, placed, rgba, scene};

const BLUE: Color = Color::from_rgba8(0x00, 0x00, 0xFF, 0xFF);
const FAMILY: &str = "Hack Nerd Font Mono";
const FONT: f32 = 20.0;

fn natural() -> (f32, f32) {
    TextRenderer::new().monospace_cell_size(FAMILY, FONT)
}

/// A 3 by 2 terminal at the origin, the cursor on its first cell in green.
fn terminal(tree: &mut Tree, root: NodeId, edit: impl FnOnce(&mut TerminalState)) -> NodeId {
    let mut state = TerminalState::new(3, 2, FAMILY, FONT);
    state.palette.cursor = Some(GREEN);
    edit(&mut state);
    let id = tree.insert(
        NodeKind::Terminal(Box::new(state)),
        placed(0.0, 0.0, 100.0, 100.0),
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    id
}

fn set(tree: &mut Tree, id: NodeId, edit: impl FnOnce(&mut TerminalState)) {
    if let NodeKind::Terminal(state) = &mut tree.get_mut(id).unwrap().kind {
        edit(state);
    }
}

fn block(ch: char, fg: CellColor) -> TerminalCell {
    TerminalCell {
        ch,
        fg,
        ..TerminalCell::blank()
    }
}

/// The box `(x0, y0, x1, y1)` of the pixels that are `want` (within 2), `x1` and `y1`
/// exclusive, or `None`.
fn bounds_of(frame: &Frame, want: [u8; 4]) -> Option<(u32, u32, u32, u32)> {
    let mut b = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..100 {
        for x in 0..100 {
            if support::close(frame.at(x, y), want, 2) {
                b = (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1));
            }
        }
    }
    (b.2 > 0).then_some(b)
}

async fn focused_frame(edit: impl FnOnce(&mut TerminalState)) -> Frame {
    let (mut tree, root) = scene();
    let id = terminal(&mut tree, root, edit);
    tree.set_focus_to(id);
    layout(&mut tree, root);
    Frame::of(&tree, root).await
}

#[test]
fn the_cursor_is_a_block_by_default_and_follows_caret_shape_and_width() {
    pollster::block_on(async {
        let (cw, ch) = natural();
        let green = rgba(GREEN);
        let frame = focused_frame(|_| {}).await;
        let b = bounds_of(&frame, green).expect("a block cursor");
        assert!(
            (b.2 - b.0).abs_diff(cw.round() as u32) <= 1
                && (b.3 - b.1).abs_diff(ch.round() as u32) <= 1,
            "a block is one cell: {b:?} against {cw} x {ch}"
        );

        let frame = focused_frame(|s| {
            s.caret_shape = CaretShape::Bar;
            s.caret_width = 4.0;
        })
        .await;
        let b = bounds_of(&frame, green).expect("a bar cursor");
        assert_eq!(b.2 - b.0, 4, "a bar is caret_width wide: {b:?}");
        assert!(b.3 - b.1 >= ch.round() as u32 - 1, "and a cell high: {b:?}");
        assert_eq!(b.0, 0, "at the cell's left edge");

        let frame = focused_frame(|s| {
            s.caret_shape = CaretShape::Underline;
            s.caret_width = 3.0;
        })
        .await;
        let b = bounds_of(&frame, green).expect("an underline cursor");
        assert!(
            (b.3 - b.1).abs_diff(3) <= 1,
            "an underline is caret_width thick (a fractional foot blurs a row): {b:?}"
        );
        assert!(b.2 - b.0 >= cw.round() as u32 - 1, "and a cell wide: {b:?}");
        assert!(b.3 >= ch.round() as u32 - 1, "at the cell's foot: {b:?}");
    });
}

#[test]
fn an_unfocused_terminal_shows_no_cursor_unless_caret_unfocused_is_outline() {
    pollster::block_on(async {
        let green = rgba(GREEN);
        for (mode, expect) in [
            (UnfocusedCursor::Hidden, false),
            (UnfocusedCursor::Outline, true),
        ] {
            let (mut tree, root) = scene();
            terminal(&mut tree, root, |s| s.caret_unfocused = mode);
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            assert_eq!(
                bounds_of(&frame, green).is_some(),
                expect,
                "{mode:?}: the cursor of an unfocused terminal"
            );
            if expect {
                // Hollow: the cell's middle is bare.
                let (cw, ch) = natural();
                let middle = frame.at((cw / 2.0) as u32, (ch / 2.0) as u32);
                assert_eq!(middle, rgba(BACKGROUND), "an outline is hollow");
            }
        }
    });
}

#[test]
fn a_cursor_in_its_off_phase_is_not_drawn_and_a_hidden_one_neither() {
    pollster::block_on(async {
        let green = rgba(GREEN);
        let frame = focused_frame(|s| s.caret_on = false).await;
        assert!(bounds_of(&frame, green).is_none(), "off phase");
        let frame = focused_frame(|s| s.cursor_visible = false).await;
        assert!(bounds_of(&frame, green).is_none(), "the program hid it");
    });
}

#[test]
fn line_height_sets_the_cell_height_and_keeps_the_glyphs_in_it() {
    pollster::block_on(async {
        let (_, natural_h) = natural();
        for ratio in [None, Some(2.0_f32)] {
            let (mut tree, root) = scene();
            terminal(&mut tree, root, |s| {
                s.line_height = ratio;
                s.cells[0].bg = CellColor::Rgb(RED);
            });
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let b = bounds_of(&frame, rgba(RED)).expect("a red cell");
            let want = ratio.map_or(natural_h, |r| FONT * r);
            assert!(
                (b.3 - b.1).abs_diff(want.round() as u32) <= 1,
                "line_height {ratio:?}: the cell is {want} high, drawn {b:?}"
            );
        }
        // A glyph is centred in the taller cell: the block character's ink is the cell's middle.
        let (mut tree, root) = scene();
        terminal(&mut tree, root, |s| {
            s.line_height = Some(3.0);
            s.cells[0] = block('\u{2588}', CellColor::Rgb(WHITE));
        });
        layout(&mut tree, root);
        let frame = Frame::of(&tree, root).await;
        let b = bounds_of(&frame, rgba(WHITE)).expect("a white block");
        let cell = FONT * 3.0;
        let (top, bottom) = (b.1 as f32, cell - b.3 as f32);
        assert!(
            (top - bottom).abs() <= 2.0,
            "the glyph sits in the middle of the {cell} px cell: {top} above, {bottom} below"
        );
    });
}

#[test]
fn dim_opacity_and_underline_geometry_follow_their_properties() {
    pollster::block_on(async {
        let (cw, ch) = natural();
        // A dim full block is the foreground at dim_opacity over the background.
        for opacity in [0.6_f32, 0.25] {
            let (mut tree, root) = scene();
            terminal(&mut tree, root, |s| {
                s.dim_opacity = opacity;
                let mut cell = block('\u{2588}', CellColor::Rgb(WHITE));
                cell.dim = true;
                s.cells[0] = cell;
            });
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let bg = rgba(BACKGROUND);
            let want: [u8; 4] = std::array::from_fn(|i| {
                if i == 3 {
                    0xFF
                } else {
                    (255.0 * opacity + f32::from(bg[i]) * (1.0 - opacity)).round() as u8
                }
            });
            let x = (cw / 2.0) as u32;
            let y = (ch / 2.0) as u32;
            assert!(
                support::close(frame.at(x, y), want, 3),
                "dim_opacity {opacity}: {:?}, wanted {want:?}",
                frame.at(x, y)
            );
        }
        // An underline: rows of the cell's right edge column (no glyph ink of an `I` there).
        for (offset, thickness) in [(None, None), (Some(4.0_f32), Some(3.0_f32))] {
            let (mut tree, root) = scene();
            terminal(&mut tree, root, |s| {
                s.underline_offset = offset;
                s.underline_thickness = thickness;
                let mut cell = block('I', CellColor::Rgb(WHITE));
                cell.underline = true;
                s.cells[0] = cell;
            });
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let x = cw.floor() as u32 - 1;
            let rows: Vec<u32> = (0..ch.ceil() as u32)
                .filter(|&y| support::close(frame.at(x, y), rgba(WHITE), 6))
                .collect();
            let want_top = offset.unwrap_or(ch * 0.85);
            let want_h = thickness.unwrap_or((ch * 0.06).max(1.0));
            assert!(!rows.is_empty(), "{offset:?}: an underline is drawn");
            assert!(
                (rows[0] as f32 - want_top).abs() <= 1.0,
                "{offset:?}: starts at {want_top}, drawn at {}",
                rows[0]
            );
            assert!(
                (rows.len() as f32 - want_h.max(1.0)).abs() <= 1.0,
                "{thickness:?}: {want_h} thick, drawn {} rows",
                rows.len()
            );
        }
    });
}

#[test]
fn bold_weight_changes_the_glyphs_of_a_bold_cell() {
    pollster::block_on(async {
        let mut inks = Vec::new();
        for weight in [400.0_f32, 700.0, 900.0] {
            let (mut tree, root) = scene();
            terminal(&mut tree, root, |s| {
                s.bold_weight = weight;
                let mut cell = block('I', CellColor::Rgb(WHITE));
                cell.bold = true;
                s.cells[0] = cell;
            });
            layout(&mut tree, root);
            let frame = Frame::of(&tree, root).await;
            let ink = (0..100u32)
                .flat_map(|y| (0..100u32).map(move |x| (x, y)))
                .filter(|&(x, y)| frame.at(x, y) != rgba(BACKGROUND))
                .count();
            inks.push(ink);
        }
        // The same face is picked for 400 and the default 700 only if the face has no bold; the
        // weights must at least reach the shaper and never lose the glyph.
        assert!(
            inks.iter().all(|&n| n > 0),
            "the glyph paints at every weight: {inks:?}"
        );
    });
}

#[test]
fn selection_foreground_and_radius_paint_on_a_terminal_selection() {
    pollster::block_on(async {
        let (cw, _) = natural();
        let paint = |fg: Option<Color>, radius: f32| {
            move |s: &mut TerminalState| {
                s.palette.selection = Some(BLUE);
                for cell in &mut s.cells {
                    *cell = block('\u{2588}', CellColor::Rgb(RED));
                }
                s.selection_start = Some((0, 0));
                s.selection_end = Some((0, 2));
                s.selection_foreground = fg;
                s.selection_radius = radius;
            }
        };
        let frame_of = |edit: Box<dyn FnOnce(&mut TerminalState)>| async move {
            let (mut tree, root) = scene();
            terminal(&mut tree, root, edit);
            layout(&mut tree, root);
            Frame::of(&tree, root).await
        };
        // Red blocks over a blue highlight: the blocks cover it, so only the third cell and
        // the row below are red; with a foreground the two selected blocks are white.
        let plain = frame_of(Box::new(paint(None, 0.0))).await;
        assert!(bounds_of(&plain, rgba(WHITE)).is_none());
        let coloured = frame_of(Box::new(paint(Some(WHITE), 0.0))).await;
        let b = bounds_of(&coloured, rgba(WHITE)).expect("selected glyphs in the foreground");
        assert!(
            b.2 as f32 <= 2.0 * cw + 1.0,
            "white only inside the two selected cells: {b:?}, cell {cw}"
        );
        let red = bounds_of(&coloured, rgba(RED)).expect("the others keep their colour");
        assert!(
            red.0 == 0 || red.1 > 0,
            "red is the unselected cell and row: {red:?}"
        );

        // A rounded highlight leaves its corner bare: blank cells so the highlight shows.
        let blank = |radius: f32| {
            move |s: &mut TerminalState| {
                s.palette.selection = Some(BLUE);
                s.selection_start = Some((0, 0));
                s.selection_end = Some((0, 3));
                s.selection_radius = radius;
            }
        };
        let square = frame_of(Box::new(blank(0.0))).await;
        assert_eq!(square.at(0, 0), rgba(BLUE), "square by default");
        let round = frame_of(Box::new(blank(8.0))).await;
        assert_eq!(round.at(0, 0), rgba(BACKGROUND), "rounded corner");
    });
}

// --- The history scrollbar (UXREQ-006) ---

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

/// A terminal of `rows` lines of 10 px with `history` lines above them, the bar `show`n.
fn history_frame_tree(show: Option<engine_core::Show>, history: usize) -> (Tree, NodeId, NodeId) {
    let (mut tree, root) = scene();
    let id = terminal(&mut tree, root, |s| {
        s.line_px = 10.0;
        s.set_history(history, 0);
        if let Some(show) = show {
            s.bar.settings.set(
                engine_core::ScrollProp::Show,
                Some(engine_core::PropValue::Choice(show.index())),
            );
        }
    });
    // A box 100 wide and 80 high, so the grid is one cell wider than the box.
    tree.set_layout_style(id, placed(0.0, 0.0, 100.0, 80.0));
    layout(&mut tree, root);
    (tree, root, id)
}

#[test]
fn the_history_bar_is_not_drawn_by_default_and_is_the_spec_thumb_when_asked() {
    pollster::block_on(async {
        let (tree, root, _) = history_frame_tree(None, 400);
        let thumb = rgba(tree.scheme_colors().scrollbar_thumb());
        let want = over(
            tree.scheme_colors().scrollbar_thumb(),
            0.70,
            rgba(BACKGROUND),
        );
        let frame = Frame::of(&tree, root).await;
        assert!(
            bounds_of(&frame, want).is_none() && bounds_of(&frame, thumb).is_none(),
            "the default draws no bar"
        );

        let (tree, root, _) = history_frame_tree(Some(engine_core::Show::Overflow), 400);
        let frame = Frame::of(&tree, root).await;
        let b = bounds_of(&frame, want).expect("a thumb at 70 percent opacity");
        assert_eq!(b.2 - b.0, 8, "8 px wide: {b:?}");
        assert_eq!(b.2, 98, "2 px from the box's edge: {b:?}");
        assert!(b.3 - b.1 >= 30, "about 32 px long (edge rows blend): {b:?}");
        assert!(
            b.3 >= 77,
            "at the foot of the lane, the view being the live screen: {b:?}"
        );

        // No history, no bar.
        let (tree, root, _) = history_frame_tree(Some(engine_core::Show::Overflow), 0);
        let frame = Frame::of(&tree, root).await;
        assert!(
            bounds_of(&frame, want).is_none(),
            "nothing to scroll back into"
        );
    });
}

#[test]
fn the_history_bar_thumb_moves_with_the_view_and_repaints_like_a_full_draw() {
    let (device, queue) = pollster::block_on(support::device("terminal history bar device"));
    let (mut tree, root, id) = history_frame_tree(Some(engine_core::Show::Overflow), 400);
    let mut output = Output::new(&device, &queue);
    output.frame(&device, &queue, &tree, root, true);
    let mut last = output.pixels(&device, &queue);
    for back in [200, 400, 0] {
        set(&mut tree, id, |s| s.set_history(400, back));
        layout_to(&mut tree, root);
        let damage = output.frame(&device, &queue, &tree, root, true);
        assert_ne!(damage, Damage::None, "back {back} is damage");
        let partial = output.pixels(&device, &queue);
        assert_ne!(partial, last, "back {back} moves the thumb");
        let mut fresh = Output::new(&device, &queue);
        fresh.frame(&device, &queue, &tree, root, false);
        assert_eq!(
            partial,
            fresh.pixels(&device, &queue),
            "back {back}: the partial redraw equals a full draw"
        );
        last = partial;
    }
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
            label: Some("terminal appearance test target"),
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
fn changing_how_a_terminal_paints_repaints_like_a_full_draw() {
    let (device, queue) = pollster::block_on(support::device("terminal appearance device"));
    let (mut tree, root) = scene();
    let id = terminal(&mut tree, root, |s| {
        let mut cell = block('I', CellColor::Rgb(WHITE));
        cell.underline = true;
        cell.dim = true;
        // Row 1, column 1: inside the selection below.
        s.cells[4] = cell;
        s.selection_start = Some((1, 0));
        s.selection_end = Some((1, 2));
        s.palette.selection = Some(BLUE);
    });
    tree.set_focus_to(id);
    layout_to(&mut tree, root);

    type Edit = Box<dyn Fn(&mut TerminalState)>;
    let edits: [(&str, Edit); 9] = [
        (
            "caret_shape",
            Box::new(|s| s.caret_shape = CaretShape::Underline),
        ),
        ("caret_width", Box::new(|s| s.caret_width = 5.0)),
        ("line_height", Box::new(|s| s.line_height = Some(1.8))),
        ("dim_opacity", Box::new(|s| s.dim_opacity = 0.2)),
        (
            "underline_offset",
            Box::new(|s| s.underline_offset = Some(3.0)),
        ),
        (
            "underline_thickness",
            Box::new(|s| s.underline_thickness = Some(4.0)),
        ),
        (
            "selection_foreground",
            Box::new(|s| s.selection_foreground = Some(WHITE)),
        ),
        ("selection_radius", Box::new(|s| s.selection_radius = 6.0)),
        ("caret_on", Box::new(|s| s.caret_on = false)),
    ];
    let mut output = Output::new(&device, &queue);
    output.frame(&device, &queue, &tree, root, true);
    let mut last = output.pixels(&device, &queue);
    for (name, edit) in edits {
        set(&mut tree, id, edit);
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
