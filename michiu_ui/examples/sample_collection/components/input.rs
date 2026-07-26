use std::time::Duration;

use crate::{app::theme::Theme, components::section_title};
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0)).children([
        basic_container(),
        restrict_container(),
        btn_container(),
    ])
}

fn section_style() -> ThisStyle {
    ts().gap(12.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap((16.0, 10.0)).p(6.0)
}

fn input_item(d: &'static str, el: Element) -> Element {
    let desc = div_n().label(
        d,
        ts().text_color(dynamic(|t: &Theme| t.text))
            .font_size(14.0)
            .font_weight(400),
    );

    let wrapper = div(ts()
        .flex()
        .items_center()
        .p((6.0, 4.0))
        .r(4.0)
        .size((160.0, 40.0))
        .bg_color(dynamic(|t: &Theme| t.background_hover))
        .border_solid(1.0)
        .border_color(dynamic(|t: &Theme| t.border)));

    v_flex(ts().gap(6.0).justify_center().items_center()).children([desc, wrapper.child(el)])
}

fn input_style(family: &'static str) -> ThisStyle {
    ts().h_auto()
        .w_full()
        .font_size(16.0)
        .font_family(family)
        .text_color(dynamic(|t: &Theme| t.text))
        .select_text()
        .cursor_text()
        .overflow_hidden()
}

fn basic_container() -> Element {
    let (read_1, write_1) = create_signal(String::new());
    let basic_1 = input_item(
        "Font Family : Segoe UI",
        input(InputContents::new((read_1, write_1))).style(input_style("Segoe UI")),
    );

    let (read_2, write_2) = create_signal(String::new());
    let basic_2 = input_item(
        "Font Family : Arial",
        input(
            InputContents::new((read_2, write_2))
                .caret_color(Color::BLUE)
                .caret_height(16.0)
                .caret_width(20.0)
                .caret_offset(1.0)
                .is_blink(true)
                .blink_frequency(Duration::from_millis(500)),
        )
        .style(input_style("Arial")),
    );

    let (read_3, write_3) = create_signal(String::new());
    let basic_3 = input_item(
        "Font Family : Consolas",
        input(
            InputContents::new((read_3, write_3))
                .placeholder("placeholder")
                .placeholder_color(Color::BLACK),
        )
        .style(input_style("Consolas")),
    );

    let (read_4, write_4) = create_signal("Yu Gothic UI".to_string());
    let basic_4 = input_item(
        "Font Family : Yu Gothic UI",
        input(InputContents::new((read_4, write_4))).style(input_style("Yu Gothic UI")),
    );

    v_flex(section_style()).children([
        section_title("Input Basic"),
        h_flex(wrapper_style()).children([basic_1, basic_2, basic_3, basic_4]),
    ])
}

fn restrict_container() -> Element {
    let (read_1, write_1) = create_signal(String::new());
    let basic_1 = input_item(
        "Numeric only",
        input(InputContents::new((read_1, write_1)).numeric_only(true))
            .style(input_style("Segoe UI")),
    );

    let (read_2, write_2) = create_signal(String::new());
    let basic_2 = input_item(
        "Password",
        input(
            InputContents::new((read_2, write_2))
                .password(true)
                .mask_text("*"),
        )
        .style(input_style("Segoe UI")),
    );

    let (read_3, write_3) = create_signal(String::new());
    let basic_3 = input_item(
        "Max length",
        input(InputContents::new((read_3, write_3)).max_length(10)).style(input_style("Segoe UI")),
    );

    let (read_4, write_4) = create_signal(String::new());
    let basic_4 = input_item(
        "No IME",
        input(InputContents::new((read_4, write_4)).is_ime(false)).style(input_style("Segoe UI")),
    );

    v_flex(section_style()).children([
        section_title("Input Restrict"),
        h_flex(wrapper_style()).children([basic_1, basic_2, basic_3, basic_4]),
    ])
}

fn btn_container() -> Element {
    v_flex(section_style()).children([
        section_title("Input with button"),
        h_flex(wrapper_style()).children([spin_box()]),
    ])
}

fn spin_box() -> Element {
    let btn = ts()
        .justify_center()
        .items_center()
        .size(40.0)
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.border)));
    let label = ts()
        .text_color(dynamic(|t: &Theme| t.secondary))
        .font_size(25.0)
        .font_weight(400)
        .press_parent(ts().transform_scale(0.9, 0.9));

    let (read_1, write_1) = create_signal(String::new());

    let btn_left = div(btn.clone().r_left(3.0))
        .label("-", &label)
        .on_click(move || {
            let mut i = read_1.get().parse::<i32>().unwrap_or(0);
            if i <= -99 {
                i = -99;
            } else {
                i -= 1;
            }
            write_1.set(i.to_string());
        });

    let btn_right = div(btn.clone().r_right(3.0))
        .label("+", &label)
        .on_click(move || {
            let mut i = read_1.get().parse::<i32>().unwrap_or(0);
            if i >= 99 {
                i = 99;
            } else {
                i += 1;
            }
            write_1.set(i.to_string());
        });

    let input_center = h_flex(
        ts().items_center()
            .justify_center()
            .p((6.0, 4.0))
            .size((80.0, 40.0))
            .border_solid((0.0, 1.0))
            .border_color(dynamic(|t: &Theme| t.border))
            .focus_within(
                ts().outline_solid(1.0)
                    .outline_color(dynamic(|t: &Theme| t.text)),
            ),
    )
    .child(
        input_d(move |t: &Theme| {
            InputContents::new((read_1, write_1))
                .is_ime(false)
                .numeric_only(true)
                .max_length(3)
                .placeholder("-99~99")
                .placeholder_color(t.text_muted)
        })
        .style(input_style("Segoe UI").w_auto()),
    );

    h_flex(
        ts().r(4.0)
            .bg_color(dynamic(|t: &Theme| t.background_hover))
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .children([btn_left, input_center, btn_right])
}
