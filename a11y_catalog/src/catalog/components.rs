use crate::app::theme::Theme;
use michiu::prelude::*;

pub mod button;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
pub enum ComponentType {
    Button,
}

pub(crate) fn create_component_entity(comp_type: ComponentType) -> Entity {
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
