//! 0.6.12 (P10): the placement policy of layers (`Window.show_layer`) as properties.
//!
//! The window sets the defaults (`layer_viewport_margin`, `layer_flip_threshold`, on
//! `Window.set`/`get`); a layer overrides them at `show_layer(viewport_margin=, flip_threshold=,
//! align=, shift=)` and afterwards through `node.set`/`get` of `layer_viewport_margin`,
//! `layer_flip_threshold`, `layer_align`, `layer_flip` and `layer_shift`. `None` is the default
//! again. Numbers only: the placement itself is `engine_core::overlay::place`.

use engine_core::{Align, Edges, OverlayMeta};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// The node properties of a layer's placement policy.
pub(crate) const NAMES: [&str; 5] = [
    "layer_viewport_margin",
    "layer_flip_threshold",
    "layer_align",
    "layer_flip",
    "layer_shift",
];

const ALIGN: [(&str, Align); 3] = [
    ("start", Align::Start),
    ("center", Align::Center),
    ("end", Align::End),
];

/// One placement property to put on a layer.
#[derive(Clone, Copy)]
pub(crate) enum Edit {
    /// `None` follows the window's.
    ViewportMargin(Option<Edges>),
    /// `None` follows the window's.
    FlipThreshold(Option<f64>),
    Align(Align),
    Flip(bool),
    Shift(bool),
}

impl Edit {
    pub(crate) fn apply(self, meta: &mut OverlayMeta) {
        match self {
            Self::ViewportMargin(margin) => meta.viewport_margin = margin,
            Self::FlipThreshold(threshold) => meta.flip_threshold = threshold,
            Self::Align(align) => meta.align = align,
            Self::Flip(on) => meta.no_flip = !on,
            Self::Shift(on) => meta.no_shift = !on,
        }
    }
}

fn invalid(what: &str, name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("{what} `{name}` must be {expected}"))
}

/// One non-negative finite number of pixels, or four of them as `(top, right, bottom, left)`.
pub(crate) fn parse_margin(what: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<Edges> {
    let expected = "a number of pixels, zero or more, or (top, right, bottom, left)";
    let good = |v: f64| v.is_finite() && v >= 0.0;
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(what, name, expected));
    }
    if let Ok(all) = value.extract::<f64>() {
        return good(all)
            .then(|| Edges::all(all))
            .ok_or_else(|| invalid(what, name, expected));
    }
    let (top, right, bottom, left): (f64, f64, f64, f64) =
        value.extract().map_err(|_| invalid(what, name, expected))?;
    if ![top, right, bottom, left].into_iter().all(good) {
        return Err(invalid(what, name, expected));
    }
    Ok(Edges {
        left,
        top,
        right,
        bottom,
    })
}

/// A non-negative finite number of pixels.
pub(crate) fn parse_threshold(what: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<f64> {
    let expected = "a number of pixels, zero or more";
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(what, name, expected));
    }
    value
        .extract::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| invalid(what, name, expected))
}

pub(crate) fn align_from_name(what: &str, name: &str, text: &str) -> PyResult<Align> {
    ALIGN
        .iter()
        .find(|(n, _)| *n == text)
        .map(|(_, a)| *a)
        .ok_or_else(|| {
            invalid(
                what,
                name,
                &format!(
                    "one of: {}",
                    ALIGN.map(|(n, _)| format!("{n:?}")).join(", ")
                ),
            )
        })
}

pub(crate) fn parse_align(what: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<Align> {
    match value.extract::<String>() {
        Ok(text) => align_from_name(what, name, &text),
        Err(_) => align_from_name(what, name, ""),
    }
}

fn parse_flag(what: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
    value
        .extract::<bool>()
        .map_err(|_| invalid(what, name, "a bool"))
}

/// The `(top, right, bottom, left)` a margin reads back as.
pub(crate) fn margin_tuple(edges: Edges) -> (f64, f64, f64, f64) {
    (edges.top, edges.right, edges.bottom, edges.left)
}

/// A node property of the layer policy, parsed; `None` when `name` is not one.
pub(crate) fn parse(name: &str, value: &Bound<'_, PyAny>) -> Option<PyResult<Edit>> {
    const WHAT: &str = "node property";
    Some(match name {
        "layer_viewport_margin" => if value.is_none() {
            Ok(None)
        } else {
            parse_margin(WHAT, name, value).map(Some)
        }
        .map(Edit::ViewportMargin),
        "layer_flip_threshold" => if value.is_none() {
            Ok(None)
        } else {
            parse_threshold(WHAT, name, value).map(Some)
        }
        .map(Edit::FlipThreshold),
        "layer_align" => parse_align(WHAT, name, value).map(Edit::Align),
        "layer_flip" => parse_flag(WHAT, name, value).map(Edit::Flip),
        "layer_shift" => parse_flag(WHAT, name, value).map(Edit::Shift),
        _ => return None,
    })
}

/// What a layer's policy reads as, with the window's defaults filled in; `None` for a node that is
/// not a layer.
pub(crate) fn read(
    name: &str,
    meta: Option<&OverlayMeta>,
    defaults: engine_core::LayerDefaults,
    py: Python<'_>,
) -> Option<PyResult<Py<PyAny>>> {
    if !NAMES.contains(&name) {
        return None;
    }
    let Some(meta) = meta else {
        return Some(Ok(py.None()));
    };
    Some((|| {
        Ok(match name {
            "layer_viewport_margin" => {
                margin_tuple(meta.viewport_margin.unwrap_or(defaults.viewport_margin))
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
            "layer_flip_threshold" => meta
                .flip_threshold
                .unwrap_or(defaults.flip_threshold)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "layer_align" => ALIGN
                .iter()
                .find(|(_, a)| *a == meta.align)
                .map(|(n, _)| *n)
                .into_pyobject(py)?
                .into_any()
                .unbind(),
            "layer_flip" => (!meta.no_flip)
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
            _ => (!meta.no_shift)
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind(),
        })
    })())
}
