use std::time::Duration;

pub use michiu_ui::prelude::*;
use michiu_ui::{WebView2Visual, external_visual};

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).r(4.0).size_full())
        .child_d(|visual: &WebView2Visual| webview2(visual))
}

fn webview2(visual: &WebView2Visual) -> Element {
    let (is_open, set_is_open) = create_signal(false);
    let (is_active, set_is_active) = create_signal(false);
    let (is_opacity, set_is_opacity) = create_signal(false);

    let google_map = external_visual(visual.clone())
        .style({
            let base = ts()
                .size_full()
                .r(2.0)
                .resizable_bottom(true)
                .dnd_droppable(DndDropTarget::Child, DndDragPayload::Element)
                .overflow_hidden()
                .transform(Transform::new().scale(1.0, 1.0))
                .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
                .pressed(ts().transform(Transform::new().scale(1.01, 1.01)));

            is_opacity.get_else(base.clone().opacity_50(), base.opacity_100())
        })
        .on_focus(move || set_is_active.set(false))
        .child(v_flex({
            let base = ts()
                .absolute()
                .size((300.0, 200.0))
                .r(3.0)
                .inset_x(100.0)
                .inset_y(50.0)
                .bg_color(Color::DARK_GRAY)
                .opacity(0.9)
                .resizable_all(true)
                .dnd_draggable_root(DndDragPayload::Element, true)
                .dnd_draggable_original(ts().opacity_0())
                .dnd_draggable_placeholder(
                    ts().size(100.0)
                        .r(3.0)
                        .bg_color(Color::DARK_GRAY)
                        .opacity(0.9),
                );

            is_active.get_else(base.clone().flex(), base.hidden())
        }));

    let btn_style = |color: Color| {
        ts().justify_center()
            .items_center()
            .r(3.0)
            .size((100.0, 40.0))
            .border_dashed(1.0)
            .border_color(color)
            .hovered(ts().border_solid(1.0))
            .actived(ts().border_solid(1.0).bg_color(color))
    };

    let overlay_btn = h_flex(move || btn_style(Color::BLUE))
        .label("Overlay", ts().font_size(14.0).text_color(Color::WHITE))
        .active(is_active)
        .on_click(move || set_is_active.set(!is_active.get()));

    let opacity_btn = h_flex(move || btn_style(Color::CYAN))
        .label("Opacity", ts().font_size(14.0).text_color(Color::WHITE))
        .active(is_opacity)
        .on_click(move || set_is_opacity.set(!is_opacity.get()));

    let close_btn = h_flex(
        ts().justify_center()
            .items_center()
            .r(3.0)
            .size((100.0, 40.0))
            .border_dashed(1.0)
            .border_color(Color::RED)
            .hovered(ts().border_solid(1.0)),
    )
    .label("Close", ts().font_size(14.0).text_color(Color::RED))
    .on_click(move || set_is_open.set(false));

    v_flex(
        ts().size_full()
            .p(20.0)
            .gap(20.0)
            .items_center()
            .bg_color(hsla(0.0, 0.0, 3.0, 0.4)),
    )
    .children([
        v_flex(
            ts().r(3.0)
                .size((150.0, 40.0))
                .justify_center()
                .items_center()
                .p(10.0)
                .bg_color(hsl(0.0, 0.0, 50.0)),
        )
        .label("Open", ts().font_size(16.0).text_color(Color::BLACK))
        .on_click(move || set_is_open.set(true)),
        v_flex(
            ts().r(3.0)
                .size_full()
                .justify_center()
                .items_center()
                .border_dashed(2.0)
                .border_color(Color::GRAY),
        )
        .children([
            div(ts().r(3.0).size(pct(50.0)).bg_color(hsl(0.0, 0.0, 10.0))),
            v_flex({
                let base = ts()
                    .size_full()
                    .p(10.0)
                    .gap(10.0)
                    .justify_center()
                    .items_center()
                    .absolute()
                    .top(0.0)
                    .left(0.0)
                    .overflow_hidden();

                is_open.get_else(base.clone().flex(), base.hidden())
            })
            .children([
                google_map,
                h_flex(
                    ts().p(10.0)
                        .gap(10.0)
                        .w_full()
                        .h_auto()
                        .justify_center()
                        .items_center(),
                )
                .children([overlay_btn, opacity_btn, close_btn]),
            ]),
        ]),
    ])
}
