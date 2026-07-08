use michiu_ui::Element;
pub use michiu_ui::prelude::*;

use crate::theme::Theme;

pub fn header() -> Element {
    h_flex(
        ts().w_full()
            .height(40.0)
            .shrink_0()
            .justify_center()
            .items_center()
            .border_bottom(BorderStyle::Solid, 1.0)
            .border_color(Theme::DARK_BORDER),
    )
    .child(search_box())
}

pub fn search_box() -> Element {
    let (input_text, set_input_text) = create_signal(String::new());

    let input_wrapper = h_flex(
        ts().justify_center()
            .items_center()
            .size((pct(30.0), 25.0))
            .min_size((400.0, 25.0))
            .max_size((800.0, 25.0))
            .bg_color(Theme::DARK_SURFACE)
            .rounded_full()
            .p_x(16.0)
            .p_y(8.0),
    );

    let text_input = input(
        InputContents::new((input_text, set_input_text))
            .placeholder("Search for components...")
            .placeholder_color(Theme::DARK_TEXT_MUTED),
    )
    .style(
        ts().grow()
            .h_auto()
            .bg_color(Color::TRANSPARENT)
            .font_size(Theme::FONT_SIZE_MD)
            .font_family(Theme::FONT_MAIN)
            .text_color(Theme::DARK_TEXT)
            .select_text()
            .overflow_scroll()
            .cursor_text(),
    );

    let base_style = ts()
        .text_color(Theme::DARK_TEXT_MUTED)
        .font_size(13.0)
        .hovered(ts().text_color(Color::WHITE));

    let suffix_element = text("✕")
        .style(move || {
            let has_text = !input_text.get().is_empty();
            let base_style = base_style.clone();

            if has_text {
                base_style.opacity(1.0)
            } else {
                base_style.opacity(0.0).pointer_events_none()
            }
        })
        .on_click(move || {
            set_input_text.set(String::new());
        });

    input_wrapper.child(text_input).child(suffix_element)
}
