use crate::app::{
    theme::Theme,
    utils::{label_style, section_title},
};
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(24.0).p(16.0)).children([style_container()])
}

fn section_style() -> ThisStyle {
    ts().grow().gap(14.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap(14.0).p(6.0)
}

fn item(label: &'static str, style: ThisStyle) -> Element {
    div(style
        .p(12.0)
        .r(4.0)
        .size((160.0, 70.0))
        .justify_center()
        .items_center()
        .outline_color(dynamic(|t: &Theme| t.border))
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover))))
    .label(label, label_style())
}

fn style_container() -> Element {
    let solid = item("solid", ts().outline_solid(2.0));
    let dashed = item("dashed", ts().outline_dashed(2.0));
    let dotted = item("dotted", ts().outline_dotted(2.0));
    let double = item("double", ts().outline_double(4.0));

    v_flex(section_style()).children([
        section_title("Border Style"),
        h_flex(wrapper_style()).children([solid, dashed, dotted, double]),
    ])
}

fn weight_container() -> Element {
    v_flex(section_style()).children([section_title("Border Weight"), h_flex(wrapper_style())])
}

fn length_container() -> Element {
    v_flex(section_style()).children([section_title("Border Lengths"), h_flex(wrapper_style())])
}

fn align_container() -> Element {
    v_flex(section_style()).children([
        section_title("Border Alignment (50% Length)"),
        h_flex(wrapper_style()),
    ])
}

fn color_container() -> Element {
    v_flex(section_style()).children([section_title("Border Color"), h_flex(wrapper_style())])
}
