use michiu::{accessibility::*, prelude::*};

pub fn test_button() -> Entity {
    flex(
        ii().justify_center()
            .size((72.0, 36.0))
            .bg_color(Color::WHITE)
            .cursor_pointer()
            .hovered(ii().bg_color(Color::WHITE.darken(0.2)))
            .pressed(ii().bg_color(Color::WHITE.darken(0.3)))
            .focusable_self_keyboard()
            .focused(
                ii().outline_solid(3.0)
                    .outline_offset(2.0)
                    .outline_color(Color::BLACK),
            ),
    )
    .tag::<a11y::AButton>()
    .label(
        "Button",
        ii().font_size(18.0)
            .font_family("Segoe UI")
            .text_color(Color::BLACK),
    )
}
