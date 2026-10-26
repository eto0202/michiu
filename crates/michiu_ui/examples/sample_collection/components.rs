pub mod accessibility;
pub mod background;
pub mod border;
pub mod button;
pub mod color;
pub mod context_menu;
pub mod css;
pub mod cursor;
pub mod div;
pub mod draggable;
pub mod external_texture;
pub mod external_visual;
pub mod flexbox;
pub mod focusable;
pub mod gradient;
pub mod hover;
pub mod input;
pub mod outline;
pub mod resizable;
pub mod scrollbar;
pub mod size;
pub mod stress_test;
pub mod transform;

use crate::app::theme::Theme;
use michiu_ui::prelude::*;

pub fn section_title(text_val: &'static str) -> Entity {
    text(text_val).modify(
        ii().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(14.0)
            .font_weight(400),
    )
}

pub fn label_style() -> Modifier {
    ii().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(16.0)
        .font_weight(400)
}
