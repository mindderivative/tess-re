//! 0.6.11: a window's colour scheme -- which baseline the engine's own defaults paint in.
//!
//! The setting (`"light"`, `"dark"` or `"auto"`) comes from the window, or from the `App` when
//! the window sets none; `"auto"` follows the OS appearance flag the window last heard of
//! (falling back to light while it is unknown). The resolved baseline, with the window's
//! per-role overrides on top, is the colour table of the window's `Tree`
//! (`Tree::set_scheme_colors`), which the renderer reads at paint time. Nothing runs per frame:
//! the table is rebuilt only when the setting, the OS flag, the overrides or the app default change.

use std::cell::{Cell, RefCell};

use engine_core::{Scheme, SchemeColors, SchemeRole, Tree};
use peniko::Color;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

use crate::node_props::{color_to_py, parse_color};

/// What a window or an app asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Setting {
    Light,
    Dark,
    Auto,
}

impl Setting {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Setting::Light => "light",
            Setting::Dark => "dark",
            Setting::Auto => "auto",
        }
    }

    /// `"light"`, `"dark"` or `"auto"`.
    pub(crate) fn parse(value: &Bound<'_, PyAny>, what: &str) -> PyResult<Self> {
        let word = value
            .cast::<PyString>()
            .ok()
            .and_then(|s| s.to_str().ok().map(str::to_owned));
        match word.as_deref() {
            Some("light") => Ok(Setting::Light),
            Some("dark") => Ok(Setting::Dark),
            Some("auto") => Ok(Setting::Auto),
            _ => Err(PyValueError::new_err(format!(
                "{what} `scheme` must be \"light\", \"dark\" or \"auto\""
            ))),
        }
    }
}

/// One window's scheme state, shared by its handles like every other window setting.
pub(crate) struct SchemeState {
    /// The window's own setting; `None` takes the app's.
    own: Cell<Option<Setting>>,
    /// The app's default, as last pushed by `App`.
    inherited: Cell<Setting>,
    /// The OS appearance as last heard (`Some(true)` is dark); `None` while unknown.
    os_dark: Cell<Option<bool>>,
    /// Roles the owner set on this window, over the baseline.
    overrides: RefCell<Vec<(SchemeRole, Color)>>,
}

impl SchemeState {
    pub(crate) fn new() -> Self {
        Self {
            own: Cell::new(None),
            inherited: Cell::new(Setting::Auto),
            os_dark: Cell::new(None),
            overrides: RefCell::new(Vec::new()),
        }
    }

    /// What the window follows: its own setting, else the app's.
    pub(crate) fn setting(&self) -> Setting {
        self.own.get().unwrap_or(self.inherited.get())
    }

    /// Whether the window follows the OS appearance (`"auto"`), so it is worth asking for.
    pub(crate) fn follows_os(&self) -> bool {
        self.setting() == Setting::Auto
    }

    /// The baseline in force: the setting, with `"auto"` read from the OS flag.
    pub(crate) fn resolved(&self) -> Scheme {
        match self.setting() {
            Setting::Light => Scheme::Light,
            Setting::Dark => Scheme::Dark,
            Setting::Auto => {
                if self.os_dark.get() == Some(true) {
                    Scheme::Dark
                } else {
                    Scheme::Light
                }
            }
        }
    }

    /// The baseline of `resolved()` with the window's overrides on top.
    pub(crate) fn colors(&self) -> SchemeColors {
        let mut colors = SchemeColors::baseline(self.resolved());
        for &(role, color) in self.overrides.borrow().iter() {
            colors.set(role, color);
        }
        colors
    }

    pub(crate) fn set_own(&self, setting: Option<Setting>) {
        self.own.set(setting);
    }

    pub(crate) fn set_inherited(&self, setting: Setting) {
        self.inherited.set(setting);
    }

    /// Whether the OS appearance has been heard of or read.
    pub(crate) fn os_dark_known(&self) -> bool {
        self.os_dark.get().is_some()
    }

    pub(crate) fn set_os_dark(&self, dark: Option<bool>) {
        self.os_dark.set(dark);
    }

    pub(crate) fn set_overrides(&self, overrides: Vec<(SchemeRole, Color)>) {
        *self.overrides.borrow_mut() = overrides;
    }

    /// Gives `tree` the colours of the current state; `true` when they changed.
    pub(crate) fn sync(&self, tree: &mut Tree) -> bool {
        tree.set_scheme_colors(self.colors())
    }
}

/// The role table as a dict of `name -> (r, g, b, a)`.
pub(crate) fn colors_to_py(colors: &SchemeColors, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (role, color) in colors.iter() {
        dict.set_item(role.name(), color_to_py(color, py)?)?;
    }
    Ok(dict.into_any().unbind())
}

/// `scheme_colors={role: (r, g, b, a), ...}`; `None` clears every override. An unknown role
/// raises, listing the valid ones.
pub(crate) fn parse_overrides(value: &Bound<'_, PyAny>) -> PyResult<Vec<(SchemeRole, Color)>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    let dict = value.cast::<PyDict>().map_err(|_| {
        PyValueError::new_err(
            "window property `scheme_colors` must be a dict of role name -> (r, g, b, a), or None",
        )
    })?;
    let mut parsed = Vec::with_capacity(dict.len());
    for (key, color) in dict.iter() {
        let name: String = key.extract().map_err(|_| {
            PyValueError::new_err("window property `scheme_colors` has a key that is not a str")
        })?;
        let role = SchemeRole::from_name(&name).ok_or_else(|| {
            let valid: Vec<&str> = SchemeRole::ALL.iter().map(|r| r.name()).collect();
            PyValueError::new_err(format!(
                "window property `scheme_colors` has an unknown role {name:?} -- valid: {}",
                valid.join(", ")
            ))
        })?;
        parsed.push((role, parse_color(&color, "scheme_colors")?));
    }
    Ok(parsed)
}

/// `tess_re.scheme_colors("light" | "dark")`: the baseline role table, to start a toolkit's own from.
#[pyfunction]
#[pyo3(name = "scheme_colors")]
pub(crate) fn baseline_colors(scheme: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let scheme = Scheme::from_name(scheme).ok_or_else(|| {
        PyValueError::new_err(format!(
            "scheme_colors: unknown scheme {scheme:?} -- valid: light, dark"
        ))
    })?;
    colors_to_py(&SchemeColors::baseline(scheme), py)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_inherits_the_app_until_it_sets_its_own() {
        let state = SchemeState::new();
        assert_eq!(state.setting(), Setting::Auto);
        state.set_inherited(Setting::Dark);
        assert_eq!(state.setting(), Setting::Dark);
        assert_eq!(state.resolved(), Scheme::Dark);
        state.set_own(Some(Setting::Light));
        assert_eq!(state.resolved(), Scheme::Light);
        state.set_own(None);
        assert_eq!(state.resolved(), Scheme::Dark);
    }

    #[test]
    fn auto_follows_the_os_flag_and_is_light_while_it_is_unknown() {
        let state = SchemeState::new();
        assert_eq!(state.resolved(), Scheme::Light);
        state.set_os_dark(Some(true));
        assert_eq!(state.resolved(), Scheme::Dark);
        state.set_os_dark(Some(false));
        assert_eq!(state.resolved(), Scheme::Light);
        state.set_own(Some(Setting::Dark));
        assert_eq!(state.resolved(), Scheme::Dark);
    }

    #[test]
    fn overrides_sit_on_the_resolved_baseline() {
        let state = SchemeState::new();
        let red = Color::from_rgba8(255, 0, 0, 255);
        state.set_overrides(vec![(SchemeRole::OnSurface, red)]);
        assert_eq!(state.colors().text(), red);
        state.set_own(Some(Setting::Dark));
        assert_eq!(state.colors().text(), red);
        assert_eq!(
            state.colors().placeholder(),
            SchemeColors::baseline(Scheme::Dark).placeholder()
        );
        state.set_overrides(Vec::new());
        assert_eq!(
            state.colors().text(),
            SchemeColors::baseline(Scheme::Dark).text()
        );
    }
}
