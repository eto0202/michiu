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
