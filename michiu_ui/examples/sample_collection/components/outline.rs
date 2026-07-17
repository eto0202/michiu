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

fn base() -> ThisStyle {
    ts().p(12.0)
        .r(4.0)
        .size((160.0, 70.0))
        .justify_center()
        .items_center()
        .outline_color(dynamic(|t: &Theme| t.border))
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover)))
}

fn style_container() -> Element {
    let solid = div(base().outline_solid(2.0).outline_offset(4.0)).label("solid", label_style());

    v_flex(section_style()).children([
        section_title("Border Style"),
        h_flex(wrapper_style()).children([solid]),
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
