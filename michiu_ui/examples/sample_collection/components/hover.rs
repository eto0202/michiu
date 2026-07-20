use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
use michiu_ui::prelude::*;
use std::time::Duration;

pub fn container() -> Element {
    v_flex(ts().gap(28.0).p(16.0)).children([section_hover(), section_hover_transition()])
}

fn section_style() -> ThisStyle {
    ts().grow().gap(16.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap(16.0)
}

fn item_style() -> ThisStyle {
    ts().p(12.0)
        .r(4.0)
        .size((200.0, 100.0))
        .justify_center()
        .items_center()
        .bg_color(dynamic(|t: &Theme| t.background))
        .border_dashed(1.0)
        .border_color(dynamic(|t: &Theme| t.border))
}

fn section_hover() -> Element {
    let item_1 = div(item_style().hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover))))
        .label("Background", label_style());

    let item_2 = div(item_style().hovered(
        ts().border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border_hover)),
    ))
    .label("Border", label_style());

    let item_3 =
        div(item_style().hovered(ts().transform_scale(1.05, 1.05))).label("Scale", label_style());

    let item_4 =
        div(item_style().hovered(ts().transform_scale(0.95, 0.95))).label("Scale", label_style());

    let wrapper = h_flex(wrapper_style()).children([item_1, item_2, item_3, item_4]);

    v_flex(section_style()).children([section_title("Hover"), wrapper])
}

fn transition_item(ms: u64, label_text: &'static str) -> Element {
    div(item_style()
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover)))
        .trans_bg_color(Duration::from_millis(ms), AnimationCurve::EaseInOutQuad))
    .label(label_text, label_style())
}

fn section_hover_transition() -> Element {
    let wrapper = h_flex(wrapper_style()).children([
        transition_item(150, "150 ms"),
        transition_item(300, "300 ms"),
        transition_item(450, "450 ms"),
        transition_item(600, "600 ms"),
    ]);

    v_flex(section_style()).children([section_title("Hover Transition"), wrapper])
}
