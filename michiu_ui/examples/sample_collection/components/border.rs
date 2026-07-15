use crate::app::{
    theme::Theme,
    utils::{label_style, section_title},
};
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).grow()).children_d(|t: &Theme| {
        [
            style_container(t),
            weight_container(t),
            length_container(t),
            align_container(t),
            color_container(t),
        ]
    })
}

fn base(t: &Theme) -> ThisStyle {
    ts().grow()
        .basis_0()
        .justify_center()
        .items_center()
        .p(10.0)
        .r(4.0)
        .border_color(t.border)
        .hovered(ts().bg_color(t.background_hover))
}

fn style_container(t: &Theme) -> Element {
    let solid = div(base(t).border_solid(2.0)).label("solid", label_style());
    let dashed = div(base(t).border_dashed(2.0)).label("dashed", label_style());
    let dotted = div(base(t).border_dotted(2.0)).label("dotted", label_style());
    let double = div(base(t).border_double(4.0)).label("double", label_style());

    v_flex(ts().gap(12.0).grow()).children([
        section_title("Border Style"),
        h_flex(ts().gap(12.0).grow()).children([solid, dashed, dotted, double]),
    ])
}

fn weight_container(t: &Theme) -> Element {
    let one = div(base(t).border_solid(1.0)).label("1.0 px", label_style());
    let two = div(base(t).border_solid(2.0)).label("2.0 px", label_style());
    let four = div(base(t).border_solid(4.0)).label("4.0 px", label_style());
    let eight = div(base(t).border_solid(8.0)).label("8.0 px", label_style());

    v_flex(ts().gap(12.0).grow()).children([
        section_title("Border Weight"),
        h_flex(ts().gap(12.0).grow()).children([one, two, four, eight]),
    ])
}

fn length_container(t: &Theme) -> Element {
    let top_only = div(base(t)
        .border_solid(2.0)
        .border_lengths((1.0, 0.0, 0.0, 0.0)))
    .label("top only", label_style());

    let half_all =
        div(base(t).border_solid(2.0).border_lengths(0.5)).label("all 50%", label_style());

    let vertical_only =
        div(base(t).border_solid(2.0).border_lengths((1.0, 0.0))).label("vertical", label_style());

    let horizontal_only = div(base(t).border_solid(2.0).border_lengths((0.0, 1.0)))
        .label("horizontal", label_style());

    v_flex(ts().gap(12.0).grow()).children([
        section_title("Border Lengths"),
        h_flex(ts().gap(12.0).grow()).children([
            top_only,
            half_all,
            vertical_only,
            horizontal_only,
        ]),
    ])
}

fn align_container(t: &Theme) -> Element {
    let start = div(base(t)
        .border_solid(2.0)
        .border_lengths(0.5)
        .border_align(BorderAlignment::Start))
    .label("Start", label_style());

    let center = div(base(t)
        .border_solid(2.0)
        .border_lengths(0.5)
        .border_align(BorderAlignment::Center))
    .label("Center", label_style());

    let end = div(base(t)
        .border_solid(2.0)
        .border_lengths(0.5)
        .border_align(BorderAlignment::End))
    .label("End", label_style());

    v_flex(ts().gap(12.0).grow()).children([
        section_title("Border Alignment (50% Length)"),
        h_flex(ts().gap(12.0).grow()).children([start, center, end]),
    ])
}

fn color_container(t: &Theme) -> Element {
    let red = div(base(t).border_solid(2.0).border_color(Color::RED)).label("Red", label_style());
    let yellow =
        div(base(t).border_solid(2.0).border_color(Color::YELLOW)).label("Yellow", label_style());
    let green =
        div(base(t).border_solid(2.0).border_color(Color::GREEN)).label("Green", label_style());
    let blue =
        div(base(t).border_solid(2.0).border_color(Color::BLUE)).label("Blue", label_style());

    v_flex(ts().gap(12.0).grow()).children([
        section_title("Border Color"),
        h_flex(ts().gap(12.0).grow()).children([red, yellow, green, blue]),
    ])
}
