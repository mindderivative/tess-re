//! M94: `node.set(**props)` / `node.get(name)` for the accessibility,
//! focus, and interaction properties of the M93 target API, and
//! `node.focus()`. `set` is atomic: every value is parsed and checked
//! before any is applied, so a bad call changes nothing. M96 extends the
//! same two methods to every property.

use std::cell::RefCell;
use std::rc::Rc;

use engine_core::{
    AccessValue, Anchored, Animated, CalcLen, CornerRadii, Cursor, Interpolate, Live, NodeId,
    NodeKind, PathData, Placement, Role, Shadow, Shadows, TerminalPalette, Tree, WindowRegion,
};
use peniko::Color;
use peniko::kurbo::Rect;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use taffy::prelude::{AvailableSpace, Dimension};
use taffy::style::ExpandedDimension;

use crate::dispatch::{HandlerKey, fire_focus_transition};
use crate::node::Node;
use crate::node_kind_props::{KIND_PROPS, KindChange, parse_kind_prop, read_kind_prop};
use crate::node_layout::{LAYOUT_PROPS, StyleEdit, parse_layout, read_layout};

/// Every property `set` accepts besides the layout ones
/// (`node_layout::LAYOUT_PROPS`), in the order its error lists them.
const SETTABLE: [&str; 93] = [
    // 0.5.7 (#179): placed against another node.
    "anchor",
    "placement",
    "offset",
    "visible",
    // 0.6.13: whether a container that chooses its children can choose this one.
    "item_selectable",
    "z_index",
    "clip_children",
    "snap_align",
    "mask",
    "sticky",
    "translate_x",
    "translate_y",
    "scale",
    "rotation_deg",
    "transform_origin",
    "fill",
    "stroke_color",
    "stroke_width",
    "opacity",
    "corner_radius",
    "corner_style",
    "hit_path",
    "morph",
    "params",
    "shadows",
    "blur",
    "backdrop_blur",
    "blend_mode",
    "data",
    "view_box",
    "hit_shape",
    "trim_start",
    "trim_end",
    "placeholder",
    "placeholder_fill",
    "caret_color",
    "max_length",
    "read_only",
    "input_mode",
    "caret_visible",
    "caret_width",
    "caret_shape",
    "caret_blink",
    "selection_fill",
    // 0.6.12 (P8): how a selection is painted.
    "selection_foreground",
    "selection_radius",
    "obscured",
    "palette",
    "role",
    "label",
    "value",
    "value_min",
    "value_max",
    "value_step",
    "checked",
    "selected",
    "expanded",
    "disabled",
    "level",
    "live",
    "a11y_hidden",
    "pressed",
    "set_size",
    "pos_in_set",
    "a11y_orientation",
    "modal",
    "invalid",
    "busy",
    "current",
    "description",
    "describedby",
    "controls",
    "active_descendant",
    "value_now",
    "value_text",
    "focusable",
    "tab_index",
    "cursor",
    "hit_testable",
    "window_region",
    "shader",
    // 0.6.11 (P13): the state layers (`state_layer_props`).
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

/// The M93 role vocabulary.
const ROLES: [(&str, Role); 42] = [
    ("button", Role::Button),
    ("checkbox", Role::CheckBox),
    ("radio", Role::RadioButton),
    ("switch", Role::Switch),
    ("slider", Role::Slider),
    ("progressbar", Role::ProgressIndicator),
    ("link", Role::Link),
    ("textbox", Role::TextInput),
    ("tab", Role::Tab),
    ("tablist", Role::TabList),
    ("tabpanel", Role::TabPanel),
    ("menu", Role::Menu),
    ("menuitem", Role::MenuItem),
    ("dialog", Role::Dialog),
    ("alert", Role::Alert),
    ("list", Role::List),
    ("listitem", Role::ListItem),
    ("tree", Role::Tree),
    ("treeitem", Role::TreeItem),
    ("heading", Role::Heading),
    ("img", Role::Image),
    ("group", Role::Group),
    ("none", Role::GenericContainer),
    // 0.5.6 (#160): AccessKit has no separator role; a splitter is the one the
    // platforms expose as a separator (AT-SPI) or separator control (UIA).
    ("separator", Role::Splitter),
    // 0.6.11 (UXREQ-004): the next 18 names; `option` is a listbox option (a menu's is `menuitem`).
    ("status", Role::Status),
    ("spinbutton", Role::SpinButton),
    ("radiogroup", Role::RadioGroup),
    ("option", Role::ListBoxOption),
    ("listbox", Role::ListBox),
    ("toolbar", Role::Toolbar),
    ("navigation", Role::Navigation),
    ("complementary", Role::Complementary),
    ("region", Role::Region),
    ("grid", Role::Grid),
    ("row", Role::Row),
    ("gridcell", Role::GridCell),
    ("columnheader", Role::ColumnHeader),
    ("rowheader", Role::RowHeader),
    ("searchbox", Role::SearchInput),
    ("combobox", Role::ComboBox),
    ("tooltip", Role::Tooltip),
    ("scrollbar", Role::ScrollBar),
];

const ORIENTATIONS: [(&str, engine_core::Orientation); 2] = [
    ("horizontal", engine_core::Orientation::Horizontal),
    ("vertical", engine_core::Orientation::Vertical),
];

const CURRENT: [(&str, engine_core::AriaCurrent); 5] = [
    ("page", engine_core::AriaCurrent::Page),
    ("step", engine_core::AriaCurrent::Step),
    ("location", engine_core::AriaCurrent::Location),
    ("date", engine_core::AriaCurrent::Date),
    ("time", engine_core::AriaCurrent::Time),
];

const LIVE: [(&str, Live); 3] = [
    ("off", Live::Off),
    ("polite", Live::Polite),
    ("assertive", Live::Assertive),
];

/// 0.5.7 (#179): one of `anchor`, `placement` or `offset`; a `set` applies them
/// in order to the node's current anchoring.
pub(crate) enum AnchorEdit {
    /// `None` puts the node back in the flow.
    Anchor(Option<NodeId>),
    Placement(Placement),
    Offset((f64, f64)),
}

/// 0.5.7 (#176, #177, #178): what a `width` or `height` value is.
#[derive(Clone, Copy)]
pub(crate) enum SizeMode {
    /// A number, `"auto"` or a percentage: the style holds it.
    Plain,
    /// `"fit-content"`: the style side is `auto` and a text node takes its text's size.
    Fit,
    /// A percentage plus pixels, such as `"calc(100% - 16)"`: the style side is
    /// `auto` until the tree resolves it against the parent.
    Calc(CalcLen),
}

/// One parsed, validated property write.
pub(crate) enum Change {
    Role(Role),
    Label(Option<String>),
    Value(Option<AccessValue>),
    ValueMin(Option<f64>),
    ValueMax(Option<f64>),
    ValueStep(Option<f64>),
    Checked(Option<engine_core::Toggled>),
    Selected(Option<bool>),
    Expanded(Option<bool>),
    Disabled(bool),
    Level(Option<usize>),
    Live(Option<Live>),
    A11yHidden(bool),
    /// 0.5.6 (#160): the accessibility states a screen reader hears about.
    Pressed(Option<engine_core::Toggled>),
    SetSize(Option<usize>),
    PosInSet(Option<usize>),
    Orientation(Option<engine_core::Orientation>),
    Modal(bool),
    Invalid(bool),
    Busy(bool),
    Current(Option<engine_core::AriaCurrent>),
    Description(Option<String>),
    ValueNow(Option<f64>),
    ValueText(Option<String>),
    DescribedBy(Vec<NodeId>),
    Controls(Vec<NodeId>),
    /// TRE-REQ-023.
    ActiveDescendant(Option<NodeId>),
    Focusable(bool),
    TabIndex(i32),
    Cursor(Option<Cursor>),
    /// 0.5.0 M3: `window_region`.
    WindowRegion(WindowRegion),
    HitTestable(Option<bool>),
    /// 0.6.11 (P13): one of the state-layer properties (`state_layer_props`).
    StateLayer(crate::state_layer_props::Edit),
    /// 0.5.1 (#66): the node's `shader`; `None` clears it.
    Shader(Option<std::sync::Arc<engine_core::Shader>>),
    Style(StyleEdit),
    /// 0.5.7 (#176, #177, #178): a `width` or `height`, and which kind of
    /// size it is. Any other value clears the last.
    Size {
        height: bool,
        mode: SizeMode,
        edit: StyleEdit,
    },
    Kind(KindChange),
    /// 0.5.7 (#179): `anchor`, `placement` or `offset`.
    Anchor(AnchorEdit),
    /// M96: a canvas's `draw`, or a virtual list's `materialize` or
    /// `size_hint` -- stored in the handler map.
    Callback(HandlerKey, Py<PyAny>),
    Visible(bool),
    ItemSelectable(bool),
    ZIndex(i32),
    ClipChildren(bool),
    /// 0.5.6 (#166): `snap_align`; `None` takes the scroll view's own.
    SnapAlign(Option<engine_core::SnapAlign>),
    /// 0.5.6 (#164): `mask`; `None` clears it.
    Mask(Option<Box<engine_core::Mask>>),
    /// 0.5.4 (#139): `sticky`, an inset from the scroller's start edge, or `None`.
    Sticky(Option<f64>),
    TranslateX(f64),
    TranslateY(f64),
    Scale(f64),
    RotationDeg(f64),
    TransformOrigin((f64, f64)),
    CornerStyle([bool; 4]),
    HitPath(Option<std::sync::Arc<peniko::kurbo::BezPath>>),
    Data(PathData),
    ViewBox(Option<Rect>),
    /// 0.5.9 (#236).
    HitShape(engine_core::HitShape),
    Morph(engine_core::MorphMode),
    /// #272: `params`, name and value pairs.
    Params(Vec<(String, f64)>),
    /// #272: `data` given as a template.
    Template(engine_core::PathTemplate),
    TrimStart(f64),
    TrimEnd(f64),
    Fill(Color),
    /// 0.6.11: `fill` set to `None`: a text or text input goes back to its window scheme's colour.
    FillUnset,
    /// 0.5.4 (#110).
    Blur(f64),
    BackdropBlur(f64),
    BlendMode(engine_core::Blend),
    /// 0.5.4 (#109): `fill` set to a `Gradient`.
    FillGradient(engine_core::Gradient),
    StrokeColor(Color),
    StrokeGradient(engine_core::Gradient),
    StrokeWidth(f64),
    /// 0.6.11 (UXREQ-008).
    Outline(crate::outline_props::OutlineChange),
    /// 0.6.12 (P10): a shown layer's placement policy (`layer_viewport_margin`, ...).
    Layer(crate::layer_policy::Edit),
    Opacity(f64),
    CornerRadius(Radius),
    Shadows(Vec<Shadow>),
    Placeholder(String),
    PlaceholderFill(Option<Color>),
    CaretColor(Option<Color>),
    /// 0.5.6 (#162, #165): more text input properties.
    MaxLength(Option<usize>),
    ReadOnly(bool),
    InputMode(engine_core::InputMode),
    CaretVisible(bool),
    CaretWidth(f32),
    CaretShape(engine_core::CaretShape),
    CaretBlink(u32),
    SelectionFill(Option<Color>),
    /// 0.6.12 (P8).
    SelectionForeground(Option<Color>),
    SelectionRadius(f32),
    Obscured(bool),
    Palette(Box<PalettePatch>),
}

/// M95: the palette keys a `set(palette={...})` gives -- the rest keep
/// their current colors. 0.6.11: `foreground`, `cursor` and `selection` also take `None`,
/// which gives back the window's scheme (the outer `Option` is "given", the inner "a colour").
#[derive(Default)]
pub(crate) struct PalettePatch {
    ansi: Option<[Color; 16]>,
    foreground: Option<Option<Color>>,
    background: Option<Color>,
    cursor: Option<Option<Color>>,
    selection: Option<Option<Color>>,
}

impl PalettePatch {
    fn apply(&self, palette: &mut TerminalPalette) {
        if let Some(ansi) = self.ansi {
            palette.ansi = ansi;
        }
        if let Some(background) = self.background {
            palette.background = background;
        }
        let fields = [
            (self.foreground, &mut palette.foreground),
            (self.cursor, &mut palette.cursor),
            (self.selection, &mut palette.selection),
        ];
        for (value, slot) in fields {
            if let Some(color) = value {
                *slot = color;
            }
        }
    }
}

fn parse_palette(value: &Bound<'_, PyAny>, name: &str) -> PyResult<PalettePatch> {
    let expected = "a dict with any of: ansi (16 colors), foreground, background, cursor, \
                    selection (foreground, cursor and selection also take None)";
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid(name, expected))?;
    let mut patch = PalettePatch::default();
    for (key, color) in dict.iter() {
        let key: String = key.extract().map_err(|_| invalid(name, expected))?;
        match key.as_str() {
            "ansi" => {
                let colors: Vec<Bound<'_, PyAny>> = required(&color, name, expected)?;
                if colors.len() != 16 {
                    return Err(invalid(name, "a dict whose `ansi` has exactly 16 colors"));
                }
                let mut ansi = [Color::TRANSPARENT; 16];
                for (slot, color) in ansi.iter_mut().zip(&colors) {
                    *slot = parse_color(color, name)?;
                }
                patch.ansi = Some(ansi);
            }
            "foreground" => patch.foreground = Some(optional_color(&color, name)?),
            "background" => patch.background = Some(parse_color(&color, name)?),
            "cursor" => patch.cursor = Some(optional_color(&color, name)?),
            "selection" => patch.selection = Some(optional_color(&color, name)?),
            _ => return Err(invalid(name, expected)),
        }
    }
    Ok(patch)
}

fn palette_to_py(palette: &TerminalPalette, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    let ansi = palette
        .ansi
        .iter()
        .map(|c| color_to_py(*c, py))
        .collect::<PyResult<Vec<_>>>()?;
    dict.set_item("ansi", ansi)?;
    let optional = |c: Option<Color>| c.map_or_else(|| Ok(py.None()), |c| color_to_py(c, py));
    dict.set_item("foreground", optional(palette.foreground)?)?;
    dict.set_item("background", color_to_py(palette.background, py)?)?;
    dict.set_item("cursor", optional(palette.cursor)?)?;
    dict.set_item("selection", optional(palette.selection)?)?;
    Ok(dict.into_any().unbind())
}

fn optional_color(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<Color>> {
    if value.is_none() {
        return Ok(None);
    }
    parse_color(value, name).map(Some)
}

/// M95: `corner_radius` -- one radius, or `[top_left, top_right,
/// bottom_right, bottom_left]`.
#[derive(Clone, Copy)]
pub(crate) enum Radius {
    Uniform(f64),
    Corners([f64; 4]),
}

/// An `(r, g, b, a)` tuple of 0-255 ints.
pub(crate) fn parse_color(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Color> {
    let (r, g, b, a): (u8, u8, u8, u8) =
        required(value, name, "an (r, g, b, a) tuple of 0-255 ints")?;
    Ok(Color::from_rgba8(r, g, b, a))
}

/// A color as the `(r, g, b, a)` tuple it was set from.
pub(crate) fn color_to_py(color: Color, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let [r, g, b, a] = color.to_rgba8().to_u8_array();
    Ok((r, g, b, a).into_pyobject(py)?.into_any().unbind())
}

/// A non-negative, finite number.
pub(crate) fn parse_non_negative(value: &Bound<'_, PyAny>, name: &str) -> PyResult<f64> {
    let number: f64 = required(value, name, "a non-negative number")?;
    if number < 0.0 || !number.is_finite() {
        return Err(invalid(name, "a non-negative number"));
    }
    Ok(number)
}

/// 0.5.6 (#164): `mask`: `None`, `"circle"`, `{"rounded": radius}` (a number or a
/// four-corner tuple) or `{"path": svg_path_data, "view_box": (x, y, w, h)}`.
fn parse_mask(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<Box<engine_core::Mask>>> {
    use engine_core::{CornerRadii, Mask};
    const EXPECTED: &str = "None, \"circle\", {\"rounded\": radius}, or {\"path\": svg_path_data, \"view_box\": (x, y, w, h)}";
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(text) = value.extract::<String>() {
        return if text == "circle" {
            Ok(Some(Box::new(Mask::Circle)))
        } else {
            Err(invalid(name, EXPECTED))
        };
    }
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| invalid(name, EXPECTED))?;
    let keys: Vec<String> = dict
        .keys()
        .iter()
        .map(|k| k.extract::<String>())
        .collect::<PyResult<_>>()
        .map_err(|_| invalid(name, EXPECTED))?;
    if keys == ["rounded"] {
        let radius = dict.get_item("rounded")?.expect("the key is there");
        let radii = match parse_radius(&radius, name)? {
            Radius::Uniform(r) => [r; 4],
            Radius::Corners(corners) => corners,
        };
        return Ok(Some(Box::new(Mask::Rounded(CornerRadii(radii)))));
    }
    if keys.len() == 2 && keys.iter().all(|k| k == "path" || k == "view_box") {
        let data: String = dict
            .get_item("path")?
            .expect("the key is there")
            .extract()
            .map_err(|_| invalid(name, "a mask whose path is a str of SVG path data"))?;
        let data = PathData::from_svg(&data).map_err(|err| {
            PyValueError::new_err(format!(
                "node property `{name}`: the path isn't valid SVG path data: {err}"
            ))
        })?;
        let view_box = dict.get_item("view_box")?.expect("the key is there");
        let (x, y, w, h): (f64, f64, f64, f64) = view_box.extract().map_err(|_| {
            invalid(
                name,
                "a mask whose view_box is an (x, y, width, height) tuple",
            )
        })?;
        if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite())
            || w <= 0.0
            || h <= 0.0
        {
            return Err(invalid(name, "a mask whose view_box has a positive size"));
        }
        return Ok(Some(Box::new(Mask::Path {
            data,
            view_box: Rect::new(x, y, x + w, y + h),
        })));
    }
    Err(invalid(name, EXPECTED))
}

/// A mask as `parse_mask` takes it.
fn mask_to_py(mask: Option<&engine_core::Mask>, py: Python<'_>) -> PyResult<Py<PyAny>> {
    use engine_core::Mask;
    let Some(mask) = mask else {
        return Ok(py.None());
    };
    Ok(match mask {
        Mask::Circle => "circle".into_pyobject(py)?.into_any().unbind(),
        Mask::Rounded(radii) => {
            let dict = PyDict::new(py);
            let [a, b, c, d] = radii.0;
            if a == b && b == c && c == d {
                dict.set_item("rounded", a)?;
            } else {
                dict.set_item("rounded", (a, b, c, d))?;
            }
            dict.into_any().unbind()
        }
        Mask::Path { data, view_box } => {
            let dict = PyDict::new(py);
            dict.set_item("path", data.to_svg())?;
            dict.set_item(
                "view_box",
                (
                    view_box.x0,
                    view_box.y0,
                    view_box.width(),
                    view_box.height(),
                ),
            )?;
            dict.into_any().unbind()
        }
    })
}

pub(crate) fn parse_radius(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Radius> {
    let expected =
        "a non-negative number or a (top_left, top_right, bottom_right, bottom_left) tuple";
    if let Ok(number) = value.extract::<f64>() {
        if number < 0.0 || !number.is_finite() {
            return Err(invalid(name, expected));
        }
        return Ok(Radius::Uniform(number));
    }
    let corners: (f64, f64, f64, f64) = required(value, name, expected)?;
    let corners = [corners.0, corners.1, corners.2, corners.3];
    if corners.iter().any(|c| *c < 0.0 || !c.is_finite()) {
        return Err(invalid(name, expected));
    }
    Ok(Radius::Corners(corners))
}

pub(crate) fn parse_shadows(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<Shadow>> {
    let expected = "a list of (color, offset_x, offset_y, blur, spread) tuples, blur non-negative";
    let items: Vec<Bound<'_, PyAny>> = required(value, name, expected)?;
    items
        .iter()
        .map(|item| {
            let (color, offset_x, offset_y, blur, spread): (Bound<'_, PyAny>, f64, f64, f64, f64) =
                required(item, name, expected)?;
            if blur < 0.0 {
                return Err(invalid(name, expected));
            }
            Ok(Shadow {
                color: parse_color(&color, name)?,
                offset_x,
                offset_y,
                blur,
                spread,
            })
        })
        .collect()
}

/// Reads an animatable property -- its current (possibly mid-animation)
/// value, or with `target` the value it's heading to. `None` for any
/// other name.
pub(crate) fn animatable_to_py(
    node: &engine_core::Node,
    scheme_text: Color,
    name: &str,
    target: bool,
    py: Python<'_>,
) -> PyResult<Option<Py<PyAny>>> {
    fn pick<T: Interpolate + Clone>(value: &Animated<T>, target: bool) -> &T {
        if target {
            value.target()
        } else {
            &value.current
        }
    }
    let number = |v: f64| -> PyResult<Py<PyAny>> { Ok(v.into_pyobject(py)?.into_any().unbind()) };
    let value = match name {
        "fill" => match &node.paint.gradient {
            Some(gradient) => Py::new(
                py,
                crate::gradient::PyGradient {
                    inner: pick(gradient, target).clone(),
                },
            )?
            .into_any(),
            // A text or text input with no colour of its own reads as its scheme's.
            None => color_to_py(
                match &node.kind {
                    NodeKind::TextField(state) if state.tint_scheme => scheme_text,
                    NodeKind::TextField(state) => *pick(&state.text_tint, target),
                    NodeKind::Text(_) if node.paint.scheme_fill => scheme_text,
                    _ => *pick(&node.paint.background, target),
                },
                py,
            )?,
        },
        "scroll_offset" => match &node.kind {
            NodeKind::ScrollView(state) => number(*pick(&state.scroll, target))?,
            // 0.5.6 (#166): a virtual list has one too.
            NodeKind::VirtualList(state) => number(*pick(&state.scroll_offset, target))?,
            _ => {
                return Err(PyValueError::new_err(
                    "node property `scroll_offset` applies only to a scroll_view or virtual_list node",
                ));
            }
        },
        // 0.6.11: a `both` view's horizontal offset.
        "scroll_offset_x" => match &node.kind {
            NodeKind::ScrollView(state) => number(*pick(&state.scroll_x, target))?,
            _ => {
                return Err(PyValueError::new_err(
                    "node property `scroll_offset_x` applies only to a scroll_view node",
                ));
            }
        },
        "stroke_color" => match &node.paint.border_gradient {
            Some(gradient) => Py::new(
                py,
                crate::gradient::PyGradient {
                    inner: (**gradient).clone(),
                },
            )?
            .into_any(),
            None => color_to_py(*pick(&node.paint.border_color, target), py)?,
        },
        "caret_color" => {
            let NodeKind::TextField(state) = &node.kind else {
                return Err(PyValueError::new_err(
                    "node property `caret_color` applies only to a text_input node",
                ));
            };
            match &state.caret_color {
                Some(caret) => color_to_py(*pick(caret, target), py)?,
                None => py.None(),
            }
        }
        "stroke_width" => number(*pick(&node.paint.border_width, target))?,
        "translate_x" => number(*pick(&node.paint.node_transform.translate_x, target))?,
        "translate_y" => number(*pick(&node.paint.node_transform.translate_y, target))?,
        "scale" => number(*pick(&node.paint.node_transform.scale, target))?,
        "rotation_deg" => number(*pick(&node.paint.node_transform.rotation_deg, target))?,
        "hit_path" => match &node.paint.hit_outline {
            Some(outline) => outline.to_svg().into_pyobject(py)?.into_any().unbind(),
            None => py.None(),
        },
        "corner_style" => {
            let names = node
                .paint
                .corner_concave
                .map(|c| if c { "concave" } else { "round" });
            if names.iter().all(|n| *n == names[0]) {
                names[0].into_pyobject(py)?.into_any().unbind()
            } else {
                (names[0], names[1], names[2], names[3])
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
        }
        "transform_origin" => node
            .paint
            .node_transform
            .origin
            .into_pyobject(py)?
            .into_any()
            .unbind(),
        "opacity" => number(*pick(&node.paint.opacity, target))?,
        "blur" => number(*pick(&node.paint.blur, target))?,
        "backdrop_blur" => number(*pick(&node.paint.backdrop_blur, target))?,
        "corner_radius" => match &node.paint.corner_radii_override {
            Some(radii) => {
                let [a, b, c, d] = pick(radii, target).0;
                (a, b, c, d).into_pyobject(py)?.into_any().unbind()
            }
            None => number(*pick(&node.paint.corner_radius, target))?,
        },
        "shadows" => {
            let mut out = Vec::new();
            for shadow in &pick(&node.paint.shadows, target).0 {
                out.push((
                    color_to_py(shadow.color, py)?,
                    shadow.offset_x,
                    shadow.offset_y,
                    shadow.blur,
                    shadow.spread,
                ));
            }
            out.into_pyobject(py)?.into_any().unbind()
        }
        "svg_time" => {
            let NodeKind::Svg(state) = &node.kind else {
                return Err(PyValueError::new_err(
                    "node property `svg_time` applies only to an svg node",
                ));
            };
            number(*pick(&state.time, target))?
        }
        "data" | "trim_start" | "trim_end" => {
            let NodeKind::Path(state) = &node.kind else {
                return Err(PyValueError::new_err(format!(
                    "node property `{name}` applies only to a path node"
                )));
            };
            match name {
                "data" => pick(&state.data, target)
                    .to_svg()
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "trim_start" => number(*pick(&state.trim_start, target))?,
                _ => number(*pick(&state.trim_end, target))?,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

/// Stops `name`'s running animation where it is. `false` for a name with
/// no animation to stop.
pub(crate) fn stop_animatable(node: &mut engine_core::Node, name: &str) -> PyResult<bool> {
    match name {
        "fill" => {
            if let Some(gradient) = &mut node.paint.gradient {
                gradient.stop();
            }
            match &mut node.kind {
                NodeKind::TextField(state) => state.text_tint.stop(),
                _ => node.paint.background.stop(),
            }
        }
        "scroll_offset" => match &mut node.kind {
            NodeKind::ScrollView(state) => state.scroll.stop(),
            NodeKind::VirtualList(state) => state.scroll_offset.stop(),
            _ => {
                return Err(PyValueError::new_err(
                    "node property `scroll_offset` applies only to a scroll_view or virtual_list node",
                ));
            }
        },
        "scroll_offset_x" => match &mut node.kind {
            NodeKind::ScrollView(state) => state.scroll_x.stop(),
            _ => {
                return Err(PyValueError::new_err(
                    "node property `scroll_offset_x` applies only to a scroll_view node",
                ));
            }
        },
        "stroke_color" => node.paint.border_color.stop(),
        "caret_color" => {
            if let NodeKind::TextField(state) = &mut node.kind
                && let Some(caret) = &mut state.caret_color
            {
                caret.stop();
            }
        }
        "stroke_width" => node.paint.border_width.stop(),
        "translate_x" => node.paint.node_transform.translate_x.stop(),
        "translate_y" => node.paint.node_transform.translate_y.stop(),
        "scale" => node.paint.node_transform.scale.stop(),
        "rotation_deg" => node.paint.node_transform.rotation_deg.stop(),
        "opacity" => node.paint.opacity.stop(),
        "blur" => node.paint.blur.stop(),
        "backdrop_blur" => node.paint.backdrop_blur.stop(),
        "corner_radius" => {
            node.paint.corner_radius.stop();
            if let Some(radii) = &mut node.paint.corner_radii_override {
                radii.stop();
            }
        }
        "shadows" => node.paint.shadows.stop(),
        "svg_time" => {
            let NodeKind::Svg(state) = &mut node.kind else {
                return Err(PyValueError::new_err(
                    "node property `svg_time` applies only to an svg node",
                ));
            };
            state.time.stop();
        }
        "data" | "trim_start" | "trim_end" => {
            let NodeKind::Path(state) = &mut node.kind else {
                return Err(PyValueError::new_err(format!(
                    "node property `{name}` applies only to a path node"
                )));
            };
            match name {
                "data" => state.data.stop(),
                "trim_start" => state.trim_start.stop(),
                _ => state.trim_end.stop(),
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// A property only one kind has: its name, that kind's name, and a test
/// for it.
type KindRequirement = (&'static str, &'static str, fn(&NodeKind) -> bool);

impl Change {
    /// The kind this change requires, if it's specific to one -- checked
    /// before anything is applied.
    fn kind_requirement(&self) -> Option<KindRequirement> {
        fn path(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::Path(_))
        }
        fn text_input(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::TextField(_))
        }
        fn terminal(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::Terminal(_))
        }
        fn box_node(kind: &NodeKind) -> bool {
            matches!(
                kind,
                NodeKind::Rect | NodeKind::Container | NodeKind::Path(_) | NodeKind::Text(_)
            )
        }
        // 0.6.12 (P8): a terminal's cursor is a caret too.
        fn input_or_terminal(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::TextField(_) | NodeKind::Terminal(_))
        }
        fn selectable(kind: &NodeKind) -> bool {
            matches!(
                kind,
                NodeKind::Text(_) | NodeKind::TextField(_) | NodeKind::Terminal(_)
            )
        }
        fn text_or_input(kind: &NodeKind) -> bool {
            matches!(kind, NodeKind::Text(_) | NodeKind::TextField(_))
        }
        Some(match self {
            Change::Data(_) => ("data", "path", path),
            Change::ViewBox(_) => ("view_box", "path", path),
            Change::HitShape(_) => ("hit_shape", "path", path),
            Change::Morph(_) => ("morph", "path", path),
            Change::Params(_) => ("params", "path", path),
            Change::Template(_) => ("data", "path", path),
            Change::TrimStart(_) => ("trim_start", "path", path),
            Change::TrimEnd(_) => ("trim_end", "path", path),
            Change::Placeholder(_) => ("placeholder", "text_input", text_input),
            Change::PlaceholderFill(_) => ("placeholder_fill", "text_input", text_input),
            Change::CaretColor(_) => ("caret_color", "text_input", text_input),
            Change::MaxLength(_) => ("max_length", "text_input", text_input),
            Change::ReadOnly(_) => ("read_only", "text_input", text_input),
            Change::InputMode(_) => ("input_mode", "text_input", text_input),
            Change::CaretVisible(_) => ("caret_visible", "text_input", text_input),
            Change::CaretWidth(_) => ("caret_width", "text_input or terminal", input_or_terminal),
            Change::CaretShape(_) => ("caret_shape", "text_input or terminal", input_or_terminal),
            Change::CaretBlink(_) => ("caret_blink", "text_input or terminal", input_or_terminal),
            Change::SelectionFill(_) => ("selection_fill", "text or text_input", text_or_input),
            Change::SelectionForeground(_) => (
                "selection_foreground",
                "text, text_input or terminal",
                selectable,
            ),
            Change::SelectionRadius(_) => (
                "selection_radius",
                "text, text_input or terminal",
                selectable,
            ),
            Change::Obscured(_) => ("obscured", "text_input", text_input),
            Change::Palette(_) => ("palette", "terminal", terminal),
            Change::FillGradient(_) => ("fill", "box, path or text", box_node),
            Change::FillUnset => ("fill", "text or text_input", text_or_input),
            _ => return None,
        })
    }
}

/// A trim fraction, `0.0..=1.0`.
fn fraction(value: &Bound<'_, PyAny>, name: &str) -> PyResult<f64> {
    let fraction: f64 = required(value, name, "a number from 0.0 to 1.0")?;
    if !(0.0..=1.0).contains(&fraction) {
        return Err(invalid(name, "a number from 0.0 to 1.0"));
    }
    Ok(fraction)
}

fn invalid(name: &str, expected: &str) -> PyErr {
    PyValueError::new_err(format!("node property `{name}` must be {expected}"))
}

fn names<T>(table: &[(&str, T)]) -> String {
    table
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `value`, extracted as `T` or `None`, with a clear error otherwise.
fn optional<'py, T: for<'a> FromPyObject<'a, 'py>>(
    value: &Bound<'py, PyAny>,
    name: &str,
    expected: &str,
) -> PyResult<Option<T>> {
    if value.is_none() {
        return Ok(None);
    }
    value
        .extract::<T>()
        .map(Some)
        .map_err(|_| invalid(name, expected))
}

fn required<'py, T: for<'a> FromPyObject<'a, 'py>>(
    value: &Bound<'py, PyAny>,
    name: &str,
    expected: &str,
) -> PyResult<T> {
    value.extract::<T>().map_err(|_| invalid(name, expected))
}

/// A bool that must be a real `bool`, not any truthy value.
fn boolean(value: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    if !value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, "a bool"));
    }
    value.extract::<bool>()
}

/// `True`, `False`, `"mixed"` or `None`.
fn toggled(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<engine_core::Toggled>> {
    if value.is_none() {
        Ok(None)
    } else if value.is_instance_of::<pyo3::types::PyBool>() {
        Ok(Some(if value.extract::<bool>()? {
            engine_core::Toggled::True
        } else {
            engine_core::Toggled::False
        }))
    } else if value.extract::<String>().is_ok_and(|s| s == "mixed") {
        Ok(Some(engine_core::Toggled::Mixed))
    } else {
        Err(invalid(name, "True, False, \"mixed\", or None"))
    }
}

/// A positive int or `None` (`set_size`, `pos_in_set`).
fn positive_or_none(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<usize>> {
    const WANT: &str = "a positive int or None";
    if value.is_none() {
        return Ok(None);
    }
    if value.is_instance_of::<pyo3::types::PyBool>() {
        return Err(invalid(name, WANT));
    }
    match value.extract::<usize>() {
        Ok(n) if n > 0 => Ok(Some(n)),
        _ => Err(invalid(name, WANT)),
    }
}

fn toggled_object(py: Python<'_>, state: Option<engine_core::Toggled>) -> PyResult<Py<PyAny>> {
    Ok(match state {
        None => py.None(),
        Some(engine_core::Toggled::True) => true.into_pyobject(py)?.to_owned().into_any().unbind(),
        Some(engine_core::Toggled::False) => {
            false.into_pyobject(py)?.to_owned().into_any().unbind()
        }
        Some(engine_core::Toggled::Mixed) => "mixed".into_pyobject(py)?.into_any().unbind(),
    })
}

fn optional_bool(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<bool>> {
    if value.is_none() {
        return Ok(None);
    }
    boolean(value, name).map(Some)
}

/// 0.5.7 (#178): `"calc(100% - 16)"`, `"100% - 16px"`, `"33% + 10"`: percentages
/// and pixel counts joined by ` + ` and ` - ` (spaces required, as in CSS), with
/// or without the `calc(...)`. `None` if it is not that.
fn parse_calc(text: &str) -> Option<CalcLen> {
    let text = text.trim();
    let inner = text
        .strip_prefix("calc(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or(text);
    let tokens: Vec<&str> = inner.split_whitespace().collect();
    if tokens.is_empty() || tokens.len().is_multiple_of(2) {
        return None;
    }
    let mut calc = CalcLen::default();
    let mut sign = 1.0_f32;
    for (i, token) in tokens.iter().enumerate() {
        if i % 2 == 1 {
            sign = match *token {
                "+" => 1.0,
                "-" => -1.0,
                _ => return None,
            };
            continue;
        }
        let (number, is_percent) = match token.strip_suffix('%') {
            Some(number) => (number, true),
            None => (token.strip_suffix("px").unwrap_or(token), false),
        };
        let value: f32 = number.parse().ok().filter(|v: &f32| v.is_finite())?;
        if is_percent {
            calc.percent += sign * value / 100.0;
        } else {
            calc.px += sign * value;
        }
    }
    Some(calc)
}

/// A plain width or height of `dimension`.
fn plain_size(height: bool, dimension: Dimension) -> Change {
    Change::Size {
        height,
        mode: SizeMode::Plain,
        edit: Box::new(move |style| {
            if height {
                style.size.height = dimension;
            } else {
                style.size.width = dimension;
            }
        }),
    }
}

fn parse(name: &str, value: &Bound<'_, PyAny>) -> PyResult<Change> {
    if name == "width" || name == "height" {
        let height = name == "height";
        let auto_side: StyleEdit = Box::new(move |style| {
            if height {
                style.size.height = Dimension::auto();
            } else {
                style.size.width = Dimension::auto();
            }
        });
        if let Ok(text) = value.extract::<String>() {
            if text == "fit-content" {
                return Ok(Change::Size {
                    height,
                    mode: SizeMode::Fit,
                    edit: auto_side,
                });
            }
            let text = text.trim();
            if text.starts_with("calc(") || text.split_whitespace().count() > 1 {
                let calc = parse_calc(text).ok_or_else(|| {
                    invalid(
                        name,
                        "a number, \"auto\", \"fit-content\", a percentage like \"50%\", or a \
                         percentage plus pixels like \"calc(100% - 16)\"",
                    )
                })?;
                // A mix of the two is a calc size; a lone percentage or pixel
                // count is the plain size it says.
                return Ok(match (calc.percent != 0.0, calc.px != 0.0) {
                    (true, true) => Change::Size {
                        height,
                        mode: SizeMode::Calc(calc),
                        edit: auto_side,
                    },
                    (true, false) => plain_size(height, Dimension::percent(calc.percent)),
                    (false, _) => plain_size(height, Dimension::length(calc.px.max(0.0))),
                });
            }
        }
        if let Some(edit) = parse_layout(name, value) {
            return edit.map(|edit| Change::Size {
                height,
                mode: SizeMode::Plain,
                edit,
            });
        }
    }
    if let Some(edit) = parse_layout(name, value) {
        return edit.map(Change::Style);
    }
    if let Some(change) = crate::outline_props::parse(name, value) {
        return change.map(Change::Outline);
    }
    let number = |expected: &str| -> PyResult<f64> {
        let number: f64 = required(value, name, expected)?;
        if !number.is_finite() || value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, expected));
        }
        Ok(number)
    };
    Ok(match name {
        "visible" => Change::Visible(boolean(value, name)?),
        "item_selectable" => Change::ItemSelectable(boolean(value, name)?),
        "z_index" => {
            if value.is_instance_of::<pyo3::types::PyBool>() {
                return Err(invalid(name, "an int"));
            }
            Change::ZIndex(required(value, name, "an int")?)
        }
        "clip_children" => Change::ClipChildren(boolean(value, name)?),
        "snap_align" => Change::SnapAlign(if value.is_none() {
            None
        } else {
            let text: String = value
                .extract()
                .map_err(|_| invalid(name, "\"start\", \"center\", \"end\", or None"))?;
            Some(
                engine_core::SnapAlign::ALL
                    .iter()
                    .find(|(n, _)| *n == text)
                    .map(|(_, a)| *a)
                    .ok_or_else(|| invalid(name, "\"start\", \"center\", \"end\", or None"))?,
            )
        }),
        "mask" => Change::Mask(parse_mask(value, name)?),
        "sticky" => Change::Sticky(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, "a non-negative number of pixels, or None"));
        } else {
            Some(
                parse_non_negative(value, name)
                    .map_err(|_| invalid(name, "a non-negative number of pixels, or None"))?,
            )
        }),
        "translate_x" => Change::TranslateX(number("a number")?),
        "translate_y" => Change::TranslateY(number("a number")?),
        "scale" => Change::Scale(parse_non_negative(value, name)?),
        "rotation_deg" => Change::RotationDeg(number("a number")?),
        "hit_path" => Change::HitPath(if value.is_none() {
            None
        } else {
            let expected = "SVG path data (a str) in the node's own pixels, or None";
            let data: String = required(value, name, expected)?;
            let path = engine_core::PathData::from_svg(&data)
                .map_err(|e| invalid(name, &format!("{expected}; {e}")))?;
            Some(std::sync::Arc::new(path.0))
        }),
        "corner_style" => {
            let expected = "\"round\" or \"concave\", or four of them (top-left, top-right, bottom-right, bottom-left)";
            let one = |text: &str| match text {
                "round" => Some(false),
                "concave" => Some(true),
                _ => None,
            };
            let styles = if let Ok(text) = value.extract::<String>() {
                one(&text).map(|c| [c; 4])
            } else {
                value
                    .extract::<(String, String, String, String)>()
                    .ok()
                    .and_then(|(a, b, c, d)| Some([one(&a)?, one(&b)?, one(&c)?, one(&d)?]))
            };
            Change::CornerStyle(styles.ok_or_else(|| invalid(name, expected))?)
        }
        "transform_origin" => {
            let pair: (f64, f64) = value
                .extract()
                .ok()
                .filter(|(x, y): &(f64, f64)| {
                    x.is_finite() && y.is_finite() && !value.is_instance_of::<pyo3::types::PyBool>()
                })
                .ok_or_else(|| invalid(name, "a pair of numbers (x, y), fractions of the box"))?;
            Change::TransformOrigin(pair)
        }
        "role" => {
            let role: String = required(value, name, "a str")?;
            let role = ROLES
                .iter()
                .find(|(n, _)| *n == role)
                .map(|(_, r)| *r)
                .ok_or_else(|| invalid(name, &format!("one of: {}", names(&ROLES))))?;
            Change::Role(role)
        }
        "label" => Change::Label(optional(value, name, "a str or None")?),
        "value" => Change::Value(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, "a str, a number, or None"));
        } else if let Ok(text) = value.extract::<String>() {
            Some(AccessValue::Text(text))
        } else if let Ok(number) = value.extract::<f64>() {
            Some(AccessValue::Number(number))
        } else {
            return Err(invalid(name, "a str, a number, or None"));
        }),
        "value_min" => Change::ValueMin(optional(value, name, "a number or None")?),
        "value_max" => Change::ValueMax(optional(value, name, "a number or None")?),
        "value_step" => Change::ValueStep(optional(value, name, "a number or None")?),
        "checked" => Change::Checked(toggled(value, name)?),
        "set_size" => Change::SetSize(positive_or_none(value, name)?),
        "pos_in_set" => Change::PosInSet(positive_or_none(value, name)?),
        "a11y_orientation" => {
            let text: Option<String> =
                optional(value, name, "\"horizontal\", \"vertical\", or None")?;
            Change::Orientation(match text {
                None => None,
                Some(text) => Some(
                    ORIENTATIONS
                        .iter()
                        .find(|(n, _)| *n == text)
                        .map(|(_, o)| *o)
                        .ok_or_else(|| invalid(name, "\"horizontal\", \"vertical\", or None"))?,
                ),
            })
        }
        "modal" => Change::Modal(boolean(value, name)?),
        "selected" => Change::Selected(optional_bool(value, name)?),
        "expanded" => Change::Expanded(optional_bool(value, name)?),
        "disabled" => Change::Disabled(boolean(value, name)?),
        "level" => {
            let level: Option<usize> = optional(value, name, "a positive int or None")?;
            if level == Some(0) {
                return Err(invalid(name, "a positive int or None"));
            }
            Change::Level(level)
        }
        "live" => {
            let live: Option<String> = optional(value, name, "a str or None")?;
            Change::Live(match live {
                None => None,
                Some(live) => Some(
                    LIVE.iter()
                        .find(|(n, _)| *n == live)
                        .map(|(_, l)| *l)
                        .ok_or_else(|| invalid(name, &format!("one of: {}", names(&LIVE))))?,
                ),
            })
        }
        "a11y_hidden" => Change::A11yHidden(boolean(value, name)?),
        "pressed" => Change::Pressed(toggled(value, name)?),
        "invalid" => Change::Invalid(boolean(value, name)?),
        "busy" => Change::Busy(boolean(value, name)?),
        "current" => Change::Current(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            Some(if value.extract::<bool>()? {
                engine_core::AriaCurrent::True
            } else {
                engine_core::AriaCurrent::False
            })
        } else {
            let text: String = required(value, name, "a str, a bool, or None")?;
            Some(
                CURRENT
                    .iter()
                    .find(|(n, _)| *n == text)
                    .map(|(_, c)| *c)
                    .ok_or_else(|| {
                        invalid(
                            name,
                            &format!("True, False, None, or one of: {}", names(&CURRENT)),
                        )
                    })?,
            )
        }),
        "description" => Change::Description(optional(value, name, "a str or None")?),
        "value_now" => Change::ValueNow(optional(value, name, "a number or None")?),
        "value_text" => Change::ValueText(optional(value, name, "a str or None")?),
        "focusable" => Change::Focusable(boolean(value, name)?),
        "tab_index" => Change::TabIndex(required(value, name, "an int")?),
        // 0.5.4 (#140): a `CursorImage` is a cursor too.
        "cursor"
            if value
                .extract::<crate::cursor_image::PyCursorImage>()
                .is_ok() =>
        {
            let image = value
                .extract::<crate::cursor_image::PyCursorImage>()
                .expect("checked");
            Change::Cursor(Some(Cursor::Custom(image.id)))
        }
        "cursor" => {
            let cursor: Option<String> = optional(value, name, "a str, a CursorImage, or None")?;
            Change::Cursor(match cursor {
                None => None,
                Some(cursor) => Some(Cursor::from_name(&cursor).ok_or_else(|| {
                    let valid: Vec<&str> = Cursor::ALL.iter().map(|c| c.name()).collect();
                    invalid(name, &format!("one of: {}", valid.join(", ")))
                })?),
            })
        }
        "hit_testable" => Change::HitTestable(optional_bool(value, name)?),
        name if crate::state_layer_props::NAMES.contains(&name) => {
            Change::StateLayer(crate::state_layer_props::parse(name, value)?)
        }
        // 0.5.0 M3 (issue #28): `"drag"` makes the node a title bar -- a
        // primary press on it, or on a non-interactive node inside, moves
        // the window -- `"none"` rules that out, `None` leaves it to the
        // nodes around it.
        "window_region" => {
            let region: Option<String> = optional(value, name, "\"drag\", \"none\", or None")?;
            Change::WindowRegion(match region.as_deref() {
                None => WindowRegion::Default,
                Some("drag") => WindowRegion::Drag,
                Some("none") => WindowRegion::NoDrag,
                Some("drag_margin") => WindowRegion::DragMargin,
                Some(_) => {
                    return Err(invalid(
                        name,
                        "\"drag\", \"drag_margin\" (a LayerWindow), \"none\", or None",
                    ));
                }
            })
        }
        "data" => {
            let data: String = required(value, name, "SVG path data (a str)")?;
            if engine_core::PathTemplate::is_template(&data) {
                return Ok(Change::Template(
                    engine_core::PathTemplate::parse(&data).map_err(|err| {
                        PyValueError::new_err(format!(
                            "node property `data` isn't a valid path template: {err}"
                        ))
                    })?,
                ));
            }
            Change::Data(PathData::from_svg(&data).map_err(|err| {
                PyValueError::new_err(format!(
                    "node property `data` isn't valid SVG path data: {err}"
                ))
            })?)
        }
        "view_box" => Change::ViewBox(if value.is_none() {
            None
        } else {
            let expected = "a (min_x, min_y, width, height) tuple with a positive size, or None";
            let (x, y, w, h): (f64, f64, f64, f64) = required(value, name, expected)?;
            if w <= 0.0 || h <= 0.0 {
                return Err(invalid(name, expected));
            }
            Some(Rect::new(x, y, x + w, y + h))
        }),
        "params" => {
            let expected = "a dict of names to numbers";
            let dict: Bound<'_, pyo3::types::PyDict> = required(value, name, expected)?;
            let mut out = Vec::with_capacity(dict.len());
            for (key, number) in dict.iter() {
                let key: String = key
                    .extract()
                    .map_err(|_| invalid(name, "names must be str"))?;
                let number: f64 = number
                    .extract()
                    .ok()
                    .filter(|n: &f64| {
                        n.is_finite() && !number.is_instance_of::<pyo3::types::PyBool>()
                    })
                    .ok_or_else(|| invalid(name, &format!("`{key}` must be a finite number")))?;
                if key.is_empty()
                    || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    || key.starts_with(|c: char| c.is_ascii_digit())
                {
                    return Err(invalid(
                        name,
                        &format!("`{key}` isn't a name (letters, digits and underscores)"),
                    ));
                }
                out.push((key, number));
            }
            Change::Params(out)
        }
        "morph" => {
            let expected = "\"resample\", \"structure\" or \"strict\"";
            let word: String = required(value, name, expected)?;
            Change::Morph(
                engine_core::MorphMode::parse(&word).ok_or_else(|| invalid(name, expected))?,
            )
        }
        "hit_shape" => {
            let expected = "\"box\" or \"painted\"";
            let word: String = required(value, name, expected)?;
            Change::HitShape(
                engine_core::HitShape::parse(&word).ok_or_else(|| invalid(name, expected))?,
            )
        }
        "trim_start" => Change::TrimStart(fraction(value, name)?),
        "trim_end" => Change::TrimEnd(fraction(value, name)?),
        "fill" if value.is_none() => Change::FillUnset,
        "fill" => match value.extract::<crate::gradient::PyGradient>() {
            Ok(gradient) => Change::FillGradient(gradient.inner),
            Err(_) => Change::Fill(parse_color(value, name).map_err(|_| {
                invalid(
                    name,
                    "an (r, g, b, a) tuple of 0-255 ints, or a Gradient (None for the scheme's colour, on a text or text_input)",
                )
            })?),
        },
        "stroke_color" => match value.extract::<crate::gradient::PyGradient>() {
            Ok(gradient) => Change::StrokeGradient(gradient.inner),
            Err(_) => Change::StrokeColor(parse_color(value, name)?),
        },
        "stroke_width" => Change::StrokeWidth(parse_non_negative(value, name)?),
        "opacity" => Change::Opacity(fraction(value, name)?),
        "blur" => Change::Blur(parse_non_negative(value, name)?),
        "backdrop_blur" => Change::BackdropBlur(parse_non_negative(value, name)?),
        "blend_mode" => {
            let mode: String = required(value, name, "a blend mode name")?;
            Change::BlendMode(engine_core::Blend::from_name(&mode).ok_or_else(|| {
                let valid: Vec<&str> = engine_core::Blend::ALL.iter().map(|(n, _)| *n).collect();
                invalid(name, &format!("one of: {}", valid.join(", ")))
            })?)
        }
        "corner_radius" => Change::CornerRadius(parse_radius(value, name)?),
        "shadows" => Change::Shadows(parse_shadows(value, name)?),
        "placeholder" => Change::Placeholder(required(value, name, "a str")?),
        "placeholder_fill" => Change::PlaceholderFill(optional_color(value, name)?),
        "caret_color" => Change::CaretColor(optional_color(value, name)?),
        "max_length" => Change::MaxLength(if value.is_none() {
            None
        } else if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(invalid(name, "a non-negative int or None"));
        } else {
            Some(
                value
                    .extract::<usize>()
                    .map_err(|_| invalid(name, "a non-negative int or None"))?,
            )
        }),
        "read_only" => Change::ReadOnly(boolean(value, name)?),
        "input_mode" => {
            let mode: String = required(value, name, "a str")?;
            Change::InputMode(
                engine_core::InputMode::ALL
                    .iter()
                    .find(|(n, _)| *n == mode)
                    .map(|(_, m)| *m)
                    .ok_or_else(|| {
                        invalid(
                            name,
                            &format!("one of: {}", names(&engine_core::InputMode::ALL)),
                        )
                    })?,
            )
        }
        "caret_visible" => Change::CaretVisible(boolean(value, name)?),
        "caret_width" => Change::CaretWidth(parse_non_negative(value, name)? as f32),
        "caret_shape" => {
            let shape: String = required(value, name, "a str")?;
            Change::CaretShape(
                engine_core::CaretShape::ALL
                    .iter()
                    .find(|(n, _)| *n == shape)
                    .map(|(_, s)| *s)
                    .ok_or_else(|| {
                        invalid(
                            name,
                            &format!("one of: {}", names(&engine_core::CaretShape::ALL)),
                        )
                    })?,
            )
        }
        "caret_blink" => Change::CaretBlink(
            match optional::<f64>(value, name, "a number of milliseconds or None")? {
                None => 0,
                Some(ms)
                    if ms.is_finite()
                        && (0.0..=3_600_000.0).contains(&ms)
                        && !value.is_instance_of::<pyo3::types::PyBool>() =>
                {
                    ms as u32
                }
                Some(_) => {
                    return Err(invalid(
                        name,
                        "a number of milliseconds from 0 to an hour, or None",
                    ));
                }
            },
        ),
        "selection_fill" => Change::SelectionFill(optional_color(value, name)?),
        "selection_foreground" => Change::SelectionForeground(optional_color(value, name)?),
        "selection_radius" => Change::SelectionRadius(parse_non_negative(value, name)? as f32),
        "obscured" => Change::Obscured(boolean(value, name)?),
        "palette" => Change::Palette(Box::new(parse_palette(value, name)?)),
        _ => {
            return Err(PyValueError::new_err(format!(
                "unknown node property {name:?} -- settable: {}, {}, {}, draw, materialize, \
                 size_hint",
                LAYOUT_PROPS.join(", "),
                SETTABLE.join(", "),
                KIND_PROPS
                    .iter()
                    .copied()
                    .chain(crate::scrollbar_props::names())
                    .chain(crate::list_selection::NAMES)
                    .chain(crate::outline_props::NAMES)
                    .chain(crate::layer_policy::NAMES)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    })
}

/// The callback properties, each stored under its handler key.
const CALLBACKS: [(&str, HandlerKey, &str); 3] = [
    ("draw", HandlerKey::Draw, "canvas"),
    ("materialize", HandlerKey::Materialize, "virtual_list"),
    ("size_hint", HandlerKey::SizeHint, "virtual_list"),
];

fn parse_callback(
    name: &str,
    value: &Bound<'_, PyAny>,
    kind: &NodeKind,
) -> Option<PyResult<Change>> {
    let (_, key, kind_name) = CALLBACKS.iter().find(|(n, _, _)| *n == name)?;
    let applies = match key {
        HandlerKey::Draw => matches!(kind, NodeKind::Canvas(_)),
        _ => matches!(kind, NodeKind::VirtualList(_)),
    };
    Some(if !applies {
        Err(PyValueError::new_err(format!(
            "node property `{name}` applies only to a {kind_name} node"
        )))
    } else if !value.is_callable() {
        Err(invalid(name, "a callable"))
    } else {
        Ok(Change::Callback(*key, value.clone().unbind()))
    })
}

/// 0.5.1 (#66): `shader` takes a `tess_re.Shader` or `None`. Its input nodes
/// must live in the same window as the node it is set on.
fn parse_shader(value: &Bound<'_, PyAny>, tree: &Rc<RefCell<Tree>>) -> PyResult<Change> {
    if value.is_none() {
        return Ok(Change::Shader(None));
    }
    let shader = value
        .cast::<crate::shader::Shader>()
        .map_err(|_| invalid("shader", "a tess_re.Shader or None"))?
        .get();
    if !shader.inputs_in(value.py(), tree) {
        return Err(PyValueError::new_err(
            "node property `shader`: the shader's input nodes belong to a different Window",
        ));
    }
    Ok(Change::Shader(Some(shader.core.clone())))
}

/// 0.5.6 (#160): `describedby` / `controls`: a node, a list of nodes, or `None`
/// -- all of this window's.
fn parse_related(
    name: &str,
    value: &Bound<'_, PyAny>,
    tree: &Rc<RefCell<Tree>>,
) -> PyResult<Vec<NodeId>> {
    const EXPECTED: &str = "a Node, a list of Nodes, or None";
    if value.is_none() {
        return Ok(Vec::new());
    }
    let items: Vec<Bound<'_, PyAny>> = if let Ok(node) = value.cast::<Node>() {
        vec![node.clone().into_any()]
    } else if value.is_instance_of::<pyo3::types::PyList>()
        || value.is_instance_of::<pyo3::types::PyTuple>()
    {
        value.try_iter()?.collect::<PyResult<Vec<_>>>()?
    } else {
        return Err(invalid(name, EXPECTED));
    };
    let mut ids = Vec::new();
    for item in items {
        let node: PyRef<'_, Node> = item.extract().map_err(|_| invalid(name, EXPECTED))?;
        if !Rc::ptr_eq(&node.tree, tree) {
            return Err(PyValueError::new_err(format!(
                "node property `{name}`: the node belongs to a different Window"
            )));
        }
        if !ids.contains(&node.id) {
            ids.push(node.id);
        }
    }
    Ok(ids)
}

/// Every property `set` accepts, parsed and checked against `kind` --
/// nothing is applied until all of them pass.
/// 0.5.7 (#179): `anchor` (a node of this window, or `None`), `placement` (a
/// layer placement name) or `offset` (a pair of numbers).
fn parse_anchor(
    name: &str,
    value: &Bound<'_, PyAny>,
    tree: &Rc<RefCell<Tree>>,
) -> PyResult<Change> {
    Ok(Change::Anchor(match name {
        "anchor" => {
            if value.is_none() {
                AnchorEdit::Anchor(None)
            } else {
                let node: PyRef<'_, Node> = value
                    .extract()
                    .map_err(|_| invalid(name, "a node of this window, or None"))?;
                if !Rc::ptr_eq(&node.tree, tree) {
                    return Err(crate::error::EngineError::ForeignNode.into());
                }
                node.check_alive()?;
                AnchorEdit::Anchor(Some(node.id))
            }
        }
        "placement" => {
            let text: String = required(value, name, "a str")?;
            AnchorEdit::Placement(
                crate::node_layout::lookup(&crate::window_layers::PLACEMENT, &text)
                    .map_err(|expected| invalid(name, &expected))?,
            )
        }
        _ => {
            let expected = "a pair of numbers (dx, dy)";
            let pair: (f64, f64) = value
                .extract()
                .ok()
                .filter(|(x, y): &(f64, f64)| {
                    x.is_finite() && y.is_finite() && !value.is_instance_of::<pyo3::types::PyBool>()
                })
                .ok_or_else(|| invalid(name, expected))?;
            AnchorEdit::Offset(pair)
        }
    }))
}

/// 0.5.7 (#179): checks a `set` or `create`'s anchor changes before any is
/// applied: an anchor may not be the node itself or inside it (`id`, when the node
/// exists), and `placement` and `offset` need an anchor, from this call or before
/// (`anchored`).
pub(crate) fn check_anchor_changes(
    tree: &Tree,
    id: Option<NodeId>,
    mut anchored: bool,
    changes: &[Change],
) -> PyResult<()> {
    for change in changes {
        if let Change::WindowRegion(WindowRegion::Drag) = change
            && !tree.window_drag_allowed()
        {
            return Err(PyValueError::new_err(
                "node property `window_region`: \"drag\" moves a window, and the compositor \
                 places a LayerWindow, so there is none to drag",
            ));
        }
        if let Change::WindowRegion(WindowRegion::DragMargin) = change
            && !tree.margin_drag_allowed()
        {
            return Err(PyValueError::new_err(
                "node property `window_region`: \"drag_margin\" moves a layer surface by its \
                 margins, so only the nodes of a LayerWindow have it",
            ));
        }
        if matches!(change, Change::Layer(_)) && !id.is_some_and(|id| tree.is_layer(id)) {
            return Err(PyValueError::new_err(
                "node properties `layer_viewport_margin`, `layer_flip_threshold`, `layer_align`, \
                 `layer_flip` and `layer_shift` need a node shown with `show_layer`",
            ));
        }
        let Change::Anchor(edit) = change else {
            continue;
        };
        match edit {
            AnchorEdit::Anchor(Some(anchor)) => {
                if id.is_some_and(|id| tree.anchor_cycle(id, *anchor)) {
                    return Err(PyValueError::new_err(
                        "node property `anchor`: a node cannot be placed against itself \
                         or a node inside it",
                    ));
                }
                anchored = true;
            }
            AnchorEdit::Anchor(None) => anchored = false,
            AnchorEdit::Placement(_) | AnchorEdit::Offset(_) if !anchored => {
                return Err(PyValueError::new_err(
                    "node properties `placement` and `offset` need an `anchor`: set one \
                     in the same call or before",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// #272: a path template, as it will be once `changes` are applied, must have every parameter it
/// names and fill in to path data; checked before anything is changed.
pub(crate) fn check_template_changes(kind: &NodeKind, changes: &[Change]) -> PyResult<()> {
    let NodeKind::Path(state) = kind else {
        return Ok(());
    };
    let mut template = state.template.clone();
    let mut params: Vec<(String, f64)> = state
        .params
        .iter()
        .map(|(n, p)| (n.clone(), p.current))
        .collect();
    for change in changes {
        match change {
            Change::Data(_) => template = None,
            Change::Template(t) => template = Some(t.clone()),
            Change::Params(set) => {
                for (name, value) in set {
                    match params.iter_mut().find(|(n, _)| n == name) {
                        Some(slot) => slot.1 = *value,
                        None => params.push((name.clone(), *value)),
                    }
                }
            }
            _ => {}
        }
    }
    let Some(template) = template else {
        return Ok(());
    };
    if !changes
        .iter()
        .any(|c| matches!(c, Change::Template(_) | Change::Params(_)))
    {
        return Ok(());
    }
    if let Some(missing) = template
        .names()
        .into_iter()
        .find(|name| !params.iter().any(|(n, _)| n == name))
    {
        return Err(PyValueError::new_err(format!(
            "node property `data` uses the parameter `{missing}`, which `params` does not give"
        )));
    }
    template
        .evaluate(&|name| params.iter().find(|(n, _)| n == name).map(|(_, v)| *v))
        .map(|_| ())
        .map_err(|err| PyValueError::new_err(format!("node property `data`: {err}")))
}

pub(crate) fn parse_all(
    props: Option<&Bound<'_, PyDict>>,
    kind: &NodeKind,
    tree: &Rc<RefCell<Tree>>,
) -> PyResult<Vec<Change>> {
    let mut changes = Vec::new();
    if let Some(props) = props {
        for (name, value) in props.iter() {
            let name: String = name.extract()?;
            if let Some(kind_change) = parse_kind_prop(&name, &value, kind, props) {
                changes.push(Change::Kind(kind_change?));
                continue;
            }
            if let Some(callback) = parse_callback(&name, &value, kind) {
                changes.push(callback?);
                continue;
            }
            if name == "shader" {
                changes.push(parse_shader(&value, tree)?);
                continue;
            }
            if let Some(edit) = crate::layer_policy::parse(&name, &value) {
                changes.push(Change::Layer(edit?));
                continue;
            }
            if matches!(name.as_str(), "anchor" | "placement" | "offset") {
                changes.push(parse_anchor(&name, &value, tree)?);
                continue;
            }
            if name == "active_descendant" {
                // One node or `None`: the relation names the single current item.
                if !value.is_none() && value.cast::<Node>().is_err() {
                    return Err(invalid(&name, "a Node or None"));
                }
                let ids = parse_related(&name, &value, tree)?;
                changes.push(Change::ActiveDescendant(ids.first().copied()));
                continue;
            }
            if name == "describedby" || name == "controls" {
                let ids = parse_related(&name, &value, tree)?;
                changes.push(if name == "controls" {
                    Change::Controls(ids)
                } else {
                    Change::DescribedBy(ids)
                });
                continue;
            }
            let change = parse(&name, &value)?;
            if let Some((prop, kind_name, applies)) = change.kind_requirement()
                && !applies(kind)
            {
                return Err(PyValueError::new_err(format!(
                    "node property `{prop}` applies only to a {kind_name} node"
                )));
            }
            changes.push(change);
        }
    }
    changes.sort_by_key(|change| matches!(change, Change::Kind(k) if k.late));
    Ok(changes)
}

/// 0.5.10 (#253): a `set` call checked and parsed, not yet applied. `Node.set` is `prepare_set`
/// then `run_set`; `Window.apply` prepares every entry first, so one bad entry changes nothing.
pub(crate) struct PreparedSet {
    changes: Vec<Change>,
    stopped: Vec<engine_core::LayoutProp>,
    scroll_offset: bool,
}

impl Node {
    /// Parses and checks `props` for this node, with no change to anything.
    pub(crate) fn prepare_set(&self, props: Option<&Bound<'_, PyDict>>) -> PyResult<PreparedSet> {
        let changes = {
            let tree = self.tree.borrow();
            let node = tree
                .get(self.id)
                .ok_or(crate::error::EngineError::Destroyed)?;
            let mut changes = parse_all(props, &node.kind, &self.tree)?;
            // 0.6.13: which nodes are a container's items is the tree's to say.
            if let Some(selection) =
                crate::container_selection::check(&tree, &self.tree, self.id, props)?
            {
                changes.push(Change::Kind(selection));
            }
            check_anchor_changes(&tree, Some(self.id), node.anchored.is_some(), &changes)?;
            check_template_changes(&node.kind, &changes)?;
            for change in &changes {
                if let Change::Shader(Some(shader)) = change
                    && tree.shader_cycle(self.id, shader)
                {
                    return Err(PyValueError::new_err(
                        "node property `shader`: the shader would read itself -- through its \
                         inputs it depends on this node's own shader",
                    ));
                }
            }
            changes
        };
        // 0.5.6 (#161): setting a layout property outright ends its animation.
        let stopped = props
            .map(|props| {
                props
                    .keys()
                    .iter()
                    .filter_map(|name| name.extract::<String>().ok())
                    .flat_map(|name| engine_core::LayoutProp::from_name(&name))
                    .collect()
            })
            .unwrap_or_default();
        let scroll_offset = props.is_some_and(|p| {
            p.contains("scroll_offset").unwrap_or(false)
                || p.contains("scroll_offset_x").unwrap_or(false)
        });
        Ok(PreparedSet {
            changes,
            stopped,
            scroll_offset,
        })
    }

    /// Applies what `prepare_set` parsed.
    pub(crate) fn run_set(&self, prepared: PreparedSet, py: Python<'_>) -> PyResult<()> {
        let PreparedSet {
            changes,
            stopped,
            scroll_offset,
        } = prepared;
        let redraw = changes
            .iter()
            .any(|change| matches!(change, Change::Callback(HandlerKey::Draw, _)));
        if !stopped.is_empty() {
            self.tree.borrow_mut().stop_layout_anims(self.id, &stopped);
        }
        self.apply(changes);
        if redraw {
            crate::node_callbacks::redraw(&self.tree, &self.handlers, self.id, py)?;
        }
        // 0.4.3 M15: a new offset is clamped to the view's range before it's
        // reported -- layout does the clamping -- so it reads back right at
        // once and fires one `scroll`, not a second when the frame lays out.
        if scroll_offset {
            self.layout_box(py);
        }
        self.fire_scroll_changes(py);
        self.fire_selection_changes(py);
        Ok(())
    }
}

#[pymethods]
impl Node {
    /// Sets any number of properties at once, atomically: every value is
    /// checked first, and a bad one raises `ValueError` without changing
    /// anything. An optional property takes `None` to clear it.
    #[pyo3(signature = (**props))]
    fn set(&self, props: Option<&Bound<'_, PyDict>>, py: Python<'_>) -> PyResult<()> {
        let prepared = self.prepare_set(props)?;
        self.run_set(prepared, py)
    }

    /// Reads one property: any `set` property, `focused`, or -- for the
    /// animatable numeric properties -- its current, possibly
    /// mid-animation value. `value` is the accessibility value.
    fn get(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = {
            let tree = self.tree.borrow();
            let node = tree.get(self.id).ok_or_else(|| {
                PyValueError::new_err("this node has been removed from its window")
            })?;
            if let Some(value) =
                animatable_to_py(node, tree.scheme_colors().text(), name, false, py)?
            {
                return Ok(value);
            }
            match name {
                // 0.6.13: a container's chosen children, and its anchor, are nodes.
                "selection" | "selection_anchor" if Tree::holds_items(&node.kind) => {
                    return crate::container_selection::read(self, &tree, node, name, py);
                }
                "anchor" => {
                    return match node.anchored.as_deref() {
                        Some(a) => Ok(Py::new(py, self.handle_to(a.anchor))?.into_any()),
                        None => Ok(py.None()),
                    };
                }
                "placement" => {
                    return Ok(match node.anchored.as_deref() {
                        Some(a) => crate::window_layers::PLACEMENT
                            .iter()
                            .find(|(_, p)| *p == a.placement)
                            .map(|(n, _)| *n)
                            .into_pyobject(py)?
                            .into_any()
                            .unbind(),
                        None => py.None(),
                    });
                }
                "offset" => {
                    return Ok(match node.anchored.as_deref() {
                        Some(a) => a.offset.into_pyobject(py)?.into_any().unbind(),
                        None => py.None(),
                    });
                }
                _ => {}
            }
            let (fit, calc) = (node.fit_content, node.calc_size);
            if let Some(len) = match name {
                "width" => calc.width,
                "height" => calc.height,
                _ => None,
            } {
                let sign = if len.px < 0.0 { '-' } else { '+' };
                let text = format!("calc({}% {sign} {}px)", len.percent * 100.0, len.px.abs());
                return Ok(pyo3::types::PyString::new(py, &text).into_any().unbind());
            }
            if (name == "width" && fit.width) || (name == "height" && fit.height) {
                return Ok(pyo3::types::PyString::new(py, "fit-content")
                    .into_any()
                    .unbind());
            }
            if let Some(value) = read_layout(name, &node.layout_style, py) {
                return value;
            }
            if let Some(value) = read_kind_prop(name, node, py) {
                return value;
            }
            if crate::state_layer_props::NAMES.contains(&name) {
                return crate::state_layer_props::read(name, node.paint.state_layer.as_deref(), py);
            }
            if let Some(value) = crate::outline_props::read(name, node, py) {
                return value;
            }
            if let Some(value) = crate::layer_policy::read(
                name,
                tree.overlay_meta(self.id),
                tree.layer_defaults(),
                py,
            ) {
                return value;
            }
            if let Some((_, key, _)) = CALLBACKS.iter().find(|(n, _, _)| *n == name) {
                return Ok(
                    crate::node_callbacks::callback(&self.handlers, self.id, *key, py)
                        .unwrap_or_else(|| py.None()),
                );
            }
            let access = &node.access;
            let any = |v: Bound<'_, PyAny>| v.unbind();
            match name {
                "kind" => any(crate::node::kind_id(&node.kind)
                    .into_pyobject(py)?
                    .into_any()),
                "role" => ROLES
                    .iter()
                    .find(|(_, r)| *r == access.role)
                    .map(|(n, _)| *n)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "label" => access.label.clone().into_pyobject(py)?.into_any().unbind(),
                "value" => match &access.value {
                    Some(AccessValue::Text(text)) => {
                        any(text.clone().into_pyobject(py)?.into_any())
                    }
                    Some(AccessValue::Number(n)) => any(n.into_pyobject(py)?.into_any()),
                    None => py.None(),
                },
                "value_min" => access.value_min.into_pyobject(py)?.into_any().unbind(),
                "value_max" => access.value_max.into_pyobject(py)?.into_any().unbind(),
                "value_step" => access.value_step.into_pyobject(py)?.into_any().unbind(),
                "checked" => toggled_object(py, access.checked)?,
                "selected" => access.selected.into_pyobject(py)?.into_any().unbind(),
                "expanded" => access.expanded.into_pyobject(py)?.into_any().unbind(),
                "disabled" => any(access
                    .states
                    .disabled
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()),
                "level" => access.level.into_pyobject(py)?.into_any().unbind(),
                "live" => LIVE
                    .iter()
                    .find(|(_, l)| Some(*l) == access.live)
                    .map(|(n, _)| *n)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "a11y_hidden" => any(access.hidden.into_pyobject(py)?.to_owned().into_any()),
                "pressed" => toggled_object(py, access.pressed)?,
                "set_size" => access.set_size.into_pyobject(py)?.into_any().unbind(),
                "pos_in_set" => access.pos_in_set.into_pyobject(py)?.into_any().unbind(),
                "a11y_orientation" => ORIENTATIONS
                    .iter()
                    .find(|(_, o)| Some(*o) == access.orientation)
                    .map(|(n, _)| *n)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "modal" => any(access.modal.into_pyobject(py)?.to_owned().into_any()),
                "invalid" => any(access.invalid.into_pyobject(py)?.to_owned().into_any()),
                "busy" => any(access.busy.into_pyobject(py)?.to_owned().into_any()),
                "current" => match access.current {
                    None => py.None(),
                    Some(engine_core::AriaCurrent::True) => {
                        any(true.into_pyobject(py)?.to_owned().into_any())
                    }
                    Some(engine_core::AriaCurrent::False) => {
                        any(false.into_pyobject(py)?.to_owned().into_any())
                    }
                    Some(current) => CURRENT
                        .iter()
                        .find(|(_, c)| *c == current)
                        .map(|(n, _)| *n)
                        .into_pyobject(py)?
                        .into_any()
                        .unbind(),
                },
                "description" => access
                    .description
                    .clone()
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "value_now" => match &access.value {
                    Some(AccessValue::Number(n)) => any(n.into_pyobject(py)?.into_any()),
                    _ => py.None(),
                },
                "value_text" => access
                    .extra
                    .as_ref()
                    .and_then(|x| x.value_text.clone())
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "active_descendant" => {
                    // A destroyed target reads as `None`.
                    let tree = self.tree.borrow();
                    let target = access
                        .extra
                        .as_ref()
                        .and_then(|x| x.active_descendant)
                        .filter(|id| tree.get(*id).is_some());
                    drop(tree);
                    any(target
                        .map(|id| self.handle_to(id))
                        .into_pyobject(py)?
                        .into_any())
                }
                "describedby" | "controls" => {
                    let empty = Vec::new();
                    let ids = access.extra.as_ref().map_or(&empty, |x| {
                        if name == "controls" {
                            &x.controls
                        } else {
                            &x.described_by
                        }
                    });
                    let tree = self.tree.borrow();
                    let nodes: Vec<Node> = ids
                        .iter()
                        .filter(|id| tree.get(**id).is_some())
                        .map(|id| self.handle_to(*id))
                        .collect();
                    drop(tree);
                    any(nodes.into_pyobject(py)?.into_any())
                }
                "focusable" => {
                    let focusable = access.focusable.unwrap_or(!access.actions.is_empty());
                    any(focusable.into_pyobject(py)?.to_owned().into_any())
                }
                "tab_index" => any(access.tab_index.into_pyobject(py)?.into_any()),
                "cursor" => match node.cursor {
                    Some(Cursor::Custom(id)) => {
                        Py::new(py, crate::cursor_image::PyCursorImage::from_id(id))?.into_any()
                    }
                    other => other
                        .map(Cursor::name)
                        .into_pyobject(py)?
                        .into_any()
                        .unbind(),
                },
                "shader" => match &node.shader {
                    None => py.None(),
                    Some(core) => Py::new(
                        py,
                        crate::shader::Shader::wrap(py, core.clone(), |id| self.handle_to(id)),
                    )?
                    .into_any(),
                },
                "hit_testable" => match node.hit_testable {
                    None => py.None(),
                    Some(on) => any(on.into_pyobject(py)?.to_owned().into_any()),
                },
                "window_region" => match node.window_region {
                    WindowRegion::Default => py.None(),
                    WindowRegion::Drag => any("drag".into_pyobject(py)?.into_any()),
                    WindowRegion::NoDrag => any("none".into_pyobject(py)?.into_any()),
                    WindowRegion::DragMargin => any("drag_margin".into_pyobject(py)?.into_any()),
                },
                "blend_mode" => any(node.paint.blend.name().into_pyobject(py)?.into_any()),
                "visible" => any(node.visible.into_pyobject(py)?.to_owned().into_any()),
                "item_selectable" => any(node
                    .item_selectable
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()),
                "z_index" => any(node.z_index.into_pyobject(py)?.into_any()),
                "snap_align" => node
                    .snap_align
                    .map(|a| {
                        engine_core::SnapAlign::ALL
                            .iter()
                            .find(|(_, x)| *x == a)
                            .map_or("start", |(n, _)| *n)
                    })
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
                "mask" => mask_to_py(node.paint.mask.as_deref(), py)?,
                "clip_children" => any(node
                    .paint
                    .clip_children
                    .into_pyobject(py)?
                    .to_owned()
                    .into_any()),
                "sticky" => node.sticky.into_pyobject(py)?.into_any().into(),
                "layout_x" | "layout_y" | "layout_width" | "layout_height" => {
                    drop(tree);
                    let (x, y, w, h) = self.layout_box(py);
                    let value = match name {
                        "layout_x" => x,
                        "layout_y" => y,
                        "layout_width" => w,
                        _ => h,
                    };
                    return Ok(any(value.into_pyobject(py)?.into_any()));
                }
                "selection_fill" | "selection_foreground" | "selection_radius" => {
                    let (fill, foreground, radius) = match &node.kind {
                        NodeKind::Text(state) => (
                            state.options.selection_fill,
                            state.options.selection_foreground,
                            state.options.selection_radius,
                        ),
                        NodeKind::TextField(state) => (
                            state.selection_fill,
                            state.selection_foreground,
                            state.selection_radius,
                        ),
                        // 0.6.12 (P8): a terminal's highlight colour is `palette["selection"]`.
                        NodeKind::Terminal(state) if name != "selection_fill" => {
                            (None, state.selection_foreground, state.selection_radius)
                        }
                        _ => {
                            let kinds = if name == "selection_fill" {
                                "a text or text_input"
                            } else {
                                "a text, text_input or terminal"
                            };
                            return Err(PyValueError::new_err(format!(
                                "node property `{name}` applies only to {kinds} node"
                            )));
                        }
                    };
                    let color = |c: Option<Color>| -> PyResult<Py<PyAny>> {
                        c.map_or_else(|| Ok(py.None()), |c| color_to_py(c, py))
                    };
                    match name {
                        "selection_fill" => color(fill)?,
                        "selection_foreground" => color(foreground)?,
                        _ => any(f64::from(radius).into_pyobject(py)?.into_any()),
                    }
                }
                "placeholder" | "placeholder_fill" | "caret_color" | "obscured" | "max_length"
                | "read_only" | "input_mode" | "caret_visible" | "caret_width" | "caret_shape"
                | "caret_blink" => {
                    // 0.6.12 (P8): a terminal's cursor.
                    if let NodeKind::Terminal(state) = &node.kind
                        && matches!(name, "caret_width" | "caret_shape" | "caret_blink")
                    {
                        match name {
                            "caret_width" => {
                                return Ok(any(f64::from(state.caret_width)
                                    .into_pyobject(py)?
                                    .into_any()));
                            }
                            "caret_shape" => {
                                return Ok(any(engine_core::CaretShape::ALL
                                    .iter()
                                    .find(|(_, s)| *s == state.caret_shape)
                                    .map(|(n, _)| *n)
                                    .into_pyobject(py)?
                                    .into_any()));
                            }
                            _ => {
                                return Ok(any((state.caret_blink_ms > 0)
                                    .then_some(f64::from(state.caret_blink_ms))
                                    .into_pyobject(py)?
                                    .into_any()));
                            }
                        }
                    }
                    let NodeKind::TextField(state) = &node.kind else {
                        return Err(PyValueError::new_err(format!(
                            "node property `{name}` applies only to a text_input node"
                        )));
                    };
                    let color = |c: Option<Color>| -> PyResult<Py<PyAny>> {
                        c.map_or_else(|| Ok(py.None()), |c| color_to_py(c, py))
                    };
                    match name {
                        "placeholder" => {
                            any(state.placeholder.clone().into_pyobject(py)?.into_any())
                        }
                        "placeholder_fill" => color(state.placeholder_fill)?,
                        "caret_color" => color(state.caret_color.as_ref().map(|c| c.current))?,
                        "max_length" => any(state.max_length.into_pyobject(py)?.into_any()),
                        "read_only" => {
                            any(state.read_only.into_pyobject(py)?.to_owned().into_any())
                        }
                        "input_mode" => any(engine_core::InputMode::ALL
                            .iter()
                            .find(|(_, m)| *m == state.input_mode)
                            .map(|(n, _)| *n)
                            .into_pyobject(py)?
                            .into_any()),
                        "caret_visible" => {
                            any(state.caret_visible.into_pyobject(py)?.to_owned().into_any())
                        }
                        "caret_width" => {
                            any(f64::from(state.caret_width).into_pyobject(py)?.into_any())
                        }
                        "caret_shape" => any(engine_core::CaretShape::ALL
                            .iter()
                            .find(|(_, s)| *s == state.caret_shape)
                            .map(|(n, _)| *n)
                            .into_pyobject(py)?
                            .into_any()),
                        "caret_blink" => any((state.caret_blink_ms > 0)
                            .then_some(f64::from(state.caret_blink_ms))
                            .into_pyobject(py)?
                            .into_any()),
                        _ => any(state.obscured.into_pyobject(py)?.to_owned().into_any()),
                    }
                }
                "palette" => {
                    let NodeKind::Terminal(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `palette` applies only to a terminal node",
                        ));
                    };
                    palette_to_py(&state.palette, py)?
                }
                "params" => {
                    let NodeKind::Path(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `params` applies only to a path node",
                        ));
                    };
                    let dict = pyo3::types::PyDict::new(py);
                    for (name, param) in &state.params {
                        dict.set_item(name, param.current)?;
                    }
                    dict.into_any().unbind()
                }
                "morph" => {
                    let NodeKind::Path(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `morph` applies only to a path node",
                        ));
                    };
                    state.morph.name().into_pyobject(py)?.into_any().unbind()
                }
                "hit_shape" => {
                    let NodeKind::Path(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `hit_shape` applies only to a path node",
                        ));
                    };
                    state
                        .hit_shape
                        .name()
                        .into_pyobject(py)?
                        .into_any()
                        .unbind()
                }
                "view_box" => {
                    let NodeKind::Path(state) = &node.kind else {
                        return Err(PyValueError::new_err(
                            "node property `view_box` applies only to a path node",
                        ));
                    };
                    state
                        .view_box
                        .map(|r| (r.x0, r.y0, r.width(), r.height()))
                        .into_pyobject(py)?
                        .into_any()
                        .unbind()
                }
                "layer_placement" => {
                    // Placed at layout: run any pending layout first.
                    drop(tree);
                    self.layout_box(py);
                    let tree = self.tree.borrow();
                    return Ok(tree
                        .overlay_meta(self.id)
                        .and_then(|meta| meta.placed)
                        .and_then(|side| {
                            crate::window_layers::PLACEMENT
                                .iter()
                                .find(|(_, s)| *s == side)
                                .map(|(n, _)| *n)
                        })
                        .into_pyobject(py)?
                        .into_any()
                        .unbind());
                }
                "focused" => {
                    let focused = tree.focused() == Some(self.id);
                    any(focused.into_pyobject(py)?.to_owned().into_any())
                }
                _ => {
                    return Err(PyValueError::new_err(format!(
                        "unknown node property {name:?} -- `get` reads any property `set` \
                         takes, plus kind, focused, layer_placement, layout_x, layout_y, \
                         layout_width, and layout_height"
                    )));
                }
            }
        };
        Ok(value)
    }

    /// 0.5.6 (#165): the caret of a text input as `(x, y, width, height)` in
    /// the node's own coordinates, with scrolling and padding in it. While an
    /// IME is composing, it is at the end of the composition. Shaped fresh, so
    /// it is right straight after `set(text=...)` or an edit.
    fn caret_rect(&self, py: Python<'_>) -> PyResult<(f64, f64, f64, f64)> {
        self.require_text_input("caret_rect")?;
        self.layout_box(py);
        crate::shaper::with(|shaper| crate::text_interaction::follow_fields(&self.tree, shaper));
        let rect = crate::text_interaction::field_caret_rect(&self.tree, self.id)
            .ok_or(crate::error::EngineError::Destroyed)?;
        Ok((rect.x0, rect.y0, rect.width(), rect.height()))
    }

    /// 0.5.6 (#165): the rectangles covering bytes `start..end` of a text
    /// input's text, one per line it crosses, as `(x, y, width, height)` in the
    /// node's coordinates. Offsets are UTF-8 byte offsets on character
    /// boundaries, as `selection` takes.
    fn text_rects(
        &self,
        start: usize,
        end: usize,
        py: Python<'_>,
    ) -> PyResult<Vec<(f64, f64, f64, f64)>> {
        self.require_text_input("text_rects")?;
        {
            let tree = self.tree.borrow();
            if let Some(NodeKind::TextField(state)) = tree.get(self.id).map(|n| &n.kind) {
                let ok = |i: usize| state.content.is_char_boundary(i);
                if start > end || end > state.content.len() || !ok(start) || !ok(end) {
                    return Err(PyValueError::new_err(format!(
                        "text_rects({start}, {end}): the offsets must be UTF-8 byte offsets \
                         on character boundaries within the text (0 to {}), start first",
                        state.content.len()
                    )));
                }
            }
        }
        self.layout_box(py);
        crate::shaper::with(|shaper| crate::text_interaction::follow_fields(&self.tree, shaper));
        let rects = crate::text_interaction::field_range_rects(&self.tree, self.id, start, end)
            .ok_or(crate::error::EngineError::Destroyed)?;
        Ok(rects
            .into_iter()
            .map(|r| (r.x0, r.y0, r.width(), r.height()))
            .collect())
    }

    /// M95: the value `name`'s running animation is heading to -- equal to
    /// `get(name)` when nothing is animating it. Animatable properties
    /// only.
    fn get_target(&self, name: &str, py: Python<'_>) -> PyResult<Py<PyAny>> {
        // 0.5.6 (#161): a layout property's animation target, or its value.
        let layout = engine_core::LayoutProp::from_name(name);
        if layout.len() == 1 {
            let target = self.tree.borrow().layout_target(self.id, layout[0]);
            return match target {
                Some(pixels) => Ok(pixels.into_pyobject(py)?.into_any().unbind()),
                None => self.get(name, py),
            };
        }
        let tree = self.tree.borrow();
        let node = tree
            .get(self.id)
            .ok_or_else(|| PyValueError::new_err("this node has been removed from its window"))?;
        animatable_to_py(node, tree.scheme_colors().text(), name, true, py)?.ok_or_else(|| {
            PyValueError::new_err(format!("node property {name:?} isn't animatable"))
        })
    }

    /// M95: stops `name`'s running animation where it is; its
    /// `on_complete` never fires. A no-op when nothing is animating it.
    fn stop_animation(&self, name: &str) -> PyResult<()> {
        let layout = engine_core::LayoutProp::from_name(name);
        if !layout.is_empty() {
            self.tree.borrow_mut().stop_layout_anims(self.id, &layout);
            return Ok(());
        }
        let mut tree = self.tree.borrow_mut();
        let node = tree
            .get_mut(self.id)
            .ok_or_else(|| PyValueError::new_err("this node has been removed from its window"))?;
        if stop_animatable(node, name)? {
            Ok(())
        } else {
            Err(PyValueError::new_err(format!(
                "node property {name:?} isn't animatable"
            )))
        }
    }

    /// Moves keyboard focus to this node, firing `unfocus` and `focus` as
    /// any focus change does.
    fn focus(&self, py: Python<'_>) {
        // 0.4.2 M12: focusing scrolls the node into view, from a current
        // layout.
        self.layout_box(py);
        let transition = {
            let mut tree = self.tree.borrow_mut();
            tree.set_focus_visible(crate::listeners::keyboard_modality());
            tree.set_focus_to(self.id)
        };
        if let Some((old, new)) = transition {
            fire_focus_transition(&self.handlers, &self.tree, &self.completions, old, new, py);
        }
        self.fire_scroll_changes(py);
    }

    /// 0.4.2 M12 (issue #24): scrolls every scroll view around this node
    /// just enough to show it, innermost first.
    fn scroll_into_view(&self, py: Python<'_>) {
        self.layout_box(py);
        self.tree.borrow_mut().scroll_into_view(self.id);
        self.fire_scroll_changes(py);
    }

    /// 0.6.11: scrolls this virtual list so row `index` shows as `align`
    /// says -- `"nearest"` (the least scroll that shows it, none if it is
    /// already shown), `"start"`, `"center"` or `"end"` -- at once, whether
    /// or not the row is built. Offsets come from the list's `item_extent`
    /// or the `size_hint` of every row, so they are exact. Returns whether
    /// the offset changed; raises `ValueError` for another kind of node, an
    /// unknown `align`, or an `index` outside `0..item_count`.
    #[pyo3(signature = (index, align = "nearest"))]
    fn scroll_to_index(&self, py: Python<'_>, index: i64, align: &str) -> PyResult<bool> {
        let align = engine_core::ScrollAlign::from_name(align).ok_or_else(|| {
            PyValueError::new_err(format!(
                "`align` must be one of \"nearest\", \"start\", \"center\", \"end\", got {align:?}"
            ))
        })?;
        let count = match self.tree.borrow().get(self.id).map(|n| &n.kind) {
            Some(NodeKind::VirtualList(state)) => state.item_count,
            Some(_) => {
                return Err(PyValueError::new_err(
                    "scroll_to_index applies only to a virtual_list node",
                ));
            }
            None => return Err(crate::error::EngineError::Destroyed.into()),
        };
        // The viewport height and a `size_hint` list's offsets come from layout.
        self.layout_box(py);
        let count_now = match self.tree.borrow().get(self.id).map(|n| &n.kind) {
            Some(NodeKind::VirtualList(state)) => state.item_count,
            _ => count,
        };
        let index = usize::try_from(index)
            .ok()
            .filter(|&i| i < count_now)
            .ok_or_else(|| {
                PyValueError::new_err(format!(
                    "index {index} is outside the list's {count_now} rows"
                ))
            })?;
        let moved = self
            .tree
            .borrow_mut()
            .scroll_virtual_list_to_index(self.id, index, align)
            .unwrap_or(false);
        self.fire_scroll_changes(py);
        Ok(moved)
    }
}

impl Node {
    /// 0.4.2 M12: `scroll` events for any scroll view this call moved.
    fn fire_scroll_changes(&self, py: Python<'_>) {
        crate::listeners::fire_scroll_changes(
            &crate::event::NodeContext {
                tree: &self.tree,
                handlers: &self.handlers,
                completions: &self.completions,
            },
            py,
        );
    }

    /// 0.6.12: `selection_change` for any list whose rows this call chose or released.
    fn fire_selection_changes(&self, py: Python<'_>) {
        crate::list_selection::fire_changes(
            &crate::event::NodeContext {
                tree: &self.tree,
                handlers: &self.handlers,
                completions: &self.completions,
            },
            py,
        );
    }

    /// M96: this node's computed box in window space -- `(x, y, width,
    /// height)` -- running any pending layout of its tree first, so it
    /// always matches the current tree. A detached subtree is laid out on
    /// its own, at its content size.
    pub(crate) fn layout_box(&self, py: Python<'_>) -> (f64, f64, f64, f64) {
        let available = |dim: Dimension| match ExpandedDimension::from(dim) {
            ExpandedDimension::Length(v) => AvailableSpace::Definite(v),
            _ => AvailableSpace::MaxContent,
        };
        let (root, size) = {
            let tree = self.tree.borrow();
            let root = tree.root_of(self.id);
            (root, tree.get(root).map(|n| n.layout_style.size))
        };
        if let Some(size) = size {
            let size = taffy::prelude::Size {
                width: available(size.width),
                height: available(size.height),
            };
            crate::node_callbacks::layout(
                &self.tree,
                root,
                size,
                &self.handlers,
                &self.completions,
                py,
            );
        }
        let tree = self.tree.borrow();
        let (x, y) = tree.absolute_position(self.id);
        let layout = tree.layout(self.id);
        (
            x,
            y,
            f64::from(layout.size.width),
            f64::from(layout.size.height),
        )
    }

    /// Applies already-checked changes (`parse_all`) -- the second half of
    /// an atomic `set`, shared with `Window.create`.
    pub(crate) fn apply(&self, changes: Vec<Change>) {
        let mut tree = self.tree.borrow_mut();
        let Some(node) = tree.get_mut(self.id) else {
            return;
        };
        let mut style = None;
        let mut fit = node.fit_content;
        let mut calc = node.calc_size;
        let mut anchored = node.anchored.as_deref().copied();
        let mut anchor_changed = false;
        let mut layer_edits = Vec::new();
        let mut kind_changed = false;
        let mut state_layer_changed = false;
        let mut resize_terminal = false;
        let mut reset_rows = false;
        let mut callbacks = Vec::new();
        for change in changes {
            let access = &mut node.access;
            match change {
                Change::Role(role) => access.role = role,
                Change::Label(label) => access.label = label,
                Change::Value(value) => access.value = value,
                Change::ValueMin(min) => access.value_min = min,
                Change::ValueMax(max) => access.value_max = max,
                Change::ValueStep(step) => access.value_step = step,
                Change::Checked(checked) => access.checked = checked,
                Change::Selected(selected) => access.selected = selected,
                Change::Expanded(expanded) => access.expanded = expanded,
                Change::Disabled(disabled) => {
                    access.states.disabled = disabled;
                    state_layer_changed = true;
                }
                Change::Level(level) => access.level = level,
                Change::Live(live) => access.live = live,
                Change::A11yHidden(hidden) => access.hidden = hidden,
                Change::Pressed(pressed) => access.pressed = pressed,
                Change::SetSize(size) => access.set_size = size,
                Change::PosInSet(pos) => access.pos_in_set = pos,
                Change::Orientation(o) => access.orientation = o,
                Change::Modal(modal) => access.modal = modal,
                Change::Invalid(invalid) => access.invalid = invalid,
                Change::Busy(busy) => access.busy = busy,
                Change::Current(current) => access.current = current,
                Change::Description(description) => access.description = description,
                Change::ValueNow(Some(now)) => access.value = Some(AccessValue::Number(now)),
                Change::ValueNow(None) => {
                    if matches!(access.value, Some(AccessValue::Number(_))) {
                        access.value = None;
                    }
                }
                Change::ValueText(text) => access.edit_extra(|x| x.value_text = text),
                Change::DescribedBy(ids) => access.edit_extra(|x| x.described_by = ids),
                Change::Controls(ids) => access.edit_extra(|x| x.controls = ids),
                Change::ActiveDescendant(id) => access.edit_extra(|x| x.active_descendant = id),
                Change::Focusable(focusable) => access.focusable = Some(focusable),
                Change::TabIndex(index) => access.tab_index = index,
                Change::Cursor(cursor) => node.cursor = cursor,
                Change::WindowRegion(region) => node.window_region = region,
                Change::HitTestable(hit_testable) => node.hit_testable = hit_testable,
                Change::StateLayer(edit) => {
                    edit(node.paint.state_layer.get_or_insert_with(Default::default));
                    state_layer_changed = true;
                }
                Change::Shader(shader) => node.shader = shader,
                Change::Style(edit) => edit(style.get_or_insert_with(|| node.layout_style.clone())),
                Change::Size { height, mode, edit } => {
                    let style = style.get_or_insert_with(|| node.layout_style.clone());
                    edit(style);
                    // #263: a text node's `"auto"` is the same as `"fit-content"` (and as
                    // unset): its text's size. Any other plain value, a percentage or a
                    // calc takes the side off fit.
                    let auto = if height {
                        style.size.height.is_auto()
                    } else {
                        style.size.width.is_auto()
                    };
                    let on = matches!(mode, SizeMode::Fit)
                        || (matches!(mode, SizeMode::Plain)
                            && auto
                            && matches!(node.kind, NodeKind::Text(_)));
                    let mixed = match mode {
                        SizeMode::Calc(len) => Some(len),
                        _ => None,
                    };
                    if height {
                        fit.height = on;
                        calc.height = mixed;
                    } else {
                        fit.width = on;
                        calc.width = mixed;
                    }
                }
                Change::Layer(edit) => layer_edits.push(edit),
                Change::Anchor(edit) => {
                    anchor_changed = true;
                    match edit {
                        AnchorEdit::Anchor(None) => anchored = None,
                        AnchorEdit::Anchor(Some(anchor)) => {
                            let before = anchored;
                            anchored = Some(Anchored {
                                anchor,
                                placement: before.map_or(Placement::default(), |a| a.placement),
                                offset: before.map_or((0.0, 0.0), |a| a.offset),
                            });
                        }
                        AnchorEdit::Placement(placement) => {
                            if let Some(a) = anchored.as_mut() {
                                a.placement = placement;
                            }
                        }
                        AnchorEdit::Offset(offset) => {
                            if let Some(a) = anchored.as_mut() {
                                a.offset = offset;
                            }
                        }
                    }
                }
                Change::Kind(kind_change) => {
                    kind_changed = true;
                    resize_terminal |= kind_change.resizes_terminal;
                    reset_rows |= kind_change.resets_rows;
                    let old_ratio = match &node.kind {
                        NodeKind::Svg(state) => Some(state.document.width / state.document.height),
                        _ => None,
                    };
                    (kind_change.edit)(node);
                    // A document's shape sizes the node's other side, unless
                    // the app chose an aspect ratio of its own.
                    if let (Some(ratio), Some(old)) = (kind_change.svg_ratio, old_ratio) {
                        let current = style
                            .as_ref()
                            .map_or(node.layout_style.aspect_ratio, |s| s.aspect_ratio);
                        if current.is_none_or(|c| (f64::from(c) - old).abs() < 1e-4) {
                            style
                                .get_or_insert_with(|| node.layout_style.clone())
                                .aspect_ratio = Some(ratio as f32);
                        }
                    }
                }
                Change::Callback(key, callback) => {
                    if key == HandlerKey::SizeHint
                        && let NodeKind::VirtualList(state) = &mut node.kind
                    {
                        state.item_extent = engine_core::ItemExtent::Variable;
                        state.resolved_offsets.clear();
                        reset_rows = true;
                    }
                    reset_rows |= key == HandlerKey::Materialize;
                    callbacks.push((key, callback));
                }
                Change::Visible(visible) => {
                    node.visible = visible;
                    style
                        .get_or_insert_with(|| node.layout_style.clone())
                        .display = if visible {
                        taffy::Display::DEFAULT
                    } else {
                        taffy::Display::None
                    };
                }
                Change::ItemSelectable(on) => node.item_selectable = on,
                Change::ZIndex(z) => node.z_index = z,
                Change::ClipChildren(clip) => node.paint.clip_children = clip,
                Change::Outline(change) => change.apply(&mut node.paint.outline),
                Change::SnapAlign(align) => node.snap_align = align,
                Change::Mask(mask) => node.paint.mask = mask,
                Change::Sticky(inset) => node.sticky = inset,
                Change::TranslateX(v) => node.paint.node_transform.translate_x.set_now(v),
                Change::TranslateY(v) => node.paint.node_transform.translate_y.set_now(v),
                Change::Scale(v) => node.paint.node_transform.scale.set_now(v),
                Change::RotationDeg(v) => {
                    node.paint.node_transform.rotation_deg.set_now(v);
                }
                Change::HitPath(outline) => node.paint.hit_outline = outline,
                Change::CornerStyle(style) => node.paint.corner_concave = style,
                Change::TransformOrigin(origin) => node.paint.node_transform.origin = origin,
                Change::Data(data) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.template = None;
                        state.data.set_now(data);
                    }
                }
                Change::Params(params) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        for (name, value) in params {
                            state.set_param(&name, value);
                        }
                        let _ = state.reevaluate();
                    }
                }
                Change::Template(template) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.template = Some(template);
                        let _ = state.reevaluate();
                    }
                }
                Change::ViewBox(view_box) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.view_box = view_box;
                    }
                }
                Change::Morph(mode) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.morph = mode;
                    }
                }
                Change::HitShape(hit_shape) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.hit_shape = hit_shape;
                    }
                }
                Change::TrimStart(start) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.trim_start.set_now(start);
                    }
                }
                Change::TrimEnd(end) => {
                    if let NodeKind::Path(state) = &mut node.kind {
                        state.trim_end.set_now(end);
                    }
                }
                Change::Fill(color) => {
                    match &mut node.kind {
                        NodeKind::TextField(state) => {
                            state.text_tint.set_now(color);
                            state.tint_scheme = false;
                        }
                        _ => {
                            node.paint.background.set_now(color);
                            node.paint.scheme_fill = false;
                        }
                    }
                    // A colour replaces any gradient.
                    node.paint.gradient = None;
                }
                Change::FillUnset => {
                    match &mut node.kind {
                        NodeKind::TextField(state) => state.tint_scheme = true,
                        _ => node.paint.scheme_fill = true,
                    }
                    node.paint.gradient = None;
                }
                Change::FillGradient(gradient) => {
                    node.paint.gradient = Some(Box::new(Animated::new(gradient)));
                    node.paint.scheme_fill = false;
                }
                Change::StrokeColor(color) => {
                    node.paint.border_color.set_now(color);
                    node.paint.border_gradient = None;
                }
                Change::StrokeGradient(gradient) => {
                    node.paint.border_gradient = Some(Box::new(gradient));
                }
                Change::StrokeWidth(width) => node.paint.border_width.set_now(width),
                Change::Opacity(opacity) => node.paint.opacity.set_now(opacity),
                Change::Blur(blur) => node.paint.blur.set_now(blur),
                Change::BackdropBlur(blur) => node.paint.backdrop_blur.set_now(blur),
                Change::BlendMode(mode) => node.paint.blend = mode,
                Change::CornerRadius(Radius::Uniform(radius)) => {
                    node.paint.corner_radius.set_now(radius);
                    node.paint.corner_radii_override = None;
                }
                Change::CornerRadius(Radius::Corners(corners)) => {
                    node.paint.corner_radii_override = Some(Animated::new(CornerRadii(corners)));
                }
                Change::Shadows(shadows) => {
                    node.paint.shadows.set_now(Shadows(shadows));
                }
                Change::Placeholder(text) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.placeholder = text;
                    }
                }
                Change::PlaceholderFill(color) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.placeholder_fill = color;
                    }
                }
                Change::CaretColor(color) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        match (&mut state.caret_color, color) {
                            (Some(current), Some(color)) => current.set_now(color),
                            (slot, color) => *slot = color.map(Animated::new),
                        }
                    }
                }
                Change::MaxLength(max) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.max_length = max;
                    }
                }
                Change::ReadOnly(read_only) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.read_only = read_only;
                        if read_only {
                            state.preedit = None;
                        }
                    }
                }
                Change::InputMode(mode) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.input_mode = mode;
                    }
                }
                Change::CaretVisible(visible) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.caret_visible = visible;
                    }
                }
                Change::CaretWidth(width) => match &mut node.kind {
                    NodeKind::TextField(state) => state.caret_width = width,
                    NodeKind::Terminal(state) => state.caret_width = width,
                    _ => {}
                },
                Change::CaretShape(shape) => match &mut node.kind {
                    NodeKind::TextField(state) => state.caret_shape = shape,
                    NodeKind::Terminal(state) => state.caret_shape = shape,
                    _ => {}
                },
                Change::CaretBlink(ms) => match &mut node.kind {
                    NodeKind::TextField(state) => {
                        state.caret_blink_ms = ms;
                        state.caret_on = true;
                        state.caret_epoch = None;
                    }
                    NodeKind::Terminal(state) => {
                        state.caret_blink_ms = ms;
                        state.caret_on = true;
                        state.caret_epoch = None;
                    }
                    _ => {}
                },
                Change::SelectionFill(color) => match &mut node.kind {
                    NodeKind::TextField(state) => state.selection_fill = color,
                    NodeKind::Text(state) => state.options.selection_fill = color,
                    _ => {}
                },
                Change::SelectionForeground(color) => match &mut node.kind {
                    NodeKind::TextField(state) => state.selection_foreground = color,
                    NodeKind::Text(state) => state.options.selection_foreground = color,
                    NodeKind::Terminal(state) => state.selection_foreground = color,
                    _ => {}
                },
                Change::SelectionRadius(radius) => match &mut node.kind {
                    NodeKind::TextField(state) => state.selection_radius = radius,
                    NodeKind::Text(state) => state.options.selection_radius = radius,
                    NodeKind::Terminal(state) => state.selection_radius = radius,
                    _ => {}
                },
                Change::Obscured(obscured) => {
                    if let NodeKind::TextField(state) = &mut node.kind {
                        state.obscured = obscured;
                    }
                }
                Change::Palette(patch) => {
                    if let NodeKind::Terminal(state) = &mut node.kind {
                        patch.apply(&mut state.palette);
                    }
                }
            }
        }
        // M96: a terminal's box follows its grid and font.
        if resize_terminal && let NodeKind::Terminal(state) = &node.kind {
            let (cols, rows) = (f32::from(state.cols), f32::from(state.rows));
            let (cell_width, cell_height) = state.cell_size(crate::shaper::with(|shaper| {
                shaper.monospace_cell_size(&state.font_family, state.font_size)
            }));
            let size = &mut style.get_or_insert_with(|| node.layout_style.clone()).size;
            size.width = Dimension::length(cell_width * cols);
            size.height = Dimension::length(cell_height * rows);
            // 0.6.12 (UXREQ-006): a line is the unit of the history bar's offsets; its bar
            // moves to the new unit with the view where it was.
            if let NodeKind::Terminal(state) = &mut node.kind {
                state.line_px = cell_height;
                state.set_history(state.history_len, state.history_back);
            }
        }
        if let Some(style) = style {
            tree.set_layout_style(self.id, style);
        }
        // 0.5.7 (#176, #177): a side that fits the text depends on the text,
        // its font and its wrapping, so a change to any of them lays it out again.
        tree.set_fit_content(self.id, fit);
        tree.set_calc_size(self.id, false, calc.width);
        tree.set_calc_size(self.id, true, calc.height);
        if anchor_changed {
            tree.set_anchor(self.id, anchored);
        }
        if !layer_edits.is_empty() {
            tree.update_layer(self.id, |meta| {
                for edit in layer_edits {
                    edit.apply(meta);
                }
            });
        }
        if state_layer_changed {
            tree.mark_state_layer(self.id);
        }
        if kind_changed && fit.any() {
            tree.relayout(self.id);
        }
        if kind_changed {
            // 0.6.13: a container's `selection_mode` may have changed.
            tree.refresh_selection_host(self.id);
        }
        let released = if reset_rows {
            tree.virtual_list_release_outside(self.id, 0..0)
        } else {
            Vec::new()
        };
        for &row in &released {
            tree.detach_collectible(row);
        }
        tree.adopt_text_selection(self.id);
        drop(tree);
        if !callbacks.is_empty() {
            let mut handlers = self.handlers.borrow_mut();
            for (key, callback) in callbacks {
                handlers.insert((self.id, key), (callback, false));
            }
        }
        for row in released {
            crate::node_handles::collect(&self.tree, &self.handlers, row);
        }
    }
}

impl Node {
    fn require_text_input(&self, what: &str) -> PyResult<()> {
        let tree = self.tree.borrow();
        match tree.get(self.id).map(|n| &n.kind) {
            Some(NodeKind::TextField(_)) => Ok(()),
            Some(_) => Err(PyValueError::new_err(format!(
                "{what}() applies only to a text_input node"
            ))),
            None => Err(crate::error::EngineError::Destroyed.into()),
        }
    }
}
