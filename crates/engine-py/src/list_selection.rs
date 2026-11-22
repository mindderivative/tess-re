//! 0.6.12: the selection of a `virtual_list` as properties, input and an event (UX spec "Selection",
//! AC-SLT-1 to AC-SLT-6 and 13). `selection_mode` (`"none"` by default: the engine chooses nothing and
//! handles no selection input), `selection` (the chosen rows' keys -- a row's key is its index --
//! settable), `selection_anchor` (read-only), `select_all_key`, `select_on_focus` and
//! `unselectable_keys`. The rules themselves live in `engine_core::SelectionState` and
//! `Tree::list_selection_key`; this module parses and reads the properties, hands the keys to the tree
//! when no node on the way uses them, and reports `selection_change`.

use engine_core::{
    DispatchOutcome, InputEvent, Key, NodeKind, RangeSet, SelectionMode, SelectionSource,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::dispatch::HandlerKey;
use crate::event::NodeContext;
use crate::listeners::{self, EventType};
use crate::node_kind_props::{KindChange, change};

/// The properties of this module that no other module lists (`selection` is a text property too).
pub(crate) const NAMES: [&str; 5] = [
    "selection_mode",
    "selection_anchor",
    "select_all_key",
    "select_on_focus",
    "unselectable_keys",
];

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

fn mode_names() -> String {
    SelectionMode::ALL
        .iter()
        .map(|(name, _)| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_mode(name: &str, value: &Bound<'_, PyAny>) -> PyResult<SelectionMode> {
    let text: String = value.extract().unwrap_or_default();
    SelectionMode::ALL
        .iter()
        .find(|(n, _)| *n == text)
        .map(|(_, mode)| *mode)
        .ok_or_else(|| invalid(name, &format!("one of {}", mode_names())))
}

/// `value` as a set of row keys below `count`: any iterable of ints.
fn parse_keys(name: &str, value: &Bound<'_, PyAny>, count: usize) -> PyResult<RangeSet> {
    let expected = "a list of row keys (ints from 0 to item_count - 1)";
    if value.is_instance_of::<pyo3::types::PyString>() {
        return Err(invalid(name, expected));
    }
    let mut keys = Vec::new();
    for key in value.try_iter().map_err(|_| invalid(name, expected))? {
        let key = key?;
        if key.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, expected));
        }
        let key: i64 = key.extract().map_err(|_| invalid(name, expected))?;
        match usize::try_from(key) {
            Ok(key) if key < count => keys.push(key),
            _ => {
                return Err(PyValueError::new_err(format!(
                    "node property `{name}` has an unknown key {key}: the list has {count} rows"
                )));
            }
        }
    }
    Ok(RangeSet::from_indices(keys))
}

/// What `props` or else the node says of the list: its rows, its mode and its unselectable rows --
/// the values `selection` is checked against, as they will be once the whole call is applied.
struct Effective {
    count: usize,
    mode: SelectionMode,
    unselectable: RangeSet,
}

fn effective(kind: &NodeKind, props: &Bound<'_, PyDict>) -> PyResult<Effective> {
    let NodeKind::VirtualList(state) = kind else {
        unreachable!("checked by `parse`");
    };
    let count = match props.get_item("item_count")? {
        Some(value) => value.extract().unwrap_or(state.item_count),
        None => state.item_count,
    };
    let mode = match props.get_item("selection_mode")? {
        Some(value) => parse_mode("selection_mode", &value).unwrap_or(state.selection.mode),
        None => state.selection.mode,
    };
    let unselectable = match props.get_item("unselectable_keys")? {
        Some(value) => parse_keys("unselectable_keys", &value, count)
            .unwrap_or_else(|_| state.selection.unselectable().clone()),
        None => state.selection.unselectable().clone(),
    };
    Ok(Effective {
        count,
        mode,
        unselectable,
    })
}

/// Parses property `name` for a node of `kind`, or `None` if it isn't one of these (a text's
/// `selection` is not). Applies to a virtual list only.
pub(crate) fn parse(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
    props: &Bound<'_, PyDict>,
) -> Option<PyResult<KindChange>> {
    let is_list = matches!(kind, NodeKind::VirtualList(_));
    if name == "selection" && !is_list || name != "selection" && !NAMES.contains(&name) {
        return None;
    }
    if !is_list {
        return Some(Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to a virtual_list node"
        ))));
    }
    Some(parse_known(name, value, kind, props))
}

fn parse_known(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
    props: &Bound<'_, PyDict>,
) -> PyResult<KindChange> {
    let api = SelectionSource::Api;
    match name {
        "selection_mode" => {
            let mode = parse_mode(name, value)?;
            change(move |node| {
                if let NodeKind::VirtualList(state) = &mut node.kind {
                    state.selection.set_mode(mode, api);
                }
            })
        }
        "select_all_key" | "select_on_focus" => {
            if !value.is_instance_of::<pyo3::types::PyBool>() {
                return Err(invalid(name, "a bool"));
            }
            let on: bool = value.extract()?;
            let all = name == "select_all_key";
            change(move |node| {
                if let NodeKind::VirtualList(state) = &mut node.kind {
                    if all {
                        state.selection.select_all_key = on;
                    } else {
                        state.selection.select_on_focus = on;
                    }
                }
            })
        }
        "selection_anchor" => Err(PyValueError::new_err(format!(
            "node property `{name}` is read-only"
        ))),
        "unselectable_keys" => {
            let count = effective(kind, props)?.count;
            let keys = if value.is_none() {
                RangeSet::new()
            } else {
                parse_keys(name, value, count)?
            };
            change(move |node| {
                if let NodeKind::VirtualList(state) = &mut node.kind {
                    state.selection.set_unselectable(keys, api);
                }
            })
        }
        _ => {
            // "selection": exactly these rows, which must exist, be selectable, and be one at most
            // in a single list. Applied after the rest of the call: it is checked against it.
            let Effective {
                count,
                mode,
                unselectable,
            } = effective(kind, props)?;
            let keys = if value.is_none() {
                RangeSet::new()
            } else {
                parse_keys(name, value, count)?
            };
            if mode == SelectionMode::None && !keys.is_empty() {
                return Err(PyValueError::new_err(
                    "node property `selection` needs a `selection_mode` of \"single\" or \"multiple\"",
                ));
            }
            if mode == SelectionMode::Single && keys.len() > 1 {
                return Err(PyValueError::new_err(
                    "node property `selection` takes at most one key in a \"single\" list",
                ));
            }
            if let Some(key) = keys.iter().find(|&k| unselectable.contains(k)) {
                return Err(PyValueError::new_err(format!(
                    "node property `selection` has key {key}, which is in `unselectable_keys`"
                )));
            }
            let mut set = change(move |node| {
                if let NodeKind::VirtualList(state) = &mut node.kind {
                    state.selection.set_selected(keys, api);
                }
            })?;
            set.late = true;
            Ok(set)
        }
    }
}

/// The value of property `name` of `node`, or `None` if it isn't one of these.
pub(crate) fn read(
    name: &str,
    node: &engine_core::Node,
    py: Python<'_>,
) -> Option<PyResult<Py<PyAny>>> {
    if name != "selection" && !NAMES.contains(&name) {
        return None;
    }
    let NodeKind::VirtualList(state) = &node.kind else {
        return (name != "selection").then(|| {
            Err(PyValueError::new_err(format!(
                "node property `{name}` applies only to a virtual_list node"
            )))
        });
    };
    let sel = &state.selection;
    let value = match name {
        "selection" => to_py(sel.selected().iter().collect::<Vec<_>>(), py),
        "selection_anchor" => to_py(sel.anchor(), py),
        "selection_mode" => {
            let mode = SelectionMode::ALL.iter().find(|(_, m)| *m == sel.mode);
            to_py(mode.map_or("none", |(name, _)| *name), py)
        }
        "select_all_key" => to_py(sel.select_all_key, py),
        "select_on_focus" => to_py(sel.select_on_focus, py),
        _ => to_py(sel.unselectable().iter().collect::<Vec<_>>(), py),
    };
    Some(value)
}

fn to_py<'py, T: IntoPyObject<'py>>(value: T, py: Python<'py>) -> PyResult<Py<PyAny>> {
    pyo3::IntoPyObjectExt::into_py_any(value, py)
}

/// A `selection_change` for every list whose chosen set changed since the last call
/// (`Tree::take_selection_changes`): once for a batch of changes, however many a key press or a
/// `set` made. Called after input and after each Python call that can change a selection.
pub(crate) fn fire_changes(ctx: &NodeContext<'_>, py: Python<'_>) {
    let changes = ctx.tree.borrow_mut().take_selection_changes();
    for change in changes {
        listeners::deliver(
            ctx,
            py,
            EventType::SelectionChange,
            change.list,
            None,
            |e| {
                e.selection = Some(change.selected.iter().collect());
                e.ranges = Some(
                    change
                        .selected
                        .ranges()
                        .iter()
                        .map(|r| (r.start, r.end))
                        .collect(),
                );
                e.count = u32::try_from(change.selected.len()).ok();
                e.anchor = change.anchor;
                e.source = Some(change.source.name().to_string());
            },
        );
    }
}

/// A key press, offered to the list that chooses rows around the focused node, unless a node
/// between the focused one and the list uses the key itself: a text input, or a node with its own
/// `key_down` listener (a slider in a row). A `key_down` listener on the list itself does not stop
/// it: it hears the keys of its rows and still lets the engine choose. Returns the dispatch outcome
/// (focus moving to a row is a `FocusChanged`; a Space that chose or toggled is no click) and whether
/// the list took the key, so that it does not also scroll the list.
pub(crate) fn key(
    ctx: &NodeContext<'_>,
    event: &InputEvent,
    outcome: DispatchOutcome,
) -> (DispatchOutcome, bool) {
    let InputEvent::KeyPressed { key, shift } = *event else {
        return (outcome, false);
    };
    {
        let tree = ctx.tree.borrow();
        let handlers = ctx.handlers.borrow();
        let Some(focused) = tree.focused() else {
            return (outcome, false);
        };
        let mut in_list = false;
        for id in tree.ancestors(focused) {
            let node = tree.get(id);
            if matches!(node.map(|n| &n.kind),
                Some(NodeKind::VirtualList(s)) if s.selection.mode != SelectionMode::None)
            {
                in_list = true;
                break;
            }
            let own = matches!(
                node.map(|n| &n.kind),
                Some(NodeKind::TextField(_) | NodeKind::Terminal(_))
            ) || handlers.contains_key(&(id, HandlerKey::Listener(EventType::KeyDown)));
            if own {
                return (outcome, false);
            }
        }
        if !in_list {
            return (outcome, false);
        }
    }
    let result = ctx.tree.borrow_mut().list_selection_key(key, shift);
    let outcome = match result.focus {
        Some((old, new)) => DispatchOutcome::FocusChanged { old, new },
        None if result.consumed && key == Key::Space => DispatchOutcome::None,
        None => outcome,
    };
    (outcome, result.consumed)
}
