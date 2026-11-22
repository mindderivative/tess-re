//! 0.6.11 (P3): `vertical_align` for `text` and `text_input`. The lines sit at the top of the
//! content box (`start`), in the middle (`center`) or at the bottom (`end`), and the caret,
//! the selection rectangles, click-to-caret and the IME's area follow the same offset. A single
//! line input is centred by default, a multiline one starts at the top.
//!
//! Pixel tests find the inked rows of a render; geometry tests ask the renderer's own queries
//! (no GPU), which paint and input share.

use engine_core::{
    NodeId, NodeKind, PaintProperties, TextAlign, TextFieldState, TextState, Tree, VerticalAlign,
};
use engine_render::{FrameRenderer, GeometryCache, TextPlacement, TextRenderer, build_tree_scene};
use peniko::Color;
use peniko::kurbo::Point;
use taffy::prelude::{AvailableSpace, Size, Style, length};
use vello_gpu::RenderTargetConfig;

mod support;

const BACKGROUND: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0xFF);
const FILL: Color = Color::from_rgba8(0xEE, 0xEE, 0xEE, 0xFF);
const INK: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0xFF);

async fn render(tree: &Tree, root: NodeId, width: u16, height: u16) -> (Vec<u8>, u32) {
    let (device, queue) = support::device("engine-render text-valign test device").await;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text-valign test target"),
        size: wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut frame_renderer = FrameRenderer::new(
        &device,
        &queue,
        &RenderTargetConfig {
            format: texture.format(),
            width,
            height,
        },
    );
    let mut text_renderer = TextRenderer::new();
    let mut geometry_cache = GeometryCache::new();
    let scene = build_tree_scene(
        tree,
        root,
        width,
        height,
        &mut frame_renderer.resources_mut(),
        &mut text_renderer,
        &mut geometry_cache,
    );
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    frame_renderer.render(&scene, &mut encoder, &view);
    queue.submit([encoder.finish()]);
    let bytes_per_row = (u32::from(width) * 4).next_multiple_of(256);
    (
        support::read_texture(&device, &queue, &texture),
        bytes_per_row,
    )
}

/// The first and last rows (and the leftmost column) in `[x0, x1)` whose pixels are not `plain`.
fn inked_rows(
    data: &[u8],
    bpr: u32,
    (x0, x1): (u32, u32),
    height: u32,
    plain: [u8; 4],
) -> Option<(u32, u32)> {
    let mut rows = (0..height).filter(|&y| {
        (x0..x1).any(|x| {
            let at = (y * bpr + x * 4) as usize;
            data[at..at + 4] != plain
        })
    });
    let first = rows.next()?;
    Some((first, rows.next_back().unwrap_or(first)))
}

fn style(width: f32, height: f32, padding: f32) -> Style {
    Style {
        size: Size {
            width: length(width),
            height: length(height),
        },
        padding: taffy::Rect {
            left: length(padding),
            right: length(padding),
            top: length(padding),
            bottom: length(padding),
        },
        ..Default::default()
    }
}

fn root_with(tree: &mut Tree, width: f32, height: f32) -> NodeId {
    tree.insert(
        NodeKind::Rect,
        style(width, height, 0.0),
        PaintProperties::new(BACKGROUND, 0.0, 1.0),
    )
}

fn layout(tree: &mut Tree, root: NodeId, width: f32, height: f32) {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        },
    );
}

/// A 100x32 text input with padding 4 (the owner's repro: text sat high), unfocused.
fn input_tree(
    content: &str,
    align: Option<VerticalAlign>,
    multiline: bool,
) -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let root = root_with(&mut tree, 100.0, 32.0);
    let mut state = TextFieldState::new(content, "Roboto", 400.0, 16.0);
    state.vertical_align = align;
    state.multiline = multiline;
    let field = tree.insert(
        NodeKind::TextField(Box::new(state)),
        style(100.0, 32.0, 4.0),
        PaintProperties::new(FILL, 0.0, 1.0),
    );
    tree.add_child(root, field);
    layout(&mut tree, root, 100.0, 32.0);
    (tree, root, field)
}

fn input_rows(align: Option<VerticalAlign>, multiline: bool) -> (u32, u32) {
    pollster::block_on(async {
        let (tree, root, _) = input_tree("HEH", align, multiline);
        let (data, bpr) = render(&tree, root, 100, 32).await;
        inked_rows(&data, bpr, (4, 96), 32, [0xEE, 0xEE, 0xEE, 0xFF]).expect("the text is inked")
    })
}

/// The height of one line of 16 px Roboto text, as the shaped layout has it (the box the
/// alignment offsets by).
fn line_height() -> f64 {
    let options = engine_core::TextOptions::default();
    let font = engine_render::FontSpec {
        family: "Roboto",
        weight: 400.0,
        size: 16.0,
        line_height: None,
        options: &options,
    };
    f64::from(TextRenderer::new().measure("H", &font, None).1)
}

#[test]
fn a_single_line_input_centres_its_text_by_default() {
    let lh = line_height();
    let (start_top, _) = input_rows(Some(VerticalAlign::Start), false);
    let (top, bottom) = input_rows(None, false);
    let spare = 24.0 - lh;
    assert!(spare > 2.0, "the 24 px content box has room: line is {lh}");
    // Content box: 4..28. The default is `center`: the ink moved down by half the spare room.
    let moved = f64::from(top) - f64::from(start_top);
    assert!(
        (moved - spare / 2.0).abs() <= 1.0,
        "centred ink moved {moved}, expected {}",
        spare / 2.0
    );
    // And it is centred in the box: the ink's centre is near 16 (caps are not the line box).
    let centre = f64::from(top + bottom) / 2.0;
    assert!(
        (centre - 16.0).abs() <= 3.0,
        "ink centre {centre}, box centre 16"
    );
}

#[test]
fn start_and_end_put_the_input_text_at_the_top_and_bottom_of_the_content_box() {
    let lh = line_height();
    let (start_top, _) = input_rows(Some(VerticalAlign::Start), false);
    let (end_top, _) = input_rows(Some(VerticalAlign::End), false);
    let moved = f64::from(end_top) - f64::from(start_top);
    assert!(
        (moved - (24.0 - lh)).abs() <= 1.0,
        "end moved {moved}, expected {}",
        24.0 - lh
    );
    // Start is as before this property: the ink starts in the first rows of the content box.
    assert!(start_top >= 4 && f64::from(start_top) < 4.0 + lh / 2.0);
}

#[test]
fn a_multiline_input_starts_at_the_top_by_default_and_can_centre() {
    let (default_top, _) = input_rows(None, true);
    let (start_top, _) = input_rows(Some(VerticalAlign::Start), true);
    let (centre_top, _) = input_rows(Some(VerticalAlign::Center), true);
    assert_eq!(
        default_top, start_top,
        "a multiline input defaults to start"
    );
    assert!(centre_top > start_top);
}

#[test]
fn the_caret_selection_and_click_follow_the_centred_line() {
    let mut renderer = TextRenderer::new();
    let mut state = TextFieldState::new("hello", "Roboto", 400.0, 16.0);
    let lh = line_height();
    let placement = |box_height| TextPlacement {
        x: 4.0,
        y: 4.0,
        max_width: 92.0,
        color: INK,
        box_height,
    };
    // Without alignment the caret is at the top of the content box.
    let top = renderer.field_caret_rect(&state, placement(None));
    assert!(top.y0 < 4.0 + lh / 2.0, "{top:?}");
    // Centred (the default of a single-line input) it sits inside the centred line.
    let centred = renderer.field_caret_rect(&state, placement(Some(24.0)));
    let expected = top.y0 + (24.0 - lh) / 2.0;
    assert!(
        (centred.y0 - expected).abs() <= 1.0,
        "{centred:?} vs {expected}"
    );
    assert!(
        centred.y0 >= 4.0 && centred.y1 <= 29.0,
        "inside the content box: {centred:?}"
    );

    // The selection rectangles follow.
    let rects = renderer.field_range_rects(&state, placement(Some(24.0)), 1, 4);
    assert_eq!(rects.len(), 1);
    assert!((rects[0].y0 - expected).abs() < 1.0);

    // A click in the middle of the centred line lands in the text; at the very top of the
    // content box (above the line, in the padding of the centred layout) it still resolves to
    // the nearest position, and the horizontal answer is unchanged.
    let mid_y = expected + lh / 2.0;
    let click_start =
        renderer.hit_test_position(&state, placement(Some(24.0)), Point::new(4.0, mid_y));
    assert_eq!(click_start, 0);
    let click_end =
        renderer.hit_test_position(&state, placement(Some(24.0)), Point::new(95.0, mid_y));
    assert_eq!(click_end, 5);

    // The end alignment puts the caret at the bottom.
    state.vertical_align = Some(VerticalAlign::End);
    let bottom = renderer.field_caret_rect(&state, placement(Some(24.0)));
    assert!(
        (bottom.y1 - top.y1 - (24.0 - lh)).abs() <= 1.0,
        "{bottom:?}"
    );
}

#[test]
fn clicking_a_row_of_a_centred_multiline_input_hits_that_row() {
    let mut renderer = TextRenderer::new();
    let mut state = TextFieldState::new("ab\ncd", "Roboto", 400.0, 16.0);
    state.multiline = true;
    let lh = line_height();
    let box_height = 100.0;
    let placement = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: INK,
        box_height: Some(box_height),
    };
    let offset = f64::from(VerticalAlign::Center.offset(box_height, (2.0 * lh) as f32));
    // The middle of the first row, centred: the first row.
    let first = Point::new(1.0, offset + lh * 0.5);
    state.vertical_align = Some(VerticalAlign::Center);
    assert!(renderer.hit_test_position(&state, placement, first) <= 2);
    let second = Point::new(1.0, offset + lh * 1.5);
    assert!(renderer.hit_test_position(&state, placement, second) >= 3);
    // The same point with the text at the top lands below both rows: the last row.
    state.vertical_align = Some(VerticalAlign::Start);
    assert!(renderer.hit_test_position(&state, placement, first) >= 3);
    // A double click resolves against the centred rows too.
    state.vertical_align = Some(VerticalAlign::Center);
    let (_, start, end) = renderer
        .field_unit_at(&state, placement, first)
        .expect("a unit under the point");
    assert_eq!((start, end), (0, 2));
}

#[test]
fn letter_spacing_and_line_height_apply_to_a_text_input() {
    let mut renderer = TextRenderer::new();
    let mut state = TextFieldState::new("abc", "Roboto", 400.0, 16.0);
    let at = TextPlacement {
        x: 0.0,
        y: 0.0,
        max_width: 200.0,
        color: INK,
        box_height: None,
    };
    state.cursor = 3;
    let plain = renderer.field_caret_rect(&state, at);
    state.letter_spacing = 4.0;
    let spaced = renderer.field_caret_rect(&state, at);
    assert!(
        (spaced.x0 - plain.x0 - 12.0).abs() < 0.5,
        "{plain:?} {spaced:?}"
    );
    state.line_height = Some(2.0);
    let tall = renderer.field_caret_rect(&state, at);
    assert!(tall.height() > plain.height() * 1.5, "{plain:?} {tall:?}");
}

fn text_tree(align: VerticalAlign) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let root = root_with(&mut tree, 100.0, 60.0);
    let mut state = TextState {
        content: "HEH".to_string(),
        font_family: "Roboto".to_string(),
        font_weight: 400.0,
        font_size: 16.0,
        align: TextAlign::Start,
        line_height: None,
        options: Default::default(),
    };
    state.options.vertical_align = align;
    let label = tree.insert(
        NodeKind::Text(state),
        style(100.0, 60.0, 0.0),
        PaintProperties::new(FILL, 0.0, 1.0),
    );
    tree.add_child(root, label);
    layout(&mut tree, root, 100.0, 60.0);
    (tree, root)
}

#[test]
fn a_text_nodes_lines_sit_at_the_start_centre_or_end_of_its_box() {
    pollster::block_on(async {
        let mut rows = Vec::new();
        for align in [
            VerticalAlign::Start,
            VerticalAlign::Center,
            VerticalAlign::End,
        ] {
            let (tree, root) = text_tree(align);
            let (data, bpr) = render(&tree, root, 100, 60).await;
            rows.push(inked_rows(&data, bpr, (0, 100), 60, [0x11, 0x11, 0x11, 0xFF]).unwrap());
        }
        let (start, centre, end) = (rows[0], rows[1], rows[2]);
        let spare = 60.0 - line_height();
        assert!(start.0 <= 4, "start is at the top: {start:?}");
        let moved = |a: (u32, u32), b: (u32, u32)| f64::from(b.0) - f64::from(a.0);
        assert!(
            (moved(start, centre) - spare / 2.0).abs() <= 1.0,
            "{start:?} {centre:?}"
        );
        assert!(
            (moved(start, end) - spare).abs() <= 1.0,
            "{start:?} {end:?}"
        );
    });
}
