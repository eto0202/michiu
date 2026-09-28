#![allow(
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::struct_excessive_bools
)]

//! # `michiu_ui`
//! `michiu_ui` is a GUI library for Windows.
//!
//! Please note that since this is still under development, breaking changes may occur without prior notice.
//!
//!
//!
//!
//!
//!

mod bitmap;
mod context;
mod element;
mod external;
mod option;
mod renderer;
mod signal;
mod style;
mod tag;
mod types;
mod utils;

pub use a11y;
pub use bitmap::*;
pub use context::*;
pub use element::*;
pub use external::*;
pub use option::*;
pub use renderer::*;
pub use signal::*;
pub use soa::MichiuSoA;
pub use style::*;
pub use tag::*;
pub use types::*;
pub use utils::*;

pub mod prelude {
    pub const A11Y_NO_LABEL: crate::Prop<Option<std::borrow::Cow<'static, str>>> =
        crate::Prop::None;
    pub use crate::{
        bitmap::PropertyList,
        context::{Context, EntityId, MichiuInspector},
        dnd::{DndDragPayload, DndDropTarget},
        element::{Element, build_ui},
        input::InputContents,
        pipeline::{TickType, UserAction},
        renderer::ComposedRenderer,
        scrollbar::{ScrollbarDisplay, ScrollbarMode, ScrollbarStyle},
        signal::{ReadSignal, SignalId, WriteSignal},
        style::ThisStyle,
        types::{
            AnimationCurve, Backdrop, BorderAlignment, BorderStyle, Color, CursorIcon, Display,
            ElementState, InteractionState, KeyframeAnimation, LayoutPoint, LayoutRect, LayoutSize,
            Modifiers, MouseButton, PlaybackCount, PointerEvents, Transform, Transition,
        },
        utils::{
            auto, block_box, blur, create_signal, div, div_d, div_n, dynamic, external_texture,
            get_win32_clipboard, grid_box, h_flex, h_flex_d, hex, hidden_box, hsl, hsla, input,
            input_area, input_area_d, input_d, offset, pct, px, rgb, rgba, set_win32_clipboard,
            shadow, spread, text, text_d, ts, use_provided, use_provided_setter, v_flex, v_flex_d,
        },
    };
    pub use accesskit::Role as role;
}
