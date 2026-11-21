//! 0.6.11 (UXREQ-008): the outline properties of `node.set` / `node.get`, on every node kind:
//! `outline_width`, `outline_offset`, `outline_color`, `outline_gap_color`, `outline_radius`, and
//! the same five with the prefix `focus_outline_`, which replace them while the node has
//! visible keyboard focus. Numbers and colours only: nothing is a callback.

use engine_core::{Node, Outlines};
use peniko::Color;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::node_props::{color_to_py, parse_color};

/// Every outline property's name: the plain ones, then the focus ones.
pub(crate) const NAMES: [&str; 10] = [
    "outline_width",
    "outline_offset",
    "outline_color",
    "outline_gap_color",
    "outline_radius",
    "focus_outline_width",
    "focus_outline_offset",
    "focus_outline_color",
    "focus_outline_gap_color",
    "focus_outline_radius",
];

/// One outline field to set.
#[derive(Clone, Copy)]
pub(crate) enum Field {
    Width(f64),
    Offset(f64),
    Color(Option<Color>),
    GapColor(Option<Color>),
    Radius(Option<f64>),
}

/// A parsed outline property: which of the two styles, and what to put in it.
#[derive(Clone, Copy)]
pub(crate) struct OutlineChange {
    focus: bool,
    field: Field,
}

impl OutlineChange {
    pub(crate) fn apply(self, outline: &mut Option<Box<Outlines>>) {
        let outlines = outline.get_or_insert_with(Default::default);
        let style = if self.focus {
            &mut outlines.focus
        } else {
            &mut outlines.always
        };
        match self.field {
            Field::Width(width) => style.width = width,
            Field::Offset(offset) => style.offset = offset,
            Field::Color(color) => style.color = color,
            Field::GapColor(color) => style.gap_color = color,
            Field::Radius(radius) => style.radius = radius,
        }
        // Back to nothing set: the node pays no more than one that never had an outline.
        if **outlines == Outlines::default() {
            *outline = None;
        }
    }
}

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

fn number(value: &Bound<'_, PyAny>, name: &str, expected: &str) -> PyResult<f64> {
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, expected));
    }
    let number: f64 = value.extract().map_err(|_| invalid(name, expected))?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err(invalid(name, expected))
    }
}

/// `value` as outline property `name`, or `None` if `name` is not one of them.
pub(crate) fn parse(name: &str, value: &Bound<'_, PyAny>) -> Option<PyResult<OutlineChange>> {
    let (focus, field_name) = match name.strip_prefix("focus_") {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    if !NAMES.contains(&name) {
        return None;
    }
    let optional_color = || -> PyResult<Option<Color>> {
        if value.is_none() {
            return Ok(None);
        }
        parse_color(value, name).map(Some)
    };
    let field = match field_name {
        "outline_width" => number(value, name, "a non-negative number").and_then(|n| {
            if n < 0.0 {
                Err(invalid(name, "a non-negative number"))
            } else {
                Ok(Field::Width(n))
            }
        }),
        "outline_offset" => number(value, name, "a number").map(Field::Offset),
        "outline_color" => optional_color()
            .map_err(|_| invalid(name, "an (r, g, b, a) tuple of 0-255 ints, or None"))
            .map(Field::Color),
        "outline_gap_color" => optional_color()
            .map_err(|_| invalid(name, "an (r, g, b, a) tuple of 0-255 ints, or None"))
            .map(Field::GapColor),
        "outline_radius" => {
            if value.is_none() {
                Ok(Field::Radius(None))
            } else {
                number(value, name, "a non-negative number, or None").and_then(|n| {
                    if n < 0.0 {
                        Err(invalid(name, "a non-negative number, or None"))
                    } else {
                        Ok(Field::Radius(Some(n)))
                    }
                })
            }
        }
        _ => return None,
    };
    Some(field.map(|field| OutlineChange { focus, field }))
}

/// What `name` reads as on `node`, or `None` if `name` is not an outline property.
pub(crate) fn read(name: &str, node: &Node, py: Python<'_>) -> Option<PyResult<Py<PyAny>>> {
    if !NAMES.contains(&name) {
        return None;
    }
    let (focus, field_name) = match name.strip_prefix("focus_") {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    let style = node
        .paint
        .outline
        .as_deref()
        .map(|o| if focus { o.focus } else { o.always });
    let style = style.unwrap_or_default();
    let color = |c: Option<Color>| -> PyResult<Py<PyAny>> {
        match c {
            Some(c) => color_to_py(c, py),
            None => Ok(py.None()),
        }
    };
    let float = |v: f64| -> PyResult<Py<PyAny>> { Ok(v.into_pyobject(py)?.into_any().unbind()) };
    Some(match field_name {
        "outline_width" => float(style.width),
        "outline_offset" => float(style.offset),
        "outline_color" => color(style.color),
        "outline_gap_color" => color(style.gap_color),
        _ => match style.radius {
            Some(radius) => float(radius),
            None => Ok(py.None()),
        },
    })
}
