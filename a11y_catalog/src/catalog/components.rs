use crate::app::theme::Theme;
use michiu::prelude::*;

mod simple_accordion;
mod simple_button;
mod simple_checkbox;
mod simple_group_button;
mod simple_input;
mod simple_list;
mod simple_modal;
mod simple_popover;
mod simple_slider;
mod simple_tab;
mod simple_toggle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
#[allow(clippy::enum_variant_names)]
pub enum ComponentType {
    Button,
    Input,
    List,
    Modal,
    Popover,
    Tab,
    Toggle,
}

pub(crate) fn create_component_entity(comp_type: ComponentType) -> Entity {
    match comp_type {
        ComponentType::Button => wrapper([
            section([
                simple_button::button_1_light(),
                simple_button::button_1_dark(),
                simple_button::button_2_light(),
                simple_button::button_2_dark(),
            ]),
            section([
                simple_group_button::group_button_1_light(),
                simple_group_button::group_button_1_dark(),
            ]),
        ]),
        ComponentType::Input => wrapper([section([text("SimpleInput")])]),
        ComponentType::List => wrapper([section([text("SimpleList")])]),
        ComponentType::Modal => wrapper([section([text("SimpleModal")])]),
        ComponentType::Popover => wrapper([section([text("SimplePopover")])]),
        ComponentType::Tab => wrapper([section([text("SimpleTab")])]),
        ComponentType::Toggle => wrapper([section([text("SimpleToggle")])]),
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
            .r(3.0)
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
