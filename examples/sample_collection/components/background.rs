use michiu::prelude::*;

pub fn container() -> Entity {
    flex(ii().flex().grow().p(16.0).items_center().justify_center()).child(
        h_flex(
            ii().w_full()
                .h_auto()
                .max_height(pct(100.0))
                .aspect_ratio(1.618, 1.0),
        )
        .children([
            flex(
                hover_opacity()
                    .width(pct(61.8))
                    .h_full()
                    .bg_color(Color::BLUE)
                    .r_left(4.0),
            ),
            step_1().modify(ii().width(pct(38.2)).h_full()),
        ]),
    )
}

fn step_1() -> Entity {
    v_flex(ii()).children([
        flex(
            hover_opacity()
                .w_full()
                .height(pct(61.8))
                .bg_color(Color::CYAN)
                .r((0.0, 4.0, 0.0, 0.0)),
        ),
        step_2().modify(ii().w_full().height(pct(38.2))),
    ])
}

fn step_2() -> Entity {
    h_flex(ii()).children([
        step_3().modify(ii().width(pct(38.2)).h_full()),
        flex(
            hover_opacity()
                .width(pct(61.8))
                .h_full()
                .bg_color(Color::GREEN)
                .r((0.0, 0.0, 4.0, 0.0)),
        ),
    ])
}

fn step_3() -> Entity {
    v_flex(ii()).children([
        step_4().modify(ii().w_full().height(pct(38.2))),
        flex(
            hover_opacity()
                .w_full()
                .height(pct(61.8))
                .bg_color(Color::YELLOW),
        ),
    ])
}

fn step_4() -> Entity {
    h_flex(ii()).children([
        flex(
            hover_opacity()
                .width(pct(61.8))
                .h_full()
                .bg_color(Color::PURPLE),
        ),
        step_5().modify(ii().width(pct(38.2)).h_full()),
    ])
}

fn step_5() -> Entity {
    v_flex(ii()).children([
        flex(
            hover_opacity()
                .w_full()
                .height(pct(61.8))
                .bg_color(Color::ORANGE),
        ),
        step_6().modify(ii().w_full().height(pct(38.2))),
    ])
}

fn step_6() -> Entity {
    h_flex(ii()).children([
        step_7().modify(ii().width(pct(38.2)).h_full()),
        flex(
            hover_opacity()
                .width(pct(61.8))
                .h_full()
                .bg_color(Color::MAGENTA),
        ),
    ])
}

fn step_7() -> Entity {
    flex(hover_opacity().size_full().bg_color(Color::RED))
}

fn hover_opacity() -> Modifier {
    use std::time::Duration;

    ii().hovered(ii().opacity_50())
        .trans_opacity(Duration::from_millis(300), AnimationCurve::EaseInOutQuad)
}
