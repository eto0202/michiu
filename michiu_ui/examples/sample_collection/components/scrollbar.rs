pub use michiu_ui::prelude::*;

use crate::theme::Theme;

pub fn scrollbar_y() -> ThisStyle {
    ts().scrollbar_always()
        .scrollbar_mode(ScrollbarMode::Overlay)
        .scrollbar_thumb(
            ts().m_y(4.0)
                .p_r(5.0)
                .width(7.0)
                .rounded_full()
                .bg_color(Theme::DARK_BORDER)
                .hovered(ts().bg_color(hsl(220.0, 0.12, 0.30))),
        )
}
