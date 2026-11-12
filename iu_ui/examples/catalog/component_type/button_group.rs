use crate::component_type::{section, wrapper};
use iu_ui::*;
use michiu::prelude::*;

pub(crate) fn container(theme: &SimpleBlueTheme) -> Entity {
    wrapper([
        section(
            "Size",
            [
                simple_button_group!["Extra", "|", "Small"]
                    .size(ButtonGroupSize::Xs)
                    .build(theme),
                simple_button_group!["Small", "|", "Small"]
                    .size(ButtonGroupSize::Sm)
                    .build(theme),
                simple_button_group!["Medium", "|", "Medium"]
                    .size(ButtonGroupSize::Md)
                    .build(theme),
                simple_button_group!["Large", "|", "Large"]
                    .size(ButtonGroupSize::Lg)
                    .build(theme),
            ],
        ),
        section(
            "Solid",
            [simple_button_group!["Solid", "|", "1", "2", "|", "3"]
                .variant(ButtonGroupVariant::Solid)
                .build(theme)],
        ),
        section(
            "Outline",
            [simple_button_group!["Outline", "|", "1", "2", "|", "3"]
                .variant(ButtonGroupVariant::Outline)
                .build(theme)],
        ),
        section(
            "Ghost",
            [simple_button_group!["Ghost", "|", "1", "2", "|", "3"]
                .variant(ButtonGroupVariant::Ghost)
                .build(theme)],
        ),
        section(
            "Column",
            [
                simple_button_group!["Extra Small", "|", "1", "2", "|", "3"]
                    .size(ButtonGroupSize::Xs)
                    .direction(FlexDirection::Column)
                    .build(theme),
                simple_button_group!["Small", "|", "1", "2", "|", "3"]
                    .size(ButtonGroupSize::Sm)
                    .direction(FlexDirection::Column)
                    .build(theme),
                simple_button_group!["Medium", "|", "1", "2", "|", "3"]
                    .size(ButtonGroupSize::Md)
                    .direction(FlexDirection::Column)
                    .build(theme),
                simple_button_group!["Large", "|", "1", "2", "|", "3"]
                    .size(ButtonGroupSize::Lg)
                    .direction(FlexDirection::Column)
                    .build(theme),
            ],
        ),
    ])
}
