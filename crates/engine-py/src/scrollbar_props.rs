//! 0.6.11: the scrollbar and scroll-feel properties of `node.set` / `node.get`
//! on a scroll view and a virtual list. One parser and one reader, driven by the
//! engine's own table (`engine_core::ScrollProp`), so every property of the
//! table works on both kinds and nothing is a callback: a number, a flag, a
//! colour, a name or an easing, `None` for the default.

use engine_core::{Easing, NodeKind, PropKind, PropValue, ScrollProp};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::node_kind_props::{KindChange, change};
use crate::node_props::{color_to_py, parse_color};

/// Every one of these properties' names, in table order.
pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    ScrollProp::ALL.iter().map(|p| p.name())
}

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

/// What a number kind takes, in words, for the error message.
fn number_words(min: f64, max: f64) -> String {
    if max >= f64::MAX {
        "a non-negative number, or None".to_string()
    } else if max >= engine_core::MAX_SCROLL_MILLIS {
        format!("a number of milliseconds from {min} to an hour, or None")
    } else {
        format!("a number from {min} to {max}, or None")
    }
}

/// The value `value` is for `prop`, checked against its kind.
fn parse_value(prop: ScrollProp, value: &Bound<'_, PyAny>) -> PyResult<PropValue> {
    let name = prop.name();
    match prop.kind() {
        PropKind::Number { min, max } => {
            let words = number_words(min, max);
            if value.is_instance_of::<pyo3::types::PyBool>() {
                return Err(invalid(name, &words));
            }
            let number: f64 = value.extract().map_err(|_| invalid(name, &words))?;
            let parsed = PropValue::Number(number);
            if prop.kind().accepts(&parsed) {
                Ok(parsed)
            } else {
                Err(invalid(name, &words))
            }
        }
        PropKind::Colour => Ok(PropValue::Colour(parse_color(value, name)?)),
        PropKind::Flag => {
            if !value.is_instance_of::<pyo3::types::PyBool>() {
                return Err(invalid(name, "a bool, or None"));
            }
            Ok(PropValue::Flag(value.extract()?))
        }
        PropKind::Choice(names) => {
            let text: String = value.extract().unwrap_or_default();
            match names.iter().position(|n| *n == text) {
                Some(i) => Ok(PropValue::Choice(i as u8)),
                None => Err(invalid(
                    name,
                    &format!("one of: {}, or None", names.join(", ")),
                )),
            }
        }
        PropKind::Easing => {
            let expected = format!("{}, or None", crate::easing::preset_or_bezier_expected());
            crate::easing::parse_preset_or_bezier(value)
                .map(PropValue::Easing)
                .ok_or_else(|| invalid(name, &expected))
        }
    }
}

/// Parses property `name` for a node of `kind`, or `None` if it isn't one of
/// these properties.
pub(crate) fn parse(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
) -> Option<PyResult<KindChange>> {
    let prop = ScrollProp::from_name(name)?;
    if kind.scroller().is_none() {
        return Some(Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to a scroll_view or virtual_list node"
        ))));
    }
    let parsed = if value.is_none() {
        Ok(None)
    } else {
        parse_value(prop, value).map(Some)
    };
    Some(parsed.and_then(|parsed| {
        change(move |node| {
            if let Some(scroller) = node.kind.scroller_mut() {
                scroller.bar_mut().settings.set(prop, parsed);
            }
        })
    }))
}

/// The value of property `name` of `node`, or `None` if it isn't one of these.
/// A colour or a side reads what was set (`None`: the default); the rest read
/// the value in force.
pub(crate) fn read(
    name: &str,
    node: &engine_core::Node,
    py: Python<'_>,
) -> Option<PyResult<Py<PyAny>>> {
    let prop = ScrollProp::from_name(name)?;
    let Some(scroller) = node.kind.scroller() else {
        return Some(Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to a scroll_view or virtual_list node"
        ))));
    };
    Some(to_py(
        scroller.bar().get(prop, scroller.defaults()),
        prop,
        py,
    ))
}

fn to_py(value: Option<PropValue>, prop: ScrollProp, py: Python<'_>) -> PyResult<Py<PyAny>> {
    Ok(match value {
        None => py.None(),
        Some(PropValue::Number(n)) => n.into_pyobject(py)?.into_any().unbind(),
        Some(PropValue::Flag(f)) => f.into_pyobject(py)?.to_owned().into_any().unbind(),
        Some(PropValue::Colour(c)) => return color_to_py(c, py),
        Some(PropValue::Choice(i)) => {
            let names = prop.choices().unwrap_or(&[]);
            names
                .get(usize::from(i))
                .copied()
                .unwrap_or("?")
                .into_pyobject(py)?
                .into_any()
                .unbind()
        }
        Some(PropValue::Easing(e)) => match e.name() {
            Some(name) => name.into_pyobject(py)?.into_any().unbind(),
            None => match e {
                Easing::Bezier(a, b, c, d) => (a, b, c, d).into_pyobject(py)?.into_any().unbind(),
                Easing::Linear => "linear".into_pyobject(py)?.into_any().unbind(),
            },
        },
    })
}
