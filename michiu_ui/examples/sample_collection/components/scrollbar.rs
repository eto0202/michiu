use crate::theme::Theme;
pub use michiu_ui::prelude::*;

pub fn scrollbar_y(t: &Theme) -> ThisStyle {
    ts().scrollbar_always()
        .scrollbar_mode(ScrollbarMode::Overlay)
        .scrollbar_thumb(
            ts().m_y(4.0)
                .p_r(5.0)
                .width(7.0)
                .rounded_full()
                .bg_color(t.border)
                .hovered(ts().bg_color(t.border_hover)),
        )
}
