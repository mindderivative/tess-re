//! 0.6.13: the children of a box or container -- of a scroll view, the children of its content --
//! as the items of a selection (UX spec "Selection"): `selection` is a list of those nodes,
//! `selection_anchor` one of them or `None`.
//! The other selection properties are the virtual list's (`list_selection.rs`); the rules are the
//! engine's (`Tree::list_selection_key` and `engine_core::SelectionState` over the children's
//! indices). A child is marked unselectable with `item_selectable=False` -- or is invisible.

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{NodeId, RangeSet, SelectionMode, SelectionSource, Tree};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::node::Node;
use crate::node_kind_props::{KindChange, change};

const EXPECTED: &str = "a list of child nodes of this node, or None";

fn invalid() -> PyErr {
    PyValueError::new_err(format!("node property `selection` must be {EXPECTED}"))
}

/// The nodes `value` lists, with the tree each belongs to.
fn nodes_of(value: &Bound<'_, PyAny>) -> PyResult<Vec<(NodeId, Rc<RefCell<Tree>>)>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    if value.is_instance_of::<pyo3::types::PyString>() || value.cast::<Node>().is_ok() {
        return Err(invalid());
    }
    let mut nodes = Vec::new();
    for item in value.try_iter().map_err(|_| invalid())? {
        let node: PyRef<'_, Node> = item?.extract().map_err(|_| invalid())?;
        nodes.push((node.id, node.tree.clone()));
    }
    Ok(nodes)
}

/// `selection` of a container: its shape is checked here, and nothing changes -- which nodes
/// are the items is the tree's to say, so `check` makes the change.
pub(crate) fn parse_selection(value: &Bound<'_, PyAny>) -> PyResult<KindChange> {
    nodes_of(value)?;
    change(|_| {})
}

/// Checks the `selection` of a `set` call on container `host` against the tree -- every node must
/// be an item of `host` that can be chosen, one at most in a `"single"` container, none without a
/// mode -- and makes its change, applied after the rest of the call. `shared` is the tree
/// `Node.set` runs in. `None` if the call has no `selection` for a container.
pub(crate) fn check(
    tree: &Tree,
    shared: &Rc<RefCell<Tree>>,
    host: NodeId,
    props: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<KindChange>> {
    let Some(value) = props
        .map(|p| p.get_item("selection"))
        .transpose()?
        .flatten()
    else {
        return Ok(None);
    };
    let Some(node) = tree.get(host).filter(|n| Tree::holds_items(&n.kind)) else {
        return Ok(None);
    };
    let nodes = nodes_of(&value)?;
    let mode = match props
        .map(|p| p.get_item("selection_mode"))
        .transpose()?
        .flatten()
    {
        Some(value) => crate::list_selection::parse_mode("selection_mode", &value)
            .unwrap_or(SelectionMode::None),
        None => node
            .selection
            .as_deref()
            .map_or(SelectionMode::None, |s| s.mode),
    };
    if mode == SelectionMode::None && !nodes.is_empty() {
        return Err(PyValueError::new_err(
            "node property `selection` needs a `selection_mode` of \"single\" or \"multiple\"",
        ));
    }
    let items = tree.container_items(host);
    let mut chosen = RangeSet::new();
    for (id, owner) in &nodes {
        if !Rc::ptr_eq(owner, shared) {
            return Err(PyValueError::new_err(
                "node property `selection`: the node belongs to a different Window",
            ));
        }
        let Some(index) = items.iter().position(|c| c == id) else {
            return Err(PyValueError::new_err(
                "node property `selection` has a node that is not a child of this node",
            ));
        };
        let child = tree.get(*id).expect("a child of a live node is alive");
        if !child.item_selectable || !child.visible {
            return Err(PyValueError::new_err(format!(
                "node property `selection` has child {index}, which cannot be chosen ({})",
                if child.item_selectable {
                    "it is not visible"
                } else {
                    "its `item_selectable` is False"
                }
            )));
        }
        chosen.insert(index..index + 1);
    }
    if mode == SelectionMode::Single && chosen.len() > 1 {
        return Err(PyValueError::new_err(
            "node property `selection` takes at most one node in a \"single\" container",
        ));
    }
    let mut set = change(move |node| {
        crate::list_selection::edit_selection(node, |sel| {
            sel.set_selected(chosen, SelectionSource::Api);
        });
    })?;
    set.late = true;
    Ok(Some(set))
}

/// A container is created with no children: a `selection` that lists any is refused.
pub(crate) fn check_created(
    kind: &engine_core::NodeKind,
    props: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if !Tree::holds_items(kind) {
        return Ok(());
    }
    match props.get_item("selection")? {
        Some(value) if !nodes_of(&value)?.is_empty() => Err(PyValueError::new_err(
            "node property `selection` has a node that is not a child of this node",
        )),
        _ => Ok(()),
    }
}

/// `selection` or `selection_anchor` of container `node`, as child nodes.
pub(crate) fn read(
    handle: &Node,
    tree: &Tree,
    node: &engine_core::Node,
    name: &str,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    let default = engine_core::SelectionState::default();
    let sel = node.selection.as_deref().unwrap_or(&default);
    let items = tree.container_items(node.id);
    let child = |index: usize| items.get(index).map(|&id| handle.handle_to(id));
    if name == "selection_anchor" {
        return pyo3::IntoPyObjectExt::into_py_any(sel.anchor().and_then(child), py);
    }
    let chosen: Vec<Node> = sel.selected().iter().filter_map(child).collect();
    pyo3::IntoPyObjectExt::into_py_any(chosen, py)
}
