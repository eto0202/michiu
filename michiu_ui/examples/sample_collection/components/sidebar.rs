use michiu_ui::Element;
pub use michiu_ui::prelude::*;

use crate::{components, theme::Theme};

pub fn sidebar() -> Element {
    let item_style = ts()
        .justify_center()
        .items_center()
        .hovered(ts().bg_color(hsl(220.0, 0.12, 0.15)));

    v_flex(
        components::scrollbar_y()
            .h_full()
            .width(150.0)
            .shrink_0()
            .overflow_y_scroll()
            .border_right(BorderStyle::Solid, 1.0)
            .border_color(Theme::DARK_BORDER),
    )
    .children([
        sidebar_item(item_style.clone()),
        sidebar_item(item_style.clone()),
        sidebar_item(item_style.clone()),
        sidebar_item(item_style.clone()),
        sidebar_item(item_style.clone()),
    ])
}

fn sidebar_item(style: ThisStyle) -> Element {
    h_flex(style).label("Item", ts().p(10.0).text_color(Theme::DARK_TEXT))
}
