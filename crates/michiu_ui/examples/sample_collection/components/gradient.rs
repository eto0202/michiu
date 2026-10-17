use michiu_ui::{LinearGradient, prelude::*};

pub fn container() -> Element {
    v_flex(ts().p(16.0).gap(16.0)).children([section_1(), section_2(), section_3()])
}

fn base_style() -> ThisStyle {
    ts().r(4.0).size((200.0, 200.0))
}

fn section_1() -> Element {
    h_flex(ts().gap(16.0)).children([
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 225.0))),
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 270.0))),
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 315.0))),
    ])
}

fn section_2() -> Element {
    h_flex(ts().gap(16.0)).children([
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 180.0))),
        div(base_style()),
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 0.0))),
    ])
}

fn section_3() -> Element {
    h_flex(ts().gap(16.0)).children([
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 135.0))),
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 90.0))),
        div(base_style().bg_gradient(LinearGradient::new(Color::WHITE, Color::BLACK, 45.0))),
    ])
}
