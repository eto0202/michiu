#![allow(
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::struct_excessive_bools
)]

mod bitmap;
mod context;
mod dss;
mod element;
mod input;
mod key;
mod renderer;
mod signal;
mod style;
mod transition;
mod types;
mod utils;
mod webview2;

#[allow(unused)]
pub use bitmap::*;
pub use context::*;
pub use dss::*;
pub use element::*;
pub use input::*;
pub use key::*;
#[allow(unused)]
pub use renderer::*;
pub use signal::*;
pub use style::*;
#[allow(unused)]
pub use transition::*;
pub use types::*;
pub use utils::*;
pub use webview2::*;

pub mod prelude {
    pub use crate::{
        bitmap::PropertyList,
        context::{Context, EntityId},
        element::{Element, build_ui},
        input::InputContents,
        pipeline::{TickType, UserAction},
        signal::{ReadSignal, SignalId, WriteSignal},
        style::ThisStyle,
        types::{
            AnimationCurve, Backdrop, BorderAlignment, BorderStyle, Color, CursorIcon, Display,
            DndDragPayload, DndDropTarget, ElementState, KeyframeAnimation, LayoutPoint,
            LayoutRect, LayoutSize, Modifiers, MouseButton, PlaybackCount, PointerEvents,
            ScrollbarDisplay, ScrollbarMode, ScrollbarStyle, Transform, Transition,
        },
        utils::{
            auto, block_box, blur, create_signal, div, div_d, div_n, dynamic, external_texture,
            get_win32_clipboard, grid_box, h_flex, h_flex_d, hex, hidden_box, hsl, hsla, input,
            input_area, input_area_d, input_d, offset, pct, px, rgb, rgba, set_win32_clipboard,
            shadow, spread, text, text_d, ts, use_provided, use_provided_setter, v_flex, v_flex_d,
            webview2,
        },
        webview2::WebView2Contents,
    };
}
