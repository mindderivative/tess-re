//! 0.6.13: a multiline text input's own scrollbar. Off until `scrollbar_show` says otherwise;
//! then its thumb is the spec's (8 px, 32 px at least, 2 px margin) over the shaped text and its
//! padding as content, a drag, a lane click or an arrow writes the input's own scroll offsets (the
//! ones the caret-follow and the paint read), the wheel moves it while the bar is on, and the keys
//! stay the caret's.

use std::time::Instant;

use engine_core::{
    InputEvent, Key, NodeId, NodeKind, PaintProperties, PointerButton, PropValue, ScrollDelta,
    ScrollProp, Show, TextFieldState, Tree,
};
use peniko::Color;
use peniko::kurbo::Point;
use taffy::prelude::{AvailableSpace, Rect, Size, Style, length};

/// A 200 x 100 multiline input with 10 px of padding all round (a content box of 180 x 80) at
/// the corner of a 300 x 400 root, its shaped text measured as `extent`, `wrap` as given.
fn field(extent: (f64, f64), wrap: Option<bool>, multiline: bool) -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let root = tree.insert(
        NodeKind::Container,
        Style {
            size: Size {
                width: length(300.0),
                height: length(400.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
    );
    let mut state = TextFieldState::new("text", "Roboto", 400.0, 14.0);
    state.multiline = multiline;
    state.wrap = wrap;
    let id = tree.insert(
        NodeKind::TextField(Box::new(state)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            padding: Rect {
                left: length(10.0),
                right: length(10.0),
                top: length(10.0),
                bottom: length(10.0),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0),
    );
    tree.add_child(root, id);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(400.0),
        },
    );
    tree.set_text_field_extent(id, extent, 1);
    (tree, root, id)
}

fn show(tree: &mut Tree, id: NodeId, show: Show) {
    let NodeKind::TextField(state) = &mut tree.get_mut(id).unwrap().kind else {
        unreachable!()
    };
    assert!(
        state
            .bar
            .settings
            .set(ScrollProp::Show, Some(PropValue::Choice(show.index())))
    );
}

fn state(tree: &Tree, id: NodeId) -> &TextFieldState {
    match &tree.get(id).unwrap().kind {
        NodeKind::TextField(state) => state,
        _ => unreachable!(),
    }
}

fn state_mut(tree: &mut Tree, id: NodeId) -> &mut TextFieldState {
    match &mut tree.get_mut(id).unwrap().kind {
        NodeKind::TextField(state) => state,
        _ => unreachable!(),
    }
}

fn press(tree: &mut Tree, root: NodeId, at: Point) {
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: at,
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
}

fn moved(tree: &mut Tree, root: NodeId, at: Point) {
    tree.dispatch(
        root,
        InputEvent::PointerMoved { position: at },
        Instant::now(),
    );
}

fn release(tree: &mut Tree, root: NodeId, at: Point) {
    tree.dispatch(
        root,
        InputEvent::PointerReleased {
            position: at,
            button: PointerButton::Primary,
        },
        Instant::now(),
    );
}

fn wheel(tree: &mut Tree, root: NodeId, lines: f64) {
    tree.dispatch(
        root,
        InputEvent::Scroll {
            delta: ScrollDelta::Lines(0.0, lines),
            position: Point::new(100.0, 50.0),
        },
        Instant::now(),
    );
}

#[test]
fn the_bar_is_off_by_default_and_a_press_on_its_place_is_the_inputs() {
    let (mut tree, root, id) = field((180.0, 1000.0), Some(true), true);
    let thumbs = tree.scroll_thumbs(id).expect("an input is a scroller");
    assert!(thumbs.iter().all(Option::is_none), "no bar until asked for");
    assert!(!state(&tree, id).bar_on());
    press(&mut tree, root, Point::new(196.0, 50.0));
    assert!(
        !tree.scrollbar_pressed(),
        "so a press at its edge is not the bar's"
    );
    wheel(&mut tree, root, -3.0);
    assert_eq!(
        state(&tree, id).scroll_offset.current,
        0.0,
        "and the wheel is not the input's, as it was"
    );
}

#[test]
fn a_single_line_input_keeps_the_properties_and_shows_no_bar() {
    let (mut tree, _, id) = field((180.0, 1000.0), None, false);
    show(&mut tree, id, Show::Always);
    assert!(!state(&tree, id).bar_on());
    assert!(tree.scroll_thumbs(id).unwrap().iter().all(Option::is_none));
    // Made multiline afterwards, the bar is there: the order does not matter.
    let s = state_mut(&mut tree, id);
    s.multiline = true;
    s.wrap = Some(true);
    assert!(tree.scroll_thumbs(id).unwrap()[0].is_some());
}

#[test]
fn the_thumb_follows_the_spec_and_the_text_with_its_padding_as_content() {
    let (mut tree, _, id) = field((180.0, 380.0), Some(true), true);
    show(&mut tree, id, Show::Overflow);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].expect("a bar over text taller than the box");
    // The viewport is the 100 px box of a content of 380 + 2 x 10: the lane (100 less two 2 px
    // margins) times that share, not under the 32 px minimum; 8 px wide, 2 px from the edge.
    let want = ((100.0f64 - 4.0) * 100.0 / 400.0).max(32.0);
    assert!(
        (thumb.length - want).abs() < 1e-6,
        "{} against {want}",
        thumb.length
    );
    assert_eq!(thumb.thumb.width(), 8.0);
    assert_eq!(thumb.thumb.x1, 200.0 - 2.0);
    assert_eq!(thumb.max_scroll, 300.0);
    assert!(
        (thumb.thumb.y0 - 2.0).abs() < 1e-6,
        "at the top: {:?}",
        thumb.thumb
    );
    // Scrolled to the end, the thumb is at the foot of the lane.
    state_mut(&mut tree, id).scroll_offset.current = 300.0;
    let thumb = tree.scroll_thumbs(id).unwrap()[0].unwrap();
    assert!(
        (thumb.thumb.y1 - 98.0).abs() < 1e-6,
        "at the foot: {:?}",
        thumb.thumb
    );
    // Text that fits shows no bar for `overflow`, and one for `always`.
    tree.set_text_field_extent(id, (180.0, 60.0), 2);
    assert!(tree.scroll_thumbs(id).unwrap()[0].is_none());
    show(&mut tree, id, Show::Always);
    assert!(tree.scroll_thumbs(id).unwrap()[0].is_some());
}

#[test]
fn a_wrapped_input_has_a_vertical_bar_and_an_unwrapped_one_a_horizontal_one_too() {
    let (mut tree, _, wrapped) = field((180.0, 380.0), Some(true), true);
    show(&mut tree, wrapped, Show::Overflow);
    let thumbs = tree.scroll_thumbs(wrapped).unwrap();
    assert!(thumbs[0].is_some() && thumbs[1].is_none());

    let (mut tree, _, plain) = field((500.0, 380.0), None, true);
    show(&mut tree, plain, Show::Overflow);
    let thumbs = tree.scroll_thumbs(plain).unwrap();
    let (v, h) = (thumbs[0].expect("vertical"), thumbs[1].expect("horizontal"));
    assert_eq!(h.max_scroll, 500.0 + 20.0 - 200.0);
    assert!(
        v.thumb.y1 < h.thumb.y0,
        "the lanes stop short of the corner"
    );
    // Only wide text: the horizontal bar alone.
    tree.set_text_field_extent(plain, (500.0, 40.0), 2);
    let thumbs = tree.scroll_thumbs(plain).unwrap();
    assert!(thumbs[0].is_none() && thumbs[1].is_some());
}

#[test]
fn dragging_the_thumb_writes_the_inputs_own_offset() {
    let (mut tree, root, id) = field((180.0, 380.0), Some(true), true);
    show(&mut tree, id, Show::Overflow);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].unwrap();
    let (x, y) = (thumb.thumb.center().x, thumb.thumb.center().y);
    press(&mut tree, root, Point::new(x, y));
    assert!(tree.scrollbar_pressed(), "the press is the bar's");
    moved(&mut tree, root, Point::new(x, y + 20.0));
    // 20 px of thumb travel is 20 / travel of the 300 px the text scrolls.
    let want = 20.0 * 300.0 / thumb.travel();
    let got = state(&tree, id).scroll_offset.current;
    assert!((got - want).abs() < 1e-6, "{got} against {want}");
    moved(&mut tree, root, Point::new(x, y + 5000.0));
    assert_eq!(
        state(&tree, id).scroll_offset.current,
        300.0,
        "to the end, no further"
    );
    release(&mut tree, root, Point::new(x, y + 5000.0));
    // The horizontal offset was never touched.
    assert_eq!(state(&tree, id).horizontal_scroll_offset.current, 0.0);
}

#[test]
fn a_lane_click_pages_the_inputs_offset() {
    let (mut tree, root, id) = field((180.0, 1000.0), Some(true), true);
    show(&mut tree, id, Show::Overflow);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].unwrap();
    // Below the thumb, in the lane: a page of `page_fraction` (0.9) of the 100 px box.
    let at = Point::new(thumb.lane.center().x, 90.0);
    press(&mut tree, root, at);
    release(&mut tree, root, at);
    let got = state(&tree, id).scroll_offset.current;
    assert!((got - 90.0).abs() < 1e-6, "{got}");
    assert_eq!(state(&tree, id).selection_anchor, None);
}

#[test]
fn the_wheel_moves_the_input_while_its_bar_is_on_and_the_keys_never_do() {
    let (mut tree, root, id) = field((180.0, 1000.0), Some(true), true);
    show(&mut tree, id, Show::Overflow);
    // The wheel's sign is the other way round to the offset's.
    wheel(&mut tree, root, -1.0);
    let after = state(&tree, id).scroll_offset.current;
    assert!(after > 0.0, "the wheel scrolled it: {after}");
    wheel(&mut tree, root, 1.0);
    assert_eq!(state(&tree, id).scroll_offset.current, 0.0, "and back");
    // The arrows, Page Up and Page Down, Home and End are the caret's.
    for key in [
        Key::ArrowDown,
        Key::PageDown,
        Key::End,
        Key::ArrowUp,
        Key::Home,
    ] {
        assert_eq!(tree.scroll_view_for_key(id, key), None, "{key:?}");
    }
}

#[test]
fn shorter_text_pulls_the_scroll_back_to_what_is_left() {
    let (mut tree, _, id) = field((180.0, 1000.0), Some(true), true);
    show(&mut tree, id, Show::Overflow);
    state_mut(&mut tree, id).scroll_offset.current = 900.0;
    tree.set_text_field_extent(id, (180.0, 300.0), 2);
    // 300 + 20 of content over 100: 220 is the furthest.
    assert_eq!(state(&tree, id).scroll_offset.current, 220.0);
    // The same answer again is no change.
    tree.take_dirty();
    tree.set_text_field_extent(id, (180.0, 300.0), 2);
    assert!(!tree.take_dirty());
}
