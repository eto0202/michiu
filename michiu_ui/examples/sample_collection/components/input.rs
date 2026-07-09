use crate::theme::Theme;
use michiu_ui::prelude::*;

pub fn search_box() -> Element {
    let input_wrapper = h_flex_c(|t: &Theme| {
        ts().justify_center()
            .items_center()
            .size((pct(30.0), 26.0))
            .min_size((400.0, 26.0))
            .max_size((800.0, 26.0))
            .bg_color(t.surface)
            .r(4.0)
            .p_x(16.0)
    });

    let (input_text, set_input_text) = create_signal(String::new());

    let text_input = input_c(move |t: &Theme| {
        InputContents::new((input_text, set_input_text))
            .placeholder("Search for components...")
            .placeholder_color(t.text_muted)
            .caret_color(t.text)
            .caret_width(1.0)
    })
    .style_c(|t: &Theme| {
        ts().grow()
            .h_auto()
            .bg_color(Color::TRANSPARENT)
            .font_size(t.font_size_base)
            .font_family(t.font_family.clone())
            .text_color(t.text)
            .select_text()
            .overflow_scroll()
            .cursor_text()
    });

    let suffix_element = text("✕")
        .style_c(move |t: &Theme| {
            let has_text = !input_text.get().is_empty();
            let base_style = ts()
                .text_color(t.text_muted)
                .font_size(t.font_size_base)
                .hovered(ts().text_color(t.text));

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
