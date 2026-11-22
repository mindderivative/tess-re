//! 0.6.12 (UXREQ-006): a terminal's history scrollbar. Off until `scrollbar_show` says
//! otherwise; then its thumb is the spec's (8 px, 32 px at least, 2 px margin) over the history
//! and the screen as content, a drag, a lane click or an arrow moves the view through the
//! terminal's `scroll` (which the session follows), and the wheel and the keys stay the
//! terminal's own.

use std::time::Instant;

use engine_core::{
    InputEvent, Key, NodeId, NodeKind, PaintProperties, PointerButton, PropValue, ScrollDelta,
    ScrollProp, Show, TerminalState, Tree,
};
use peniko::Color;
use peniko::kurbo::Point;
use taffy::prelude::{AvailableSpace, Size, Style, length};

const LINE: f32 = 10.0;

/// A `20 x rows` terminal a line of which is 10 px high, with `history` lines above the screen,
/// the view at the live screen.
fn terminal(rows: u16, history: usize) -> (Tree, NodeId, NodeId) {
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
    let mut state = TerminalState::new(20, rows, "Hack Nerd Font Mono", 8.0);
    state.line_px = LINE;
    state.set_history(history, 0);
    let id = tree.insert(
        NodeKind::Terminal(Box::new(state)),
        Style {
            size: Size {
                width: length(200.0),
                height: length(f32::from(rows) * LINE),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0),
    );
    tree.add_child(root, id);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(300.0),
            height: AvailableSpace::Definite(400.0),
        },
    );
    (tree, root, id)
}

fn show(tree: &mut Tree, id: NodeId, show: Show) {
    if let NodeKind::Terminal(state) = &mut tree.get_mut(id).unwrap().kind {
        assert!(
            state
                .bar
                .settings
                .set(ScrollProp::Show, Some(PropValue::Choice(show.index())))
        );
    }
}

fn state(tree: &Tree, id: NodeId) -> &TerminalState {
    match &tree.get(id).unwrap().kind {
        NodeKind::Terminal(state) => state,
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

#[test]
fn the_bar_is_off_by_default_and_a_press_on_its_place_is_the_terminals() {
    let (mut tree, root, id) = terminal(24, 500);
    let thumbs = tree.scroll_thumbs(id).expect("a terminal is a scroller");
    assert!(thumbs.iter().all(Option::is_none), "no bar until asked for");
    press(&mut tree, root, Point::new(196.0, 230.0));
    assert!(
        !tree.scrollbar_pressed(),
        "so a press at its edge is not the bar's"
    );
    assert_eq!(state(&tree, id).history_back_wanted(), 0);
}

#[test]
fn the_thumb_follows_the_spec_and_the_history_as_content() {
    let (mut tree, _, id) = terminal(24, 500);
    show(&mut tree, id, Show::Overflow);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].expect("a bar over 500 lines of history");
    // The viewport is 24 lines of a content of 524: the lane (240 less two 2 px margins) times
    // that share, not under the 32 px minimum; the spec's 8 px wide, 2 px from the edge.
    let lane: f64 = 240.0 - 2.0 * 2.0;
    let want = (lane * 24.0 / 524.0).max(32.0);
    assert!(
        (thumb.length - want).abs() < 1e-6,
        "{} against {want}",
        thumb.length
    );
    assert_eq!(thumb.thumb.width(), 8.0);
    assert_eq!(thumb.thumb.x1, 200.0 - 2.0);
    assert_eq!(thumb.max_scroll, 500.0 * f64::from(LINE));
    // At the live screen the thumb is at the foot of the lane.
    assert!(
        (thumb.thumb.y1 - (240.0 - 2.0)).abs() < 1e-6,
        "{:?}",
        thumb.thumb
    );
}

#[test]
fn a_terminal_without_history_shows_no_bar_unless_always() {
    let (mut tree, _, id) = terminal(24, 0);
    show(&mut tree, id, Show::Overflow);
    assert!(tree.scroll_thumbs(id).unwrap()[0].is_none());
    show(&mut tree, id, Show::Always);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].expect("always shows a bar");
    assert!(
        (thumb.length - thumb.track).abs() < 1e-6,
        "as long as its lane"
    );
}

#[test]
fn a_thumb_drag_asks_the_session_for_the_line_under_it() {
    let (mut tree, root, id) = terminal(24, 500);
    show(&mut tree, id, Show::Overflow);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].unwrap();
    let grab = Point::new(
        thumb.thumb.x0 + 2.0,
        f64::midpoint(thumb.thumb.y0, thumb.thumb.y1),
    );
    press(&mut tree, root, grab);
    assert!(tree.scrollbar_pressed(), "the thumb takes the press");
    // Half the thumb's travel toward the top is half of the history.
    let up = thumb.travel() / 2.0;
    moved(&mut tree, root, Point::new(grab.x, grab.y - up));
    let back = state(&tree, id).history_back_wanted();
    assert!(
        (back as i64 - 250).abs() <= 1,
        "{back} lines back, wanted 250"
    );
    // The whole way is the whole history, and no further.
    moved(&mut tree, root, Point::new(grab.x, grab.y - 1000.0));
    assert_eq!(state(&tree, id).history_back_wanted(), 500);
    release(&mut tree, root, Point::new(grab.x, grab.y - 1000.0));
    // The session reports it got there; the bar stays.
    if let NodeKind::Terminal(s) = &mut tree.get_mut(id).unwrap().kind {
        s.set_history(500, 500);
    }
    assert_eq!(state(&tree, id).history_back_wanted(), 500);
    let thumb = tree.scroll_thumbs(id).unwrap()[0].unwrap();
    assert!(
        (thumb.thumb.y0 - 2.0).abs() < 1e-6,
        "the thumb is at the top: {:?}",
        thumb.thumb
    );
}

#[test]
fn a_lane_click_pages_toward_the_press() {
    let (mut tree, root, id) = terminal(24, 500);
    show(&mut tree, id, Show::Overflow);
    // The lane above the thumb, which sits at the foot: a page toward the start is `page_fraction`
    // (0.9) of the 24 lines, 21.6 of them, rounded to a line by the session.
    press(&mut tree, root, Point::new(196.0, 20.0));
    assert!(tree.scrollbar_pressed(), "the lane takes the press");
    let back = state(&tree, id).history_back_wanted();
    assert!((back as i64 - 22).abs() <= 1, "{back} lines back");
    release(&mut tree, root, Point::new(196.0, 20.0));
}

#[test]
fn the_session_owns_the_wheel_and_the_keys() {
    let (mut tree, root, id) = terminal(24, 500);
    show(&mut tree, id, Show::Always);
    tree.dispatch(
        root,
        InputEvent::Scroll {
            delta: ScrollDelta::Lines(0.0, 3.0),
            position: Point::new(50.0, 50.0),
        },
        Instant::now(),
    );
    assert_eq!(
        state(&tree, id).history_back_wanted(),
        0,
        "the wheel is the session's"
    );
    tree.set_focus_to(id);
    assert_eq!(
        tree.scroll_view_for_key(id, Key::PageUp),
        None,
        "and the keys the shell's"
    );
}

#[test]
fn the_view_follows_the_session_unless_the_thumb_is_between_two_lines() {
    let (mut tree, _, id) = terminal(24, 500);
    let NodeKind::Terminal(s) = &mut tree.get_mut(id).unwrap().kind else {
        unreachable!()
    };
    s.set_history(500, 100);
    assert_eq!(s.scroll.current, 400.0 * f64::from(LINE));
    // A thumb dragged to 4004 px (a line and a bit) keeps its place when the session says it
    // is on that line.
    s.scroll.current = 4004.0;
    assert_eq!(s.history_back_wanted(), 100);
    s.set_history(500, 100);
    assert_eq!(s.scroll.current, 4004.0);
    // New output grew the history by ten lines and the session held the view on its line (so it
    // is ten lines further from the top): the thumb moves with it.
    s.set_history(510, 100);
    assert_eq!(s.scroll.current, 410.0 * f64::from(LINE));
    // A new line size rescales the offset: the same line, in the new unit.
    s.line_px = 20.0;
    s.set_history(510, 100);
    assert_eq!(s.scroll.current, 410.0 * 20.0);
}
