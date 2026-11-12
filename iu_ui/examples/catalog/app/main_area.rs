use crate::{
    app::{ComponentType, sidebar, theme::Theme},
    component_type::create_component_entity,
};
use michiu::{prelude::*, ui::a11y};
use strum::IntoEnumIterator;

pub fn main_area() -> Entity {
    let main_contents = v_flex(ii().size_full().p(20.0)).tag::<a11y::AMain>();

    let mut children = Vec::new();
    for comp_type in ComponentType::iter() {
        let styled_child = flex_d(move |active: &ComponentType| {
            let t = use_provided::<Theme>().get();
            if *active == comp_type {
                sidebar::scrollbar(&t)
                    .size_full()
                    .flex()
                    .overflow_scroll()
                    .border_dashed(1.0)
                    .border_color(t.border)
            } else {
                ii().hidden()
            }
        })
        .child_d(move |t: &Theme| create_component_entity(comp_type, t.is_dark));

        children.push(styled_child);
    }

    main_contents.children(children)
}
