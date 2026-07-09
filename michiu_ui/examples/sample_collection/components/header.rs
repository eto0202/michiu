use crate::{
    components::{button, input},
    theme::Theme,
};
pub use michiu_ui::prelude::*;

pub fn header() -> Element {
    h_flex_c(|t: &Theme| {
        ts().w_full()
            .height(40.0)
            .shrink_0()
            .justify_center()
            .items_center()
            .border_bottom(BorderStyle::Solid, 1.0)
            .border_color(t.border)
    })
    .children([input::search_box(), button::toggle_btn()])
}
