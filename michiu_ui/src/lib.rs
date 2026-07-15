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
        signal::{
            ReadSignal, SignalId, WriteSignal, create_signal, use_provided, use_provided_setter,
        },
        style::{AnimationCurve, KeyframeAnimation, PlaybackCount, ThisStyle},
        types::{
            Backdrop, BorderAlignment, BorderStyle, Color, CursorIcon, Display, DragPayload,
            DropTarget, ElementState, LayoutPoint, LayoutRect, LayoutSize, Modifiers, MouseButton,
            PointerEvents, ScrollbarDisplay, ScrollbarMode, ScrollbarStyle, Transform, Transition,
            hex, hsl, hsla, rgb, rgba,
        },
        utils::{
            auto, block_box, blur, div, div_d, div_n, dynamic, get_win32_clipboard, grid_box,
            h_flex, h_flex_d, hidden_box, img, input, input_area, input_area_d, input_d, offset,
            pct, px, set_win32_clipboard, shadow, spread, text, text_d, ts, v_flex, v_flex_d,
            video, webview2,
        },
        webview2::WebView2Contents,
    };
}
