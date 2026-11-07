use crate::{
    app::theme::Theme,
    catalog::{
        a11y_theme::SimpleTheme1,
        components::{simple_button::SimpleButton, simple_button_group::SimpleButtonGroup},
        core::MichiuWidget,
    },
};
use michiu::prelude::*;

mod simple_accordion;
mod simple_button;
mod simple_button_group;
mod simple_checkbox;
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
                SimpleButton::new("Solid").build(&SimpleTheme1::light()),
                SimpleButton::new("Solid").build(&SimpleTheme1::dark()),
                SimpleButton::new("Outline")
                    .variant(simple_button::ButtonVariant::Outline)
                    .build(&SimpleTheme1::light()),
                SimpleButton::new("Outline")
                    .variant(simple_button::ButtonVariant::Outline)
                    .build(&SimpleTheme1::dark()),
                SimpleButton::new("Ghost")
                    .variant(simple_button::ButtonVariant::Ghost)
                    .build(&SimpleTheme1::light()),
                SimpleButton::new("Ghost")
                    .variant(simple_button::ButtonVariant::Ghost)
                    .build(&SimpleTheme1::dark()),
            ]),
            section([
                SimpleButtonGroup::default().build(&SimpleTheme1::light()),
                SimpleButtonGroup::default().build(&SimpleTheme1::dark()),
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
