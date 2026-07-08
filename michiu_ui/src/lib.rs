mod bitmap;
mod context;
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
        context::Context,
        element::build_ui,
        input::InputContents,
        signal::create_signal,
        style::{AnimationCurve, KeyframeAnimation, PlaybackCount, ThisStyle},
        types::{
            Backdrop, BorderAlignment, BorderStyle, Color, CursorIcon, Display, ElementState,
            LayoutPoint, LayoutRect, LayoutSize, Modifiers, MouseButton, PointerEvents,
            ScrollbarDisplay, ScrollbarMode, ScrollbarStyle, Transform, Transition, hex, hsl, hsla,
            rgb, rgba,
        },
        utils::{
            auto, block_box, blur, div, div_n, get_win32_clipboard, grid_box, h_flex, hidden_box,
            img, input, input_area, offset, pct, px, set_win32_clipboard, shadow, spread, text, ts,
            v_flex, video, webview2,
        },
        webview2::WebView2Contents,
    };
}
