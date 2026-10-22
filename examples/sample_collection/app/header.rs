use crate::app::{SearchText, theme::Theme};
pub use michiu::{accessibility::*, prelude::*};
use std::time::Duration;

pub fn header() -> Entity {
    h_flex(
        ii().w_full()
            .height(40.0)
            .shrink_0()
            .justify_center()
            .items_center()
            .border_bottom(BorderStyle::Solid, 1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .children([search_box(), toggle_btn(), window_controll()])
    .tag::<a11y::AHeader>()
}

fn search_box() -> Entity {
    let input_wrapper = h_flex(
        ii().justify_center()
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
        .modify_d(|t: &Theme| {
            ii().grow()
                .h_auto()
                .bg_color(Color::TRANSPARENT)
                .font_size(t.font_size_base)
                .font_family(t.font_family.clone())
                .text_color(t.text)
                .select_text()
                .overflow_scroll()
                .cursor_text()
        })
        .a11y_role(Role::SearchInput);

    let suffix_entity = text("✕")
        .modify_d(move |t: &Theme| {
            let input_text = use_provided::<SearchText>().get();

            let has_text = !input_text.0.is_empty();
            let base_style = ii()
                .text_color(t.text_muted)
                .font_size(t.font_size_base)
                .pointer_events_auto()
                .hovered(ii().text_color(t.text));

            if has_text {
                base_style.opacity(1.0)
            } else {
                base_style.opacity(0.0).pointer_events_none()
            }
        })
        .on_click(move || {
            let set_input_text = use_provided_setter::<SearchText>();
            set_input_text.set(SearchText(String::new()));
        })
        .a11y_with(Node::new(Role::Button), |n| n.set_label("Clean Button"));

    input_wrapper.child(text_input).child(suffix_entity)
}

fn toggle_btn() -> Entity {
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

        ii().r(4.0)
            .items_center()
            .size((50.0, 26.0))
            .bg_color(track_color)
            .absolute()
            .right(160.0)
            .focusable_self_keyboard()
            .focused(
                ii().outline_solid(1.0)
                    .outline_offset(2.0)
                    .outline_color(dynamic(|t: &Theme| t.text)),
            )
    })
    .on_click(mode_change);

    let btn_inner = h_flex_d(|t: &Theme| {
        ii().r(4.0)
            .m_l(4.0)
            .height(20.0)
            .width(20.0)
            .bg_color(t.primary)
            .actived(ii().transform_translate(22.0, 0.0))
            .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
    })
    .active(dynamic(|t: &Theme| t.is_dark))
    .on_click(mode_change);

    btn_wrapper.child(btn_inner)
}

fn window_controll() -> Entity {
    let base_btn = |label: &'static str| {
        flex(ii().h_full().justify_center().items_center().w(47.0)).label(
            label,
            ii().font_size(14.0).text_color(dynamic(|t: &Theme| t.text)),
        )
    };

    let style = ii().hovered(ii().bg_color(dynamic(|t: &Theme| t.background_hover)));

    h_flex(ii().absolute().right(0.0).h_full().w(141.0)).children([
        base_btn("－").modify(&style).on_click_with(|cx| {
            let handle = cx.use_provided::<WindowHandle>().get();
            handle.set_minimize();
        }),
        base_btn("▢").modify(&style),
        base_btn("✕")
            .modify(ii().hovered(ii().bg_color(rgb(232, 17, 35))))
            .on_click_with(|cx| {
                let handle = cx.use_provided::<WindowHandle>().get();
                handle.destroy();
            }),
    ])
}
