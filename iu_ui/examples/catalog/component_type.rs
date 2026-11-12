use crate::app::theme::Theme;
use iu_ui::simple_theme::SimpleBlueTheme;
use michiu::{prelude::*, ui::a11y};

mod button;
mod button_group;
mod icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
#[allow(clippy::enum_variant_names)]
pub enum ComponentType {
    Button,
    ButtonGroup,
    Input,
    List,
    Modal,
    Popover,
    Tab,
    Toggle,
    Icon,
}

pub(crate) fn create_component_entity(comp_type: ComponentType, is_dark: bool) -> Entity {
    let theme = if is_dark {
        SimpleBlueTheme::dark()
    } else {
        SimpleBlueTheme::light()
    };
    match comp_type {
        ComponentType::Button => button::container(&theme),
        ComponentType::ButtonGroup => button_group::container(&theme),
        ComponentType::Icon => icon::container(&theme),
        ComponentType::Input => wrapper([section("SimpleInput", [text("SimpleInput")])]),
        ComponentType::List => wrapper([section("SimpleList", [text("SimpleList")])]),
        ComponentType::Modal => wrapper([section("SimpleModal", [text("SimpleModal")])]),
        ComponentType::Popover => wrapper([section("SimplePopover", [text("SimplePopover")])]),
        ComponentType::Tab => wrapper([section("SimpleTab", [text("SimpleTab")])]),
        ComponentType::Toggle => wrapper([section("SimpleToggle", [text("SimpleToggle")])]),
    }
}

pub(crate) fn wrapper<I>(e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(40.0).p(20.0).r(3.0).size_max()).children(e)
}

pub(crate) fn section<I>(s: &'static str, e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(20.0)).children([
        section_header(s),
        h_flex(
            ii().justify_center()
                .items_start()
                .gap(30.0)
                .r(3.0)
                .p(40.0)
                .border_solid(1.0)
                .border_color(dynamic(|t: &Theme| t.border)),
        )
        .children(e),
    ])
}

fn section_header(s: &'static str) -> Entity {
    text(s)
        .tag::<a11y::AHeading2>()
        .modify(ii().text_color(dynamic(|t: &Theme| t.text)).font_size(18.0))
}
