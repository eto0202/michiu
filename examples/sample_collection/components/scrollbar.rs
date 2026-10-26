use crate::app::theme::Theme;
use michiu::prelude::*;

pub fn container() -> Entity {
    v_flex(ii().p(16.0).gap(16.0)).children([section_top(), section_buttom()])
}

fn section_top() -> Entity {
    h_flex(ii().gap(16.0)).children([scroll_container_1(), scroll_container_2()])
}

fn section_buttom() -> Entity {
    h_flex(ii().gap(16.0)).children([scroll_container_3(), scroll_container_4()])
}

fn scroll_container_1() -> Entity {
    flex(ii().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .modify_d(|t: &Theme| {
            ii().border_color(t.border)
                .scrollbar_always()
                .scrollbar_mode(ScrollbarMode::Overlay)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_thumb(
                            ii().m_y(8.0)
                                .p_r(4.0)
                                .width(8.0)
                                .r_full()
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        )
                        .h_thumb(
                            ii().m_x(8.0)
                                .p_b(4.0)
                                .height(8.0)
                                .r_full()
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        ),
                )
        })
        .child(scroll_item("Overlay"))
}

fn scroll_container_2() -> Entity {
    flex(ii().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .modify_d(|t: &Theme| {
            ii().border_color(t.border)
                .scrollbar_always()
                .scrollbar_mode(ScrollbarMode::Layout)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_track(ii().bg_color(t.border).m_y(8.0))
                        .v_thumb(
                            ii().width(8.0)
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        )
                        .h_track(ii().bg_color(t.border).m_x(8.0))
                        .h_thumb(
                            ii().height(8.0)
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        ),
                )
        })
        .child(scroll_item("Layout"))
}

fn scroll_container_3() -> Entity {
    flex(ii().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .modify_d(|t: &Theme| {
            ii().border_color(t.border)
                .scrollbar_auto()
                .scrollbar_mode(ScrollbarMode::Overlay)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_thumb(
                            ii().m_y(8.0)
                                .p_r(4.0)
                                .width(8.0)
                                .r_full()
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        )
                        .h_thumb(
                            ii().m_x(8.0)
                                .p_b(4.0)
                                .height(8.0)
                                .r_full()
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        ),
                )
        })
        .child(scroll_item("Auto"))
}

fn scroll_container_4() -> Entity {
    flex(ii().r(4.0).size(320.0).border_solid(1.0).overflow_scroll())
        .modify_d(|t: &Theme| {
            ii().border_color(t.border)
                .scrollbar_transient()
                .scrollbar_mode(ScrollbarMode::Overlay)
                .scrollbar(
                    ScrollbarStyle::new(8.0)
                        .v_thumb(
                            ii().m_y(8.0)
                                .p_r(4.0)
                                .width(8.0)
                                .r_full()
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        )
                        .h_thumb(
                            ii().m_x(8.0)
                                .p_b(4.0)
                                .height(8.0)
                                .r_full()
                                .bg_color(t.accent)
                                .hovered(ii().bg_color(t.accent_hover)),
                        ),
                )
        })
        .child(scroll_item("Transient"))
}

fn scroll_item(label: &'static str) -> Entity {
    flex(ii().r(4.0).p(16.0).size(500.0).shrink_0().justify_center()).child(
        flex(
            ii().justify_center()
                .r(4.0)
                .size_full()
                .bg_color(dynamic(|t: &Theme| t.background_hover)),
        )
        .label(label, ii().text_color(dynamic(|t: &Theme| t.text))),
    )
}
