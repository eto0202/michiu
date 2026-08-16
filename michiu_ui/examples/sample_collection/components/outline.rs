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
        offset_container(),
    ])
}

fn section_style() -> ThisStyle {
    ts().gap(12.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap((24.0, 10.0)).p(6.0)
}

fn item(label: &'static str, change: bool, style: ThisStyle) -> Element {
    let s = style
        .p(10.0)
        .r(4.0)
        .size((162.0, 62.0))
        .justify_center()
        .items_center()
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover)));

    let new = if change {
        s
    } else {
        s.outline_color(dynamic(|t: &Theme| t.border))
    };

    div(new).label(label, label_style())
}

fn style_container() -> Element {
    let solid = item("solid", false, ts().outline_solid(2.0));
    let dashed = item("dashed", false, ts().outline_dashed(2.0));
    let dotted = item("dotted", false, ts().outline_dotted(2.0));
    let double = item("double", false, ts().outline_double(4.0));

    v_flex(section_style()).children([
        section_title("Outline Style"),
        h_flex(wrapper_style()).children([solid, dashed, dotted, double]),
    ])
}

fn weight_container() -> Element {
    let one = item("1.0 px", false, ts().outline_solid(1.0));
    let two = item("2.0 px", false, ts().outline_solid(2.0));
    let four = item("4.0 px", false, ts().outline_solid(4.0));
    let eight = item(" 8.0 px", false, ts().outline_solid(8.0));

    v_flex(section_style()).children([
        section_title("Outline Weight"),
        h_flex(wrapper_style()).children([one, two, four, eight]),
    ])
}

fn length_container() -> Element {
    let top_only = item(
        "top only",
        true,
        ts().outline_top(BorderStyle::Solid, 2.0)
            .outline_color(Color::RED),
    );
    let half_all = item(
        "all 50%",
        true,
        ts().outline_solid(2.0)
            .outline_color(Color::YELLOW)
            .outline_lengths(0.5),
    );
    let vertical_only = item(
        "vertical",
        true,
        ts().outline_solid(2.0)
            .outline_color(Color::GREEN)
            .outline_lengths((1.0, 0.0)),
    );
    let horizontal_only = item(
        "horizontal",
        true,
        ts().outline_solid(2.0)
            .outline_color(Color::BLUE)
            .outline_lengths((0.0, 1.0)),
    );

    v_flex(section_style()).children([
        section_title("Outline Lengths"),
        h_flex(wrapper_style()).children([top_only, half_all, vertical_only, horizontal_only]),
    ])
}

fn align_container() -> Element {
    let start = item(
        "Start",
        false,
        ts().outline_solid(2.0)
            .outline_lengths(0.5)
            .outline_align(BorderAlignment::Start),
    );
    let center = item(
        "Start",
        false,
        ts().outline_solid(2.0)
            .outline_lengths(0.5)
            .outline_align(BorderAlignment::Center),
    );
    let end = item(
        "Start",
        false,
        ts().outline_solid(2.0)
            .outline_lengths(0.5)
            .outline_align(BorderAlignment::End),
    );

    v_flex(section_style()).children([
        section_title("Outline Alignment (50% Length)"),
        h_flex(wrapper_style()).children([start, center, end]),
    ])
}

fn offset_container() -> Element {
    let one = item("1.0 px", false, ts().outline_solid(2.0).outline_offset(1.0));
    let two = item("2.0 px", false, ts().outline_solid(2.0).outline_offset(2.0));
    let four = item("4.0 px", false, ts().outline_solid(2.0).outline_offset(4.0));
    let eight = item("8.0 px", false, ts().outline_solid(2.0).outline_offset(8.0));

    v_flex(section_style()).children([
        section_title("Outline Offset"),
        h_flex(wrapper_style()).children([one, two, four, eight]),
    ])
}
