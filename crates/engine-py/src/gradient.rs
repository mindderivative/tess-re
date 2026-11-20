//! 0.5.4 (#109): `tess_re.Gradient`, a gradient fill for a box's `fill`.

use engine_core::{GradientShape, GradientStop, Spread};
use peniko::Color;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// A gradient to give a box as its `fill`: `node.set(fill=Gradient.linear(...))`.
///
/// Built with `Gradient.linear`, `Gradient.radial` or `Gradient.sweep`. Positions are
/// relative to the box, so a gradient follows its box as layout resizes it.
/// Immutable: to change one, set or animate `fill` to another.
#[pyclass(frozen, name = "Gradient", eq, from_py_object)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyGradient {
    pub(crate) inner: engine_core::Gradient,
}

/// A stop as Python gives it: `(offset, (r, g, b, a))`.
type StopTuple = (f32, (u8, u8, u8, u8));

fn color_of(value: &Bound<'_, PyAny>) -> PyResult<Color> {
    let (r, g, b, a): (u8, u8, u8, u8) = value.extract().map_err(|_| {
        PyValueError::new_err("a gradient stop's color must be an (r, g, b, a) tuple of 0-255 ints")
    })?;
    Ok(Color::from_rgba8(r, g, b, a))
}

/// `stops`: either colours, spaced evenly from 0 to 1, or `(offset, color)`
/// pairs.
fn parse_stops(stops: &Bound<'_, PyAny>) -> PyResult<Vec<GradientStop>> {
    let items: Vec<Bound<'_, PyAny>> = stops
        .try_iter()
        .map_err(|_| PyValueError::new_err("gradient `stops` must be a sequence"))?
        .collect::<PyResult<_>>()?;
    let pairs: Option<Vec<StopTuple>> = items
        .iter()
        .map(|item| item.extract::<StopTuple>().ok())
        .collect();
    if let Some(pairs) = pairs.filter(|p| !p.is_empty()) {
        return Ok(pairs
            .into_iter()
            .map(|(offset, (r, g, b, a))| GradientStop {
                offset,
                color: Color::from_rgba8(r, g, b, a),
            })
            .collect());
    }
    let n = items.len();
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            Ok(GradientStop {
                offset: if n > 1 {
                    i as f32 / (n - 1) as f32
                } else {
                    0.0
                },
                color: color_of(item).map_err(|_| {
                    PyValueError::new_err(
                        "gradient `stops` must be (r, g, b, a) colors, or (offset, color) pairs",
                    )
                })?,
            })
        })
        .collect()
}

/// `spread` as Python gives it: `None` is `"repeat"` for a gradient that repeats and `"pad"` for
/// one that does not, which is the 0.6.9 behaviour.
fn parse_spread(spread: Option<&str>, repeat: f64) -> PyResult<Spread> {
    match spread {
        None if repeat == 1.0 => Ok(Spread::Pad),
        None | Some("repeat") => Ok(Spread::Repeat),
        Some("pad") => Ok(Spread::Pad),
        Some("reflect") => Ok(Spread::Reflect),
        Some(other) => Err(PyValueError::new_err(format!(
            "gradient spread {other:?} is not one of: pad, repeat, reflect"
        ))),
    }
}

fn build(
    shape: GradientShape,
    stops: &Bound<'_, PyAny>,
    repeat: f64,
    spread: Option<&str>,
) -> PyResult<PyGradient> {
    let spread = parse_spread(spread, repeat)?;
    let inner = engine_core::Gradient::new(shape, parse_stops(stops)?)
        .and_then(|g| g.with_repeat(repeat, spread))
        .map_err(PyValueError::new_err)?;
    Ok(PyGradient { inner })
}

#[pymethods]
impl PyGradient {
    /// A gradient along a line through the box's centre at `angle` degrees
    /// (0 points up, 90 right, 180 down, the default), spanning the box.
    /// `stops` are colours, spaced evenly, or `(offset, color)` pairs with
    /// offsets from 0 to 1.
    ///
    /// `repeat` (0.6.10) runs the ramp that many times along the line (a fraction leaves the last
    /// pass partial; it must be greater than 0). `spread` says what a pass after the first does:
    /// `"repeat"` (the default when `repeat` is not 1) starts again, `"reflect"` runs it back,
    /// `"pad"` (the default for one pass) holds the last colour.
    #[staticmethod]
    #[pyo3(signature = (stops, angle=180.0, repeat=1.0, spread=None))]
    fn linear(
        stops: &Bound<'_, PyAny>,
        angle: f64,
        repeat: f64,
        spread: Option<&str>,
    ) -> PyResult<Self> {
        build(
            GradientShape::Linear { angle_deg: angle },
            stops,
            repeat,
            spread,
        )
    }

    /// A gradient outward from `center` (fractions of the box; `None`, the
    /// default, is `(0.5, 0.5)`, its middle). `radius` is a fraction of the box's half-diagonal, so `1.0`
    /// reaches the far corner of a centred gradient. `repeat` and `spread` (0.6.10): see `linear`;
    /// the ramp repeats outward, each pass `radius / repeat` wide.
    #[staticmethod]
    #[pyo3(signature = (stops, center=None, radius=1.0, repeat=1.0, spread=None))]
    fn radial(
        stops: &Bound<'_, PyAny>,
        center: Option<(f64, f64)>,
        radius: f64,
        repeat: f64,
        spread: Option<&str>,
    ) -> PyResult<Self> {
        let center = center.unwrap_or((0.5, 0.5));
        build(
            GradientShape::Radial { center, radius },
            stops,
            repeat,
            spread,
        )
    }

    /// A gradient around `center`, starting `start` degrees clockwise from up. `repeat` and
    /// `spread` (0.6.10): see `linear`; the ramp runs `repeat` times around the full turn.
    #[staticmethod]
    #[pyo3(signature = (stops, center=None, start=0.0, repeat=1.0, spread=None))]
    fn sweep(
        stops: &Bound<'_, PyAny>,
        center: Option<(f64, f64)>,
        start: f64,
        repeat: f64,
        spread: Option<&str>,
    ) -> PyResult<Self> {
        let center = center.unwrap_or((0.5, 0.5));
        build(
            GradientShape::Sweep {
                center,
                start_deg: start,
            },
            stops,
            repeat,
            spread,
        )
    }

    /// How many times the ramp runs across the gradient's span (`1.0`, one pass, by default).
    #[getter]
    fn repeat(&self) -> f64 {
        self.inner.repeat()
    }

    /// `"pad"`, `"repeat"` or `"reflect"`: what a pass after the first does.
    #[getter]
    fn spread(&self) -> &'static str {
        match self.inner.spread() {
            Spread::Pad => "pad",
            Spread::Repeat => "repeat",
            Spread::Reflect => "reflect",
        }
    }

    /// `"linear"`, `"radial"` or `"sweep"`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner.shape() {
            GradientShape::Linear { .. } => "linear",
            GradientShape::Radial { .. } => "radial",
            GradientShape::Sweep { .. } => "sweep",
        }
    }

    /// The `(offset, (r, g, b, a))` stops.
    #[getter]
    fn stops(&self) -> Vec<(f32, (u8, u8, u8, u8))> {
        self.inner
            .stops()
            .iter()
            .map(|stop| {
                let [r, g, b, a] = stop.color.to_rgba8().to_u8_array();
                (stop.offset, (r, g, b, a))
            })
            .collect()
    }

    /// A linear gradient's angle in degrees, else `None`.
    #[getter]
    fn angle(&self) -> Option<f64> {
        match self.inner.shape() {
            GradientShape::Linear { angle_deg } => Some(*angle_deg),
            _ => None,
        }
    }

    /// A radial or sweep gradient's centre, else `None`.
    #[getter]
    fn center(&self) -> Option<(f64, f64)> {
        match self.inner.shape() {
            GradientShape::Radial { center, .. } | GradientShape::Sweep { center, .. } => {
                Some(*center)
            }
            GradientShape::Linear { .. } => None,
        }
    }

    /// A radial gradient's radius, else `None`.
    #[getter]
    fn radius(&self) -> Option<f64> {
        match self.inner.shape() {
            GradientShape::Radial { radius, .. } => Some(*radius),
            _ => None,
        }
    }

    /// A sweep gradient's start angle in degrees, else `None`.
    #[getter]
    fn start(&self) -> Option<f64> {
        match self.inner.shape() {
            GradientShape::Sweep { start_deg, .. } => Some(*start_deg),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        let shape = match self.inner.shape() {
            GradientShape::Linear { angle_deg } => format!("linear, angle={angle_deg}"),
            GradientShape::Radial { center, radius } => {
                format!("radial, center={center:?}, radius={radius}")
            }
            GradientShape::Sweep { center, start_deg } => {
                format!("sweep, center={center:?}, start={start_deg}")
            }
        };
        let repeat = if self.inner.repeat() == 1.0 {
            String::new()
        } else {
            format!(", repeat={}, spread={}", self.inner.repeat(), self.spread())
        };
        format!(
            "Gradient({shape}{repeat}, {} stops)",
            self.inner.stops().len()
        )
    }
}
