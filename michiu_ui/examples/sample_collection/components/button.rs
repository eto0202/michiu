use crate::app::{
    theme::Theme,
    utils::{label_style, section_title},
};
use michiu_ui::prelude::*;
use std::time::Duration;

pub fn container() -> Element {
    v_flex(ts().gap(28.0).p(16.0)).children([
        section_normal(),
        section_pseudo(),
        section_transition(),
    ])
}

fn section_style() -> ThisStyle {
    ts().grow().gap(16.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap(16.0)
}

fn item_style() -> ThisStyle {
    ts().p(12.0)
        .r(4.0)
        .size((120.0, 70.0))
        .justify_center()
        .items_center()
}

fn section_normal() -> Element {
    let wrapper = h_flex(wrapper_style()).children([signal_btn(), direct_btn(), event_btn()]);

    v_flex(section_style()).children([section_title("Button"), wrapper])
}

fn signal_btn() -> Element {
    let (count, set_count) = create_signal(0u32);

    h_flex(
        item_style()
            .border_solid(2.0)
            .border_color(dynamic(|t: &Theme| t.primary)),
    )
    .on_click(move || set_count.set(count.get() + 1))
    .label(move || format!("Signal: {}", count.get()), label_style())
}

fn direct_btn() -> Element {
    let el = h_flex(
        item_style()
            .border_solid(2.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    );

    let style = ts()
        .text_color(dynamic(|t: &Theme| t.primary))
        .font_size(16.0)
        .font_weight(400);

    let mut count = 0u32;

    el.label(
        move || format!("Direct: {}.{}", count / 10, count % 10),
        &style,
    )
    .on_click(move || {
        count += 1;
        el.set_contents(div_n().label(
            move || format!("Direct: {}.{}", count / 10, count % 10),
            &style,
        ));
    })
}

fn event_btn() -> Element {
    let style = ts()
        .text_color(dynamic(|t: &Theme| t.surface))
        .font_size(16.0)
        .font_weight(400);

    let mut count = 0u32;

    h_flex(item_style().bg_color(dynamic(|t: &Theme| t.primary)))
        .label(move || format!("Event: {}", count), &style)
        .on_click_with(move |cx| {
            count += 1;

            let id = cx.entity_id_pressed().unwrap();
            let el = Element::from(id);

            el.set_contents(div_n().label(move || format!("Event: {}", count), &style));
        })
}

fn section_pseudo() -> Element {
    let wrapper = h_flex(wrapper_style()).children([
        hovered_btn(),
        pressed_btn(),
        dragged_btn(),
        actived_btn(),
        selected_btn(),
    ]);

    v_flex(section_style()).children([section_title("Button Pseudo Class"), wrapper])
}

fn hovered_btn() -> Element {
    h_flex(
        item_style()
            .box_shadow(
                shadow()
                    .color(Color::BLACK)
                    .blur(6.0)
                    .offset(0.0)
                    .spread(1.0),
            )
            .hovered(
                ts().box_shadow(
                    shadow()
                        .color(Color::BLACK)
                        .blur(10.0)
                        .offset(0.0)
                        .spread(2.0),
                ),
            ),
    )
    .label("Hovered", label_style())
}

fn pressed_btn() -> Element {
    h_flex(
        item_style()
            .border_dashed(2.0)
            .border_color(dynamic(|t: &Theme| t.primary))
            .pressed(ts().transform_scale(0.96, 0.96)),
    )
    .label("Pressed", label_style())
}

fn dragged_btn() -> Element {
    h_flex(item_style())
        .style_d(|t: &Theme| {
            ts().bg_color(t.background_hover).dragged(
                ts().bg_color(t.background)
                    .border_dashed(2.0)
                    .border_color(t.background_hover),
            )
        })
        .label("Dragged", label_style())
}

fn actived_btn() -> Element {
    let (is_active, set_is_active) = create_signal(false);

    h_flex(item_style())
        .style_d(|t: &Theme| {
            ts().border_color(t.border)
                .border_solid(2.0)
                .bg_color(t.surface)
                .actived(ts().bg_color(t.primary))
        })
        .label("Actived", label_style())
        .active(is_active)
        .on_click(move || {
            set_is_active.set(!is_active.get());
        })
        .on_active(|| {
            println!("Actived");
        })
}

pub fn selected_btn() -> Element {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Option {
        A,
        B,
        C,
    }

    let (selected, set_selected) = create_signal(Option::A);

    let item = move |opt: Option, label_text: &'static str| {
        h_flex(item_style().border_dashed(2.0).w_full().justify_center())
            .style_d(|t: &Theme| {
                ts().border_color(t.border_hover)
                    .selected(ts().border_color(t.primary).border_solid(2.0))
            })
            .label(label_text, label_style())
            .select(move || selected.get() == opt)
            .on_click(move || {
                set_selected.set(opt);
            })
            .on_select(move || {
                println!("{:?} is now selected!", opt);
            })
    };

    h_flex(ts().gap(2.0)).children([
        item(Option::A, "Sel"),
        item(Option::B, "ec"),
        item(Option::C, "ted"),
    ])
}

fn transition_btn(ms: u64, label_text: &'static str) -> Element {
    h_flex(item_style().box_shadow(shadow().blur(6.0).color(Color::BLACK).spread(1.0))).child(
        h_flex(
            ts().w_full()
                .p_y(4.0)
                .items_center()
                .justify_center()
                .border_bottom(BorderStyle::Solid, 2.0)
                .border_align(BorderAlignment::Center)
                .border_bottom_length(0.6)
                .border_color(dynamic(|t: &Theme| t.border))
                .pressed(ts().border_color(dynamic(|t: &Theme| t.primary)))
                .trans_border_color(Duration::from_millis(ms), AnimationCurve::EaseInOutQuad),
        )
        .label(label_text, label_style()),
    )
}

fn section_transition() -> Element {
    let wrapper = h_flex(wrapper_style()).children([
        transition_btn(150, "150 ms"),
        transition_btn(300, "300 ms"),
        transition_btn(450, "450 ms"),
        transition_btn(600, "600 ms"),
    ]);

    v_flex(section_style()).children([section_title("Button Transition"), wrapper])
}
