use crate::app::{SearchText, theme::Theme};
pub use michiu_ui::prelude::*;
use std::time::Duration;

pub fn header() -> Element {
    h_flex(
        ts().w_full()
            .height(40.0)
            .shrink_0()
            .justify_center()
            .items_center()
            .border_bottom(BorderStyle::Solid, 1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .children([search_box(), toggle_btn()])
}

fn search_box() -> Element {
    let input_wrapper = h_flex(
        ts().justify_center()
            .items_center()
            .size((pct(35.0), 26.0))
            .max_size((600.0, 26.0))
            .bg_color(dynamic(|t: &Theme| t.background_hover))
            .r(4.0)
            .p_x(16.0),
    );

    let text_input = div_n()
        .input_d(|t: &Theme| {
            // create_root で provide された共有シグナルを use_provided で読み込み双方向マッピング
            let writer = use_provided_setter::<SearchText>();
            let (input_text, set_input_text) =
                use_provided::<SearchText>().bi_map(writer, |t| t.0.clone(), SearchText);

            InputContents::new((input_text, set_input_text))
                .placeholder("Search for components...")
                .placeholder_color(t.text_muted)
                .caret_color(t.text)
                .caret_width(1.0)
        })
        .style_d(|t: &Theme| {
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
        .style_d(move |t: &Theme| {
            let input_text = use_provided::<SearchText>().get();

            let has_text = !input_text.0.is_empty();
            let base_style = ts()
                .text_color(t.text_muted)
                .font_size(t.font_size_base)
                .pointer_events_auto()
                .hovered(ts().text_color(t.text));

            if has_text {
                base_style.opacity(1.0)
            } else {
                base_style.opacity(0.0).pointer_events_none()
            }
        })
        .on_click(move || {
            let set_input_text = use_provided_setter::<SearchText>();
            set_input_text.set(SearchText(String::new()));
        });

    input_wrapper.child(text_input).child(suffix_element)
}

fn toggle_btn() -> Element {
    let mode_change = || {
        let t = use_provided::<Theme>().get();
        let set_t = use_provided_setter::<Theme>();

        if t.is_dark {
            set_t.set(Theme::light());
        } else {
            set_t.set(Theme::dark());
        }
    };

    let btn_wrapper = h_flex_d(|t: &Theme| {
        // ダーク（Active）時はアクセントカラーや hover 色を、ライト時は標準の境界色を割り当てます
        let track_color = if t.is_dark { t.surface } else { t.border };

        ts().r(4.0)
            .items_center()
            .size((50.0, 26.0))
            .bg_color(track_color)
            .absolute()
            .right(10.0)
            .focusable_self_keyboard()
            .focused(
                ts().outline_solid(1.0)
                    .outline_offset(2.0)
                    .outline_color(dynamic(|t: &Theme| t.text)),
            )
    })
    .on_click(mode_change);

    let btn_inner = h_flex_d(|t: &Theme| {
        ts().r(4.0)
            .m_l(4.0)
            .height(20.0)
            .width(20.0)
            .bg_color(t.primary)
            .actived(ts().transform_translate(22.0, 0.0))
            .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
    })
    .active(dynamic(|t: &Theme| t.is_dark))
    .on_click(mode_change);

    btn_wrapper.child(btn_inner)
}
