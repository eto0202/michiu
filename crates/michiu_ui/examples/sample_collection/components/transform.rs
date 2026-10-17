use std::time::Duration;

use crate::app::theme::Theme;
use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(24.0).p(16.0)).children([
        self_group(),
        inherit_group(),
        transition_group(),
        animation_group(),
    ])
}

fn group_container(label: &'static str) -> Element {
    v_flex(
        ts().size_full()
            .r(4.0)
            .p(8.0)
            .gap(16.0)
            .border_dashed(1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .label(
        label,
        ts().font_size(18.0).text_color(dynamic(|t: &Theme| t.text)),
    )
}

fn items() -> Element {
    h_flex(ts().gap(16.0))
}

fn item(label: &'static str, style: ThisStyle) -> Element {
    flex(
        ts().r(4.0)
            .size(80.0)
            .justify_center()
            .bg_color(dynamic(|t: &Theme| t.primary))
            .pressed(style),
    )
    .label(
        label,
        ts().font_size(16.0).text_color(dynamic(|t: &Theme| t.text)),
    )
}

fn self_group() -> Element {
    group_container("Self").child(items().children([
        item("Scale", ts().transform_scale(0.95, 0.95)),
        item("Rotate", ts().transform_rotate(0.5)),
        item("Translate", ts().transform_translate(10.0, 10.0)),
        item("Origin", ts().transform_origin(1.0).transform_rotate(0.5)),
    ]))
}

fn inherit_group() -> Element {
    group_container("Inherit (child moved using absolute positioning)").child(
        items().child(
            item("Parent", ts().transform_rotate(0.5)).child(
                item("Child", ts()).style(
                    ts().size(40.0)
                        .absolute()
                        .left(110.0)
                        .transform_inherit(true),
                ),
            ),
        ),
    )
}

fn trans_item(label: &'static str, style: ThisStyle) -> Element {
    flex(
        ts().r(4.0)
            .size(80.0)
            .justify_center()
            .bg_color(dynamic(|t: &Theme| t.primary))
            .trans_transform(Duration::from_millis(200), AnimationCurve::EaseInOutQuad)
            .pressed(style),
    )
    .label(
        label,
        ts().font_size(16.0).text_color(dynamic(|t: &Theme| t.text)),
    )
}

fn transition_group() -> Element {
    group_container("Transition").child(items().children([
        trans_item("Scale", ts().transform_scale(0.95, 0.95)),
        trans_item("Rotate", ts().transform_rotate(0.5)),
        trans_item("Translate", ts().transform_translate(10.0, 10.0)),
    ]))
}

fn animation_group() -> Element {
    group_container("Animation").child(
        items().child(
            flex(
                ts().r(4.0)
                    .size(80.0)
                    .justify_center()
                    .bg_color(dynamic(|t: &Theme| t.primary))
                    .animation(KeyframeAnimation::new(
                        PropertyList::Transform,
                        Duration::from_millis(600),
                        PlaybackCount::Infinite,
                        AnimationCurve::EaseInOutQuad,
                    )),
            )
            .label(
                "Rotate",
                ts().font_size(16.0).text_color(dynamic(|t: &Theme| t.text)),
            ),
        ),
    )
}
