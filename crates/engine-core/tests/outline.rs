//! 0.6.11 (UXREQ-008): which outline a node draws now. The plain one always; the focus one only
//! while the node has keyboard focus that is visible (a text input also for pointer focus); the
//! colour from the window's scheme when the node names none.

use engine_core::{
    NodeId, NodeKind, OutlineStyle, Outlines, PaintProperties, Scheme, SchemeColors, SchemeRole,
    TextFieldState, Tree,
};
use peniko::Color;
use taffy::prelude::{AvailableSpace, Size, Style, length};

fn sized(tree: &mut Tree, kind: NodeKind, outlines: Option<Outlines>) -> NodeId {
    let mut paint = PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 4.0, 1.0);
    paint.outline = outlines.map(Box::new);
    tree.insert(
        kind,
        Style {
            size: Size {
                width: length(40.0),
                height: length(20.0),
            },
            ..Default::default()
        },
        paint,
    )
}

fn ring() -> Outlines {
    Outlines {
        always: OutlineStyle::default(),
        focus: OutlineStyle {
            width: 3.0,
            offset: 2.0,
            ..Default::default()
        },
    }
}

/// A root with one focusable child `kind`, laid out and focused.
fn focused(kind: NodeKind) -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let root = sized(&mut tree, NodeKind::Container, None);
    let child = sized(&mut tree, kind, Some(ring()));
    tree.add_child(root, child);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(100.0),
            height: AvailableSpace::Definite(100.0),
        },
    );
    tree.set_focus_to(child);
    (tree, child)
}

#[test]
fn a_node_with_no_outline_draws_none() {
    let mut tree = Tree::new();
    let id = sized(&mut tree, NodeKind::Rect, None);
    assert!(tree.outline_of(id).is_none());
}

#[test]
fn the_focus_outline_shows_only_while_focus_is_visible() {
    let (mut tree, child) = focused(NodeKind::Rect);
    assert!(tree.focus_visible(), "focus starts visible");
    let shown = tree.outline_of(child).expect("keyboard focus shows it");
    assert_eq!((shown.width, shown.offset), (3.0, 2.0));

    tree.set_focus_visible(false);
    assert!(tree.outline_of(child).is_none(), "pointer focus hides it");

    tree.set_focus_visible(true);
    assert!(tree.outline_of(child).is_some(), "the next key shows it");

    tree.clear_focus();
    assert!(tree.outline_of(child).is_none(), "no focus, no ring");
}

#[test]
fn a_text_input_shows_its_focus_outline_for_pointer_focus_too() {
    let (mut tree, child) = focused(NodeKind::TextField(Box::new(TextFieldState::new(
        "", "Roboto", 400.0, 16.0,
    ))));
    tree.set_focus_visible(false);
    assert!(tree.outline_of(child).is_some());
}

#[test]
fn the_default_colour_is_the_scheme_role_of_the_window() {
    let (mut tree, child) = focused(NodeKind::Rect);
    for scheme in [Scheme::Light, Scheme::Dark] {
        tree.set_scheme_colors(SchemeColors::baseline(scheme));
        assert_eq!(
            tree.outline_of(child).unwrap().color,
            SchemeRole::Secondary.baseline(scheme)
        );
    }
    // A colour of the node's own wins.
    let own = Color::from_rgba8(1, 2, 3, 255);
    let node = tree.get_mut(child).unwrap();
    node.paint.outline.as_mut().unwrap().focus.color = Some(own);
    assert_eq!(tree.outline_of(child).unwrap().color, own);
}
