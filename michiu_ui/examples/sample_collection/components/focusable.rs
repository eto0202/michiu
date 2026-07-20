use std::time::Duration;
use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(28.0).p(16.0)).children([section_inherit(), section_self(), section_within()])
}

fn section_style() -> ThisStyle {
    ts().grow().gap(16.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap(16.0)
}

fn item(label: &'static str, style: ThisStyle) -> Element {
    div(style
        .p(12.0)
        .r(4.0)
        .justify_center()
        .items_center()
        .border_solid(1.0)
        .border_color(dynamic(|t: &Theme| t.border))
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover))))
    .label(label, label_style())
}

fn section_inherit() -> Element {
    let item_1 = item("Focus 3", ts().focusable_inherit_keyboard());
    let item_2 = item("Focus 4", ts().focusable_inherit_keyboard());
    let item_3 = item("Focus 5", ts().focusable_inherit_keyboard());
    let item_4 = item("Focus 6", ts().focusable_inherit_keyboard());

    let wrapper = h_flex(wrapper_style()).children([item_1, item_2, item_3, item_4]);

    let parent_focused = section_style().focused(
        ts().outline_solid(1.0)
            .outline_offset(2.0)
            .outline_color(dynamic(|t: &Theme| t.text)),
    );

    v_flex(parent_focused).children([section_title("Keyboard & Inherit"), wrapper])
}

fn section_self() -> Element {
    let item_1 = item(
        "Focus 7",
        ts().focusable_self_keyboard().focused(
            ts().outline_dashed(2.0)
                .outline_offset(2.0)
                .outline_color(dynamic(|t: &Theme| t.primary)),
        ),
    );
    let item_2 = item(
        "Focus 8",
        ts().focusable_self_keyboard()
            .focused(ts().bg_color(dynamic(|t: &Theme| t.border_hover))),
    );
    let item_3 = item(
        "Focus 9",
        ts().focusable_self_keyboard()
            .trans_transform(Duration::from_millis(300), AnimationCurve::EaseInOutQuad)
            .focused(ts().transform_scale(1.1, 1.1)),
    );
    let item_4 = item(
        "Focus 10",
        ts().focusable_self_keyboard().focused(
            ts().box_shadow(
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

fn section_within() -> Element {
    let item_1 = item(
        "Focus 12",
        ts().focusable_self_keyboard().focused(
            ts().outline_dashed(2.0)
                .outline_offset(2.0)
                .outline_color(dynamic(|t: &Theme| t.primary)),
        ),
    );

    let item_2 = item("Focus 12", ts().focusable_inherit_keyboard());

    let wrapper = h_flex(
        ts().w_full()
            .gap(16.0)
            .p(6.0)
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .focused(ts().bg_color(dynamic(|t: &Theme| t.primary)))
            .focus_within(
                ts().outline_solid(1.0)
                    .outline_offset(2.0)
                    .outline_color(dynamic(|t: &Theme| t.text)),
            ),
    )
    .children([item_1, item_2]);

    v_flex(section_style()).children([section_title("Keyboard & Focus Within"), wrapper])
}
