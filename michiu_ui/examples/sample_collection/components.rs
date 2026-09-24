pub mod background;
pub mod border;
pub mod button;
pub mod color;
pub mod css;
pub mod cursor;
pub mod div;
pub mod draggable;
pub mod dropdown;
pub mod external_visual;
pub mod flexbox;
pub mod focusable;
pub mod hover;
pub mod image;
pub mod input;
pub mod input_area;
pub mod list;
pub mod outline;
pub mod resizable;
pub mod scrollbar;
pub mod stress_test;

use crate::app::theme::Theme;
use michiu_ui::prelude::*;

pub fn section_title(text_val: &'static str) -> Element {
    text(text_val).style(
        ts().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(14.0)
            .font_weight(400),
    )
}

pub fn label_style() -> ThisStyle {
    ts().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(16.0)
        .font_weight(400)
}
