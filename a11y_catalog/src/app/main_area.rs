use crate::{
    app::{ComponentType, theme::Theme},
    components::button,
};
use michiu::prelude::*;
use michiu::ui::a11y;
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

fn create_component_entity(comp_type: ComponentType) -> Entity {
    match comp_type {
        ComponentType::Button => {
            wrapper([section([button::button_1_light(), button::button_1_dark()])])
        }
    }
}

fn wrapper<I>(e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(
        ii().size_full()
            .gap(20.0)
            .p(20.0)
            .r(4.0)
            .overflow_hidden()
            .border_dashed(1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .children(e)
}

fn section<I>(e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    h_flex(ii().gap(20.0)).children(e)
}
