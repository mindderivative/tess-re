//! 0.6.12 (P11): window-level behaviour that used to be fixed -- what a double-click on the drag
//! region does, which cursor each resize edge shows, and whether an undecorated window keeps the
//! OS's shadow. Each is a window property whose default is how it always behaved (`None`).
//!
//! * `titlebar_double_click`: `None` (the OS's setting on macOS, maximize elsewhere),
//!   `"maximize"`, `"minimize"`, `"none"`, or `"event"` -- the window then fires a
//!   `titlebar_double_click` event (`x`, `y`) and does nothing itself.
//! * `resize_cursors`: a dict of direction -> cursor name for the `resize_border`'s eight zones;
//!   `None` is the stock `ns_resize` / `ew_resize` / `nwse_resize` / `nesw_resize` map.
//! * `undecorated_shadow`: bool, default `True`; Windows only (the OS drops an undecorated
//!   window's shadow unless asked to keep it), accepted and ignored elsewhere.

use engine_core::Cursor;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict};
use winit::window::ResizeDirection as Dir;

/// What a double-click on a drag region does, when the app says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DoubleClick {
    Maximize,
    Minimize,
    Nothing,
    /// Fire the window's `titlebar_double_click` event instead.
    Event,
}

const DOUBLE_CLICK_NAMES: [(&str, DoubleClick); 4] = [
    ("maximize", DoubleClick::Maximize),
    ("minimize", DoubleClick::Minimize),
    ("none", DoubleClick::Nothing),
    ("event", DoubleClick::Event),
];

/// `titlebar_double_click`'s value: `None` or one of the four names.
pub(crate) fn parse_double_click(value: &Bound<'_, PyAny>) -> PyResult<Option<DoubleClick>> {
    let bad = || {
        PyValueError::new_err(
            "window property `titlebar_double_click` must be None or one of: \
             maximize, minimize, none, event",
        )
    };
    if value.is_none() {
        return Ok(None);
    }
    let name: String = value.extract().map_err(|_| bad())?;
    DOUBLE_CLICK_NAMES
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, action)| Some(*action))
        .ok_or_else(bad)
}

pub(crate) fn double_click_name(action: Option<DoubleClick>) -> Option<&'static str> {
    action.and_then(|action| {
        DOUBLE_CLICK_NAMES
            .iter()
            .find(|(_, known)| *known == action)
            .map(|(name, _)| *name)
    })
}

/// `undecorated_shadow`'s value: a real bool.
pub(crate) fn parse_bool(name: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !value.is_instance_of::<PyBool>() {
        return Err(PyValueError::new_err(format!(
            "window property `{name}` must be a bool"
        )));
    }
    value.extract()
}

/// The cursor shown on each of the resize border's eight zones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResizeCursors([Cursor; 8]);

const DIRECTIONS: [(&str, Dir); 8] = [
    ("north", Dir::North),
    ("south", Dir::South),
    ("east", Dir::East),
    ("west", Dir::West),
    ("north_west", Dir::NorthWest),
    ("north_east", Dir::NorthEast),
    ("south_west", Dir::SouthWest),
    ("south_east", Dir::SouthEast),
];

impl Default for ResizeCursors {
    fn default() -> Self {
        Self(DIRECTIONS.map(|(_, dir)| match dir {
            Dir::North | Dir::South => Cursor::NsResize,
            Dir::East | Dir::West => Cursor::EwResize,
            Dir::NorthWest | Dir::SouthEast => Cursor::NwseResize,
            Dir::NorthEast | Dir::SouthWest => Cursor::NeswResize,
        }))
    }
}

impl ResizeCursors {
    pub(crate) fn for_direction(&self, direction: Dir) -> Cursor {
        let index = DIRECTIONS
            .iter()
            .position(|(_, dir)| *dir == direction)
            .expect("all eight directions are listed");
        self.0[index]
    }

    /// `resize_cursors`'s value: `None` for the stock map, or a dict of direction -> cursor
    /// name; directions left out keep their stock cursor.
    pub(crate) fn parse(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut cursors = Self::default();
        if value.is_none() {
            return Ok(cursors);
        }
        let bad = |detail: String| {
            PyValueError::new_err(format!("window property `resize_cursors` {detail}"))
        };
        let dict = value
            .cast::<PyDict>()
            .map_err(|_| bad("must be None or a dict of direction -> cursor name".into()))?;
        for (key, cursor) in dict.iter() {
            let key: String = key
                .extract()
                .map_err(|_| bad("keys must be direction names".into()))?;
            let Some(index) = DIRECTIONS.iter().position(|(name, _)| *name == key) else {
                let valid: Vec<&str> = DIRECTIONS.iter().map(|(name, _)| *name).collect();
                return Err(bad(format!(
                    "has no direction {key:?} -- valid: {}",
                    valid.join(", ")
                )));
            };
            let name: String = cursor
                .extract()
                .map_err(|_| bad(format!("`{key}` must be a cursor name (a str)")))?;
            cursors.0[index] = Cursor::from_name(&name)
                .filter(|cursor| !matches!(cursor, Cursor::Custom(_)))
                .ok_or_else(|| {
                    let valid: Vec<&str> = Cursor::ALL.iter().map(|c| c.name()).collect();
                    bad(format!(
                        "`{key}` must be one of the cursor names: {}",
                        valid.join(", ")
                    ))
                })?;
        }
        Ok(cursors)
    }

    pub(crate) fn into_py_dict<'py>(self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for ((name, _), cursor) in DIRECTIONS.iter().zip(self.0) {
            dict.set_item(name, cursor.name())?;
        }
        Ok(dict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_map_is_the_stock_one() {
        let cursors = ResizeCursors::default();
        assert_eq!(cursors.for_direction(Dir::North), Cursor::NsResize);
        assert_eq!(cursors.for_direction(Dir::West), Cursor::EwResize);
        assert_eq!(cursors.for_direction(Dir::SouthEast), Cursor::NwseResize);
        assert_eq!(cursors.for_direction(Dir::SouthWest), Cursor::NeswResize);
    }
}
