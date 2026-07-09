use crate::{components::scrollbar, theme::Theme};
pub use michiu_ui::prelude::*;

pub fn sidebar() -> Element {
    v_flex_c(|t: &Theme| {
        scrollbar::scrollbar_y(t)
            .h_full()
            .width(150.0)
            .shrink_0()
            .overflow_y_scroll()
            .border_right(BorderStyle::Solid, 1.0)
            .border_color(t.border)
    })
    .children([
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
        sidebar_item(),
    ])
}

fn sidebar_item() -> Element {
    h_flex_c(|t: &Theme| {
        ts().justify_center()
            .items_center()
            .hovered(ts().bg_color(t.background_hover))
    })
    .label("Item", move || {
        ts().p(10.0).text_color(consume(|t: &Theme| t.text))
    })
}
