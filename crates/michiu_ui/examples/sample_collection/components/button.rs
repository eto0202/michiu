#![allow(clippy::pedantic, clippy::restriction)]

use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
use michiu_ui::prelude::*;
use std::time::Duration;

pub fn container() -> Entity {
    v_flex(ii().gap(28.0).p(16.0)).children([
        section_normal(),
        section_pseudo(),
        section_transition(),
    ])
}

fn section_style() -> Modifier {
    ii().grow().gap(16.0)
}

fn wrapper_style() -> Modifier {
    ii().w_full().gap(16.0)
}

fn item_style() -> Modifier {
    ii().p(12.0)
        .r(4.0)
        .size((120.0, 70.0))
        .justify_center()
        .items_center()
}

fn section_normal() -> Entity {
    let wrapper =
        h_flex(wrapper_style()).children([signal_btn(), direct_btn(), event_btn(), tag_btn()]);

    v_flex(section_style()).children([section_title("Button"), wrapper])
}

fn signal_btn() -> Entity {
    let (count, set_count) = create_signal(0u32);

    h_flex(
        item_style()
            .border_solid(2.0)
            .border_color(dynamic(|t: &Theme| t.primary)),
    )
    .on_click(move || set_count.set(count.get() + 1))
    .label(move || format!("Signal: {}", count.get()), label_style())
}

fn direct_btn() -> Entity {
    let e = h_flex(
        item_style()
            .border_solid(2.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    );

    let style = ii()
        .text_color(dynamic(|t: &Theme| t.primary))
        .font_size(16.0)
        .font_weight(400);

    let mut count = 0u32;

    e.label(
        move || format!("Direct: {}.{}", count / 10, count % 10),
        &style,
    )
    .on_click(move || {
        count += 1;
        e.set_contents(div_n().label(
            move || format!("Direct: {}.{}", count / 10, count % 10),
            &style,
        ));
    })
}

fn event_btn() -> Entity {
    let style = ii()
        .text_color(dynamic(|t: &Theme| t.surface))
        .font_size(16.0)
        .font_weight(400);

    let mut count = 0u32;

    h_flex(item_style().bg_color(dynamic(|t: &Theme| t.primary)))
        .label(move || format!("Event: {count}"), &style)
        .on_click_with(move |cx| {
            count += 1;

            cx.current()
                .set_contents(div_n().label(move || format!("Event: {count}"), &style));
        })
}

fn tag_btn() -> Entity {
    struct MichiuTag;

    let style = ii()
        .text_color(dynamic(|t: &Theme| t.primary))
        .font_size(16.0)
        .font_weight(400);

    let mut count = 0u32;

    let left = h_flex(ii())
        .label(move || format!("Tag: {count}"), &style)
        .tag::<MichiuTag>();

    let right = h_flex(ii())
        .label(
            move || "Click",
            style.clone().pressed_parent(ii().text_color(Color::WHITE)),
        )
        .on_click_with(move |cx| {
            let e = cx.query_first::<MichiuTag>();
            count += 1;
            e.set_contents(div_n().label(move || format!("Tag: {count}"), &style));
        });

    h_flex(
        item_style()
            .gap(8.0)
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.primary)),
    )
    .children([left, right])
}

fn section_pseudo() -> Entity {
    let wrapper = h_flex(wrapper_style()).children([
        hovered_btn(),
        pressed_btn(),
        dragged_btn(),
        actived_btn(),
        selected_btn(),
    ]);

    v_flex(section_style()).children([section_title("Button Pseudo Class"), wrapper])
}

fn hovered_btn() -> Entity {
    h_flex(
        item_style()
            .shadow_color(Color::BLACK)
            .shadow_blur(4.0)
            .shadow_offset(0.0)
            .shadow_spread(1.0)
            .hovered(ii().shadow_blur(8.0)),
    )
    .label("Hovered", label_style())
}

fn pressed_btn() -> Entity {
    h_flex(
        ii().p(12.0)
            .r(4.0)
            .size((120.0, 70.0))
            .justify_center()
            .items_center()
            .border_dashed(2.0)
            .border_color(dynamic(|t: &Theme| t.primary))
            .pressed(ii().transform_scale(0.96, 0.96)),
    )
    .label(
        "Pressed",
        ii().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(16.0)
            .font_weight(400)
            .transform_inherit(true),
    )
}

fn dragged_btn() -> Entity {
    h_flex(item_style())
        .modify_d(|t: &Theme| {
            ii().bg_color(t.background_hover).dragged(
                ii().bg_color(t.background)
                    .border_dashed(2.0)
                    .border_color(t.background_hover),
            )
        })
        .label("Dragged", label_style())
}

fn actived_btn() -> Entity {
    let (is_active, set_is_active) = create_signal(false);

    h_flex(item_style())
        .modify_d(|t: &Theme| {
            ii().border_color(t.border)
                .border_solid(2.0)
                .bg_color(t.surface)
                .actived(ii().bg_color(t.primary))
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

pub fn selected_btn() -> Entity {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Option {
        A,
        B,
        C,
    }

    let (selected, set_selected) = create_signal(Option::A);

    let item = move |opt: Option, label_text: &'static str| {
        h_flex(item_style().border_dashed(2.0).w_full().justify_center())
            .modify_d(|t: &Theme| {
                ii().border_color(t.border_hover)
                    .selected(ii().border_color(t.primary).border_solid(2.0))
            })
            .label(label_text, label_style())
            .select(move || selected.get() == opt)
            .on_click(move || {
                set_selected.set(opt);
            })
            .on_select(move || {
                println!("{opt:?} is now selected!");
            })
    };

    h_flex(ii().gap(2.0)).children([
        item(Option::A, "Sel"),
        item(Option::B, "ec"),
        item(Option::C, "ted"),
    ])
}

fn transition_btn(ms: u64, label_text: &'static str) -> Entity {
    h_flex(item_style().box_shadow(shadow().blur(4.0).color(Color::BLACK).spread(1.0))).child(
        h_flex(
            ii().w_full()
                .p_y(4.0)
                .items_center()
                .justify_center()
                .border_bottom(BorderStyle::Solid, 2.0)
                .border_align(BorderAlignment::Center)
                .border_bottom_length(0.6)
                .border_color(dynamic(|t: &Theme| t.border))
                .pressed(ii().border_color(dynamic(|t: &Theme| t.primary)))
                .trans_border_color(Duration::from_millis(ms), AnimationCurve::EaseInOutQuad),
        )
        .label(label_text, label_style()),
    )
}

fn section_transition() -> Entity {
    let wrapper = h_flex(wrapper_style()).children([
        transition_btn(150, "150 ms"),
        transition_btn(300, "300 ms"),
        transition_btn(450, "450 ms"),
        transition_btn(600, "600 ms"),
    ]);

    v_flex(section_style()).children([section_title("Button Transition"), wrapper])
}
