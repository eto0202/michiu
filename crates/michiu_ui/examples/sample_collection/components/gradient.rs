use michiu_ui::{LinearGradient, prelude::*};

pub fn container() -> Entity {
    v_flex(ii().p(16.0).gap(16.0)).children([section_1(), section_2(), section_3()])
}

fn base_style() -> Modifier {
    ii().r(4.0).size((200.0, 200.0))
}

fn section_1() -> Entity {
    h_flex(ii().gap(16.0)).children([
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 225.0))),
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 270.0))),
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 315.0))),
    ])
}

fn section_2() -> Entity {
    h_flex(ii().gap(16.0)).children([
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 180.0))),
        div(base_style()),
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 0.0))),
    ])
}

fn section_3() -> Entity {
    h_flex(ii().gap(16.0)).children([
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 135.0))),
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 90.0))),
        div(base_style().bg_gradient(LinearGradient::two_stops(Color::WHITE, Color::BLACK, 45.0))),
    ])
}
