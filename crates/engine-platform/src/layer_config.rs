//! 0.5.8 (#198): a layer surface's settings and the rules the protocol puts on them.
//!
//! Pure data and validation, with no Wayland in it, so the rules are checked (and
//! the Python errors raised) on every platform and without a compositor. The
//! `zwlr_layer_shell_v1` protocol makes some combinations a protocol error that
//! kills the client's connection (`invalid_size`, `invalid_anchor`,
//! `invalid_exclusive_edge`, ...); [`LayerConfig::validate`] refuses them first, with
//! the key that is wrong, so the app sees an error it can read.

use std::fmt;

/// The stacking layer: what the surface sits above and below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Background,
    Bottom,
    Top,
    Overlay,
}

impl LayerKind {
    pub const NAMES: [&'static str; 4] = ["background", "bottom", "top", "overlay"];

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "background" => Self::Background,
            "bottom" => Self::Bottom,
            "top" => Self::Top,
            "overlay" => Self::Overlay,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// How the surface takes the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardMode {
    /// Never: it gets no key events.
    None,
    /// Whenever the user focuses it (a click, say).
    OnDemand,
    /// At once, and no other surface gets keys until it lets go.
    Exclusive,
}

impl KeyboardMode {
    pub const NAMES: [&'static str; 3] = ["none", "on_demand", "exclusive"];

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "none" => Self::None,
            "on_demand" => Self::OnDemand,
            "exclusive" => Self::Exclusive,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// The screen edges a surface is anchored to, as the protocol's bit field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchors(pub u8);

impl Anchors {
    pub const TOP: u8 = 1;
    pub const BOTTOM: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
    pub const NAMES: [&'static str; 4] = ["top", "bottom", "left", "right"];

    pub fn edge(name: &str) -> Option<u8> {
        Some(match name {
            "top" => Self::TOP,
            "bottom" => Self::BOTTOM,
            "left" => Self::LEFT,
            "right" => Self::RIGHT,
            _ => return None,
        })
    }

    pub fn contains(self, edge: u8) -> bool {
        self.0 & edge == edge
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The edge names set, in the order top, bottom, left, right.
    pub fn names(self) -> Vec<&'static str> {
        [Self::TOP, Self::BOTTOM, Self::LEFT, Self::RIGHT]
            .into_iter()
            .zip(Self::NAMES)
            .filter(|(bit, _)| self.contains(*bit))
            .map(|(_, name)| name)
            .collect()
    }

    /// Whether the protocol gives a positive exclusive zone to this anchor set: one
    /// edge alone, or one edge with both edges across it (a bar anchored top, left and
    /// right). Any other set, a positive zone is treated as 0.
    pub fn can_reserve(self) -> bool {
        let Anchors(bits) = self;
        let horizontal = Self::LEFT | Self::RIGHT;
        let vertical = Self::TOP | Self::BOTTOM;
        matches!(bits, Self::TOP | Self::BOTTOM | Self::LEFT | Self::RIGHT)
            || bits == Self::TOP | horizontal
            || bits == Self::BOTTOM | horizontal
            || bits == Self::LEFT | vertical
            || bits == Self::RIGHT | vertical
    }
}

/// Everything that makes a layer surface what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerConfig {
    /// What the surface is for (`"tesserae-shell.bar"`): the protocol's namespace.
    pub namespace: String,
    pub layer: LayerKind,
    pub anchor: Anchors,
    /// `(width, height)` in logical pixels; 0 on an axis stretches between the two
    /// edges anchored on it.
    pub size: (u32, u32),
    /// Pixels to reserve along the anchored edge; 0 reserves none; -1 ignores other
    /// surfaces' zones.
    pub exclusive_zone: i32,
    /// For a surface anchored to a corner: which edge its positive zone is for.
    pub exclusive_edge: Option<u8>,
    /// `(top, right, bottom, left)`, in logical pixels. Negative is accepted.
    pub margin: (i32, i32, i32, i32),
    pub keyboard: KeyboardMode,
    /// A connector name (`"DP-1"`); `None` lets the compositor choose.
    pub output: Option<String>,
    /// Where the compositor delivers pointer and touch input, as `(x, y, width, height)`
    /// rectangles in logical pixels from the surface's top-left. `None` is the whole
    /// surface; `Some(vec![])` is nothing (clicks fall through).
    pub input_region: Option<Vec<Rect>>,
}

/// `(x, y, width, height)` in whole logical pixels.
pub type Rect = (i32, i32, i32, i32);

/// The most rectangles one input region may have. A shape the engine decomposes into
/// scanline bands stays well inside this; a hand-written list that exceeds it is refused.
pub const MAX_REGION_RECTS: usize = 256;

/// Explicit rectangles as given (floats, logical pixels) to whole-pixel ones, rounded
/// outward so a rectangle never shrinks: a click on a pixel the app drew is never lost.
pub fn region_from_floats(rects: &[(f64, f64, f64, f64)]) -> Result<Vec<Rect>, ConfigError> {
    if rects.len() > MAX_REGION_RECTS {
        return Err(error(
            "input_region",
            format!(
                "has {} rectangles; at most {MAX_REGION_RECTS} are allowed",
                rects.len()
            ),
        ));
    }
    let mut out = Vec::with_capacity(rects.len());
    for &(x, y, w, h) in rects {
        if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) {
            return Err(error(
                "input_region",
                "rectangle values must be finite numbers",
            ));
        }
        if w <= 0.0 || h <= 0.0 {
            return Err(error(
                "input_region",
                "each rectangle needs a width and a height above 0 (use [] for no input)",
            ));
        }
        let (left, top) = (x.floor(), y.floor());
        let (right, bottom) = ((x + w).ceil(), (y + h).ceil());
        if [left, top, right, bottom]
            .iter()
            .any(|v| v.abs() > f64::from(i32::MAX / 2))
        {
            return Err(error("input_region", "rectangle values are out of range"));
        }
        out.push((
            left as i32,
            top as i32,
            (right - left) as i32,
            (bottom - top) as i32,
        ));
    }
    Ok(out)
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            namespace: String::new(),
            layer: LayerKind::Top,
            anchor: Anchors(0),
            size: (0, 0),
            exclusive_zone: 0,
            exclusive_edge: None,
            margin: (0, 0, 0, 0),
            keyboard: KeyboardMode::None,
            output: None,
            input_region: None,
        }
    }
}

/// A point on a rectangle, or the direction a popup extends from one (`xdg_positioner`'s anchor
/// and gravity share these nine values).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

impl PopupEdge {
    pub const NAMES: [&'static str; 9] = [
        "none",
        "top",
        "bottom",
        "left",
        "right",
        "top_left",
        "bottom_left",
        "top_right",
        "bottom_right",
    ];

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "none" => Self::None,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "left" => Self::Left,
            "right" => Self::Right,
            "top_left" => Self::TopLeft,
            "bottom_left" => Self::BottomLeft,
            "top_right" => Self::TopRight,
            "bottom_right" => Self::BottomRight,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// How the compositor may move a popup that would leave the screen (`xdg_positioner`'s
/// constraint adjustment, as bits).
pub mod adjust {
    pub const SLIDE_X: u32 = 1;
    pub const SLIDE_Y: u32 = 2;
    pub const FLIP_X: u32 = 4;
    pub const FLIP_Y: u32 = 8;
    pub const RESIZE_X: u32 = 16;
    pub const RESIZE_Y: u32 = 32;
    pub const NAMES: [(&str, u32); 6] = [
        ("slide_x", SLIDE_X),
        ("slide_y", SLIDE_Y),
        ("flip_x", FLIP_X),
        ("flip_y", FLIP_Y),
        ("resize_x", RESIZE_X),
        ("resize_y", RESIZE_Y),
    ];

    /// The bit for one adjustment name.
    pub fn parse(name: &str) -> Option<u32> {
        NAMES.iter().find(|(n, _)| *n == name).map(|(_, bit)| *bit)
    }

    /// The names of the bits set, in a fixed order.
    pub fn names(bits: u32) -> Vec<&'static str> {
        NAMES
            .iter()
            .filter(|(_, bit)| bits & bit != 0)
            .map(|(n, _)| *n)
            .collect()
    }
}

/// A popup (`xdg_popup`) parented to a layer surface or to another popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopupConfig {
    /// The parent surface's number (the layer thread's id for it).
    pub parent: u64,
    /// `(width, height)` in logical pixels: the positioner needs it before the popup exists.
    pub size: (u32, u32),
    /// A rectangle in the parent's surface coordinates the popup is placed against.
    pub anchor_rect: (i32, i32, i32, i32),
    /// The point on that rectangle the popup attaches to.
    pub anchor: PopupEdge,
    /// The direction the popup extends from that point.
    pub gravity: PopupEdge,
    /// What the compositor may do to keep it on screen (`adjust` bits).
    pub adjustments: u32,
    /// A fixed shift after placement, in logical pixels.
    pub offset: (i32, i32),
    /// Whether the popup grabs the keyboard and pointer, so an outside click ends it (Escape does not: the popup gets the key, on KWin and sway alike).
    pub grab: bool,
    /// #266: the popup's body, `(x, y, width, height)` in the popup's own pixels, inside `size`.
    /// The compositor places and constrains this rectangle and ignores the rest of the surface
    /// (a transparent shadow margin). `None` is the whole surface.
    pub geometry: Option<Rect>,
    /// 0.6.12 (P10): `(top, right, bottom, left)`, the distance in logical pixels the compositor
    /// keeps between the body and the edge of the output when it flips or slides the popup. The
    /// compositor constrains the window geometry, so the margin is the transparent space the
    /// surface keeps around the body (`geometry`): a side with less is kept as far as it goes
    /// (see [`PopupConfig::edge_fit`]). `(0, 0, 0, 0)`, the default, is what popups always did.
    pub edge_margin: (i32, i32, i32, i32),
    /// #282: where the compositor delivers input, `(x, y, width, height)` rectangles in the
    /// popup's own logical pixels (margin included). `None` is the whole surface, `Some(vec![])`
    /// nothing, so a click in a shadow margin can reach the window below.
    pub input_region: Option<Vec<Rect>>,
}

/// 0.6.12 (P10): what the positioner is given for a popup with an `edge_margin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeFit {
    /// The margin that fits in the surface, `(top, right, bottom, left)`.
    pub applied: (i32, i32, i32, i32),
    /// The window geometry and positioner size: the body grown by `applied`.
    pub geometry: Rect,
    /// The anchor rectangle, moved so the body, not the grown rectangle, lands where the
    /// unmargined popup would, flipped or not.
    pub anchor_rect: Rect,
}

/// One axis' side of an anchor or gravity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Low,
    Middle,
    High,
}

impl PopupEdge {
    fn sides(self) -> (Side, Side) {
        use PopupEdge::*;
        let x = match self {
            Left | TopLeft | BottomLeft => Side::Low,
            Right | TopRight | BottomRight => Side::High,
            _ => Side::Middle,
        };
        let y = match self {
            Top | TopLeft | TopRight => Side::Low,
            Bottom | BottomLeft | BottomRight => Side::High,
            _ => Side::Middle,
        };
        (x, y)
    }
}

/// The anchor rectangle's `(start, length)` on one axis, moved for a popup grown by `before` and
/// `after` on that axis. The positioner puts the grown rectangle's edge on the anchor edge; a
/// popup that extends away from the edge it attaches to (anchor and gravity on the same side)
/// needs the edge pulled back by the margin to keep the body on it, one on the other side needs the
/// rectangle grown by both margins, so that the flipped popup (anchor and gravity both inverted) is
/// as far from it. A popup centred on the anchor point moves by half the difference; a centred
/// anchor with an edge gravity cannot be right both ways and is left alone.
fn grow_axis(
    start: i32,
    len: i32,
    before: i32,
    after: i32,
    anchor: Side,
    gravity: Side,
) -> (i32, i32) {
    match (anchor, gravity) {
        (_, Side::Middle) => (start + (after - before) / 2, len),
        (Side::Middle, _) => (start, len),
        (a, g) if a == g => {
            if len >= before + after {
                (start + after, len - before - after)
            } else {
                // Narrower than the margins: a point in the middle.
                (start + len / 2, 0)
            }
        }
        _ => (start - before, len + before + after),
    }
}

impl PopupConfig {
    /// 0.6.12 (P10): the window geometry, positioner size and anchor rectangle that give the body
    /// `edge_margin` of clearance from the output's edge: the body grown by the margin on each
    /// side, as far as the surface has transparent room (`geometry`; a popup without one has none,
    /// its whole surface is the body).
    pub fn edge_fit(&self) -> EdgeFit {
        let (sw, sh) = (self.size.0 as i32, self.size.1 as i32);
        let (gx, gy, gw, gh) = self.geometry.unwrap_or((0, 0, sw, sh));
        let (top, right, bottom, left) = self.edge_margin;
        let applied = (
            top.clamp(0, gy),
            right.clamp(0, (sw - gx - gw).max(0)),
            bottom.clamp(0, (sh - gy - gh).max(0)),
            left.clamp(0, gx),
        );
        let (t, r, b, l) = applied;
        let (x, y, w, h) = self.anchor_rect;
        let ((anchor_x, anchor_y), (gravity_x, gravity_y)) =
            (self.anchor.sides(), self.gravity.sides());
        let (rx, rw) = grow_axis(x, w, l, r, anchor_x, gravity_x);
        let (ry, rh) = grow_axis(y, h, t, b, anchor_y, gravity_y);
        EdgeFit {
            applied,
            geometry: (gx - l, gy - t, gw + l + r, gh + t + b),
            anchor_rect: (rx, ry, rw, rh),
        }
    }

    /// Refuses what the protocol would answer with a fatal error, naming the key.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let (t, r, b, l) = self.edge_margin;
        if t < 0 || r < 0 || b < 0 || l < 0 {
            return Err(error(
                "edge_margin",
                "must be a number of pixels, zero or more, or (top, right, bottom, left)",
            ));
        }
        if self.size.0 == 0 || self.size.1 == 0 {
            return Err(error("size", "must be a width and a height above 0"));
        }
        let (_, _, w, h) = self.anchor_rect;
        if w < 0 || h < 0 {
            return Err(error(
                "anchor_rect",
                "needs a width and a height of 0 or more",
            ));
        }
        if let Some(rects) = &self.input_region
            && rects.len() > MAX_REGION_RECTS
        {
            return Err(error(
                "input_region",
                format!(
                    "has {} rectangles; at most {MAX_REGION_RECTS} are allowed",
                    rects.len()
                ),
            ));
        }
        if let Some((x, y, w, h)) = self.geometry {
            let (sw, sh) = (i64::from(self.size.0), i64::from(self.size.1));
            let inside = w > 0
                && h > 0
                && x >= 0
                && y >= 0
                && i64::from(x) + i64::from(w) <= sw
                && i64::from(y) + i64::from(h) <= sh;
            if !inside {
                return Err(error(
                    "geometry",
                    "must be (x, y, width, height) with a width and height above 0, inside the popup's size",
                ));
            }
        }
        Ok(())
    }
}

/// Why a configuration was refused: the Python-level key, and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub key: &'static str,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Something the protocol accepts but that probably is not what was meant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigWarning {
    pub key: &'static str,
    pub message: String,
}

fn error(key: &'static str, message: impl Into<String>) -> ConfigError {
    ConfigError {
        key,
        message: message.into(),
    }
}

impl LayerConfig {
    /// Refuses what the protocol would answer with a fatal error, naming the key; for
    /// what it accepts but ignores, returns a warning.
    pub fn validate(&self) -> Result<Vec<ConfigWarning>, ConfigError> {
        if self.namespace.is_empty() {
            return Err(error(
                "namespace",
                "must not be empty: it says what the surface is for",
            ));
        }
        if self.namespace.contains('\0') {
            return Err(error("namespace", "must not contain a NUL character"));
        }
        let (width, height) = self.size;
        if width == 0
            && !(self.anchor.contains(Anchors::LEFT) && self.anchor.contains(Anchors::RIGHT))
        {
            return Err(error(
                "size",
                "a width of 0 stretches between the left and right edges, so `anchor` must \
                 include both \"left\" and \"right\"",
            ));
        }
        if height == 0
            && !(self.anchor.contains(Anchors::TOP) && self.anchor.contains(Anchors::BOTTOM))
        {
            return Err(error(
                "size",
                "a height of 0 stretches between the top and bottom edges, so `anchor` must \
                 include both \"top\" and \"bottom\"",
            ));
        }
        if let Some(edge) = self.exclusive_edge
            && !self.anchor.contains(edge)
        {
            return Err(error(
                "exclusive_edge",
                "must be one of the edges in `anchor`",
            ));
        }
        if self.exclusive_zone < -1 {
            return Err(error(
                "exclusive_zone",
                "must be a number of pixels, 0, or -1 (ignore other surfaces' zones)",
            ));
        }
        if let Some(rects) = &self.input_region
            && rects.len() > MAX_REGION_RECTS
        {
            return Err(error(
                "input_region",
                format!(
                    "has {} rectangles; at most {MAX_REGION_RECTS} are allowed",
                    rects.len()
                ),
            ));
        }
        let mut warnings = Vec::new();
        if self.exclusive_zone > 0 && self.exclusive_edge.is_none() && !self.anchor.can_reserve() {
            warnings.push(ConfigWarning {
                key: "exclusive_zone",
                message: "a positive zone counts only for a surface anchored to one edge, or \
                          to one edge and both edges across it; with this `anchor` the \
                          compositor treats it as 0 (name an `exclusive_edge` for a corner)"
                    .into(),
            });
        }
        Ok(warnings)
    }
}

/// Whether layer surfaces can be made here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    /// The compositor offers `zwlr_layer_shell_v1`.
    Supported,
    /// No display at all (a CI run): a layer window can be built and driven with
    /// `simulate`, `advance` and `snapshot`, but not opened.
    Headless,
    /// A display with no layer shell (X11, GNOME's Mutter, Windows, macOS); the string
    /// says why.
    Unsupported(String),
}

/// Where this process stands on layer surfaces, as first found out.
pub fn availability() -> Availability {
    availability_with(false)
}

/// As [`availability`]; `refresh` asks the compositor again instead of using the remembered answer.
pub fn availability_with(refresh: bool) -> Availability {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return if crate::layer::supported(refresh) {
                Availability::Supported
            } else {
                Availability::Unsupported(
                    "this Wayland compositor has no zwlr_layer_shell_v1 (GNOME's Mutter has none)"
                        .into(),
                )
            };
        }
        if std::env::var_os("DISPLAY").is_some() {
            return Availability::Unsupported(
                "an X11 session has no layer shell: layer surfaces are Wayland only".into(),
            );
        }
        Availability::Headless
    }
    #[cfg(not(target_os = "linux"))]
    {
        Availability::Unsupported("layer surfaces are Wayland only (Linux)".into())
    }
}

/// The edge names the Python layer accepts for `anchor` and `exclusive_edge`.
pub fn parse_anchor<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Anchors, ConfigError> {
    let mut bits = 0;
    for name in names {
        let Some(bit) = Anchors::edge(name) else {
            return Err(error(
                "anchor",
                format!(
                    "unknown edge {name:?}; valid: {}",
                    Anchors::NAMES.join(", ")
                ),
            ));
        };
        bits |= bit;
    }
    Ok(Anchors(bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> LayerConfig {
        LayerConfig {
            namespace: "tesserae-shell.bar".into(),
            anchor: Anchors(Anchors::TOP | Anchors::LEFT | Anchors::RIGHT),
            size: (0, 32),
            exclusive_zone: 32,
            ..Default::default()
        }
    }

    #[test]
    fn a_bar_is_valid_and_says_nothing() {
        assert_eq!(bar().validate(), Ok(vec![]));
    }

    #[test]
    fn a_stretched_axis_needs_both_opposite_edges() {
        let mut c = bar();
        c.anchor = Anchors(Anchors::TOP | Anchors::LEFT);
        let e = c.validate().unwrap_err();
        assert_eq!(e.key, "size");
        assert!(e.message.contains("left") && e.message.contains("right"));
        let mut c = bar();
        c.size = (200, 0);
        assert_eq!(c.validate().unwrap_err().key, "size");
        c.anchor = Anchors(Anchors::TOP | Anchors::BOTTOM | Anchors::LEFT);
        assert_eq!(c.validate(), Ok(vec![]));
        // Both axes stretched: a whole-output surface.
        let mut c = bar();
        c.size = (0, 0);
        c.anchor = Anchors(Anchors::TOP | Anchors::BOTTOM | Anchors::LEFT | Anchors::RIGHT);
        c.exclusive_zone = -1;
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn an_exclusive_edge_must_be_an_anchor() {
        let mut c = bar();
        c.exclusive_edge = Some(Anchors::BOTTOM);
        assert_eq!(c.validate().unwrap_err().key, "exclusive_edge");
        c.exclusive_edge = Some(Anchors::TOP);
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn a_zone_the_protocol_would_ignore_is_a_warning_not_an_error() {
        let mut c = bar();
        c.anchor = Anchors(Anchors::TOP | Anchors::LEFT);
        c.size = (200, 32);
        let w = c.validate().unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].key, "exclusive_zone");
        // Naming the edge makes it a corner clock that reserves the top.
        c.exclusive_edge = Some(Anchors::TOP);
        assert_eq!(c.validate(), Ok(vec![]));
        // No zone, no warning.
        c.exclusive_edge = None;
        c.exclusive_zone = 0;
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn which_anchor_sets_can_reserve_space() {
        let a = |bits| Anchors(bits).can_reserve();
        assert!(a(Anchors::TOP));
        assert!(a(Anchors::TOP | Anchors::LEFT | Anchors::RIGHT));
        assert!(a(Anchors::LEFT | Anchors::TOP | Anchors::BOTTOM));
        assert!(!a(0));
        assert!(!a(Anchors::TOP | Anchors::LEFT));
        assert!(!a(Anchors::TOP | Anchors::BOTTOM));
        assert!(!a(Anchors::TOP
            | Anchors::BOTTOM
            | Anchors::LEFT
            | Anchors::RIGHT));
    }

    #[test]
    fn the_namespace_and_the_zone_are_checked() {
        let mut c = bar();
        c.namespace = String::new();
        assert_eq!(c.validate().unwrap_err().key, "namespace");
        let mut c = bar();
        c.exclusive_zone = -2;
        assert_eq!(c.validate().unwrap_err().key, "exclusive_zone");
        c.exclusive_zone = -1;
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn names_parse_and_round_trip() {
        for name in LayerKind::NAMES {
            assert_eq!(LayerKind::parse(name).unwrap().name(), name);
        }
        for name in KeyboardMode::NAMES {
            assert_eq!(KeyboardMode::parse(name).unwrap().name(), name);
        }
        assert!(LayerKind::parse("above").is_none());
        assert!(KeyboardMode::parse("always").is_none());
        let a = parse_anchor(["top", "left", "right"]).unwrap();
        assert_eq!(a.names(), vec!["top", "left", "right"]);
        assert_eq!(parse_anchor(["middle"]).unwrap_err().key, "anchor");
        assert!(parse_anchor([]).unwrap().is_empty());
    }

    #[test]
    fn a_region_rounds_outward_and_refuses_empty_or_huge_ones() {
        assert_eq!(
            region_from_floats(&[(0.5, 1.2, 10.0, 2.0)]).unwrap(),
            vec![(0, 1, 11, 3)]
        );
        assert_eq!(
            region_from_floats(&[(-4.0, 0.0, 4.0, 4.0)]).unwrap(),
            vec![(-4, 0, 4, 4)]
        );
        assert!(region_from_floats(&[(0.0, 0.0, 0.0, 5.0)]).is_err());
        assert!(region_from_floats(&[(0.0, 0.0, f64::NAN, 5.0)]).is_err());
        assert!(region_from_floats(&vec![(0.0, 0.0, 1.0, 1.0); MAX_REGION_RECTS + 1]).is_err());
        assert!(region_from_floats(&vec![(0.0, 0.0, 1.0, 1.0); MAX_REGION_RECTS]).is_ok());
        assert_eq!(region_from_floats(&[]).unwrap(), vec![]);
    }

    #[test]
    fn popup_edges_and_adjustments_parse_by_name() {
        for name in PopupEdge::NAMES {
            assert_eq!(PopupEdge::parse(name).map(PopupEdge::name), Some(name));
        }
        assert_eq!(PopupEdge::parse("middle"), None);
        assert_eq!(adjust::parse("flip_y"), Some(adjust::FLIP_Y));
        assert_eq!(adjust::parse("wobble"), None);
        assert_eq!(
            adjust::names(adjust::SLIDE_X | adjust::FLIP_Y),
            vec!["slide_x", "flip_y"]
        );
    }

    #[test]
    fn a_popup_needs_a_size_and_a_sane_anchor_rectangle() {
        let ok = PopupConfig {
            parent: 1,
            size: (100, 50),
            anchor_rect: (0, 0, 10, 10),
            anchor: PopupEdge::BottomLeft,
            gravity: PopupEdge::BottomRight,
            adjustments: adjust::FLIP_Y,
            offset: (0, 0),
            grab: true,
            geometry: None,
            edge_margin: (0, 0, 0, 0),
            input_region: None,
        };
        assert!(ok.validate().is_ok());
        assert!(
            PopupConfig {
                edge_margin: (0, -1, 0, 0),
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            PopupConfig {
                size: (0, 50),
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            PopupConfig {
                anchor_rect: (0, 0, -1, 4),
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            PopupConfig {
                geometry: Some((10, 8, 80, 34)),
                ..ok.clone()
            }
            .validate()
            .is_ok()
        );
        assert!(
            PopupConfig {
                input_region: Some(vec![(0, 0, 10, 10); MAX_REGION_RECTS]),
                ..ok.clone()
            }
            .validate()
            .is_ok()
        );
        assert!(
            PopupConfig {
                input_region: Some(vec![(0, 0, 10, 10); MAX_REGION_RECTS + 1]),
                ..ok.clone()
            }
            .validate()
            .is_err()
        );
        for bad in [
            (0, 0, 0, 10),
            (-1, 0, 10, 10),
            (10, 0, 91, 10),
            (0, 45, 10, 6),
        ] {
            assert!(
                PopupConfig {
                    geometry: Some(bad),
                    ..ok.clone()
                }
                .validate()
                .is_err(),
                "{bad:?}"
            );
        }
    }

    /// A 120 x 80 popup whose 100 x 60 body sits 10 px in from every side, placed from a 40 x 20
    /// anchor rectangle at (200, 100).
    fn margined(
        anchor: PopupEdge,
        gravity: PopupEdge,
        edge_margin: (i32, i32, i32, i32),
    ) -> PopupConfig {
        PopupConfig {
            parent: 1,
            size: (120, 80),
            anchor_rect: (200, 100, 40, 20),
            anchor,
            gravity,
            adjustments: adjust::FLIP_Y | adjust::SLIDE_X,
            offset: (0, 0),
            grab: true,
            geometry: Some((10, 10, 100, 60)),
            edge_margin,
            input_region: None,
        }
    }

    #[test]
    fn no_edge_margin_leaves_the_positioner_as_it_was() {
        let fit = margined(PopupEdge::BottomLeft, PopupEdge::BottomRight, (0, 0, 0, 0)).edge_fit();
        assert_eq!(fit.applied, (0, 0, 0, 0));
        assert_eq!(fit.geometry, (10, 10, 100, 60));
        assert_eq!(fit.anchor_rect, (200, 100, 40, 20));
    }

    #[test]
    fn the_margin_grows_the_geometry_as_far_as_the_surface_has_room() {
        let fit = margined(PopupEdge::BottomLeft, PopupEdge::BottomRight, (8, 4, 25, 8)).edge_fit();
        // 25 below does not fit in 10: clamped.
        assert_eq!(fit.applied, (8, 4, 10, 8));
        assert_eq!(fit.geometry, (2, 2, 112, 78));
        // Without `geometry` the whole surface is the body and nothing is left to grow into.
        let mut plain = margined(PopupEdge::BottomLeft, PopupEdge::BottomRight, (8, 8, 8, 8));
        plain.geometry = None;
        let fit = plain.edge_fit();
        assert_eq!(fit.applied, (0, 0, 0, 0));
        assert_eq!(fit.geometry, (0, 0, 120, 80));
    }

    #[test]
    fn the_anchor_rectangle_follows_where_the_body_attaches() {
        // Below the rectangle (anchor bottom, gravity bottom) and left-aligned (anchor left,
        // gravity right): y shrinks by both margins, x grows by both, so a flip above or right
        // keeps the body against the rectangle too.
        let fit = margined(PopupEdge::BottomLeft, PopupEdge::BottomRight, (8, 6, 4, 2)).edge_fit();
        assert_eq!(fit.anchor_rect, (200 - 2, 100 + 4, 40 + 2 + 6, 20 - 8 - 4));
        // The mirror: above and right-aligned.
        let fit = margined(PopupEdge::TopRight, PopupEdge::TopLeft, (8, 6, 4, 2)).edge_fit();
        assert_eq!(fit.anchor_rect, (200 - 2, 100 + 4, 40 + 2 + 6, 20 - 8 - 4));
        // A rectangle thinner than the margins becomes a point in its middle.
        let mut thin = margined(PopupEdge::BottomLeft, PopupEdge::BottomRight, (8, 6, 8, 2));
        thin.anchor_rect = (200, 100, 40, 10);
        assert_eq!(
            thin.edge_fit().anchor_rect.1..=thin.edge_fit().anchor_rect.1 + 1,
            105..=106
        );
        assert_eq!(thin.edge_fit().anchor_rect.3, 0);
    }

    #[test]
    fn a_popup_centred_on_the_anchor_moves_by_half_the_difference() {
        let fit = margined(PopupEdge::Bottom, PopupEdge::None, (0, 6, 0, 2)).edge_fit();
        assert_eq!(fit.anchor_rect.0, 200 + 2);
        // A centred anchor with an edge gravity is left alone.
        let fit = margined(PopupEdge::None, PopupEdge::BottomRight, (8, 6, 4, 2)).edge_fit();
        assert_eq!(fit.anchor_rect, (200, 100, 40, 20));
    }
}
