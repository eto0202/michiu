use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(24.0).p(16.0)).children([
        style_container(),
        weight_container(),
        length_container(),
        align_container(),
        color_container(),
    ])
}

fn section_style() -> ThisStyle {
    ts().grow().gap(14.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap(14.0)
}

fn base(change: bool) -> ThisStyle {
    let s = ts()
        .p(12.0)
        .r(4.0)
        .size((170.0, 75.0))
        .justify_center()
        .items_center()
        .p(10.0)
        .r(4.0)
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover)));

    if change {
        s
    } else {
        s.border_color(dynamic(|t: &Theme| t.border))
    }
}

fn style_container() -> Element {
    let solid = div(base(false).border_solid(2.0)).label("solid", label_style());
    let dashed = div(base(false).border_dashed(2.0)).label("dashed", label_style());
    let dotted = div(base(false).border_dotted(2.0)).label("dotted", label_style());
    let double = div(base(false).border_double(4.0)).label("double", label_style());

    v_flex(section_style()).children([
        section_title("Border Style"),
        h_flex(wrapper_style()).children([solid, dashed, dotted, double]),
    ])
}

fn weight_container() -> Element {
    let one = div(base(false).border_solid(1.0)).label("1.0 px", label_style());
    let two = div(base(false).border_solid(2.0)).label("2.0 px", label_style());
    let four = div(base(false).border_solid(4.0)).label("4.0 px", label_style());
    let eight = div(base(false).border_solid(8.0)).label("8.0 px", label_style());

    v_flex(section_style()).children([
        section_title("Border Weight"),
        h_flex(wrapper_style()).children([one, two, four, eight]),
    ])
}

fn length_container() -> Element {
    let top_only = div(base(false)
        .border_solid(2.0)
        .border_lengths((1.0, 0.0, 0.0, 0.0)))
    .label("top only", label_style());

    let half_all =
        div(base(false).border_solid(2.0).border_lengths(0.5)).label("all 50%", label_style());

    let vertical_only = div(base(false).border_solid(2.0).border_lengths((1.0, 0.0)))
        .label("vertical", label_style());

    let horizontal_only = div(base(false).border_solid(2.0).border_lengths((0.0, 1.0)))
        .label("horizontal", label_style());

    v_flex(section_style()).children([
        section_title("Border Lengths"),
        h_flex(wrapper_style()).children([top_only, half_all, vertical_only, horizontal_only]),
    ])
}

fn align_container() -> Element {
    let start = div(base(false)
        .border_solid(2.0)
        .border_lengths(0.5)
        .border_align(BorderAlignment::Start))
    .label("Start", label_style());

    let center = div(base(false)
        .border_solid(2.0)
        .border_lengths(0.5)
        .border_align(BorderAlignment::Center))
    .label("Center", label_style());

    let end = div(base(false)
        .border_solid(2.0)
        .border_lengths(0.5)
        .border_align(BorderAlignment::End))
    .label("End", label_style());

    v_flex(section_style()).children([
        section_title("Border Alignment (50% Length)"),
        h_flex(wrapper_style()).children([start, center, end]),
    ])
}

fn color_container() -> Element {
    let red =
        div(base(true).border_solid(2.0).border_color(Color::RED)).label("Red", label_style());
    let yellow = div(base(true).border_solid(2.0).border_color(Color::YELLOW))
        .label("Yellow", label_style());
    let green =
        div(base(true).border_solid(2.0).border_color(Color::GREEN)).label("Green", label_style());
    let blue =
        div(base(true).border_solid(2.0).border_color(Color::BLUE)).label("Blue", label_style());

    v_flex(section_style()).children([
        section_title("Border Color"),
        h_flex(wrapper_style()).children([red, yellow, green, blue]),
    ])
}
