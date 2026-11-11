use crate::app::theme::Theme;
use iu_ui::{components::*, simple_theme::SimpleBlueTheme, *};
use michiu::{prelude::*, ui::a11y};

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
        ComponentType::Button => wrapper([
            section(
                "Size",
                [
                    SimpleButton::new("Extra Small")
                        .size(ButtonSize::Xs)
                        .build(&theme),
                    SimpleButton::new("Small")
                        .size(ButtonSize::Sm)
                        .build(&theme),
                    SimpleButton::new("Medium")
                        .size(ButtonSize::Md)
                        .build(&theme),
                    SimpleButton::new("Large")
                        .size(ButtonSize::Lg)
                        .build(&theme),
                ],
            ),
            section("Solid", [SimpleButton::new("Solid").build(&theme)]),
            section(
                "Secondary",
                [SimpleButton::new("Secondary")
                    .variant(ButtonVariant::Secondary)
                    .build(&theme)],
            ),
            section(
                "Outline",
                [SimpleButton::new("Outline")
                    .variant(ButtonVariant::Outline)
                    .build(&theme)],
            ),
            section(
                "Ghost",
                [SimpleButton::new("Ghost")
                    .variant(ButtonVariant::Ghost)
                    .build(&theme)],
            ),
            section(
                "Disabled",
                [
                    SimpleButton::new("Solid")
                        .disabled(true)
                        .variant(ButtonVariant::Solid)
                        .build(&theme),
                    SimpleButton::new("Outline")
                        .disabled(true)
                        .variant(ButtonVariant::Outline)
                        .build(&theme),
                    SimpleButton::new("Secondary")
                        .disabled(true)
                        .variant(ButtonVariant::Secondary)
                        .build(&theme),
                    SimpleButton::new("Ghost")
                        .disabled(true)
                        .variant(ButtonVariant::Ghost)
                        .build(&theme),
                ],
            ),
        ]),
        ComponentType::ButtonGroup => wrapper([
            section(
                "Size",
                [
                    simple_button_group!["Extra", "|", "Small"]
                        .size(ButtonGroupSize::Xs)
                        .build(&theme),
                    simple_button_group!["Small", "|", "Small"]
                        .size(ButtonGroupSize::Sm)
                        .build(&theme),
                    simple_button_group!["Medium", "|", "Medium"]
                        .size(ButtonGroupSize::Md)
                        .build(&theme),
                    simple_button_group!["Large", "|", "Large"]
                        .size(ButtonGroupSize::Lg)
                        .build(&theme),
                ],
            ),
            section(
                "Solid",
                [simple_button_group!["Solid", "|", "1", "2", "|", "3"]
                    .variant(ButtonGroupVariant::Solid)
                    .build(&theme)],
            ),
            section(
                "Outline",
                [simple_button_group!["Outline", "|", "1", "2", "|", "3"]
                    .variant(ButtonGroupVariant::Outline)
                    .build(&theme)],
            ),
            section(
                "Ghost",
                [simple_button_group!["Ghost", "|", "1", "2", "|", "3"]
                    .variant(ButtonGroupVariant::Ghost)
                    .build(&theme)],
            ),
            section(
                "Column",
                [
                    simple_button_group!["Extra Small", "|", "1", "2", "|", "3"]
                        .size(ButtonGroupSize::Xs)
                        .direction(FlexDirection::Column)
                        .build(&theme),
                    simple_button_group!["Small", "|", "1", "2", "|", "3"]
                        .size(ButtonGroupSize::Sm)
                        .direction(FlexDirection::Column)
                        .build(&theme),
                    simple_button_group!["Medium", "|", "1", "2", "|", "3"]
                        .size(ButtonGroupSize::Md)
                        .direction(FlexDirection::Column)
                        .build(&theme),
                    simple_button_group!["Large", "|", "1", "2", "|", "3"]
                        .size(ButtonGroupSize::Lg)
                        .direction(FlexDirection::Column)
                        .build(&theme),
                ],
            ),
        ]),
        ComponentType::Icon => wrapper([
            section(
                "Icon",
                [
                    include_simple_icon!("../../src/assets/search.svg")
                        .category(IconCategory::button("Search"))
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .category(IconCategory::button("Search"))
                        .disabled(true)
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .category(IconCategory::button("Search"))
                        .variant(IconVariant::Accent)
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .category(IconCategory::button("Search"))
                        .disabled(true)
                        .variant(IconVariant::Accent)
                        .build(&theme),
                ],
            ),
            section(
                "Icon",
                [
                    include_simple_icon!("../../src/assets/search.svg")
                        .size(IconSize::Xs)
                        .category(IconCategory::button("Search"))
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .size(IconSize::Md)
                        .variant(IconVariant::Accent)
                        .category(IconCategory::button("Search"))
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .size(IconSize::Md)
                        .category(IconCategory::image("Search"))
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .size(IconSize::Md)
                        .variant(IconVariant::Accent)
                        .category(IconCategory::image("Search"))
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .size(IconSize::Md)
                        .category(IconCategory::Decoration)
                        .build(&theme),
                    include_simple_icon!("../../src/assets/search.svg")
                        .size(IconSize::Md)
                        .variant(IconVariant::Accent)
                        .category(IconCategory::Decoration)
                        .build(&theme),
                ],
            ),
        ]),
        ComponentType::Input => wrapper([section("SimpleInput", [text("SimpleInput")])]),
        ComponentType::List => wrapper([section("SimpleList", [text("SimpleList")])]),
        ComponentType::Modal => wrapper([section("SimpleModal", [text("SimpleModal")])]),
        ComponentType::Popover => wrapper([section("SimplePopover", [text("SimplePopover")])]),
        ComponentType::Tab => wrapper([section("SimpleTab", [text("SimpleTab")])]),
        ComponentType::Toggle => wrapper([section("SimpleToggle", [text("SimpleToggle")])]),
    }
}

fn wrapper<I>(e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(40.0).p(20.0).r(3.0).size_max()).children(e)
}

fn section<I>(s: &'static str, e: I) -> Entity
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
