//! 0.6.11 (P13): the state-layer properties of `node.set` / `node.get`, on every node:
//! `hover_layer`, `pressed_layer`, `focus_layer` (a colour whose alpha is the layer's opacity, or
//! `None`: draws nothing), `focus_layer_any_modality` (a bool), `state_layer_ms` (a number) and
//! `state_layer_easing` (a preset name or a cubic bezier), and the per-state overrides
//! `hover_layer_ms`, `pressed_layer_ms`, `focus_layer_ms` (a number, `None` = `state_layer_ms`) and
//! `hover_layer_easing`, `pressed_layer_easing`, `focus_layer_easing` (`None` = `state_layer_easing`). The engine tracks hover, press and
//! focus, draws the layer and fades it (`engine_core::StateLayer`); there is no callback.

use engine_core::StateLayer;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::node_props::{color_to_py, parse_color};

/// The names, in table order.
pub(crate) const NAMES: [&str; 12] = [
    "hover_layer",
    "pressed_layer",
    "focus_layer",
    "focus_layer_any_modality",
    "state_layer_ms",
    "state_layer_easing",
    "hover_layer_ms",
    "pressed_layer_ms",
    "focus_layer_ms",
    "hover_layer_easing",
    "pressed_layer_easing",
    "focus_layer_easing",
];

/// What setting one of these does to a node's `StateLayer`.
pub(crate) type Edit = Box<dyn FnOnce(&mut StateLayer)>;

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

/// A fade duration: `None` for `None`, else milliseconds from 0 to an hour.
fn parse_ms(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let words = "a number of milliseconds from 0 to an hour, or None";
    if value.is_none() {
        return Ok(None);
    }
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, words));
    }
    let ms: f64 = value.extract().map_err(|_| invalid(name, words))?;
    if !ms.is_finite() || !(0.0..=engine_core::MAX_SCROLL_MILLIS).contains(&ms) {
        return Err(invalid(name, words));
    }
    Ok(Some(ms))
}

/// An easing: `None` for `None`, else a preset name or a cubic bezier.
fn parse_easing(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Option<engine_core::Easing>> {
    if value.is_none() {
        return Ok(None);
    }
    crate::easing::parse_preset_or_bezier(value)
        .map(Some)
        .ok_or_else(|| {
            invalid(
                name,
                &format!("{}, or None", crate::easing::preset_or_bezier_expected()),
            )
        })
}

/// Parses `name` (one of `NAMES`) with `value` into its edit.
pub(crate) fn parse(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Edit> {
    let colour = |value: &Bound<'_, PyAny>| -> PyResult<Option<peniko::Color>> {
        if value.is_none() {
            Ok(None)
        } else {
            parse_color(value, name).map(Some)
        }
    };
    Ok(match name {
        "hover_layer" => {
            let colour = colour(value)?;
            Box::new(move |l| l.hover = colour)
        }
        "pressed_layer" => {
            let colour = colour(value)?;
            Box::new(move |l| l.pressed = colour)
        }
        "focus_layer" => {
            let colour = colour(value)?;
            Box::new(move |l| l.focus = colour)
        }
        "focus_layer_any_modality" => {
            let on = if value.is_none() {
                false
            } else if value.is_instance_of::<pyo3::types::PyBool>() {
                value.extract()?
            } else {
                return Err(invalid(name, "a bool, or None"));
            };
            Box::new(move |l| l.focus_any_modality = on)
        }
        "state_layer_ms" => {
            let ms = parse_ms(name, value)?.unwrap_or(engine_core::STATE_LAYER_MS);
            Box::new(move |l| l.ms = ms)
        }
        "hover_layer_ms" => {
            let ms = parse_ms(name, value)?;
            Box::new(move |l| l.hover_ms = ms)
        }
        "pressed_layer_ms" => {
            let ms = parse_ms(name, value)?;
            Box::new(move |l| l.pressed_ms = ms)
        }
        "focus_layer_ms" => {
            let ms = parse_ms(name, value)?;
            Box::new(move |l| l.focus_ms = ms)
        }
        "state_layer_easing" => {
            let easing = parse_easing(name, value)?.unwrap_or(StateLayer::default().easing);
            Box::new(move |l: &mut StateLayer| l.easing = easing)
        }
        "hover_layer_easing" => {
            let easing = parse_easing(name, value)?;
            Box::new(move |l: &mut StateLayer| l.hover_easing = easing)
        }
        "pressed_layer_easing" => {
            let easing = parse_easing(name, value)?;
            Box::new(move |l: &mut StateLayer| l.pressed_easing = easing)
        }
        "focus_layer_easing" => {
            let easing = parse_easing(name, value)?;
            Box::new(move |l: &mut StateLayer| l.focus_easing = easing)
        }
        _ => unreachable!("not a state-layer property: {name}"),
    })
}

/// The value of `name` for `layer` (`None`: the node never had state layers, which reads as the
/// defaults).
pub(crate) fn read(name: &str, layer: Option<&StateLayer>, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let colour = |c: Option<peniko::Color>| c.map_or_else(|| Ok(py.None()), |c| color_to_py(c, py));
    match name {
        "hover_layer" => colour(layer.and_then(|l| l.hover)),
        "pressed_layer" => colour(layer.and_then(|l| l.pressed)),
        "focus_layer" => colour(layer.and_then(|l| l.focus)),
        "focus_layer_any_modality" => Ok(layer
            .is_some_and(|l| l.focus_any_modality)
            .into_pyobject(py)?
            .to_owned()
            .into_any()
            .unbind()),
        "state_layer_ms" => Ok(layer
            .map_or(engine_core::STATE_LAYER_MS, |l| l.ms)
            .into_pyobject(py)?
            .into_any()
            .unbind()),
        "state_layer_easing" => {
            easing_to_py(layer.map_or(StateLayer::default().easing, |l| l.easing), py)
        }
        "hover_layer_ms" => opt_ms(layer.and_then(|l| l.hover_ms), py),
        "pressed_layer_ms" => opt_ms(layer.and_then(|l| l.pressed_ms), py),
        "focus_layer_ms" => opt_ms(layer.and_then(|l| l.focus_ms), py),
        "hover_layer_easing" => opt_easing(layer.and_then(|l| l.hover_easing), py),
        "pressed_layer_easing" => opt_easing(layer.and_then(|l| l.pressed_easing), py),
        "focus_layer_easing" => opt_easing(layer.and_then(|l| l.focus_easing), py),
        _ => unreachable!("not a state-layer property: {name}"),
    }
}

fn opt_ms(ms: Option<f64>, py: Python<'_>) -> PyResult<Py<PyAny>> {
    ms.map_or_else(
        || Ok(py.None()),
        |ms| Ok(ms.into_pyobject(py)?.into_any().unbind()),
    )
}

fn opt_easing(easing: Option<engine_core::Easing>, py: Python<'_>) -> PyResult<Py<PyAny>> {
    easing.map_or_else(|| Ok(py.None()), |e| easing_to_py(e, py))
}

/// An easing for Python: its preset name, else the bezier tuple.
fn easing_to_py(easing: engine_core::Easing, py: Python<'_>) -> PyResult<Py<PyAny>> {
    Ok(match easing.name() {
        Some(name) => name.into_pyobject(py)?.into_any().unbind(),
        None => match easing {
            engine_core::Easing::Bezier(a, b, c, d) => {
                (a, b, c, d).into_pyobject(py)?.into_any().unbind()
            }
            engine_core::Easing::Linear => "linear".into_pyobject(py)?.into_any().unbind(),
        },
    })
}
