use crate::{components, theme::Theme};
use michiu_ui::prelude::*;

pub fn create_root(t: ReadSignal<Theme>) -> Element {
    v_flex(move || ts().size_full().bg_color(t.get().background))
        .provide(t)
        .children([
            components::header(),
            h_flex(ts().size_full().grow())
                .children([components::sidebar(), components::main_area()]),
        ])
}
