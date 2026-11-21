//! 0.6.11 (P13): hover, pressed and focus state layers -- what drives the shown level, in what
//! order, and how it fades.

use super::*;
use peniko::Color;
use std::time::Duration;
use taffy::prelude::length;

fn grey(alpha: u8) -> Color {
    Color::from_rgba8(0, 0, 0, alpha)
}

fn boxed(width: f32, height: f32) -> (NodeKind, Style, PaintProperties) {
    (
        NodeKind::Rect,
        Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Default::default()
        },
        PaintProperties::new(Color::from_rgba8(200, 200, 200, 255), 0.0, 1.0),
    )
}

/// A 200x200 root holding a 100x100 `button` at the origin, itself holding a 40x40 `label`;
/// the button has the three layers (hover 8%, pressed 10%, focus 10%) and is focusable.
fn scene() -> (Tree, NodeId, NodeId, NodeId) {
    let mut tree = Tree::new();
    let (k, s, p) = boxed(200.0, 200.0);
    let root = tree.insert(k, s, p);
    let (k, s, p) = boxed(100.0, 100.0);
    let button = tree.insert(k, s, p);
    let (k, s, p) = boxed(40.0, 40.0);
    let label = tree.insert(k, s, p);
    tree.add_child(root, button);
    tree.add_child(button, label);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(200.0),
            height: AvailableSpace::Definite(200.0),
        },
    );
    tree.get_mut(button).unwrap().access.focusable = Some(true);
    tree.edit_state_layer(button, |l| {
        l.hover = Some(grey(20));
        l.pressed = Some(grey(26));
        l.focus = Some(grey(26));
    });
    (tree, root, button, label)
}

fn mv(tree: &mut Tree, root: NodeId, x: f64, y: f64, now: Instant) {
    tree.dispatch(
        root,
        InputEvent::PointerMoved {
            position: Point::new(x, y),
        },
        now,
    );
}

fn press(tree: &mut Tree, root: NodeId, x: f64, y: f64, now: Instant) {
    tree.dispatch(
        root,
        InputEvent::PointerPressed {
            position: Point::new(x, y),
            button: PointerButton::Primary,
        },
        now,
    );
}

fn alpha(tree: &Tree, id: NodeId) -> f32 {
    tree.get(id)
        .and_then(|n| n.paint.state_layer.as_deref())
        .map_or(0.0, |l| l.shown.current.components[3])
}

fn settle(tree: &mut Tree, t0: Instant) -> Instant {
    // The first tick starts the fades the events asked for, the second is long after.
    tree.tick_all(t0);
    let end = t0 + Duration::from_millis(500);
    tree.tick_all(end);
    end
}

#[test]
fn hover_on_a_child_shows_the_hover_layer_of_the_ancestor_with_one() {
    let (mut tree, root, button, label) = scene();
    let t0 = Instant::now();
    // The pointer is on the label (40x40, at the button's corner).
    mv(&mut tree, root, 10.0, 10.0, t0);
    assert_eq!(tree.hovered, Some(label));
    tree.tick_all(t0);
    let t1 = settle(&mut tree, t0);
    assert!((alpha(&tree, button) - grey(20).components[3]).abs() < 1e-6);
    // Leaving to the root (no layers there) fades the button's layer out.
    mv(&mut tree, root, 150.0, 150.0, t1);
    let t2 = settle(&mut tree, t1);
    assert!(alpha(&tree, button) < 1e-6);
    assert!(t2 > t1);
}

#[test]
fn a_node_without_layers_carries_none_and_a_layer_less_tree_notes_nothing() {
    let (mut tree, root, button, label) = scene();
    assert!(tree.get(label).unwrap().paint.state_layer.is_none());
    assert!(tree.get(root).unwrap().paint.state_layer.is_none());
    let t0 = Instant::now();
    tree.tick_all(t0);
    assert!(tree.state_layer_pending.is_empty());
    mv(&mut tree, root, 10.0, 10.0, t0);
    assert_eq!(tree.state_layer_pending, vec![button]);
}

#[test]
fn the_fade_takes_state_layer_ms_on_the_easing() {
    let (mut tree, root, button, _) = scene();
    let t0 = Instant::now();
    mv(&mut tree, root, 70.0, 70.0, t0);
    tree.tick_all(t0);
    assert!(alpha(&tree, button) < 1e-6, "starts from nothing");
    tree.tick_all(t0 + Duration::from_millis(50));
    let mid = alpha(&tree, button);
    assert!(mid > 0.0 && mid < grey(20).components[3], "{mid}");
    let (active, _) = tree.tick_all(t0 + Duration::from_millis(100));
    assert!(!active, "done at state_layer_ms");
    assert!((alpha(&tree, button) - grey(20).components[3]).abs() < 1e-6);
    // A custom duration and the linear easing.
    tree.edit_state_layer(button, |l| {
        l.ms = 200.0;
        l.easing = crate::Easing::Linear;
    });
    let t1 = t0 + Duration::from_secs(1);
    mv(&mut tree, root, 150.0, 150.0, t1);
    tree.tick_all(t1);
    tree.tick_all(t1 + Duration::from_millis(100));
    let half = alpha(&tree, button);
    assert!(
        (half - grey(20).components[3] / 2.0).abs() < 1e-3,
        "linear halfway: {half}"
    );
    tree.tick_all(t1 + Duration::from_millis(200));
    assert!(alpha(&tree, button) < 1e-6);
}

#[test]
fn pressed_beats_hover_and_returns_to_hover_on_release() {
    let (mut tree, root, button, _) = scene();
    let t0 = Instant::now();
    // A pointer press moves focus to the button; the host says that focus is not visible.
    tree.set_focus_visible(false);
    mv(&mut tree, root, 70.0, 70.0, t0);
    let t1 = settle(&mut tree, t0);
    press(&mut tree, root, 70.0, 70.0, t1);
    let t2 = settle(&mut tree, t1);
    let shown = tree
        .get(button)
        .unwrap()
        .paint
        .state_layer
        .as_ref()
        .unwrap()
        .visible();
    assert_eq!(shown, Some(grey(26)), "pressed, not hover");
    tree.dispatch(
        root,
        InputEvent::PointerReleased {
            position: Point::new(70.0, 70.0),
            button: PointerButton::Primary,
        },
        t2,
    );
    settle(&mut tree, t2);
    let shown = tree
        .get(button)
        .unwrap()
        .paint
        .state_layer
        .as_ref()
        .unwrap()
        .visible();
    assert_eq!(shown, Some(grey(20)), "hover again");
}

#[test]
fn a_press_on_the_label_presses_the_button_around_it() {
    let (mut tree, root, button, _) = scene();
    let t0 = Instant::now();
    tree.set_focus_visible(false);
    mv(&mut tree, root, 10.0, 10.0, t0);
    press(&mut tree, root, 10.0, 10.0, t0);
    settle(&mut tree, t0);
    let shown = tree
        .get(button)
        .unwrap()
        .paint
        .state_layer
        .as_ref()
        .unwrap()
        .visible();
    assert_eq!(shown, Some(grey(26)));
}

#[test]
fn focus_shows_only_for_visible_focus_unless_asked_for_any_modality() {
    let (mut tree, _, button, _) = scene();
    let t0 = Instant::now();
    // Pointer-modality focus: nothing.
    tree.set_focus_visible(false);
    tree.set_focus_to(button);
    let t1 = settle(&mut tree, t0);
    assert!(alpha(&tree, button) < 1e-6);
    // The same focus, shown: the host says it is visible now.
    tree.set_focus_visible(true);
    let t2 = settle(&mut tree, t1);
    assert!((alpha(&tree, button) - grey(26).components[3]).abs() < 1e-6);
    // Losing focus fades it.
    tree.clear_focus();
    let t3 = settle(&mut tree, t2);
    assert!(alpha(&tree, button) < 1e-6);
    // A pointer focus with the flag set draws it.
    tree.set_focus_visible(false);
    tree.edit_state_layer(button, |l| l.focus_any_modality = true);
    tree.set_focus_to(button);
    settle(&mut tree, t3);
    assert!((alpha(&tree, button) - grey(26).components[3]).abs() < 1e-6);
}

#[test]
fn focus_beats_hover() {
    let (mut tree, root, button, _) = scene();
    let t0 = Instant::now();
    mv(&mut tree, root, 70.0, 70.0, t0);
    tree.set_focus_to(button);
    settle(&mut tree, t0);
    let shown = tree
        .get(button)
        .unwrap()
        .paint
        .state_layer
        .as_ref()
        .unwrap()
        .visible();
    assert_eq!(shown, Some(grey(26)));
}

#[test]
fn a_disabled_node_draws_none_and_gets_it_back_when_enabled() {
    let (mut tree, root, button, _) = scene();
    tree.get_mut(button).unwrap().access.states.disabled = true;
    let t0 = Instant::now();
    mv(&mut tree, root, 70.0, 70.0, t0);
    let t1 = settle(&mut tree, t0);
    assert!(alpha(&tree, button) < 1e-6);
    tree.get_mut(button).unwrap().access.states.disabled = false;
    tree.mark_state_layer(button);
    settle(&mut tree, t1);
    assert!((alpha(&tree, button) - grey(20).components[3]).abs() < 1e-6);
    // Disabling a node that shows a layer takes it away.
    tree.get_mut(button).unwrap().access.states.disabled = true;
    tree.mark_state_layer(button);
    tree.tick_all(t1 + Duration::from_secs(1));
    assert_eq!(
        tree.get(button)
            .unwrap()
            .paint
            .state_layer
            .as_ref()
            .unwrap()
            .shown
            .target()
            .components[3],
        0.0
    );
}

#[test]
fn reduced_motion_shows_the_layer_in_the_same_tick() {
    let (mut tree, root, button, _) = scene();
    tree.set_os_reduced_motion(true);
    let t0 = Instant::now();
    mv(&mut tree, root, 70.0, 70.0, t0);
    let (active, _) = tree.tick_all(t0);
    assert!(!active);
    assert!((alpha(&tree, button) - grey(20).components[3]).abs() < 1e-6);
}

#[test]
fn a_layer_set_while_hovered_applies_and_unset_layers_draw_nothing() {
    let (mut tree, root, _, label) = scene();
    let t0 = Instant::now();
    mv(&mut tree, root, 10.0, 10.0, t0);
    let t1 = settle(&mut tree, t0);
    assert!(alpha(&tree, label) < 1e-6);
    // The label gets a hover layer while the pointer is on it.
    tree.edit_state_layer(label, |l| l.hover = Some(grey(20)));
    settle(&mut tree, t1);
    assert!((alpha(&tree, label) - grey(20).components[3]).abs() < 1e-6);
    // A node with a state-layer record but no colours draws nothing however it is driven.
    let (mut tree, root, button, _) = scene();
    tree.edit_state_layer(button, |l| {
        l.hover = None;
        l.pressed = None;
        l.focus = None;
    });
    mv(&mut tree, root, 70.0, 70.0, t0);
    press(&mut tree, root, 70.0, 70.0, t0);
    settle(&mut tree, t0);
    assert!(
        tree.get(button)
            .unwrap()
            .paint
            .state_layer
            .as_ref()
            .unwrap()
            .visible()
            .is_none()
    );
}

#[test]
fn removing_the_hovered_child_takes_the_hover_layer_off_the_parent() {
    let (mut tree, root, button, label) = scene();
    let t0 = Instant::now();
    mv(&mut tree, root, 10.0, 10.0, t0);
    let t1 = settle(&mut tree, t0);
    assert!(alpha(&tree, button) > 0.0);
    tree.remove(label);
    settle(&mut tree, t1);
    assert!(alpha(&tree, button) < 1e-6);
}

#[test]
fn the_pointer_leaving_the_window_clears_the_hover_layer() {
    let (mut tree, root, button, _) = scene();
    let t0 = Instant::now();
    mv(&mut tree, root, 70.0, 70.0, t0);
    let t1 = settle(&mut tree, t0);
    tree.dispatch(root, InputEvent::PointerLeft, t1);
    settle(&mut tree, t1);
    assert!(alpha(&tree, button) < 1e-6);
}
