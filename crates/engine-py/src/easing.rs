//! 0.6.11: the one parser of an easing taken from Python. `Node.animate`,
//! `animate_param`, `LayerWindow.animate_margin`, `Shader.animate_uniform` and
//! the path and layout animations all take `animate`'s forms (`parse_easing`);
//! the scrollbar, scroll and snap properties take the subset a property can
//! hold, a preset or a bezier (`parse_preset_or_bezier`). The presets are the
//! table of `engine_core::EASINGS`: one table, so a name added there works
//! everywhere.

use std::time::Duration;

use engine_core::{Easing, MotionCurve};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// What an easing that is a name or a bezier may be, for an error message.
pub(crate) fn preset_or_bezier_expected() -> String {
    format!(
        "a preset name ({}) or a cubic bezier (x1, y1, x2, y2) with x1 and x2 from 0 to 1",
        Easing::names()
    )
}

/// A preset name or a cubic bezier `(x1, y1, x2, y2)` (`x1` and `x2` in
/// `0..=1`, as CSS `cubic-bezier()` takes them); `None` for anything else.
pub(crate) fn parse_preset_or_bezier(value: &Bound<'_, PyAny>) -> Option<Easing> {
    if let Ok(name) = value.extract::<String>() {
        return Easing::from_name(&name);
    }
    let (x1, y1, x2, y2): (f64, f64, f64, f64) = value.extract().ok()?;
    Easing::bezier(x1, y1, x2, y2)
}

/// `animate`'s `easing`: `None` (linear), a preset name (`engine_core::EASINGS`),
/// a cubic bezier `(x1, y1, x2, y2)`, or (0.5.4, #138) a spring: `"spring"` or
/// `("spring", bounce)` with `bounce` from -1 to 1 (exclusive), whose period is
/// `duration`. A spring of zero duration snaps. Anything else is a `ValueError`
/// that lists the valid forms.
pub(crate) fn parse_easing(
    easing: Option<&Bound<'_, PyAny>>,
    duration: Duration,
) -> PyResult<MotionCurve> {
    let invalid = || {
        PyValueError::new_err(format!(
            "easing must be {}, \"spring\", or (\"spring\", bounce) with bounce from -1 to 1 \
             (exclusive); None is linear",
            preset_or_bezier_expected()
        ))
    };
    let Some(easing) = easing.filter(|e| !e.is_none()) else {
        return Ok(MotionCurve::Linear);
    };
    let spring = |bounce: f64| -> PyResult<MotionCurve> {
        if !(bounce > -1.0 && bounce < 1.0) {
            return Err(invalid());
        }
        Ok(if duration.is_zero() {
            MotionCurve::Linear
        } else {
            MotionCurve::spring_for(duration, bounce)
        })
    };
    if let Ok(name) = easing.extract::<String>() {
        return if name == "spring" {
            spring(0.2)
        } else {
            Easing::from_name(&name)
                .map(Easing::curve)
                .ok_or_else(invalid)
        };
    }
    if let Ok((name, bounce)) = easing.extract::<(String, f64)>() {
        return if name == "spring" {
            spring(bounce)
        } else {
            Err(invalid())
        };
    }
    parse_preset_or_bezier(easing)
        .map(Easing::curve)
        .ok_or_else(invalid)
}
