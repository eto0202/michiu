use crate::{app::theme::Theme, components::section_title};
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(20.0).p(16.0)).children([basic_container()])
}

fn section_style() -> ThisStyle {
    ts().gap(12.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap((20.0, 10.0)).p(6.0)
}

fn input_basic(d: &'static str, el: Element) -> Element {
    let desc = div_n().label(
        d,
        ts().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(14.0)
            .font_weight(400),
    );

    let wrapper = div(ts()
        .flex()
        .items_center()
        .p(4.0)
        .r(4.0)
        .size((160.0, 40.0))
        .bg_color(dynamic(|t: &Theme| t.surface))
        .border_solid(1.0)
        .border_color(dynamic(|t: &Theme| t.border)));

    v_flex(ts().gap(6.0).justify_center().items_center()).children([desc, wrapper.child(el)])
}

fn basic_container() -> Element {
    let (read_1, write_1) = create_signal(String::new());
    let basic_1 = input_basic(
        "Description",
        input(InputContents::new((read_1, write_1)).multiline(true)).style(
            ts().h_auto()
                .w_full()
                .font_size(18.0)
                .font_family("Segoe UI")
                .text_color(dynamic(|t: &Theme| t.text))
                .select_text()
                .cursor_text()
                .overflow_hidden(),
        ),
    );

    v_flex(section_style()).children([
        section_title("Input Basic"),
        h_flex(wrapper_style()).children([basic_1]),
    ])
}
