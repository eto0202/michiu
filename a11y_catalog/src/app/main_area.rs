use crate::{app::ComponentType, catalog::create_component_entity};
use michiu::{prelude::*, ui::a11y};
use strum::IntoEnumIterator;

pub fn main_area() -> Entity {
    let main_contents = v_flex(ii().size_full().overflow_hidden().p(20.0)).tag::<a11y::AMain>();

    let mut children = Vec::new();
    for comp_type in ComponentType::iter() {
        let child_el = create_component_entity(comp_type);

        let styled_child = flex_d(move |active: &ComponentType| {
            if *active == comp_type {
                ii().size_full().flex()
            } else {
                ii().hidden()
            }
        })
        .child(child_el);

        children.push(styled_child);
    }

    main_contents.children(children)
}
