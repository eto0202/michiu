use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(20.0).p(16.0)).children([input_container()])
}

fn section_style() -> ThisStyle {
    ts().gap(12.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap((20.0, 10.0)).p(6.0)
}

fn item(d: &'static str, el: Element) -> Element {
    let desc = div_n().label(
        d,
        ts().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(14.0)
            .font_weight(400),
    );

    let wrapper = div(ts()
        .p(4.0)
        .w(240.0)
        .bg_color(dynamic(|t: &Theme| t.surface))
        .border_solid(1.0)
        .border_color(dynamic(|t: &Theme| t.border)));

    let content = el.style_d(|t: &Theme| {
        ts().size_full()
            .font_size(t.font_size_lg)
            .font_family(t.font_family.clone())
            .text_color(t.text)
            .select_text()
            .cursor_text()
            .overflow_hidden()
            .border_solid(1.0)
            .border_color(Color::RED)
    });

    v_flex(ts().gap(6.0).justify_center().items_center()).children([desc, wrapper.child(content)])
}

fn input_container() -> Element {
    let (read, write) = create_signal(String::new());
    let input = item("Description", input(InputContents::new((read, write))));

    v_flex(section_style()).children([
        section_title("Input"),
        h_flex(wrapper_style()).children([input]),
    ])
}
