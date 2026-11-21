//! 0.6.11 (P12): `Window.set_defaults` and `App.set_defaults` -- the baseline look of each node
//! kind, declared once instead of passed to every `create`. Defaults are plain data (a dict of
//! properties per kind) merged into the props a node is created with, before they are parsed,
//! so every `create` check applies to them. They are read at creation only: a node that exists
//! is never changed by a later `set_defaults`.
//!
//! Precedence, strongest first: the props passed to `create` / `build`, the window's defaults for
//! the kind, the window's `"*"` defaults, the app's defaults for the kind, the app's `"*"`
//! defaults, the engine's own default.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use engine_core::{
    CanvasState, ImageState, ItemExtent, NodeKind, PaintProperties, PathData, PathState,
    ScrollViewState, TerminalState, TextAlign, TextFieldState, TextState, Tree, VirtualListState,
};
use engine_render::MONOSPACE_FONT_FAMILY;
use peniko::Color;
use peniko::kurbo::BezPath;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::node::Node;
use crate::node_props::{check_anchor_changes, parse_all};
use crate::window::PyWindow;

/// Every kind `create` builds.
pub(crate) const KINDS: [&str; 10] = [
    "box",
    "text",
    "text_input",
    "image",
    "path",
    "svg",
    "canvas",
    "scroll_view",
    "virtual_list",
    "terminal",
];

/// The kind name that stands for every kind.
const ANY: &str = "*";

/// A blank node of `kind` as `create` starts from, with its paint; `None` for an unknown kind.
/// A terminal has `cols` by `rows` cells.
pub(crate) fn blank_kind(kind: &str, cols: u16, rows: u16) -> Option<(NodeKind, PaintProperties)> {
    let transparent = PaintProperties::new(Color::from_rgba8(0, 0, 0, 0), 0.0, 1.0);
    // A text's fill is its glyph color. Until it is set, that is the window scheme's
    // `on_surface`, resolved at paint time (`scheme_fill`).
    let mut black = PaintProperties::new(Color::from_rgba8(0, 0, 0, 255), 0.0, 1.0);
    black.scheme_fill = true;
    Some(match kind {
        "box" => (NodeKind::Rect, transparent),
        "path" => (
            NodeKind::Path(PathState::new(PathData(
                BezPath::new(),
                engine_core::MorphMode::Resample,
            ))),
            transparent,
        ),
        "svg" => (NodeKind::Svg(engine_core::SvgState::empty()), transparent),
        // A text's fill is its glyph color: opaque black, like CSS.
        "text" => (
            NodeKind::Text(TextState {
                content: String::new(),
                font_family: "Roboto".to_string(),
                font_weight: 400.0,
                font_size: 16.0,
                align: TextAlign::Start,
                line_height: None,
                options: Default::default(),
            }),
            black,
        ),
        "text_input" => {
            // A text input's colours are the window scheme's until `fill` sets them.
            let mut state = TextFieldState::new("", "Roboto", 400.0, 16.0);
            state.tint_scheme = true;
            (NodeKind::TextField(Box::new(state)), transparent)
        }
        "image" => (NodeKind::Image(ImageState::blank()), transparent),
        "scroll_view" => (
            NodeKind::ScrollView(ScrollViewState::new(false)),
            transparent,
        ),
        "canvas" => (NodeKind::Canvas(CanvasState::new()), transparent),
        "virtual_list" => (
            NodeKind::VirtualList(VirtualListState::new(0, ItemExtent::Fixed(1.0))),
            transparent,
        ),
        "terminal" => (
            NodeKind::Terminal(Box::new(TerminalState::new(
                cols.max(1),
                rows.max(1),
                MONOSPACE_FONT_FAMILY,
                14.0,
            ))),
            transparent,
        ),
        _ => return None,
    })
}

/// A terminal's creation-only properties: given to `create`, not parsed as node properties.
fn is_terminal_creation_prop(name: &str) -> bool {
    matches!(name, "shell" | "scrollback_lines")
}

/// The defaults of one scope (a window, or an app): a dict of properties per kind name.
#[derive(Default)]
pub(crate) struct Defaults {
    kinds: BTreeMap<String, Py<PyDict>>,
    /// `(property, kind)` pairs of the `"*"` defaults that do not apply to that kind: skipped.
    star_skips: HashSet<(String, &'static str)>,
}

pub(crate) type SharedDefaults = Rc<RefCell<Defaults>>;

/// One checked entry of a `set_defaults` call, not yet stored.
struct Entry<'py> {
    kind: String,
    name: String,
    /// `None` removes the default.
    value: Option<Bound<'py, PyAny>>,
    /// For a `"*"` entry: the kinds it does not apply to.
    skips: Vec<&'static str>,
}

/// What checking one property against one kind found.
enum Fit {
    Yes,
    /// The kind has no such property; the error says so.
    No(PyErr),
}

/// Whether `err` says the property does not exist for the kind (as opposed to a bad value).
fn says_not_applicable(err: &PyErr, py: Python<'_>) -> bool {
    let message = err.value(py).to_string();
    message.starts_with("unknown node property") || message.contains("applies only to")
}

/// Checks that `name` = `value` is a property of a node of `kind` with a good value, by parsing
/// it for a blank node of the kind, exactly as `create` would.
fn fit(
    py: Python<'_>,
    tree: &Rc<RefCell<Tree>>,
    kind: &str,
    name: &str,
    value: &Bound<'_, PyAny>,
) -> PyResult<Fit> {
    if is_terminal_creation_prop(name) {
        if kind != "terminal" {
            return Ok(Fit::No(PyValueError::new_err(format!(
                "node property `{name}` applies only to a terminal node"
            ))));
        }
        let ok = if name == "shell" {
            value.extract::<String>().is_ok()
        } else {
            value.extract::<usize>().is_ok() && !value.is_instance_of::<pyo3::types::PyBool>()
        };
        return if ok {
            Ok(Fit::Yes)
        } else {
            Err(PyValueError::new_err(format!(
                "create(\"terminal\"): `{name}` must be {}",
                if name == "shell" {
                    "a str"
                } else {
                    "a non-negative int"
                }
            )))
        };
    }
    let (node_kind, _) = blank_kind(kind, 1, 1).expect("a known kind");
    let one = PyDict::new(py);
    one.set_item(name, value)?;
    match parse_all(Some(&one), &node_kind, tree) {
        Ok(changes) => {
            check_anchor_changes(&tree.borrow(), None, false, &changes)?;
            Ok(Fit::Yes)
        }
        Err(err) if says_not_applicable(&err, py) => Ok(Fit::No(err)),
        Err(err) => Err(err),
    }
}

fn unknown_kind(what: &str, kind: &str) -> PyErr {
    PyValueError::new_err(format!(
        "{what}: unknown node kind {kind:?} -- a kind is one of: *, {}",
        KINDS.join(", ")
    ))
}

fn prefixed(kind: &str, err: &PyErr, py: Python<'_>) -> PyErr {
    PyValueError::new_err(format!("set_defaults({kind:?}): {}", err.value(py)))
}

impl Defaults {
    /// Checks every property of the call -- `first` is a kind name (with `props` its
    /// properties) or a dict of kind name to properties dict -- and stores them all, or none.
    /// `plain_data` is for an app, which has no window to hold nodes: a `Node` is refused.
    pub(crate) fn set(
        &mut self,
        tree: &Rc<RefCell<Tree>>,
        plain_data: bool,
        first: &Bound<'_, PyAny>,
        props: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = first.py();
        let mut groups: Vec<(String, Bound<'_, PyDict>)> = Vec::new();
        if let Ok(all) = first.cast::<PyDict>() {
            if props.is_some_and(|p| !p.is_empty()) {
                return Err(PyValueError::new_err(
                    "set_defaults: pass a dict of kind -> properties, or a kind and keyword \
                     properties, not both",
                ));
            }
            for (kind, one) in all.iter() {
                let kind: String = kind
                    .extract()
                    .map_err(|_| PyValueError::new_err("set_defaults: kind names must be str"))?;
                let one = one.cast::<PyDict>().map_err(|_| {
                    PyValueError::new_err(format!(
                        "set_defaults: the properties of {kind:?} must be a dict"
                    ))
                })?;
                groups.push((kind, one.clone()));
            }
        } else {
            let kind: String = first.extract().map_err(|_| {
                PyValueError::new_err(
                    "set_defaults: the first argument is a node kind name, or a dict of kind -> \
                     properties",
                )
            })?;
            groups.push((
                kind,
                props.cloned().unwrap_or_else(|| PyDict::new(first.py())),
            ));
        }
        let mut entries: Vec<Entry<'_>> = Vec::new();
        for (kind, one) in &groups {
            if kind != ANY && !KINDS.contains(&kind.as_str()) {
                return Err(unknown_kind("set_defaults", kind));
            }
            for (name, value) in one.iter() {
                let name: String = name.extract().map_err(|_| {
                    PyValueError::new_err(format!(
                        "set_defaults({kind:?}): property names must be str"
                    ))
                })?;
                if value.is_none() {
                    entries.push(Entry {
                        kind: kind.clone(),
                        name,
                        value: None,
                        skips: Vec::new(),
                    });
                    continue;
                }
                if plain_data && value.cast::<Node>().is_ok() {
                    return Err(PyValueError::new_err(format!(
                        "set_defaults({kind:?}): `{name}` is a Node, which belongs to one \
                         window -- set it with Window.set_defaults"
                    )));
                }
                let mut skips = Vec::new();
                if kind == ANY {
                    let mut first_error = None;
                    for each in KINDS {
                        match fit(py, tree, each, &name, &value)
                            .map_err(|err| prefixed(each, &err, py))?
                        {
                            Fit::Yes => {}
                            Fit::No(err) => {
                                first_error.get_or_insert(err);
                                skips.push(each);
                            }
                        }
                    }
                    if skips.len() == KINDS.len() {
                        let err = first_error.expect("every kind refused it");
                        return Err(PyValueError::new_err(format!(
                            "set_defaults(\"*\"): `{name}` applies to no node kind -- {}",
                            err.value(py)
                        )));
                    }
                } else {
                    match fit(py, tree, kind, &name, &value)
                        .map_err(|err| prefixed(kind, &err, py))?
                    {
                        Fit::Yes => {}
                        Fit::No(err) => return Err(prefixed(kind, &err, py)),
                    }
                }
                entries.push(Entry {
                    kind: kind.clone(),
                    name,
                    value: Some(value),
                    skips,
                });
            }
        }
        for entry in entries {
            let dict = match self.kinds.get(&entry.kind) {
                Some(dict) => dict.bind(py).clone(),
                None => {
                    let dict = PyDict::new(py);
                    self.kinds.insert(entry.kind.clone(), dict.clone().unbind());
                    dict
                }
            };
            if entry.kind == ANY {
                self.star_skips.retain(|(name, _)| *name != entry.name);
            }
            match entry.value {
                Some(value) => {
                    dict.set_item(&entry.name, value)?;
                    for skipped in entry.skips {
                        self.star_skips.insert((entry.name.clone(), skipped));
                    }
                }
                None => dict.del_item(&entry.name).or_else(|err| {
                    if err.is_instance_of::<pyo3::exceptions::PyKeyError>(py) {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })?,
            }
        }
        self.kinds.retain(|_, dict| !dict.bind(py).is_empty());
        Ok(())
    }

    /// A copy of the defaults of `kind` (empty when none), or of every kind's when `kind` is
    /// `None`, as `{kind: {property: value}}`.
    pub(crate) fn get(&self, py: Python<'_>, kind: Option<&str>) -> PyResult<Py<PyAny>> {
        match kind {
            Some(kind) => {
                if kind != ANY && !KINDS.contains(&kind) {
                    return Err(unknown_kind("get_defaults", kind));
                }
                let copy = match self.kinds.get(kind) {
                    Some(dict) => dict.bind(py).copy()?,
                    None => PyDict::new(py),
                };
                Ok(copy.into_any().unbind())
            }
            None => {
                let all = PyDict::new(py);
                for (kind, dict) in &self.kinds {
                    all.set_item(kind, dict.bind(py).copy()?)?;
                }
                Ok(all.into_any().unbind())
            }
        }
    }

    /// Removes the defaults of `kind`, or of every kind when `kind` is `None`.
    pub(crate) fn clear(&mut self, kind: Option<&str>) -> PyResult<()> {
        match kind {
            Some(kind) => {
                if kind != ANY && !KINDS.contains(&kind) {
                    return Err(unknown_kind("clear_defaults", kind));
                }
                self.kinds.remove(kind);
                if kind == ANY {
                    self.star_skips.clear();
                }
            }
            None => {
                self.kinds.clear();
                self.star_skips.clear();
            }
        }
        Ok(())
    }

    /// Puts the defaults that apply to `kind` into `out`: the `"*"` ones that fit it, then the
    /// kind's own, which win.
    fn merge_into(&self, kind: &str, out: &Bound<'_, PyDict>) -> PyResult<()> {
        let py = out.py();
        if let Some(star) = self.kinds.get(ANY) {
            for (name, value) in star.bind(py).iter() {
                let skipped = name
                    .extract::<String>()
                    .is_ok_and(|n| self.star_skips.iter().any(|(s, k)| *s == n && *k == kind));
                if !skipped {
                    out.set_item(name, value)?;
                }
            }
        }
        if let Some(own) = self.kinds.get(kind) {
            for (name, value) in own.bind(py).iter() {
                out.set_item(name, value)?;
            }
        }
        Ok(())
    }

    /// The stored dicts, for the owner's `__traverse__`.
    pub(crate) fn dicts(&self) -> impl Iterator<Item = &Py<PyDict>> {
        self.kinds.values()
    }
}

impl PyWindow {
    /// What `create(kind, **props)` and `build` parse: the props, over this window's defaults for
    /// `kind`, over its app's. A new dict; the caller's is not changed.
    pub(crate) fn props_with_defaults<'py>(
        &self,
        kind: &str,
        props: Option<&Bound<'py, PyDict>>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let merged = PyDict::new(py);
        let app = self.handles.app_defaults.borrow();
        if let Some(app) = app.as_ref() {
            app.borrow().merge_into(kind, &merged)?;
        }
        self.handles.defaults.borrow().merge_into(kind, &merged)?;
        if let Some(props) = props {
            for (name, value) in props.iter() {
                merged.set_item(name, value)?;
            }
        }
        Ok(merged)
    }
}

#[pymethods]
impl PyWindow {
    /// 0.6.11 (P12): declares the baseline look of nodes this window creates afterwards, once,
    /// so `create` and `build` need not repeat it. `kind` is a node kind (`"box"`, `"text"`,
    /// `"text_input"`, `"image"`, `"path"`, `"svg"`, `"canvas"`, `"scroll_view"`,
    /// `"virtual_list"`, `"terminal"`) with its properties as keywords, or `"*"` for every kind
    /// -- a `"*"` property a kind does not have is skipped for it -- or a dict of kind to
    /// properties dict (`{"text": {...}, "*": {...}}`) for a whole theme in one call. A property
    /// a kind-specific default names must be one `create` takes for that kind; a bad name or
    /// value raises `ValueError` and stores nothing. `None` removes a default. A default is read
    /// when a node is created: nodes that exist keep their properties. Strongest first: what
    /// `create` gets, this window's defaults for the kind, this window's `"*"`, the app's for the
    /// kind, the app's `"*"`, the engine's own.
    #[pyo3(signature = (kind, **props))]
    fn set_defaults(
        &self,
        kind: &Bound<'_, PyAny>,
        props: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        self.handles
            .defaults
            .borrow_mut()
            .set(&self.handles.tree, false, kind, props)
    }

    /// 0.6.11 (P12): this window's own defaults (not the app's) as `{property: value}` for
    /// `kind`, or `{kind: {property: value}}` for all. Copies.
    #[pyo3(signature = (kind=None))]
    fn get_defaults(&self, kind: Option<&str>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.handles.defaults.borrow().get(py, kind)
    }

    /// 0.6.11 (P12): removes this window's defaults for `kind`, or all of them.
    #[pyo3(signature = (kind=None))]
    fn clear_defaults(&self, kind: Option<&str>) -> PyResult<()> {
        self.handles.defaults.borrow_mut().clear(kind)
    }
}
