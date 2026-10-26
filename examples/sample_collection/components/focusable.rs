use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
use michiu::prelude::*;
use std::time::Duration;

pub fn container() -> Entity {
    v_flex(ii().gap(28.0).p(16.0)).children([section_inherit(), section_self(), section_within()])
}

fn section_style() -> Modifier {
    ii().grow().gap(16.0)
}

fn wrapper_style() -> Modifier {
    ii().w_full().gap(16.0)
}

fn item(label: &'static str, style: Modifier) -> Entity {
    flex(
        style
            .p(12.0)
            .r(4.0)
            .justify_center()
            .items_center()
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .hovered(ii().bg_color(dynamic(|t: &Theme| t.background_hover))),
    )
    .label(label, label_style())
}

fn section_inherit() -> Entity {
    let item_1 = item("Focus 3", ii().focusable_inherit_keyboard());
    let item_2 = item("Focus 4", ii().focusable_inherit_keyboard());
    let item_3 = item("Focus 5", ii().focusable_inherit_keyboard());
    let item_4 = item("Focus 6", ii().focusable_inherit_keyboard());

    let wrapper = h_flex(wrapper_style()).children([item_1, item_2, item_3, item_4]);

    let parent_focused = section_style().focused(
        ii().outline_solid(1.0)
            .outline_offset(2.0)
            .outline_color(dynamic(|t: &Theme| t.text)),
    );

    v_flex(parent_focused).children([section_title("Keyboard & Inherit"), wrapper])
}

fn section_self() -> Entity {
    let item_1 = item(
        "Focus 7",
        ii().focusable_self_keyboard().focused(
            ii().outline_dashed(2.0)
                .outline_offset(2.0)
                .outline_color(dynamic(|t: &Theme| t.accent)),
        ),
    );
    let item_2 = item(
        "Focus 8",
        ii().focusable_self_keyboard()
            .focused(ii().bg_color(dynamic(|t: &Theme| t.border_hover))),
    );
    let item_3 = item(
        "Focus 9",
        ii().focusable_self_keyboard()
            .trans_transform(Duration::from_millis(300), AnimationCurve::EaseInOutQuad)
            .focused(ii().transform_scale(1.1, 1.1)),
    );
    let item_4 = item(
        "Focus 10",
        ii().focusable_self_keyboard().focused(
            ii().box_shadow(
                shadow()
                    .blur(8.0)
                    .offset(0.0)
                    .spread(1.0)
                    .color(Color::BLACK),
            ),
        ),
    );

    let wrapper = h_flex(wrapper_style()).children([item_1, item_2, item_3, item_4]);

    v_flex(section_style()).children([section_title("Keyboard & SelfStyle"), wrapper])
}

fn section_within() -> Entity {
    let item_1 = item(
        "Focus 12",
        ii().focusable_self_keyboard().focused(
            ii().outline_dashed(2.0)
                .outline_offset(2.0)
                .outline_color(dynamic(|t: &Theme| t.accent)),
        ),
    );

    let item_2 = item("Focus 12", ii().focusable_inherit_keyboard());

    let wrapper = h_flex(
        ii().w_full()
            .gap(16.0)
            .p(6.0)
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .focused(ii().bg_color(dynamic(|t: &Theme| t.accent)))
            .focused_within(
                ii().outline_solid(1.0)
                    .outline_offset(2.0)
                    .outline_color(dynamic(|t: &Theme| t.text)),
            ),
    )
    .children([item_1, item_2]);

    v_flex(section_style()).children([section_title("Keyboard & Focus Within"), wrapper])
}
