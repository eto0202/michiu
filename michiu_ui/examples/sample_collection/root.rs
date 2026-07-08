use michiu_ui::{Element, prelude::*};

use crate::{components, theme::Theme};

pub fn create_root(cx: &mut Context) -> Element {
    build_ui(cx, || {
        v_flex(ts().size_full().bg_color(Theme::DARK_BG))
            .child(components::header())
            .child(
                h_flex(ts().size_full().grow())
                    .children([components::sidebar(), components::main_area()]),
            )
    })
}
