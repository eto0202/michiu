pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    div(ts().flex().grow().p(16.0).items_center().justify_center()).child(
        h_flex(
            ts().w_full()
                .h_auto()
                .max_height(pct(100.0))
                .aspect_ratio(1.618, 1.0),
        )
        .children([
            div(hover_opacity()
                .width(pct(61.8))
                .h_full()
                .bg_color(Color::BLUE)
                .r_left(4.0)),
            step_1().style(ts().width(pct(38.2)).h_full()),
        ]),
    )
}

fn step_1() -> Element {
    v_flex(ts()).children([
        div(hover_opacity()
            .w_full()
            .height(pct(61.8))
            .bg_color(Color::CYAN)
            .r((0.0, 4.0, 0.0, 0.0))),
        step_2().style(ts().w_full().height(pct(38.2))),
    ])
}

fn step_2() -> Element {
    h_flex(ts()).children([
        step_3().style(ts().width(pct(38.2)).h_full()),
        div(hover_opacity()
            .width(pct(61.8))
            .h_full()
            .bg_color(Color::GREEN)
            .r((0.0, 0.0, 4.0, 0.0))),
    ])
}

fn step_3() -> Element {
    v_flex(ts()).children([
        step_4().style(ts().w_full().height(pct(38.2))),
        div(hover_opacity()
            .w_full()
            .height(pct(61.8))
            .bg_color(Color::YELLOW)),
    ])
}

fn step_4() -> Element {
    h_flex(ts()).children([
        div(hover_opacity()
            .width(pct(61.8))
            .h_full()
            .bg_color(Color::PURPLE)),
        step_5().style(ts().width(pct(38.2)).h_full()),
    ])
}

fn step_5() -> Element {
    v_flex(ts()).children([
        div(hover_opacity()
            .w_full()
            .height(pct(61.8))
            .bg_color(Color::ORANGE)),
        step_6().style(ts().w_full().height(pct(38.2))),
    ])
}

fn step_6() -> Element {
    h_flex(ts()).children([
        step_7().style(ts().width(pct(38.2)).h_full()),
        div(hover_opacity()
            .width(pct(61.8))
            .h_full()
            .bg_color(Color::MAGENTA)),
    ])
}

fn step_7() -> Element {
    div(hover_opacity().size_full().bg_color(Color::RED))
}

fn hover_opacity() -> ThisStyle {
    use std::time::Duration;

    ts().hovered(ts().opacity_50())
        .trans_opacity(Duration::from_millis(300), AnimationCurve::EaseInOutQuad)
}
