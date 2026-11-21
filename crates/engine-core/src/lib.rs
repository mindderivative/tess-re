//! Pure-Rust node tree, animation core, and the generic interfaces other
//! crates build on. No `pyo3`, no `winit`, no design system. Does depend
//! directly on the plain `accesskit` data crate (§14 step 7, §4/§10's
//! own reasoning: `accesskit` is small and OS-agnostic, the same class
//! of dependency as `taffy`/`parley`, both already here).
//!
//! `InputEvent` (§4) lands
//! here at M4 Phase 1 step 1, alongside `Tree::dispatch`/`hit_test`/
//! `update_hover`/`move_focus` -- the real dispatch core every M3
//! interaction-dependent step (7, 9, 11, 12, 13, 14, 15) deferred, each
//! exposing a direct `Tree` method instead and explicitly naming this
//! as the eventual real wiring. `input.rs`'s own doc comment has the
//! real correction: this step's original plan sketched a generic
//! `AppHandler` trait alongside `InputEvent` for the one meaning-
//! dependent hook `Tree::dispatch` can't resolve itself -- never
//! actually implemented anywhere; `engine-py::dispatch.rs`'s own
//! `HandlerMap` and `listeners.rs` delivery are the real mechanism
//! instead, so the dead trait was removed.

mod access;
mod animation;
mod box_shape;
mod canvas;
mod dock;
mod easing;
mod gesture;
mod gradient;
mod input;
mod layout_anim;
mod node;
mod overlay;
mod path;
mod path_template;
mod scheme;
mod scrollbar;
mod scrollbar_props;
mod shader;
mod svg;
mod svg_smil;
mod tree;

pub use access::{
    AccessExtra, AccessNodeData, AccessStates, AccessValue, Action, ActionData, AriaCurrent, Live,
    Orientation, Role, Toggled,
};
pub use animation::{
    ActiveAnimation, Animated, CompletionHandle, Interpolate, MotionCurve, animations_started,
    take_cancelled,
};
pub use box_shape::box_outline;
pub use canvas::{CanvasState, CustomHitTest, DrawCommand};
pub use dock::{DockLayout, DockSide, DockZone};
pub use easing::{Easing, PRESETS as EASINGS};
pub use gesture::{
    Gesture, GestureConfig, GestureKind, GesturePhase, GestureRecognizer, SwipeDirection,
    TouchPhase,
};
pub use gradient::{Gradient, GradientShape, GradientStop, Spread};
pub use input::{
    ChangedValue, ComposePhase, DispatchOutcome, InputEvent, Key, Modifiers, PointerButton,
    ScrollDelta, ctrl_shortcut,
};
pub use layout_anim::{LayoutAnim, LayoutProp};
pub use node::{
    Anchored, Blend, CalcLen, CalcSize, CaretShape, CellColor, ContentFit, CornerRadii, Cursor,
    FieldOverflow, FitContent, ImageState, InputMode, ItemExtent, Mask, Node, NodeId, NodeKind,
    NodeTransform, PaintProperties, ResolvedPalette, SCROLLBAR_GRAB_SLOP, SCROLLBAR_MARGIN,
    SCROLLBAR_MIN_LENGTH, SCROLLBAR_THICKNESS, ScrollViewState, Shadow, Shadows, SnapAlign,
    TerminalCell, TerminalPalette, TerminalState, TextAlign, TextFieldState, TextOptions, TextSpan,
    TextState, VerticalAlign, VirtualListState, WindowRegion,
};
pub use overlay::{AnchorBox, OverlayMeta, Placement};
pub use path::{HitShape, MorphMode, PathData, PathState, fit_transform, trim};
pub use path_template::PathTemplate;
pub use scheme::{Role as SchemeRole, Scheme, SchemeColors};
pub use scrollbar::{
    Arrow, Defaults as ScrollDefaults, FADE as SCROLLBAR_FADE, KEY_LINE, PAGE_DELAY, PAGE_FRACTION,
    PAGE_REPEAT, ScrollAxis, ScrollOrientation, ScrollbarMetrics, ScrollbarState, Scroller, Thumb,
    WHEEL_NOTCH,
};
pub use scrollbar_props::{
    Arrows, BarSide, Direction, Edge, HORIZONTAL_SIDE_NAMES, Kind as PropKind, LaneClick,
    MAX_MILLIS as MAX_SCROLL_MILLIS, Overrides, Prop as ScrollProp, Show, VERTICAL_SIDE_NAMES,
    Value as PropValue,
};
pub use shader::{
    FRAME_BLOCK_SIZE, Shader, ShaderError, ShaderMode, UniformKind, UniformValue, frame_block,
    shader_animations_pending,
};
pub use svg::{
    SvgBitmap, SvgClip, SvgDocument, SvgFill, SvgFonts, SvgGroup, SvgImage, SvgImages, SvgMask,
    SvgNode, SvgPaint, SvgPath, SvgPattern, SvgShadow, SvgState, SvgStroke,
};
pub use tree::{
    AccessLine, FIT_CONTENT_SLACK, FieldRows, FocusDirection, KEY_SCROLL_LINE, LayoutAnimError,
    LayoutChange, RowAnswer, RowQuery, ScrollAlign, ScrollChange, TextMeasure, TextPart, Touched,
    Tree, from_access_id, node_id_as_u64, to_access_id,
};
