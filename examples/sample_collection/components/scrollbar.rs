use crate::app::theme::Theme;
use michiu::prelude::*;

pub fn container() -> Element {
    v_flex(ts().p(16.0).gap(16.0)).children([section_top(), section_buttom()])
}

fn section_top() -> Element {
    h_flex(ts().gap(16.0)).children([scroll_container_1(), scroll_container_2()])
}

fn section_buttom() -> Element {
    h_flex(ts().gap(16.0)).children([scroll_container_3(), scroll_container_4()])
}

fn scroll_container_1() -> Element {
    flex(ts().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .style_d(|t: &Theme| {
            ts().border_color(t.border)
                .scrollbar_always()
                .scrollbar_mode(ScrollbarMode::Overlay)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_thumb(
                            ts().m_y(8.0)
                                .p_r(4.0)
                                .width(8.0)
                                .r_full()
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        )
                        .h_thumb(
                            ts().m_x(8.0)
                                .p_b(4.0)
                                .height(8.0)
                                .r_full()
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        ),
                )
        })
        .child(scroll_item("Overlay"))
}

fn scroll_container_2() -> Element {
    flex(ts().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .style_d(|t: &Theme| {
            ts().border_color(t.border)
                .scrollbar_always()
                .scrollbar_mode(ScrollbarMode::Layout)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_track(ts().bg_color(t.border).m_y(8.0))
                        .v_thumb(
                            ts().width(8.0)
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        )
                        .h_track(ts().bg_color(t.border).m_x(8.0))
                        .h_thumb(
                            ts().height(8.0)
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        ),
                )
        })
        .child(scroll_item("Layout"))
}

fn scroll_container_3() -> Element {
    flex(ts().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .style_d(|t: &Theme| {
            ts().border_color(t.border)
                .scrollbar_auto()
                .scrollbar_mode(ScrollbarMode::Overlay)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_thumb(
                            ts().m_y(8.0)
                                .p_r(4.0)
                                .width(8.0)
                                .r_full()
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        )
                        .h_thumb(
                            ts().m_x(8.0)
                                .p_b(4.0)
                                .height(8.0)
                                .r_full()
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        ),
                )
        })
        .child(scroll_item("Auto"))
}

fn scroll_container_4() -> Element {
    flex(ts().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .style_d(|t: &Theme| {
            ts().border_color(t.border)
                .scrollbar_transient()
                .scrollbar_mode(ScrollbarMode::Overlay)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_thumb(
                            ts().m_y(8.0)
                                .p_r(4.0)
                                .width(8.0)
                                .r_full()
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        )
                        .h_thumb(
                            ts().m_x(8.0)
                                .p_b(4.0)
                                .height(8.0)
                                .r_full()
                                .bg_color(t.primary)
                                .hovered(ts().bg_color(t.primary_hover)),
                        ),
                )
        })
        .child(scroll_item("Transient"))
}

fn scroll_item(label: &'static str) -> Element {
    flex(ts().r(4.0).p(16.0).size(500.0).shrink_0().justify_center()).child(
        flex(
            ts().justify_center()
                .r(4.0)
                .size_full()
                .bg_color(dynamic(|t: &Theme| t.background_hover)),
        )
        .label(label, ts().text_color(dynamic(|t: &Theme| t.text))),
    )
}
